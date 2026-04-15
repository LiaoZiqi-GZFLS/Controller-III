//! Command line argument types

use clap::Parser;

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
}
