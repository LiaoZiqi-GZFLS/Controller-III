//! Headless mode - non-interactive execution

use anyhow::Result;
use std::path::PathBuf;
use crate::cli::args::MultimediaSubcommands;
use crate::multimedia::{get_media_info, transcode, extract_frames, TranscoderOptions};
use crate::search::{create_search_engine, query_to_regex};

/// Run in headless mode
pub fn run(
    config_path: Option<PathBuf>,
    search: Option<String>,
    root: Option<PathBuf>,
    force_generic: bool,
    case_sensitive: bool,
    limit: Option<usize>,
) -> Result<()> {
    println!("Running in headless mode...");

    if let Some(query) = search {
        let pattern = query_to_regex(&query, case_sensitive);
        let mut engine = create_search_engine(force_generic);

        let results = if !engine.is_available(root.as_deref()) {
            eprintln!("Warning: Preferred search engine not available, using generic search");
            let mut engine = create_search_engine(true);
            engine.search(&pattern, root.as_deref(), limit)?
        } else {
            match engine.search(&pattern, root.as_deref(), limit) {
                Ok(results) => results,
                Err(e) => {
                    eprintln!("Warning: NTFS search failed: {e}\nFalling back to generic search");
                    let mut engine = create_search_engine(true);
                    engine.search(&pattern, root.as_deref(), limit)?
                }
            }
        };
        print_results(results, engine.count());
    } else if let Some(config) = config_path {
        println!("Using config file: {}", config.display());
        // TODO: Load and process config from file
    }

    Ok(())
}

/// Run multimedia command in headless mode
#[allow(dead_code)]
pub fn run_multimedia(cmd: MultimediaSubcommands) -> Result<()> {
    match cmd {
        MultimediaSubcommands::Info { input } => {
            let info = get_media_info(&input)?;
            println!("\n{}", info.format());
            Ok(())
        }

        MultimediaSubcommands::Transcode { input, output, codec, bitrate, resolution } => {
            let mut options = TranscoderOptions::default();
            options.codec = codec;
            options.bitrate = bitrate;
            options.resolution = if let Some(res_str) = resolution {
                let parts: Vec<&str> = res_str.split('x').collect();
                if parts.len() == 2 {
                    let w: Option<u32> = parts[0].parse().ok();
                    let h: Option<u32> = parts[1].parse().ok();
                    match (w, h) {
                        (Some(w), Some(h)) => Some((w, h)),
                        _ => {
                            eprintln!("Warning: Invalid resolution format, expected WxH (e.g. 1920x1080), ignoring");
                            None
                        }
                    }
                } else {
                    None
                }
            } else {
                None
            };

            println!("Transcoding {} -> {}...", input.display(), output.display());
            transcode(&input, &output, options)?;
            println!("Done!");
            Ok(())
        }

        MultimediaSubcommands::ExtractFrames { input, output_dir, times, frames, format, every } => {
            use crate::multimedia::{ExtractOptions, ExtractSelection};
            let selection = match (times, frames, every) {
                (Some(times_str), None, None) => {
                    let times: Vec<f64> = times_str.split(',')
                        .map(|s| s.trim().parse())
                        .collect::<Result<_, _>>()
                        .map_err(|_| anyhow::anyhow!("Invalid times format, expected comma-separated numbers"))?;
                    ExtractSelection::Times(times)
                }

                (None, Some(frames_str), None) => {
                    let frames: Vec<u64> = frames_str.split(',')
                        .map(|s| s.trim().parse())
                        .collect::<Result<_, _>>()
                        .map_err(|_| anyhow::anyhow!("Invalid frame numbers format, expected comma-separated integers"))?;
                    ExtractSelection::Frames(frames)
                }

                (None, None, Some(n)) => {
                    ExtractSelection::EveryNth(n)
                }

                _ => {
                    anyhow::bail!("Exactly one selection method must be used: --times, --frames, or --every");
                }
            };

            let extract_options = ExtractOptions {
                selection,
                format,
            };

            println!("Extracting frames from {} to {}...", input.display(), output_dir.display());
            let count = extract_frames(&input, &output_dir, extract_options)?;
            println!("Done! Extracted {} frames.", count);
            Ok(())
        }
    }
}

fn print_results(results: Vec<crate::search::entry::FileEntry>, total_scanned: usize) {
    println!("Found {} results (scanned {} files):", results.len(), total_scanned);
    for (i, result) in results.iter().enumerate() {
        println!("{:4}: {}", i + 1, result.path.display());
    }
}

