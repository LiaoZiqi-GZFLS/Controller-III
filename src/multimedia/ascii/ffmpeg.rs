//! FFmpeg-based ASCII video player with proper synchronization and controls

use std::path::Path;
use std::sync::mpsc;
use std::thread;
use std::time::{Instant, Duration};
use ffmpeg_the_third as ffmpeg;
use ffmpeg::{decoder, media};
use ffmpeg::software::{resampling, scaling};
use image::{ImageBuffer, RgbImage, DynamicImage};
use spin_sleep::SpinSleeper;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

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

/// FFmpeg-based ASCII video player
pub struct FfmpegAsciiPlayer;

impl FfmpegAsciiPlayer {
    pub fn new() -> Self {
        Self
    }
}

impl AsciiPlayer for FfmpegAsciiPlayer {
    fn play(
        &mut self,
        input: &Path,
        options: AsciiPlayOptions,
    ) -> Result<(), MultimediaError> {
        ffmpeg::init()?;

        let mut input_ctx = ffmpeg::format::input(input)?;

        // Find the first video stream
        let mut video_stream_index = 0;
        let mut original_width = 0;
        let mut original_height = 0;
        let mut video_time_base = ffmpeg::Rational::new(1, 1);
        let mut video_pixel_format = ffmpeg::format::Pixel::RGB24;

        // Find audio stream
        let mut audio_stream_index: Option<usize> = None;

        let mut total_duration = input_ctx.duration() as f64 / ffmpeg::ffi::AV_TIME_BASE as f64;
        if total_duration <= 0.0 {
            total_duration = f64::INFINITY;
        }

        // Just find the stream indices and get time base, don't create decoder yet
        // We'll create decoder inside the decode thread because ffmpeg objects aren't Send
        for (i, s) in input_ctx.streams().enumerate() {
            if s.parameters().medium() == media::Type::Video {
                video_stream_index = i;
                video_time_base = s.time_base();
                // Get original dimensions from parameters
                if let Ok(context) = ffmpeg::codec::context::Context::from_parameters(s.parameters()) {
                    if let Ok(decoder) = context.decoder().video() {
                        original_width = decoder.width();
                        original_height = decoder.height();
                        video_pixel_format = decoder.format();
                    }
                }
            } else if s.parameters().medium() == media::Type::Audio {
                // Found audio stream
                audio_stream_index = Some(i);
            }
        }

        if original_width == 0 || original_height == 0 {
            return Err(MultimediaError::NoVideoStream);
        }

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
                default_width * 30 / 80, // Reasonable default height based on 80x24 terminal
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
            return export_ascii_frames(
                input,
                export_dir,
                output_width,
                output_height,
                options.color_mode,
                video_stream_index,
                options.export_max_frames,
            );
        }

        // We always resample to stereo f32 for cpal output, sample rate matches device
        let out_sample_format = ffmpeg::format::Sample::F32(ffmpeg::format::sample::Type::Packed);
        let out_ch_layout = ffmpeg::channel_layout::ChannelLayout::STEREO;

        // Setup cpal audio output and get device sample rate
        // Audio setup happens on main thread because cpal is here
        let (audio_sender, audio_stream, out_rate) = if audio_stream_index.is_some() {
            let result = (|| -> Result<(Option<mpsc::SyncSender<Vec<f32>>>, Option<cpal::Stream>, u32), MultimediaError> {
                let host = cpal::default_host();
                let device = host.default_output_device().ok_or_else(|| {
                    MultimediaError::AudioError("No audio output device available".into())
                })?;
                let default_config = device.default_output_config().map_err(|e| {
                    MultimediaError::AudioError(e.to_string())
                })?;

                let out_rate = default_config.sample_rate().0;
                // Create a channel to send samples to audio callback
                // Use bounded channel for back-pressure
                let (sender, receiver) = mpsc::sync_channel::<Vec<f32>>(out_rate as usize / 2);

                // Build audio stream based on device's native format
                let stream: cpal::Stream = match default_config.sample_format() {
                    cpal::SampleFormat::F32 => {
                        device.build_output_stream(
                            &default_config.clone().into(),
                            move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                                // Fill buffer with samples from channel
                                let mut i = 0;
                                while i < data.len() {
                                    if let Ok(sample_vec) = receiver.try_recv() {
                                        for &s in &sample_vec {
                                            if i < data.len() {
                                                data[i] = s;
                                                i += 1;
                                            } else {
                                                break;
                                            }
                                        }
                                    } else {
                                        // No more samples available, fill remaining with silence
                                        data[i] = 0.0;
                                        i += 1;
                                    }
                                }
                            },
                            |err: cpal::StreamError| eprintln!("Audio output error: {}", err),
                            None
                        ).map_err(|e| MultimediaError::AudioError(e.to_string()))?
                    }
                    _ => {
                        // Device doesn't support F32, can't play audio
                        return Err(MultimediaError::AudioError(
                            format!("Device sample format {:?} not supported (needs F32)", default_config.sample_format())
                        ));
                    }
                };

                stream.play()?;

                Ok((Some(sender), Some(stream), out_rate))
            })();

            match result {
                Ok((sender, stream, rate)) => (sender, stream, rate),
                Err(e) => {
                    eprintln!("⚠️  Audio initialization failed: {}, playing without audio", e);
                    (None, None, 44100)
                }
            }
        } else {
            (None, None, 44100)
        };

        // Create channels for communication between main thread and decode thread
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

        // Save parameters for decode thread
        let input_path = input.to_path_buf();
        let video_stream_index_clone = video_stream_index;
        let audio_stream_index_clone = audio_stream_index;
        let video_time_base_clone = video_time_base;
        let video_pixel_format_clone = video_pixel_format;
        let color_mode_clone = options.color_mode;
        let original_width_clone = original_width;
        let original_height_clone = original_height;
        let output_width_clone = output_width;
        let output_height_clone = output_height;
        let audio_sender_clone = audio_sender.clone();
        let out_rate_clone = out_rate;

        // Spawn decode thread - all ffmpeg objects created inside this thread
        let _decode_handle = thread::spawn(move || -> Result<(), ()> {
            // This thread handles demuxing and decoding continuously
            // It sends pre-converted ASCII frames to the main thread via channel
            let mut input_ctx = match ffmpeg::format::input(&input_path) {
                Ok(ctx) => ctx,
                Err(_) => return Ok(()),
            };

            // Recreate all decoders in this thread (ffmpeg objects not Send/Sync)
            let mut video_decoder_ctx = None;
            let mut audio_decoder_ctx = None;
            let video_stream_index = video_stream_index_clone;
            let audio_stream_index = audio_stream_index_clone;

            for (i, s) in input_ctx.streams().enumerate() {
                if i == video_stream_index {
                    let context = match ffmpeg::codec::context::Context::from_parameters(s.parameters()) {
                        Ok(ctx) => ctx,
                        Err(_) => return Ok(()),
                    };
                    video_decoder_ctx = match context.decoder().video() {
                        Ok(d) => Some(d),
                        Err(_) => return Ok(()),
                    };
                } else if Some(i) == audio_stream_index {
                    let context = match ffmpeg::codec::context::Context::from_parameters(s.parameters()) {
                        Ok(ctx) => ctx,
                        Err(_) => continue,
                    };
                    audio_decoder_ctx = match context.decoder().audio() {
                        Ok(d) => Some(d),
                        Err(_) => continue,
                    };
                }
            }

            let Some(mut video_decoder_ctx) = video_decoder_ctx else { return Ok(()) };

            // Get original dimensions from decoder
            original_width = video_decoder_ctx.width();
            original_height = video_decoder_ctx.height();
            video_pixel_format = video_decoder_ctx.format();

            // Create swscale context once for format conversion to RGB inside this thread
            let mut sws_context: Option<scaling::Context> = None;
            if video_decoder_ctx.format() != ffmpeg::format::Pixel::RGB24 {
                // Note: ffmpeg-the-third API swaps order: src_format, src_height, src_width, dst_format, dst_height, dst_width
                sws_context = match scaling::Context::get(
                    video_decoder_ctx.format(),
                    video_decoder_ctx.height(),
                    video_decoder_ctx.width(),
                    ffmpeg::format::Pixel::RGB24,
                    video_decoder_ctx.height(),
                    video_decoder_ctx.width(),
                    scaling::Flags::BILINEAR,
                ) {
                    Ok(ctx) => Some(ctx),
                    Err(_) => return Ok(()),
                };
            }

            // Create resampler inside this thread if needed
            let mut audio_resampler: Option<resampling::Context> = None;
            // We always resample to stereo f32 for cpal output, recreate parameters here because FFmpeg types aren't Send/Sync
            let out_sample_format = ffmpeg::format::Sample::F32(ffmpeg::format::sample::Type::Packed);
            let out_ch_layout = ffmpeg::channel_layout::ChannelLayout::STEREO;
            if audio_sender_clone.is_some() {
                if let (Some(_), Some(ref mut audio_dec)) = (audio_stream_index, audio_decoder_ctx.as_mut()) {
                    let in_format = audio_dec.format();
                    let in_rate = audio_dec.rate();
                    let in_ch_layout = audio_dec.ch_layout();

                    if in_format != out_sample_format || (in_rate as u32) != out_rate_clone || in_ch_layout != out_ch_layout {
                        audio_resampler = resampling::Context::get2(
                            in_format, in_ch_layout, in_rate as u32,
                            out_sample_format, out_ch_layout.clone(), out_rate_clone,
                        ).ok();
                    }
                }
            }

            let orig_w = original_width_clone;
            let orig_h = original_height_clone;
            let mut current_output_width = output_width_clone;
            let mut current_output_height = output_height_clone;
            let mut current_color_mode = color_mode_clone;
            let video_time_base = video_time_base_clone;
            let mut input_ctx = match ffmpeg::format::input(&input_path) {
                Ok(ctx) => ctx,
                Err(_) => return Ok(()),
            };

            // Recreate decoder in this thread
            let mut video_decoder_ctx = None;
            let mut audio_decoder_ctx = None;

            for (i, s) in input_ctx.streams().enumerate() {
                if i == video_stream_index_clone {
                    let context = match ffmpeg::codec::context::Context::from_parameters(s.parameters()) {
                        Ok(ctx) => ctx,
                        Err(_) => return Ok(()),
                    };
                    video_decoder_ctx = match context.decoder().video() {
                        Ok(d) => Some(d),
                        Err(_) => return Ok(()),
                    };
                } else if Some(i) == audio_stream_index_clone {
                    let context = match ffmpeg::codec::context::Context::from_parameters(s.parameters()) {
                        Ok(ctx) => ctx,
                        Err(_) => continue,
                    };
                    audio_decoder_ctx = match context.decoder().audio() {
                        Ok(d) => Some(d),
                        Err(_) => continue,
                    };
                }
            }

            let Some(mut video_decoder_ctx) = video_decoder_ctx else { return Ok(()) };

            let orig_w = original_width;
            let orig_h = original_height;
            let mut current_output_width = output_width;
            let mut current_output_height = output_height;
            let mut current_color_mode = color_mode_clone;

            // Get packets iterator - must be outside loop so we increment correctly
            let mut packets = input_ctx.packets().filter_map(Result::ok);

            // Main decode loop - handles control messages from main thread
            'decode: loop {
                // Check for control messages (seek, stop, resize)
                match control_receiver.try_recv() {
                    Ok(DecodeMessage::Stop) => break 'decode Ok(()),
                    Ok(DecodeMessage::Seek(target_pts)) => {
                        // Flush decoder
                        video_decoder_ctx.flush();
                        // Seek input to target pts (in seconds)
                        let time_base = video_time_base;
                        let target_ts = (target_pts / time_base.numerator() as f64 * time_base.denominator() as f64) as i64;
                        // ffmpeg-the-third seek API: seek to stream timestamp
                        let _ = input_ctx.seek(target_ts, ..);
                        // After seek, restart iterator from new position
                        packets = input_ctx.packets().filter_map(Result::ok);
                        continue 'decode;
                    }
                    Ok(DecodeMessage::Resize(new_width, new_height)) => {
                        // Update output dimensions for subsequent frames
                        current_output_width = new_width;
                        current_output_height = new_height;
                        continue 'decode;
                    }
                    Ok(DecodeMessage::Continue) => {}
                    Err(_) => {} // No messages, continue
                }

                // Read next packet
                let Some((stream, packet)) = packets.next() else {
                    // End of file
                    // Send an empty frame to signal end
                    drop(frame_sender);
                    break 'decode Ok(());
                };

                let stream_index = stream.index();

                // Check if output dimensions have changed (due to resize)
                // We get the latest dimensions from the last queued frame's context if needed
                // Actually, in this implementation we just continue with current, the resize will take effect
                // on the next frame after we get here

                // Process video packet
                if stream_index == video_stream_index_clone {
                    // Calculate pts in seconds
                    let pts_seconds = if let Some(pts) = packet.pts() {
                        pts as f64 * video_time_base.numerator() as f64 / video_time_base.denominator() as f64
                    } else {
                        0.0
                    };
                    let duration_seconds = packet.duration() as f64 * video_time_base.numerator() as f64 / video_time_base.denominator() as f64;
                    let duration_seconds = if duration_seconds <= 0.0 {
                        1.0 / 30.0
                    } else {
                        duration_seconds
                    };

                    // Decode the next frame
                    match decode_packet_video(&packet, &mut video_decoder_ctx, original_width, original_height) {
                        Ok(Some(frame)) => {
                            // Convert any format to RGB
                            let rgb = match frame_to_rgb(&frame, sws_context.as_mut()) {
                                Ok(rgb) => rgb,
                                Err(_) => continue,
                            };
                            let dyn_img = DynamicImage::ImageRgb8(rgb);

                            // Convert to ASCII with current dimensions
                            let ascii_lines = image_to_ascii(
                                &dyn_img,
                                current_output_width,
                                current_output_height,
                                current_color_mode,
                            );

                            // Queue the frame for rendering
                            if frame_sender.send(QueuedFrame {
                                ascii_lines,
                                pts: pts_seconds,
                                duration: duration_seconds,
                                original_width: orig_w,
                                original_height: orig_h,
                            }).is_err() {
                                // Main thread dropped receiver, exit
                                break 'decode Ok(());
                            }
                        }
                        Ok(None) | Err(_) => {
                            continue;
                        }
                    }
                } else if Some(stream_index) == audio_stream_index_clone {
                    // Process audio packet - send to audio queue
                    if let Some(ref mut audio_dec) = audio_decoder_ctx {
                        if let Some(audio_frames) = decode_packet_audio(&packet, audio_dec, &mut audio_resampler, out_sample_format, &out_ch_layout) {
                            if let Some(sender) = &audio_sender {
                                for frame in audio_frames {
                                    // Use try_send - if channel is full, we're ahead of playback, drop frames to avoid blocking video
                                    // This keeps video timeline moving smoothly even if audio can't keep up
                                    let _ = sender.try_send(frame);
                                }
                            }
                        }
                    }
                }
            }
        });

        // Initialize terminal with guard that guarantees cleanup
        let mut guard = TerminalGuard::new()?;

        // Clear all pending input events from dialoguer before playback starts
        TerminalGuard::drain_all_events()?;

        // Start FPS calculation
        let render_start = Instant::now();
        let mut current_frame: Option<QueuedFrame> = None;
        let mut last_size = (output_width, output_height);
        let sleeper = SpinSleeper::default();

        // Main playback loop - rendering and event handling
        'main: loop {
            // Check for user input
            match guard.poll_event(Duration::from_millis(1)) {
                Ok(action) => match action {
                    UserAction::Quit => break 'main,
                    UserAction::TogglePause => {
                        ctx.toggle_pause();
                        if let Some(stream) = &audio_stream {
                            match ctx.state {
                                PlaybackState::Playing => stream.play()?,
                                PlaybackState::Paused => stream.pause()?,
                                _ => {}
                            }
                        }
                    }
                    UserAction::SeekForward => {
                        let new_pts = ctx.seek_relative(DEFAULT_SEEK_STEP);
                        // Tell decode thread to seek
                        let _ = control_sender.send(DecodeMessage::Seek(new_pts));
                        // Clear current queue - we'll get new frames from new position
                        // Drain the existing queue
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
                        // Terminal was resized, recalculate dimensions
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
                                // Dimensions changed, send to decode thread and update
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

            // Check if we've reached end of video
            if current_pts >= ctx.total_duration && ctx.total_duration.is_finite() {
                break 'main;
            }

            // If paused, just keep displaying current frame
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

            // Get next frame from queue if we need it
            let need_next = match &current_frame {
                None => true, // No frame yet
                Some(frame) => current_pts > frame.pts + frame.duration, // Need next frame
            };

            if need_next {
                // Try to get next frame from queue
                while let Ok(next_frame) = frame_receiver.try_recv() {
                    // Skip frames that are still before current playback position
                let mut next_frame = next_frame;
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
                break;
                }
            }

            // Render current frame if we have one
            if let Some(frame) = &current_frame {
                // If frame's PTS is in the future, sleep until it's time
                let frame_pts = frame.pts;
                if frame_pts > current_pts {
                    let wait = frame_pts - current_pts;
                    if wait > 0.0 {
                        sleeper.sleep(Duration::from_secs_f64(wait / ctx.speed));
                    }
                }

                // Only render if this is a new frame (not already rendered)
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

                // If we're way behind, drop frames to catch up
            let mut next_frame = match frame_receiver.try_recv() {
                Ok(f) => f,
                Err(_) => return Ok(()),
            };
            // Keep dropping frames until we get one that's current
            while next_frame.pts < current_pts {
                ctx.frames_dropped += 1;
                match frame_receiver.try_recv() {
                    Ok(f) => next_frame = f,
                    Err(_) => break,
                }
            }
            current_frame = Some(next_frame);
            } else {
                // No frame available yet - brief sleep to avoid busy waiting
                sleeper.sleep(Duration::from_millis(1));
            }
        }

        // Drain audio buffer
        if let Some(stream) = audio_stream {
            // Wait a bit for remaining audio to play
            sleeper.sleep(Duration::from_millis(300));
            let _ = stream.pause();
        }

        // Tell decode thread to stop
        let _ = control_sender.send(DecodeMessage::Stop);
        drop(frame_receiver);
        let _ = _decode_handle.join();

        // Print stats after playback ends
        println!("\x1b[0mPlayback complete: {} frames rendered, {} frames dropped", ctx.frames_rendered, ctx.frames_dropped);

        Ok(())
    }

    fn is_available(&self, _input: &Path) -> bool {
        ffmpeg::init().is_ok()
    }
}

fn decode_packet_video(
    packet: &ffmpeg::Packet,
    decoder: &mut decoder::Video,
    _width: u32,
    _height: u32,
) -> Result<Option<ffmpeg::frame::Video>, MultimediaError> {
    decoder.send_packet(packet)?;

    let mut frame = ffmpeg::frame::Video::empty();
    if decoder.receive_frame(&mut frame).is_ok() {
        Ok(Some(frame))
    } else {
        Ok(None)
    }
}

fn decode_packet_audio(
    packet: &ffmpeg::Packet,
    decoder: &mut decoder::Audio,
    resampler: &mut Option<resampling::Context>,
    out_format: ffmpeg::format::Sample,
    out_ch_layout: &ffmpeg::channel_layout::ChannelLayout,
) -> Option<Vec<Vec<f32>>> {
    // Send packet to decoder - EAGAIN means decoder needs output to be read first, still need to receive
    let _ = decoder.send_packet(packet);

    let mut frames = Vec::new();

    loop {
        let mut frame = ffmpeg::frame::Audio::empty();
        if !decoder.receive_frame(&mut frame).is_ok() {
            break;
        }

        // Resample if needed
        let sample_buf = if let Some(resampler) = resampler.as_mut() {
            let mut converted = ffmpeg::frame::Audio::empty();
            let Some(mask) = out_ch_layout.mask() else {
                continue;
            };
            unsafe {
                converted.alloc(
                    out_format,
                    out_ch_layout.channels() as usize,
                    mask,
                );
            }
            if resampler.run(&frame, &mut converted).is_ok() {
                // Convert converted frame to Vec<f32> for cpal
                samples_to_vec(&converted)
            } else {
                continue;
            }
        } else {
            // No resampling needed
            samples_to_vec(&frame)
        };

        frames.push(sample_buf);
    }

    // Even if send failed with EAGAIN, we still got any available frames from decoder
    if frames.is_empty() {
        None
    } else {
        Some(frames)
    }
}

/// Convert audio frame from ffmpeg to Vec<f32> for cpal output
fn samples_to_vec(frame: &ffmpeg::frame::Audio) -> Vec<f32> {
    let format = frame.format();
    let channels = frame.ch_layout().channels() as usize;
    let n_samples = frame.samples();

    let mut result = Vec::with_capacity(n_samples * channels);

    match format {
        ffmpeg::format::Sample::F32(ty) => {
            // Data is already f32, just copy
            match ty {
                ffmpeg::format::sample::Type::Packed => {
                    // Packed: all channels interleaved in the first plane (index 0)
                    let data = frame.data(0);
                    // Each sample is 4 bytes
                    for sample_bytes in data.chunks_exact(4) {
                        if sample_bytes.len() == 4 {
                            let bytes = [sample_bytes[0], sample_bytes[1], sample_bytes[2], sample_bytes[3]];
                            result.push(f32::from_le_bytes(bytes));
                        }
                    }
                }
                ffmpeg::format::sample::Type::Planar => {
                    // Planar: each channel in its own plane
                    for ch in 0..channels {
                        if ch >= frame.planes() {
                            continue; // bounds check
                        }
                        let data = frame.data(ch);
                        for sample_bytes in data.chunks_exact(4) {
                            if sample_bytes.len() == 4 {
                                let bytes = [sample_bytes[0], sample_bytes[1], sample_bytes[2], sample_bytes[3]];
                                result.push(f32::from_le_bytes(bytes));
                            }
                        }
                    }
                }
            }
        }
        // Other formats would need conversion, but we always resample to f32 so this shouldn't happen
        _ => {
            // If we get here, just return empty - it'll skip
        }
    }

    result
}

/// Convert any video frame format to RGB8 image buffer using pre-allocated swscale context
fn frame_to_rgb(
    frame: &ffmpeg::frame::Video,
    _sws_context: Option<&mut scaling::Context>
) -> Result<RgbImage, MultimediaError> {
    let width = frame.width();
    let height = frame.height();

    // Most common video format is YUV420p, convert directly like extract does
    // This avoids issues with swscale failing for unknown reasons
    if frame.format() == ffmpeg::format::Pixel::YUV420P {
        let mut rgb_frame: RgbImage = ImageBuffer::new(width, height);

        // ffmpeg's frame data is in YUV420p, we need to convert
        let y = frame.data(0);
        let u = frame.data(1);
        let v = frame.data(2);
        let linesize_y = frame.stride(0);
        let linesize_u = frame.stride(1);
        let linesize_v = frame.stride(2);

        for (y_row, y_line) in y.chunks_exact(linesize_y).take(height as usize).enumerate() {
            for (x_col, &y_val) in y_line.iter().take(width as usize).enumerate() {
                let uv_row = y_row / 2;
                let uv_x = x_col / 2;
                let u_val = u[uv_row * linesize_u + uv_x];
                let v_val = v[uv_row * linesize_v + uv_x];

                // Convert YUV to RGB (BT.601)
                let yf = y_val as f32;
                let uf = u_val as f32 - 128.0;
                let vf = v_val as f32 - 128.0;

                let r = (yf + 1.402 * vf).clamp(0.0, 255.0) as u8;
                let g = (yf - 0.34414 * uf - 0.71414 * vf).clamp(0.0, 255.0) as u8;
                let b = (yf + 1.772 * uf).clamp(0.0, 255.0) as u8;

                rgb_frame.put_pixel(x_col as u32, y_row as u32, image::Rgb([r, g, b]));
            }
        }

        Ok(rgb_frame)
    } else {
        // Fall back to swscale for other formats
        use ffmpeg::format::Pixel;

        let width = frame.width();
        let height = frame.height();

        let mut temp_rgb_frame = ffmpeg::frame::Video::empty();
        temp_rgb_frame.set_width(width);
        temp_rgb_frame.set_height(height);
        temp_rgb_frame.set_format(Pixel::RGB24);
        unsafe {
            temp_rgb_frame.alloc(Pixel::RGB24, width, height);
        }

        if let Some(context) = _sws_context {
            if context.run(frame, &mut temp_rgb_frame).is_err() {
                return Err(MultimediaError::InvalidMedia("Failed to convert frame format".into()));
            }
        }

        // Now convert RGB frame to image buffer
        let mut out: RgbImage = ImageBuffer::new(width, height);
        let data = temp_rgb_frame.data(0);
        let stride = temp_rgb_frame.stride(0) as usize;

        let data_len = data.len();

        for y_row in 0..height {
            let y_row_usize = y_row as usize;
            let line_start = y_row_usize * stride;

            for x_col in 0..width {
                let x_col_usize = x_col as usize;
                let offset = line_start + x_col_usize * 3;

                // Very conservative bounds checking
                if offset + 2 >= data_len {
                    continue;
                }

                let r = data[offset];
                let g = data[offset + 1];
                let b = data[offset + 2];
                out.put_pixel(x_col, y_row, image::Rgb([r, g, b]));
            }
        }

        Ok(out)
    }
}

/// Export all ASCII frames to text files in a directory (for debugging)
fn export_ascii_frames(
    input_path: &Path,
    export_dir: &std::path::PathBuf,
    output_width: u32,
    output_height: u32,
    color_mode: crate::multimedia::traits::AsciiColorMode,
    video_stream_index: usize,
    max_frames: Option<usize>,
) -> Result<(), MultimediaError> {
    ffmpeg::init()?;

    let mut input_ctx = ffmpeg::format::input(input_path)?;

    // Find video stream and recreate decoder
    let mut video_decoder_ctx = None;
    for (i, s) in input_ctx.streams().enumerate() {
        if i == video_stream_index {
            let context = match ffmpeg::codec::context::Context::from_parameters(s.parameters()) {
                Ok(ctx) => ctx,
                Err(_) => return Err(MultimediaError::InvalidMedia("Failed to create decoder".into())),
            };
            video_decoder_ctx = match context.decoder().video() {
                Ok(d) => Some(d),
                Err(_) => return Err(MultimediaError::InvalidMedia("Failed to open video decoder".into())),
            };
            break;
        }
    }

    let Some(mut video_decoder_ctx) = video_decoder_ctx else {
        return Err(MultimediaError::NoVideoStream);
    };

    let original_width = video_decoder_ctx.width();
    let original_height = video_decoder_ctx.height();

    // Create swscale context once (only used for non-YUV420p formats)
    let mut sws_context: Option<scaling::Context> = None;
    if video_decoder_ctx.format() != ffmpeg::format::Pixel::RGB24 && video_decoder_ctx.format() != ffmpeg::format::Pixel::YUV420P {
        sws_context = match scaling::Context::get(
            video_decoder_ctx.format(),
            video_decoder_ctx.height(),
            video_decoder_ctx.width(),
            ffmpeg::format::Pixel::RGB24,
            video_decoder_ctx.height(),
            video_decoder_ctx.width(),
            scaling::Flags::BILINEAR,
        ) {
            Ok(ctx) => Some(ctx),
            Err(_) => return Err(MultimediaError::InvalidMedia("Failed to create scaler".into())),
        };
    }

    let mut frame_count = 0;
    let mut packet_count = 0;
    let max_frames = max_frames.unwrap_or(usize::MAX);

    // Process every packet
    'export: for (stream, mut packet) in input_ctx.packets().filter_map(Result::ok) {
        if stream.index() != video_stream_index {
            continue;
        }

        packet_count += 1;
        if packet_count % 50 == 0 {
            println!("  Processed {} packets, decoded {} frames...", packet_count, frame_count);
        }

        if frame_count >= max_frames {
            break 'export;
        }

        // Decode the frame
        match decode_packet_video(&packet, &mut video_decoder_ctx, original_width, original_height) {
            Ok(Some(frame)) => {
                // Convert to RGB
                let rgb = match frame_to_rgb(&frame, sws_context.as_mut()) {
                    Ok(rgb) => rgb,
                    Err(_) => continue,
                };
                let dyn_img = DynamicImage::ImageRgb8(rgb);

                // Convert to ASCII
                let ascii_lines = image_to_ascii(
                    &dyn_img,
                    output_width,
                    output_height,
                    color_mode,
                );

                // Save to file with padded numbering
                let frame_path = export_dir.join(format!("frame_{:06}.txt", frame_count));
                let mut file = std::fs::File::create(frame_path)?;
                use std::io::Write;
                for line in &ascii_lines {
                    writeln!(file, "{}", line)?;
                }
                file.flush()?;

                frame_count += 1;

                if frame_count % 10 == 0 {
                    println!("Exported {} frames...", frame_count);
                }
            }
            Ok(None) | Err(_) => {
                continue;
            }
        }
    }

    println!("Export complete: {} frames saved to {}", frame_count, export_dir.display());
    Ok(())
}
