//! Top-level trim (cut by time range) functionality

use crate::multimedia::error::MultimediaError;
#[cfg(feature = "ffmpeg")]
use crate::multimedia::ffmpeg::trim::trim as ffmpeg_trim;
use anyhow::Result;
use std::path::Path;

/// Trim media to a time range using FFmpeg
#[allow(unused_variables)]
pub fn trim(input: &Path, output: &Path, start: f64, duration: Option<f64>) -> Result<()> {
    #[cfg(feature = "ffmpeg")]
    {
        ffmpeg_trim(input, output, start, duration).map_err(Into::into)
    }

    #[cfg(not(feature = "ffmpeg"))]
    {
        Err(MultimediaError::Unsupported(
            "Trimming requires FFmpeg. Rebuild with the `ffmpeg` feature.".into(),
        )
        .into())
    }
}
