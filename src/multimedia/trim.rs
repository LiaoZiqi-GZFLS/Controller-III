//! Top-level trim (cut by time range) functionality

use std::path::Path;
use anyhow::Result;
use crate::multimedia::{error::MultimediaError, ffmpeg::trim::trim as ffmpeg_trim};

/// Trim media to a time range using FFmpeg
pub fn trim(input: &Path, output: &Path, start: f64, duration: Option<f64>) -> Result<()> {
    // Always use FFmpeg for trimming (native doesn't support)
    ffmpeg_trim(input, output, start, duration)
        .map_err(Into::into)
}
