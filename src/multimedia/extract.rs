//! Top-level frame extraction with backend auto-selection

use std::path::Path;
use anyhow::Result;

use crate::multimedia::{
    error::MultimediaError,
    traits::{FrameExtractor, ExtractOptions},
    native::NativeFrameExtractor,
};
#[cfg(feature = "ffmpeg")]
use crate::multimedia::ffmpeg::FfmpegFrameExtractor;

/// Extract frames with automatic backend selection
pub fn extract_frames(
    input: &Path,
    output_dir: &Path,
    options: ExtractOptions,
) -> Result<usize> {
    #[cfg(feature = "ffmpeg")]
    {
        let mut ffmpeg_extractor = FfmpegFrameExtractor::new();

        if ffmpeg_extractor.is_available(input) {
            return Ok(ffmpeg_extractor.extract_frames(input, output_dir, options)?);
        }
    }

    // FFmpeg not available or not enabled, try native fallback (only MP4 + H.264)
    let mut native_extractor = NativeFrameExtractor::new();

    if native_extractor.is_available(input) {
        eprintln!("⚠️  FFmpeg not available, using native MP4+H.264 fallback...");
        return Ok(native_extractor.extract_frames(input, output_dir, options)?);
    }

    Err(MultimediaError::Unsupported(
        "Frame extraction failed: FFmpeg not available and input is not supported by native fallback (only MP4+H.264 supported).".into()
    ).into())
}
