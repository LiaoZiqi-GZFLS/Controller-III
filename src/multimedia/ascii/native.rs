//! Native Rust MP4+H.264 ASCII video player
//! Used as fallback when FFmpeg is not available
//! Note: This fallback does not support audio playback

use std::path::Path;
use std::fs::File;
use std::thread;
use std::time::{Instant, Duration};
use mp4::Mp4Reader;
use openh264::{decoder::Decoder, decoder::DecodedYUV, OpenH264API};
use openh264::formats::YUVSource;
use image::{ImageBuffer, RgbImage, DynamicImage};
use spin_sleep::SpinSleeper;

use crate::multimedia::{
    error::MultimediaError,
    traits::{AsciiPlayer, AsciiPlayOptions},
    ascii::{
        calculate_dimensions, image_to_ascii, TerminalGuard,
        player::{
            QueuedFrame, PlaybackContext, PlaybackState, UserAction,
            DecodeMessage, create_frame_queue, create_decode_control,
            DEFAULT_SEEK_STEP, DEFAULT_SPEED_STEP,
        },
    },
};

/// Native MP4+H.264 ASCII video player
pub struct NativeAsciiPlayer;

impl NativeAsciiPlayer {
    pub fn new() -> Self {
        Self
    }

    /// Check if input is likely MP4 with H.264
    pub fn is_mp4_h264(&self, input: &Path) -> bool {
        // Check extension
        if let Some(ext) = input.extension() {
            let ext = ext.to_string_lossy().to_lowercase();
            if ext != "mp4" {
                return false;
            }
        }

        // Try to open - if it reads, it's MP4
        if let Ok(mut file) = File::open(input) {
            if let Ok(file_size) = file.metadata() {
                if let Ok(mp4) = Mp4Reader::read_header(&mut file, file_size.len()) {
                    // Check if there's a video track with H.264 (avc1)
                    const VIDEO: mp4::FourCC = mp4::FourCC { value: *b"vide" }; // "vide"
                    mp4.tracks().values().any(|track| {
                        track.trak.mdia.hdlr.handler_type == VIDEO
                            && track.trak.mdia.minf.stbl.stsd.avc1.is_some()
                    })
                } else {
                    false
                }
            } else {
                false
            }
        } else {
            false
        }
    }
}

impl AsciiPlayer for NativeAsciiPlayer {
    fn play(
        &mut self,
        input: &Path,
        options: AsciiPlayOptions,
    ) -> Result<(), MultimediaError> {
        let mut file = File::open(input)?;
        let file_size = file.metadata()?.len();
        let mut mp4 = Mp4Reader::read_header(&mut file, file_size)?;

        // Find video track
        const VIDEO: mp4::FourCC = mp4::FourCC { value: *b"vide" };
        let (video_track_id, video_track) = match mp4.tracks().iter()
            .find(|(_, track)| track.trak.mdia.hdlr.handler_type == VIDEO && track.trak.mdia.minf.stbl.stsd.avc1.is_some()) {
                Some((id, track)) => (*id, track),
                None => return Err(MultimediaError::InvalidMedia("No H.264 video track found in MP4".into())),
            };

        // Get video dimensions
        let (original_width, original_height) = match &video_track.trak.mdia.minf.stbl.stsd.avc1 {
            Some(avc1) => (avc1.width.into(), avc1.height.into()),
            None => return Err(MultimediaError::InvalidMedia("Could not get video dimensions".into())),
        };

        // Get timescale and duration
        let timescale = mp4.moov.mvhd.timescale;
        let total_duration = mp4.moov.mvhd.duration as f64 / timescale as f64;

        // Calculate output dimensions based on mode
        let (output_width, output_height) = if options.export_dir.is_some() {
            // Export mode - if no dimensions given, default to 80 columns
            let default_width = options.width.unwrap_or(80);
            calculate_dimensions(
                original_width,
                original_height,
                options.width,
                options.height,
                default_width,
                default_width * 30 / 80,
                options.scale_mode,
            )
        } else {
            // Playback mode - use terminal dimensions
            let (term_width, term_height) = TerminalGuard::get_size()?;
            calculate_dimensions(
                original_width,
                original_height,
                options.width,
                options.height,
                term_width,
                term_height.saturating_sub(2), // Reserve 1-2 lines for status bar
                options.scale_mode,
            )
        };

        // Handle export mode - save frames to text files instead of playing
        if let Some(export_dir) = &options.export_dir {
            // Create export directory if it doesn't exist
            std::fs::create_dir_all(export_dir)?;
            return native_export_ascii_frames(
                input,
                export_dir,
                output_width,
                output_height,
                options.color_mode,
                video_track_id,
                options.export_max_frames,
            );
        }

        // Collect all samples (we need to play them all sequentially)
        let samples = collect_all_samples(video_track);
        if samples.is_empty() {
            return Err(MultimediaError::InvalidMedia("No samples found in video track".into()));
        }

        // Create channels
        let (frame_sender, frame_receiver) = create_frame_queue();
        let (control_sender, control_receiver) = create_decode_control();

        // Initialize playback context
        let mut ctx = PlaybackContext::new(
            total_duration,
            original_width,
            original_height,
            output_width,
            output_height,
            &options,
        );

        let input_path = input.to_path_buf();
        let color_mode_clone = options.color_mode;
        let _decode_handle = thread::spawn(move || {
            // Open file again in decode thread
            let mut file = match File::open(&input_path) {
                Ok(f) => f,
                Err(_) => return Ok::<(), MultimediaError>(()),
            };
            let mut mp4 = match Mp4Reader::read_header(&mut file, file_size) {
                Ok(mp4) => mp4,
                Err(_) => return Ok::<(), MultimediaError>(()),
            };

            // Create decoder
            let api = OpenH264API::from_source();
            let mut decoder = match Decoder::new(api) {
                Ok(d) => d,
                Err(_) => return Ok(()),
            };

            let mut current_sample_index = 0;
            let original_width = original_width;
            let original_height = original_height;
            let mut current_output_width = output_width;
            let mut current_output_height = output_height;

            'decode: loop {
                // Check for control messages
                match control_receiver.try_recv() {
                    Ok(DecodeMessage::Stop) => break 'decode,
                    Ok(DecodeMessage::Seek(target_pts)) => {
                        // Find the sample closest to target PTS
                        let target_ts = (target_pts * timescale as f64) as u64;
                        // Find the first sample with timestamp >= target_ts
                        current_sample_index = samples.iter()
                            .position(|&(_, ts)| ts >= target_ts)
                            .unwrap_or(samples.len() - 1);
                        continue 'decode;
                    }
                    Ok(DecodeMessage::Resize(new_width, new_height)) => {
                        // Update output dimensions for subsequent frames
                        current_output_width = new_width;
                        current_output_height = new_height;
                        continue 'decode;
                    }
                    Ok(DecodeMessage::Continue) => {}
                    Err(_) => {}
                }

                if current_sample_index >= samples.len() {
                    drop(frame_sender);
                    break 'decode;
                }

                let (sample_id, abs_timestamp) = samples[current_sample_index];
                let pts = abs_timestamp as f64 / timescale as f64;

                // Estimate duration from next sample
                let duration = if current_sample_index + 1 < samples.len() {
                    (samples[current_sample_index + 1].1 - abs_timestamp) as f64 / timescale as f64
                } else {
                    1.0 / 30.0
                };

                // Get sample data
                if let Ok(Some(sample_data)) = mp4.read_sample(video_track_id, sample_id) {
                    let data = sample_data.bytes.clone();

                    // Decode NAL unit
                    match decoder.decode(&data) {
                        Ok(Some(yuv)) => {
                            // Convert YUV to RGB
                            if let Ok(rgb) = yuv_to_rgb(yuv, original_width, original_height) {
                                let dyn_img = DynamicImage::ImageRgb8(rgb);

                                // Convert to ASCII
                                let ascii_lines = image_to_ascii(
                                    &dyn_img,
                                    current_output_width,
                                    current_output_height,
                                    color_mode_clone,
                                );

                                // Queue frame
                                if frame_sender.send(QueuedFrame {
                                    ascii_lines,
                                    pts,
                                    duration,
                                    original_width,
                                    original_height,
                                }).is_err() {
                                    break 'decode;
                                }
                            }
                        }
                        Ok(None) => {} // need more data
                        Err(e) => {
                            eprintln!("Warning: decoding error: {}", e);
                        }
                    }
                }

                current_sample_index += 1;
            }

            Ok(())
        });

        // Initialize terminal with guard that guarantees cleanup
        let mut guard = TerminalGuard::new()?;

        // Clear all pending input events
        TerminalGuard::drain_all_events()?;

        let render_start = Instant::now();
        let mut current_frame: Option<QueuedFrame> = None;
        let sleeper = SpinSleeper::default();

        // Main playback loop
        'main: loop {
            // Check for user input
            match guard.poll_event(Duration::from_millis(1)) {
                Ok(action) => match action {
                    UserAction::Quit => break 'main,
                    UserAction::TogglePause => {
                        ctx.toggle_pause();
                        // No audio to pause in native backend
                    }
                    UserAction::SeekForward => {
                        let new_pts = ctx.seek_relative(DEFAULT_SEEK_STEP);
                        let _ = control_sender.send(DecodeMessage::Seek(new_pts));
                        while let Ok(_) = frame_receiver.try_recv() {}
                        ctx.state = PlaybackState::Playing;
                        current_frame = None;
                    }
                    UserAction::SeekBackward => {
                        let new_pts = ctx.seek_relative(-DEFAULT_SEEK_STEP);
                        let _ = control_sender.send(DecodeMessage::Seek(new_pts));
                        while let Ok(_) = frame_receiver.try_recv() {}
                        ctx.state = PlaybackState::Playing;
                        current_frame = None;
                    }
                    UserAction::SpeedUp => {
                        ctx.adjust_speed(DEFAULT_SPEED_STEP);
                    }
                    UserAction::SpeedDown => {
                        ctx.adjust_speed(-DEFAULT_SPEED_STEP);
                    }
                    UserAction::Resize => {
                        if let Ok((new_term_width, new_term_height)) = TerminalGuard::get_size() {
                            let (new_width, new_height) = calculate_dimensions(
                                ctx.original_width,
                                ctx.original_height,
                                options.width,
                                options.height,
                                new_term_width,
                                new_term_height.saturating_sub(2),
                                options.scale_mode,
                            );
                            if ctx.handle_resize(new_width, new_height) {
                                // Dimensions changed, send to decode thread
                                let _ = control_sender.send(DecodeMessage::Resize(new_width, new_height));
                            }
                        }
                    }
                    UserAction::None => {}
                }
                Err(e) => {
                    eprintln!("Terminal input error: {}", e);
                    break 'main;
                }
            }

            let current_pts = ctx.get_current_pts();
            ctx.current_pts = current_pts;

            // Check for end
            if current_pts >= ctx.total_duration && ctx.total_duration.is_finite() {
                break 'main;
            }

            // If paused, keep displaying current frame
            if ctx.state == PlaybackState::Paused {
                if let Some(frame) = &current_frame {
                    let fps = if ctx.show_fps { Some(ctx.calculate_fps(render_start)) } else { None };
                    let _ = TerminalGuard::render_frame(
                        &frame.ascii_lines,
                        fps,
                        ctx.speed,
                        current_pts,
                        ctx.total_duration,
                    );
                }
                sleeper.sleep(Duration::from_millis(33));
                continue;
            }

            // Get next frame if needed
            let need_next = match &current_frame {
                None => true,
                Some(frame) => current_pts > frame.pts + frame.duration,
            };

            if need_next {
                match frame_receiver.try_recv() {
                    Ok(mut next_frame) => {
                        loop {
                            if next_frame.pts + next_frame.duration < current_pts {
                                ctx.frames_dropped += 1;
                                match frame_receiver.try_recv() {
                                    Ok(new_next) => next_frame = new_next,
                                    Err(_) => break,
                                }
                            } else {
                                break;
                            }
                        }
                        current_frame = Some(next_frame);
                    }
                    Err(_) => {}
                }
            }

            // Render current frame
            if let Some(frame) = &current_frame {
                let frame_pts = frame.pts;
                if frame_pts > current_pts {
                    let wait = frame_pts - current_pts;
                    if wait > 0.0 {
                        sleeper.sleep(Duration::from_secs_f64(wait / ctx.speed));
                    }
                }

                if frame.pts != ctx.last_rendered_pts {
                    let fps = if ctx.show_fps { Some(ctx.calculate_fps(render_start)) } else { None };
                    let _ = TerminalGuard::render_frame(
                        &frame.ascii_lines,
                        fps,
                        ctx.speed,
                        current_pts,
                        ctx.total_duration,
                    );
                    ctx.last_rendered_pts = frame.pts;
                    ctx.frames_rendered += 1;
                }

                // Drop additional frames to catch up if we're still behind
                while let Ok(mut next_frame) = frame_receiver.try_recv() {
                    if next_frame.pts < current_pts {
                        ctx.frames_dropped += 1;
                    } else {
                        current_frame = Some(next_frame);
                        break;
                    }
                }
            } else {
                sleeper.sleep(Duration::from_millis(1));
            }
        }

        // Stop decode thread
        let _ = control_sender.send(DecodeMessage::Stop);
        drop(frame_receiver);
        let _ = _decode_handle.join();

        println!("\x1b[0mPlayback complete: {} frames rendered, {} frames dropped (native backend has no audio)", ctx.frames_rendered, ctx.frames_dropped);

        Ok(())
    }

    fn is_available(&self, input: &Path) -> bool {
        self.is_mp4_h264(input)
    }
}

/// Collect all samples in order for sequential playback
fn collect_all_samples<'a>(track: &mp4::Mp4Track) -> Vec<(u32, u64)> {
    let mut track_samples = Vec::new();
    let mut sample_id = 0;
    let mut timestamp = 0;

    // Manually iterate through sample entries from stts box
    for entry in &track.trak.mdia.minf.stbl.stts.entries {
        for _ in 0..entry.sample_count {
            let abs_timestamp = timestamp + entry.sample_delta as u64;
            track_samples.push((sample_id, abs_timestamp));
            sample_id += 1;
            timestamp = abs_timestamp;
        }
    }

    track_samples
}

/// Convert YUV to RGB
fn yuv_to_rgb(yuv: DecodedYUV, width: u32, height: u32) -> Result<RgbImage, MultimediaError> {
    let mut rgb_frame: RgbImage = ImageBuffer::new(width, height);
    let y_data = yuv.y();
    let u_data = yuv.u();
    let v_data = yuv.v();
    let (stride_y, stride_u, stride_v) = yuv.strides_yuv();

    for y_row in 0..height {
        for x in 0..width {
            let y = y_data[(y_row as usize * stride_y) + x as usize];
            let uv_row = (y_row / 2) as usize;
            let uv_x = (x / 2) as usize;
            let u = u_data[uv_row * stride_u + uv_x];
            let v = v_data[uv_row * stride_v + uv_x];

            // Convert YUV to RGB (BT.601)
            let yf = y as f32 - 16.0;
            let uf = u as f32 - 128.0;
            let vf = v as f32 - 128.0;

            let r = (yf + 1.402 * vf).clamp(0.0, 255.0) as u8;
            let g = (yf - 0.34414 * uf - 0.71414 * vf).clamp(0.0, 255.0) as u8;
            let b = (yf + 1.772 * uf).clamp(0.0, 255.0) as u8;

            rgb_frame.put_pixel(x, y_row, image::Rgb([r, g, b]));
        }
    }

    Ok(rgb_frame)
}

/// Native backend export of ASCII frames to text files
fn native_export_ascii_frames(
    input_path: &Path,
    export_dir: &std::path::PathBuf,
    output_width: u32,
    output_height: u32,
    color_mode: crate::multimedia::traits::AsciiColorMode,
    video_track_id: u32,
    max_frames: Option<usize>,
) -> Result<(), MultimediaError> {
    use std::io::Write;
    use openh264::{decoder::Decoder, OpenH264API};
    use image::DynamicImage;

    let mut file = File::open(input_path)?;
    let file_size = file.metadata()?.len();
    let mut mp4 = Mp4Reader::read_header(&mut file, file_size)?;

    // Find video track again
    const VIDEO: mp4::FourCC = mp4::FourCC { value: *b"vide" };
    let (video_track_id, video_track) = match mp4.tracks().iter()
        .find(|(_, track)| track.trak.mdia.hdlr.handler_type == VIDEO && track.trak.mdia.minf.stbl.stsd.avc1.is_some()) {
            Some((id, track)) => (*id, track),
            None => return Err(MultimediaError::InvalidMedia("No H.264 video track found in MP4".into())),
        };

    // Collect all samples
    let samples = collect_all_samples(video_track);
    if samples.is_empty() {
        return Err(MultimediaError::InvalidMedia("No samples found in video track".into()));
    }

    // Create decoder
    let api = OpenH264API::from_source();
    let mut decoder = match Decoder::new(api) {
        Ok(d) => d,
        Err(_) => return Err(MultimediaError::InvalidMedia("Failed to create H.264 decoder".into())),
    };

    let (original_width, original_height) = match &video_track.trak.mdia.minf.stbl.stsd.avc1 {
        Some(avc1) => (avc1.width.into(), avc1.height.into()),
        None => return Err(MultimediaError::InvalidMedia("Could not get video dimensions".into())),
    };
    let timescale = mp4.moov.mvhd.timescale;

    let mut frame_count = 0;
    let max_frames = max_frames.unwrap_or(usize::MAX);

    // Reopen file in this thread
    let mut file = match File::open(input_path) {
        Ok(f) => f,
        Err(e) => return Err(MultimediaError::IoError(e)),
    };
    let mut mp4 = match Mp4Reader::read_header(&mut file, file_size) {
        Ok(mp4) => mp4,
        Err(e) => return Err(MultimediaError::Mp4Error(e.to_string())),
    };

    for &(sample_id, abs_timestamp) in &samples {
        if frame_count >= max_frames {
            break;
        }

        let pts = abs_timestamp as f64 / timescale as f64;

        // Get sample data
        if let Ok(Some(sample_data)) = mp4.read_sample(video_track_id, sample_id) {
            let data = sample_data.bytes.clone();

            // Decode NAL unit
            match decoder.decode(&data) {
                Ok(Some(yuv)) => {
                    // Convert YUV to RGB
                    if let Ok(rgb) = yuv_to_rgb(yuv, original_width, original_height) {
                        let dyn_img = DynamicImage::ImageRgb8(rgb);

                        // Convert to ASCII
                        let ascii_lines = image_to_ascii(
                            &dyn_img,
                            output_width,
                            output_height,
                            color_mode,
                        );

                        // Save to file
                        let frame_path = export_dir.join(format!("frame_{:06}.txt", frame_count));
                        let mut file = std::fs::File::create(frame_path)?;
                        for line in &ascii_lines {
                            writeln!(file, "{}", line)?;
                        }
                        file.flush()?;

                        frame_count += 1;

                        if frame_count % 10 == 0 {
                            println!("Exported {} frames...", frame_count);
                        }
                    }
                }
                Ok(None) => {} // need more data
                Err(e) => {
                    eprintln!("Warning: decoding error: {}", e);
                }
            }
        }
    }

    println!("Export complete: {} frames saved to {}", frame_count, export_dir.display());
    Ok(())
}
