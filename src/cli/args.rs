//! Command line argument types

use clap::Parser;
use std::path::PathBuf;

/// Multimedia subcommands
#[derive(Parser, Debug, Clone)]
pub enum MultimediaSubcommands {
    /// Show media file information (duration, resolution, codec, etc.)
    Info {
        /// Input media file path
        input: PathBuf,
    },

    /// Transcode/convert audio/video to different format
    Transcode {
        /// Input media file path
        input: PathBuf,

        /// Output file path
        output: PathBuf,

        /// Output codec (optional, auto-detected from extension)
        #[arg(short, long)]
        codec: Option<String>,

        /// Output bitrate in kbps (e.g. 5000 for 5Mbps)
        #[arg(short, long)]
        bitrate: Option<u32>,

        /// Output resolution (e.g. 1920x1080)
        #[arg(short, long)]
        resolution: Option<String>,
    },

    /// Extract frames from video
    ExtractFrames {
        /// Input video file path
        input: PathBuf,

        /// Output directory for extracted frames
        output_dir: PathBuf,

        /// Extract frames at specific times (seconds from start, comma-separated)
        #[arg(short, long, group = "selection", conflicts_with = "frames")]
        times: Option<String>,

        /// Extract specific frame numbers (comma-separated)
        #[arg(short, long, group = "selection", conflicts_with = "times")]
        frames: Option<String>,

        /// Output image format (png or jpeg)
        #[arg(short = 'F', long, default_value = "png")]
        format: String,

        /// Extract every Nth frame
        #[arg(short, long, group = "selection")]
        every: Option<u32>,
    },
}

/// Top-level CLI arguments
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
pub struct CliArgs {
    /// Run in headless mode (non-interactive)
    #[arg(short = 'H', long)]
    pub headless: bool,

    /// Optional config file path for headless mode
    #[arg(short, long)]
    pub config: Option<std::path::PathBuf>,

    /// Search for files matching the pattern (supports glob-like * ?)
    #[arg(short, long)]
    pub search: Option<String>,

    /// Root directory/drive to start search from (e.g. C:\ or ./my-folder)
    #[arg(short = 'd', long)]
    pub root: Option<std::path::PathBuf>,

    /// Force generic directory walking even when NTFS MFT is available
    #[arg(long)]
    pub force_generic: bool,

    /// Use case-sensitive search
    #[arg(long)]
    pub case_sensitive: bool,

    /// Maximum number of results to return
    #[arg(short, long)]
    pub limit: Option<usize>,

    /// Multimedia-related subcommands (info, transcode, extract-frames)
    #[command(subcommand)]
    pub multimedia: Option<MultimediaSubcommands>,
}
