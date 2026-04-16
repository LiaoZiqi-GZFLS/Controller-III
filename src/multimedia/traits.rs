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

    pub fn frames(frames: Vec<u64>) -> Self {
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
