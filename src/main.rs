use anyhow::Result;
use clap::Parser;

mod cli;
mod modes;
mod search;
mod multimedia;

use cli::args::{CliArgs, MultimediaSubcommands};
use search::{create_search_engine, query_to_regex};
use multimedia::{get_media_info, transcode, extract_frames, TranscoderOptions};

#[cfg(windows)]
fn enable_windows_ansi_support() {
    use windows::Win32::System::Console::{
        GetConsoleMode, GetStdHandle, SetConsoleMode, CONSOLE_MODE, ENABLE_VIRTUAL_TERMINAL_PROCESSING,
        STD_OUTPUT_HANDLE,
    };

    unsafe {
        if let Ok(handle) = GetStdHandle(STD_OUTPUT_HANDLE) {
            let mut mode = CONSOLE_MODE(0);
            if GetConsoleMode(handle, &mut mode).is_ok() {
                mode.0 |= ENABLE_VIRTUAL_TERMINAL_PROCESSING.0;
                let _ = SetConsoleMode(handle, mode);
            }
        }
    }
}

fn main() {
    // Enable ANSI escape code support on Windows console (required for colors)
    #[cfg(windows)]
    enable_windows_ansi_support();

    let cli = CliArgs::parse();
    let result = (|| -> Result<()> {
        if let Some(multimedia_cmd) = cli.multimedia {
            handle_multimedia(multimedia_cmd)
        } else if let Some(query) = &cli.search {
            let pattern = query_to_regex(query, cli.case_sensitive);
            let mut engine = create_search_engine(cli.force_generic);

            let results = if !engine.is_available(cli.root.as_deref()) {
                eprintln!("Search engine not available, falling back to generic search...");
                engine = create_search_engine(true);
                engine.search(&pattern, cli.root.as_deref(), cli.limit)?
            } else {
                match engine.search(&pattern, cli.root.as_deref(), cli.limit) {
                    Ok(results) => results,
                    Err(e) => {
                        eprintln!("NTFS search failed: {e}\nFalling back to generic search...");
                        let mut engine = create_search_engine(true);
                        engine.search(&pattern, cli.root.as_deref(), cli.limit)?
                    }
                }
            };

            println!("Found {} results (scanned {} files):", results.len(), engine.count());
            for result in results {
                println!("{}", result.path.display());
            }
            Ok(())
        } else if cli.headless {
            modes::headless::run(cli.config, cli.search, cli.root, cli.force_generic, cli.case_sensitive, cli.limit)
        } else {
            modes::interactive::run()
        }
    })();

fn handle_multimedia(cmd: MultimediaSubcommands) -> Result<()> {
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
                // Parse resolution string like "1920x1080"
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

    if let Err(e) = result {
        eprintln!("\n\x1b[31mERROR:\x1b[0m {}", e);
        // On Windows elevated prompt, keep window open for user to see error
        #[cfg(windows)]
        {
            use std::io;
            println!("\nPress Enter to exit...");
            let mut s = String::new();
            let _ = io::stdin().read_line(&mut s);
        }
        std::process::exit(1);
    }
}
