//! Top-level extract audio functionality

use crate::multimedia::error::MultimediaError;
#[cfg(feature = "ffmpeg")]
use crate::multimedia::ffmpeg::extract_audio::extract_audio as ffmpeg_extract_audio;
use anyhow::Result;
use std::path::Path;

/// Extract audio from media using FFmpeg
#[allow(unused_variables)]
pub fn extract_audio(
    input: &Path,
    output: &Path,
    bitrate: Option<u32>,
    codec: Option<String>,
) -> Result<()> {
    #[cfg(feature = "ffmpeg")]
    {
        ffmpeg_extract_audio(input, output, bitrate, codec).map_err(Into::into)
    }

    #[cfg(not(feature = "ffmpeg"))]
    {
        Err(MultimediaError::Unsupported(
            "Audio extraction requires FFmpeg. Rebuild with the `ffmpeg` feature.".into(),
        )
        .into())
    }
}
