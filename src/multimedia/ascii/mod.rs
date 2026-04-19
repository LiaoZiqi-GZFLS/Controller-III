//! ASCII video playback - plays video as ASCII art in terminal

use anyhow::Result;
use std::path::Path;

use crate::multimedia::{
    error::MultimediaError,
    traits::{AsciiPlayer, AsciiPlayOptions},
};

#[cfg(feature = "ffmpeg")]
use crate::multimedia::ascii::ffmpeg::FfmpegAsciiPlayer;

#[cfg(feature = "ffmpeg")]
mod ffmpeg;
mod convert;
mod terminal;
mod player;
pub mod native;

pub use convert::*;
pub use terminal::*;
pub use player::*;

/// Play video as ASCII art in terminal with automatic backend selection
pub fn play_ascii(input: &Path, options: AsciiPlayOptions) -> Result<()> {
    #[cfg(feature = "ffmpeg")]
    {
        if ffmpeg_the_third::init().is_ok() {
            let mut player = FfmpegAsciiPlayer::new();
            if player.is_available(input) {
                return player.play(input, options).map_err(Into::into);
            }
        }
    }

    // Try native fallback (MP4 + H.264 only)
    let mut player = native::NativeAsciiPlayer::new();
    if player.is_available(input) {
        eprintln!("⚠️  FFmpeg not available, using native MP4+H.264 fallback...");
        return player.play(input, options).map_err(Into::into);
    }

    Err(MultimediaError::Unsupported(
        "ASCII video playback failed: FFmpeg not available and input is not supported by native fallback (only MP4+H.264 supported).".to_string()
    ).into())
}
