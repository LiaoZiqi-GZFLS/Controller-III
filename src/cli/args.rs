//! Command line argument types

use clap::Parser;
use std::path::PathBuf;

/// Top-level CLI commands
#[derive(Parser, Debug, Clone)]
pub enum Commands {
    /// Multimedia-related subcommands (info, transcode, extract-frames)
    #[command(subcommand)]
    Multimedia(MultimediaSubcommands),
}

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

    /// Trim video/audio to a time range (start to end/duration)
    Trim {
        /// Input media file path
        input: PathBuf,

        /// Output file path
        output: PathBuf,

        /// Start time in seconds (can use 10.5 for 10.5 seconds, or 01:30 for minutes)
        #[arg(short, long)]
        start: f64,

        /// Duration in seconds (if not set, trims from start to end of media)
        #[arg(short, long)]
        duration: Option<f64>,
    },

    /// Extract audio track from video (discard video)
    ExtractAudio {
        /// Input media file (video with audio)
        input: PathBuf,

        /// Output audio file (e.g. output.mp3, output.aac)
        output: PathBuf,

        /// Output bitrate in kbps (e.g. 192 for 192kbps)
        #[arg(short, long)]
        bitrate: Option<u32>,

        /// Output codec (optional, auto-detected from extension)
        #[arg(short, long)]
        codec: Option<String>,
    },

    /// Play video as ASCII art in the terminal
    PlayAscii {
        /// Input video file to play
        input: PathBuf,

        /// Width in ASCII characters (if not set, uses terminal width)
        #[arg(short, long)]
        width: Option<u32>,

        /// Height in ASCII characters (if not set, auto-calculated from aspect ratio)
        #[arg(short = 'H', long)]
        height: Option<u32>,

        /// Speed multiplier (0.5 = half speed, 2.0 = double speed)
        #[arg(short, long, default_value = "1.0")]
        speed: f64,

        /// Show current FPS counter during playback
        #[arg(long)]
        show_fps: bool,

        /// Color output mode: none, ansi256, or truecolor
        #[arg(long, default_value = "none")]
        color_mode: String,

        /// Scaling mode: none (no scaling, use original), fit (fill window, change aspect), keep (keep aspect, default)
        #[arg(long, default_value = "keep")]
        scale_mode: String,

        /// Export ASCII frames to text files instead of playing (output directory)
        #[arg(long)]
        export: Option<PathBuf>,

        /// Maximum number of frames to export (default: all)
        #[arg(long)]
        export_max: Option<usize>,
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
    #[arg(short, long)]
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

    /// Top-level command (multimedia)
    #[command(subcommand)]
    pub command: Option<Commands>,
}
