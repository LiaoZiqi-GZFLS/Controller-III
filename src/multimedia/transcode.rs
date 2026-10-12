//! Top-level transcode logic with backend auto-selection

use anyhow::Result;
use std::path::Path;

#[cfg(feature = "ffmpeg")]
use crate::multimedia::ffmpeg::FfmpegTranscoder;
#[cfg(feature = "ffmpeg")]
use crate::multimedia::traits::Transcoder;
use crate::multimedia::{error::MultimediaError, traits::TranscoderOptions};

/// Transcode media with automatic backend selection
#[allow(unused_variables)]
pub fn transcode(input: &Path, output: &Path, options: TranscoderOptions) -> Result<()> {
    #[cfg(feature = "ffmpeg")]
    {
        let mut transcoder = FfmpegTranscoder::new();

        if !transcoder.is_available() {
            return Err(MultimediaError::Unsupported(
                "Format conversion requires FFmpeg. Please install FFmpeg to use this feature."
                    .into(),
            )
            .into());
        }

        Ok(transcoder.transcode(input, output, options)?)
    }

    #[cfg(not(feature = "ffmpeg"))]
    Err(MultimediaError::Unsupported(
        "Format conversion requires FFmpeg. Please install FFmpeg and rebuild with --features ffmpeg.".into()
    ).into())
}
