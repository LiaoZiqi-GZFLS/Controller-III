//! ASCII video playback - plays video as ASCII art in terminal

use anyhow::Result;
use std::path::Path;

use crate::multimedia::{
    error::MultimediaError,
    traits::{AsciiPlayOptions, AsciiPlayer},
};

#[cfg(feature = "ffmpeg")]
use crate::multimedia::ascii::ffmpeg::FfmpegAsciiPlayer;

mod convert;
#[cfg(feature = "ffmpeg")]
mod ffmpeg;
pub mod gif_export;
pub mod gif_renderer;
pub mod native;
mod player;
mod terminal;

pub use convert::*;
#[allow(unused_imports)]
pub use player::*;
pub use terminal::*;

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
