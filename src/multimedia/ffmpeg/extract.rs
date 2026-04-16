//! FFmpeg implementation of frame extractor

use std::path::{Path, PathBuf};
use std::fs;
use ffmpeg_the_third as ffmpeg;
use ffmpeg::{decoder};
use ffmpeg::media;
use image::{ImageBuffer, RgbImage};

use crate::multimedia::{
    error::MultimediaError,
    traits::{FrameExtractor, ExtractOptions, ExtractSelection},
};

/// FFmpeg-based frame extractor
pub struct FfmpegFrameExtractor;

impl FfmpegFrameExtractor {
    pub fn new() -> Self {
        Self
    }

    /// Calculate PTS (presentation timestamp) to seek to
    fn pts_for_time(&self, time_sec: f64, time_base: (i32, i32)) -> i64 {
        (time_sec * (time_base.0 as f64 / time_base.1 as f64)).round() as i64
    }
}

impl FrameExtractor for FfmpegFrameExtractor {
    fn extract_frames(
        &mut self,
        input: &Path,
        output_dir: &Path,
        options: ExtractOptions,
    ) -> Result<usize, MultimediaError> {
        ffmpeg::init()?;

        // Create output directory if it doesn't exist
        if !output_dir.exists() {
            fs::create_dir_all(output_dir)
                .map_err(|_| MultimediaError::CreateOutputDir(output_dir.to_string_lossy().to_string()))?;
        }

        let mut input_ctx = ffmpeg::format::input(input)?;

        // Find the first video stream and create decoder immediately
        let mut stream_index = 0;
        let mut time_base = (0, 0);
        let mut avg_frame_rate = ffmpeg::Rational::new(0, 0);
        let mut decoder_ctx = None;

        for (i, s) in input_ctx.streams().enumerate() {
            if s.parameters().medium() == media::Type::Video {
                stream_index = i;
                time_base = (s.time_base().numerator(), s.time_base().denominator());
                avg_frame_rate = s.avg_frame_rate();

                // Create decoder immediately while parameters are still borrowed
                let context = ffmpeg::codec::context::Context::from_parameters(s.parameters())?;
                decoder_ctx = Some(context.decoder().video()?);
                break;
            }
        }

        let mut decoder_ctx = decoder_ctx.ok_or(MultimediaError::NoVideoStream)?;

        // Get width and height from opened decoder
        let (width, height) = (decoder_ctx.width(), decoder_ctx.height());

        let input_stem = input.file_stem()
            .unwrap_or_default()
            .to_string_lossy();

        let output_format = match options.format.as_str() {
            "png" => "png",
            "jpeg" | "jpg" => "jpeg",
            _ => "png",
        };

        let mut extracted = 0;

        match options.selection {
            ExtractSelection::Times(times) => {
                for (i, &time) in times.iter().enumerate() {
                    let pts = self.pts_for_time(time, time_base);
                    input_ctx.seek(pts, pts..=pts)?;

                    if let Some(frame) = decode_next_frame(&mut input_ctx, stream_index, &mut decoder_ctx, width, height)? {
                        save_frame(frame, output_dir, &input_stem, i + 1, output_format)?;
                        extracted += 1;
                    }
                }
            }

            ExtractSelection::Frames(frames) => {
                // Calculate frame rate to get PTS
                let avg_fps = avg_frame_rate;
                let fps = if avg_fps.denominator() != 0 {
                    avg_fps.numerator() as f64 / avg_fps.denominator() as f64
                } else {
                    30.0
                };
                for (i, &frame_num) in frames.iter().enumerate() {
                    let time = frame_num as f64 / fps;
                    let pts = self.pts_for_time(time, time_base);
                    input_ctx.seek(pts, pts..=pts)?;

                    if let Some(frame) = decode_next_frame(&mut input_ctx, stream_index, &mut decoder_ctx, width, height)? {
                        save_frame(frame, output_dir, &input_stem, i + 1, output_format)?;
                        extracted += 1;
                    }
                }
            }

            ExtractSelection::EveryNth(every) => {
                let mut current_frame = 0;

                for (stream, packet) in input_ctx.packets().filter_map(Result::ok) {
                    if stream.index() != stream_index {
                        continue;
                    }

                    if current_frame % every == 0 {
                        if let Some(frame) = decode_packet(&packet, &mut decoder_ctx, width, height)? {
                            save_frame(frame, output_dir, &input_stem, (current_frame / every + 1) as usize, output_format)?;
                            extracted += 1;
                        }
                    }

                    current_frame += 1;
                }
            }
        }

        Ok(extracted)
    }

    fn is_available(&self, _input: &Path) -> bool {
        ffmpeg::init().is_ok()
    }
}

fn decode_next_frame(
    input_ctx: &mut ffmpeg::format::context::Input,
    stream_index: usize,
    decoder: &mut decoder::Video,
    width: u32,
    height: u32,
) -> Result<Option<ffmpeg::frame::Video>, MultimediaError> {
    for (stream, packet) in input_ctx.packets().filter_map(Result::ok) {
        if stream.index() != stream_index {
            continue;
        }
        if let Some(frame) = decode_packet(&packet, decoder, width, height)? {
            return Ok(Some(frame));
        }
    }
    Ok(None)
}

fn decode_packet(
    packet: &ffmpeg::Packet,
    decoder: &mut decoder::Video,
    _width: u32,
    _height: u32,
) -> Result<Option<ffmpeg::frame::Video>, MultimediaError> {
    decoder.send_packet(packet)?;

    let mut frame = ffmpeg::frame::Video::empty();
    if decoder.receive_frame(&mut frame).is_ok() {
        // Convert to RGB if needed
        Ok(Some(frame))
    } else {
        Ok(None)
    }
}

fn save_frame(
    frame: ffmpeg::frame::Video,
    output_dir: &Path,
    input_stem: &str,
    index: usize,
    format: &str,
) -> Result<(), MultimediaError> {
    let width = frame.width();
    let height = frame.height();

    // Convert YUV420p to RGB
    let mut rgb_frame: RgbImage = ImageBuffer::new(width, height);

    // ffmpeg's frame data is in YUV420p, we need to convert.
    // This is a simple conversion - good enough for extraction
    let y = frame.data(0);
    let u = frame.data(1);
    let v = frame.data(2);
    let linesize_y = frame.stride(0);
    let linesize_u = frame.stride(1);
    let linesize_v = frame.stride(2);

    for (y_row, y_line) in y.chunks_exact(linesize_y).take(height as usize).enumerate() {
        for (x_row, &y_val) in y_line.iter().take(width as usize).enumerate() {
            let uv_row = y_row / 2;
            let uv_x = x_row / 2;
            let u_val = u[uv_row * linesize_u + uv_x];
            let v_val = v[uv_row * linesize_v + uv_x];

            // Convert YUV to RGB (BT.601)
            let yf = y_val as f32;
            let uf = u_val as f32 - 128.0;
            let vf = v_val as f32 - 128.0;

            let r = (yf + 1.402 * vf).clamp(0.0, 255.0) as u8;
            let g = (yf - 0.34414 * uf - 0.71414 * vf).clamp(0.0, 255.0) as u8;
            let b = (yf + 1.772 * uf).clamp(0.0, 255.0) as u8;

            rgb_frame.put_pixel(x_row as u32, y_row as u32, image::Rgb([r, g, b]));
        }
    }

    let output_path: PathBuf = output_dir.join(format!("{}_frame_{:06}.{}", input_stem, index, format));

    match format {
        "png" => rgb_frame.save(output_path)?,
        "jpeg" | "jpg" => {
            let mut out_file = std::fs::File::create(output_path)?;
            let mut encoder = image::codecs::jpeg::JpegEncoder::new(&mut out_file);
            encoder.encode(&rgb_frame, width, height, image::ExtendedColorType::Rgb8)?;
        }
        _ => unreachable!(),
    }

    Ok(())
}
