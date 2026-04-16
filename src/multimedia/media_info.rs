//! Top-level media information with backend auto-selection

use std::path::Path;
use anyhow::Result;

use crate::multimedia::{
    info::{MediaInfo},
    traits::MediaInfoProvider,
    native::NativeMediaInfoProvider,
    error::MultimediaError,
};
#[cfg(feature = "ffmpeg")]
use crate::multimedia::ffmpeg::FfmpegMediaInfoProvider;

/// Get media information with automatic backend selection
pub fn get_media_info(input: &Path) -> Result<MediaInfo> {
    // Try FFmpeg first
    #[cfg(feature = "ffmpeg")]
    {
        if let Ok(_) = ffmpeg_the_third::init() {
            let mut provider = FfmpegMediaInfoProvider::new();
            return Ok(provider.get_info(input)?);
        }
    }

    // FFmpeg not available or not enabled, try native (only MP4)
    if let Some(ext) = input.extension() {
        if ext.to_string_lossy().to_lowercase() == "mp4" {
            eprintln!("⚠️  FFmpeg not available, using native MP4 parser...");
            let mut provider = NativeMediaInfoProvider::new();
            return Ok(provider.get_info(input)?);
        }
    }

    Err(MultimediaError::Unsupported(
        "Media information requires FFmpeg for non-MP4 files. Please install FFmpeg.".into()
    ).into())
}
