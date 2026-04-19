//! Top-level extract audio functionality

use std::path::Path;
use anyhow::Result;
use crate::multimedia::{error::MultimediaError, ffmpeg::extract_audio::extract_audio as ffmpeg_extract_audio};

/// Extract audio from media using FFmpeg
pub fn extract_audio(input: &Path, output: &Path, bitrate: Option<u32>, codec: Option<String>) -> Result<()> {
    // Always use FFmpeg for audio extraction (native doesn't support)
    ffmpeg_extract_audio(input, output, bitrate, codec)
        .map_err(Into::into)
}
