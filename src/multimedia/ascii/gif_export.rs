//! ASCII GIF export - decode video, convert to ASCII, render frames to GIF

use std::path::Path;
use std::fs::File;

use ffmpeg_the_third as ffmpeg;
use image::{ImageBuffer, Rgb, Frame, Delay};
use image::codecs::gif::GifEncoder;

use crate::multimedia::{
    error::MultimediaError,
    traits::{AsciiColorMode, AsciiScaleMode},
    ascii::{
        calculate_dimensions, frame_to_ascii_grid,
        gif_renderer::{GifRenderer, GifStyle},
    },
};

/// Options for ASCII GIF export
#[derive(Debug, Clone)]
pub struct GifExportOptions {
    pub fps: f64,
    pub width: u32,
    pub height: Option<u32>,
    pub style: GifStyle,
    pub color_mode: AsciiColorMode,
    pub start: Option<f64>,
    pub duration: Option<f64>,
}

/// Export video to ASCII art animated GIF
pub fn export_ascii_gif(
    input: &Path,
    output: &Path,
    options: GifExportOptions,
) -> Result<(), MultimediaError> {
    ffmpeg::init()?;

    let mut input_ctx = ffmpeg::format::input(input)?;

    // Find video stream
    let mut video_stream_index = 0usize;
    let mut original_width = 0u32;
    let mut original_height = 0u32;
    let mut video_time_base = ffmpeg::Rational::new(1, 1);

    for (i, s) in input_ctx.streams().enumerate() {
        if s.parameters().medium() == ffmpeg::media::Type::Video {
            video_stream_index = i;
            video_time_base = s.time_base();
            if let Ok(context) = ffmpeg::codec::context::Context::from_parameters(s.parameters()) {
                if let Ok(decoder) = context.decoder().video() {
                    original_width = decoder.width();
                    original_height = decoder.height();
                }
            }
        }
    }

    if original_width == 0 || original_height == 0 {
        return Err(MultimediaError::NoVideoStream);
    }

    // Calculate ASCII grid dimensions
    let default_height = options.width * original_height / original_width;
    let (output_width, output_height) = calculate_dimensions(
        original_width,
        original_height,
        Some(options.width),
        options.height,
        options.width,
        default_height,
        AsciiScaleMode::KeepAspect,
    );

    // Setup renderer and encoder
    let renderer = GifRenderer::new(options.style)?;
    let output_file = File::create(output)?;
    let mut gif_encoder = GifEncoder::new(output_file);

    // Setup decoder
    let stream = &input_ctx.streams()[video_stream_index];
    let context = ffmpeg::codec::context::Context::from_parameters(stream.parameters())?;
    let mut decoder = context.decoder().video()?;

    // Create scaler for RGB conversion
    let mut scaler = ffmpeg::software::scaling::Context::get(
        decoder.format(),
        decoder.height(),
        decoder.width(),
        ffmpeg::format::Pixel::RGB24,
        decoder.height(),
        decoder.width(),
        ffmpeg::software::scaling::flag::Flags::BILINEAR,
    )?;

    // Calculate time range
    let start_time = options.start.unwrap_or(0.0);
    let end_time = options.duration.map(|d| start_time + d).unwrap_or_else(|| {
        input_ctx.duration() as f64 / ffmpeg::ffi::AV_TIME_BASE as f64
    });

    let dt = 1.0 / options.fps;
    let frame_delay = Delay::from_numer_denom_ms((dt * 1000.0) as u32, 1);
    let mut last_sampled_pts = -1.0f64;

    // Decode and sample frames
    for (stream, packet) in input_ctx.packets() {
        if stream.index() != video_stream_index {
            continue;
        }

        decoder.send_packet(&packet)?;

        let mut decoded = ffmpeg::frame::Video::empty();
        while decoder.receive_frame(&mut decoded).is_ok() {
            let pts = decoded.timestamp().unwrap_or(0) as f64 * f64::from(video_time_base);

            if pts < start_time {
                continue;
            }
            if pts > end_time {
                break;
            }

            // Sample at fixed FPS intervals
            let frame_idx = ((pts - start_time) / dt).round() as i64;
            let target_pts = start_time + frame_idx as f64 * dt;

            if pts >= target_pts && last_sampled_pts < target_pts {
                last_sampled_pts = target_pts;

                // Convert to RGB
                let mut rgb_frame = ffmpeg::frame::Video::empty();
                scaler.run(&decoded, &mut rgb_frame)?;

                // Create RgbImage from raw data
                let width = rgb_frame.width();
                let height = rgb_frame.height();
                let data = rgb_frame.data(0);
                let img = ImageBuffer::<Rgb<u8>, Vec<u8>>::from_raw(width, height, data.to_vec())
                    .ok_or_else(|| MultimediaError::InvalidParameter(
                        "Failed to create image buffer".to_string()
                    ))?;

                // Convert to ASCII and render
                let grid = frame_to_ascii_grid(
                    &image::DynamicImage::ImageRgb8(img),
                    output_width,
                    output_height,
                    options.color_mode,
                );
                let rendered = renderer.render_frame(&grid);

                // Encode frame
                let frame = Frame::from_parts(rendered, 0, 0, frame_delay);
                gif_encoder.encode_frame(frame)
                    .map_err(|e| MultimediaError::ImageError(e.to_string()))?;
            }
        }

        if last_sampled_pts >= end_time {
            break;
        }
    }

    Ok(())
}
