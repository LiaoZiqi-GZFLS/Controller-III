//! Trait definitions for multimedia backends

use anyhow::Result;
use std::path::Path;

use crate::multimedia::{info::MediaInfo, error::MultimediaError};

/// Media information provider
pub trait MediaInfoProvider {
    /// Get media information from a file
    fn get_info(&mut self, path: &Path) -> Result<MediaInfo, MultimediaError>;
}

/// Format transcoder
pub trait Transcoder {
    /// Transcode media from input to output with given options
    fn transcode(
        &mut self,
        input: &Path,
        output: &Path,
        options: TranscoderOptions,
    ) -> Result<(), MultimediaError>;

    /// Check if this transcoder is available
    fn is_available(&self) -> bool;
}

/// Frame extractor
pub trait FrameExtractor {
    /// Extract frames from video and save them to output directory
    fn extract_frames(
        &mut self,
        input: &Path,
        output_dir: &Path,
        options: ExtractOptions,
    ) -> Result<usize, MultimediaError>;

    /// Check if this extractor is available
    fn is_available(&self, input: &Path) -> bool;
}

/// Transcoding options
#[derive(Debug, Clone, Default)]
pub struct TranscoderOptions {
    /// Output codec (if None, auto-detect from extension)
    pub codec: Option<String>,
    /// Output video bitrate in kbps
    pub bitrate: Option<u32>,
    /// Output resolution (width x height)
    pub resolution: Option<(u32, u32)>,
}

/// Frame extraction options
#[derive(Debug, Clone)]
pub struct ExtractOptions {
    /// Which frames to extract
    pub selection: ExtractSelection,
    /// Output image format (png or jpeg)
    pub format: String,
}

#[derive(Debug, Clone)]
pub enum ExtractSelection {
    /// Extract at specific timestamps (seconds)
    Times(Vec<f64>),
    /// Extract specific frame numbers
    Frames(Vec<u64>),
    /// Extract every Nth frame
    EveryNth(u32),
}

impl Default for ExtractOptions {
    fn default() -> Self {
        Self {
            selection: ExtractSelection::EveryNth(1),
            format: "png".to_string(),
        }
    }
}

// Convenience constructors
impl ExtractOptions {
    pub fn times(times: Vec<f64>) -> Self {
        Self {
            selection: ExtractSelection::Times(times),
            format: "png".to_string(),
        }
    }

    pub fn frames(frames: Vec<f64>) -> Self {
        Self {
            selection: ExtractSelection::Times(frames),
            format: "png".to_string(),
        }
    }

    pub fn frames_u64(frames: Vec<u64>) -> Self {
        Self {
            selection: ExtractSelection::Frames(frames),
            format: "png".to_string(),
        }
    }

    pub fn every_nth(n: u32) -> Self {
        Self {
            selection: ExtractSelection::EveryNth(n),
            format: "png".to_string(),
        }
    }
}

/// ASCII video player - plays video as ASCII art in terminal
pub trait AsciiPlayer {
    /// Play video from file to terminal with given options
    fn play(
        &mut self,
        input: &Path,
        options: AsciiPlayOptions,
    ) -> Result<(), MultimediaError>;

    /// Check if this player is available for input
    fn is_available(&self, input: &Path) -> bool;
}

/// Color output mode for ASCII playback
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AsciiColorMode {
    /// No color, only ASCII brightness gradient
    None,
    /// 256-color ANSI (compatible with most terminals)
    Ansi256,
    /// 24-bit RGB true color (modern terminals only)
    TrueColor,
}

/// Scaling mode for ASCII playback
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AsciiScaleMode {
    /// No scaling - use original dimensions, don't fit to window
    NoScale,
    /// Fit to entire window - may change aspect ratio
    FitWindow,
    /// Fit to window while preserving original aspect ratio (default)
    KeepAspect,
}

impl Default for AsciiScaleMode {
    fn default() -> Self {
        Self::KeepAspect
    }
}

/// ASCII playback options
#[derive(Debug, Clone)]
pub struct AsciiPlayOptions {
    /// Width in characters (auto-detect from terminal if None)
    pub width: Option<u32>,
    /// Height in characters (auto-calculate from aspect ratio if None)
    pub height: Option<u32>,
    /// Speed multiplier (1.0 = normal speed)
    pub speed: f64,
    /// Show FPS counter during playback
    pub show_fps: bool,
    /// Color output mode
    pub color_mode: AsciiColorMode,
    /// Scaling mode for fitting to terminal
    pub scale_mode: AsciiScaleMode,
    /// Export ASCII frames to this directory instead of playing
    pub export_dir: Option<std::path::PathBuf>,
    /// Maximum number of frames to export (if None, export all)
    pub export_max_frames: Option<usize>,
}

impl Default for AsciiPlayOptions {
    fn default() -> Self {
        Self {
            width: None,
            height: None,
            speed: 1.0,
            show_fps: false,
            color_mode: AsciiColorMode::None,
            scale_mode: AsciiScaleMode::default(),
            export_dir: None,
            export_max_frames: None,
        }
    }
}
