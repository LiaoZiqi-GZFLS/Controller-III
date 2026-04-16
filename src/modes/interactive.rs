//! Interactive mode - guided user interaction

use anyhow::Result;
use dialoguer::{theme::ColorfulTheme, Select, Input, Confirm};
use std::path::PathBuf;
use crate::multimedia::{get_media_info, transcode, extract_frames, TranscoderOptions};
use crate::search::{create_search_engine, query_to_regex, FileEntry};

/// Run in interactive mode
pub fn run() -> Result<()> {
    let theme = ColorfulTheme::default();

    loop {
        println!("\n\x1b[1mWelcome to Controller III Interactive Mode!\x1b[0m");

        // Select operation
        let operations = vec![
            "🔍 File Search",
            "🎬 Multimedia Tools",
            "⚙️  Configure Settings",
            "🚪 Exit",
        ];

        let selection = Select::with_theme(&theme)
            .with_prompt("Select what to do")
            .items(&operations)
            .interact()?;

        match selection {
            0 => {
                interactive_search(&theme)?;
            }
            1 => {
                multimedia_interactive(&theme)?;
            }
            2 => {
                configure_interactive(&theme)?;
            }
            3 => {
                println!("Goodbye!");
                return Ok(());
            }
            _ => unreachable!(),
        }
    }
}

fn interactive_search(theme: &ColorfulTheme) -> Result<()> {
    println!("\n\x1b[1mFile Search\x1b[0m");

    let query: String = Input::with_theme(theme)
        .with_prompt("Search pattern (supports * and ?)")
        .interact_text()?;

    if query.is_empty() {
        println!("Search query is empty");
        return Ok(());
    }

    let root_str: String = Input::with_theme(theme)
        .with_prompt("Root directory/drive (leave empty for current directory)")
        .default(".".to_string())
        .interact_text()?;

    let root = if root_str.is_empty() {
        None
    } else {
        Some(PathBuf::from(root_str))
    };

    let case_sensitive: bool = Confirm::with_theme(theme)
        .with_prompt("Case-sensitive search?")
        .default(false)
        .interact()?;

    let force_generic: bool = Confirm::with_theme(theme)
        .with_prompt("Force generic search (don't use NTFS MFT)?")
        .default(false)
        .interact()?;

    let limit_str: String = Input::with_theme(theme)
        .with_prompt("Maximum number of results (0 for unlimited)")
        .default("100".to_string())
        .interact_text()?;

    let limit = match limit_str.parse::<usize>() {
        Ok(0) => None,
        Ok(n) => Some(n),
        Err(_) => Some(100),
    };

    println!("\nSearching...");

    let pattern = query_to_regex(&query, case_sensitive);
    let mut engine = create_search_engine(force_generic);

    let results = if !engine.is_available(root.as_deref()) {
        #[cfg(windows)]
        {
            // If we're on Windows and not force_generic, ask user if they want to elevate
            if !force_generic {
                let elevate: bool = Confirm::with_theme(theme)
                    .with_prompt("NTFS fast search requires administrator privileges. Restart and elevate privileges now?")
                    .default(true)
                    .interact()?;

                if elevate {
                    crate::search::restart_as_admin()?;
                    return Ok(());
                }
            }
            println!("⚠️  Falling back to generic search (slower)...");
        }
        #[cfg(not(windows))]
        println!("⚠️  Preferred search engine not available, falling back to generic search...");

        let mut engine = create_search_engine(true);
        engine.search(&pattern, root.as_deref(), limit)?
    } else {
        match engine.search(&pattern, root.as_deref(), limit) {
            Ok(results) => results,
            Err(e) => {
                // NTFS failed, fall back to generic
                eprintln!("⚠️  NTFS fast search failed: {e}\nFalling back to generic search (slower)...");
                let mut engine = create_search_engine(true);
                engine.search(&pattern, root.as_deref(), limit)?
            }
        }
    };

    print_results(results, engine.count());

    Ok(())
}

fn multimedia_interactive(theme: &ColorfulTheme) -> Result<()> {
    println!("\n\x1b[1mMultimedia Tools\x1b[0m");

    let operations = vec![
        "📋 Show media information",
        "🔄 Convert/Transcode media",
        "🖼️  Extract frames from video",
        "🔙 Back to main menu",
    ];

    let selection = Select::with_theme(theme)
        .with_prompt("Select multimedia operation")
        .items(&operations)
        .interact()?;

    match selection {
        0 => media_info_interactive(theme),
        1 => transcode_interactive(theme),
        2 => extract_interactive(theme),
        3 => Ok(()),
        _ => unreachable!(),
    }
}

fn media_info_interactive(theme: &ColorfulTheme) -> Result<()> {
    println!("\n\x1b[1mShow Media Information\x1b[0m");

    let input_str: String = Input::with_theme(theme)
        .with_prompt("Input media file path")
        .interact_text()?;

    let input = PathBuf::from(input_str);

    if !input.exists() {
        eprintln!("ERROR: File not found: {}", input.display());
        return Ok(());
    }

    let info = get_media_info(&input)?;
    println!("\n{}", info.format());

    Ok(())
}

fn transcode_interactive(theme: &ColorfulTheme) -> Result<()> {
    println!("\n\x1b[1mConvert/Transcode Media\x1b[0m");

    let input_str: String = Input::with_theme(theme)
        .with_prompt("Input media file path")
        .interact_text()?;

    let input = PathBuf::from(input_str);

    if !input.exists() {
        eprintln!("ERROR: File not found: {}", input.display());
        return Ok(());
    }

    let output_str: String = Input::with_theme(theme)
        .with_prompt("Output file path")
        .interact_text()?;

    let output = PathBuf::from(output_str);

    let codec: String = Input::with_theme(theme)
        .with_prompt("Output codec (leave empty for auto-detect)")
        .default(String::new())
        .interact_text()?;

    let bitrate_str: String = Input::with_theme(theme)
        .with_prompt("Output bitrate in kbps (leave empty for auto)")
        .default(String::new())
        .interact_text()?;

    let resolution_str: String = Input::with_theme(theme)
        .with_prompt("Output resolution (WxH, e.g. 1920x1080, leave empty for original)")
        .default(String::new())
        .interact_text()?;

    let mut options = TranscoderOptions::default();
    options.codec = if codec.trim().is_empty() { None } else { Some(codec) };
    options.bitrate = if !bitrate_str.trim().is_empty() {
        bitrate_str.parse().ok()
    } else {
        None
    };
    options.resolution = if !resolution_str.trim().is_empty() {
        let parts: Vec<&str> = resolution_str.split('x').collect();
        if parts.len() == 2 {
            let w: Option<u32> = parts[0].parse().ok();
            let h: Option<u32> = parts[1].parse().ok();
            match (w, h) {
                (Some(w), Some(h)) => Some((w, h)),
                _ => {
                    eprintln!("Warning: Invalid resolution format, ignoring");
                    None
                }
            }
        } else {
            None
        }
    } else {
        None
    };

    let confirm: bool = Confirm::with_theme(theme)
        .with_prompt("Start transcoding?")
        .default(true)
        .interact()?;

    if !confirm {
        return Ok(());
    }

    println!("Transcoding...");
    transcode(&input, &output, options)?;
    println!("✅ Transcoding complete! Output: {}", output.display());

    Ok(())
}

fn extract_interactive(theme: &ColorfulTheme) -> Result<()> {
    println!("\n\x1b[1mExtract Frames from Video\x1b[0m");

    let input_str: String = Input::with_theme(theme)
        .with_prompt("Input video file path")
        .interact_text()?;

    let input = PathBuf::from(input_str);

    if !input.exists() {
        eprintln!("ERROR: File not found: {}", input.display());
        return Ok(());
    }

    let output_dir_str: String = Input::with_theme(theme)
        .with_prompt("Output directory for frames")
        .default("./frames".to_string())
        .interact_text()?;

    let output_dir = PathBuf::from(output_dir_str);

    let methods = vec![
        "By specific timestamps (seconds)",
        "By specific frame numbers",
        "Extract every Nth frame",
    ];

    let method = Select::with_theme(theme)
        .with_prompt("Extraction method")
        .items(&methods)
        .interact()?;

    let selection = match method {
        0 => {
            let times_str: String = Input::with_theme(theme)
                .with_prompt("Timestamps (comma-separated in seconds, e.g. 1.5,10,25)")
                .interact_text()?;

            let times: Vec<f64> = times_str.split(',')
                .map(|s| s.trim().parse())
                .collect::<Result<_, _>>()
                .map_err(|_| anyhow::anyhow!("Invalid timestamp format"))?;

            crate::multimedia::ExtractSelection::Times(times)
        }

        1 => {
            let frames_str: String = Input::with_theme(theme)
                .with_prompt("Frame numbers (comma-separated, e.g. 1,100,500)")
                .interact_text()?;

            let frames: Vec<u64> = frames_str.split(',')
                .map(|s| s.trim().parse())
                .collect::<Result<_, _>>()
                .map_err(|_| anyhow::anyhow!("Invalid frame numbers format"))?;

            crate::multimedia::ExtractSelection::Frames(frames)
        }

        2 => {
            let every: u32 = Input::with_theme(theme)
                .with_prompt("Extract every N frames (e.g. 25 = one frame every 25 frames)")
                .default("25".to_string())
                .interact_text()?
                .parse()
                .map_err(|_| anyhow::anyhow!("Invalid number"))?;

            crate::multimedia::ExtractSelection::EveryNth(every)
        }

        _ => unreachable!(),
    };

    let format = Select::with_theme(theme)
        .with_prompt("Output image format")
        .items(&["PNG (high quality)", "JPEG (smaller size)"])
        .default(0)
        .interact()?;

    let format_str = match format {
        0 => "png".to_string(),
        1 => "jpeg".to_string(),
        _ => "png".to_string(),
    };

    let extract_options = crate::multimedia::ExtractOptions {
        selection,
        format: format_str,
    };

    let confirm: bool = Confirm::with_theme(theme)
        .with_prompt("Start extraction?")
        .default(true)
        .interact()?;

    if !confirm {
        return Ok(());
    }

    println!("Extracting frames...");
    let count = extract_frames(&input, &output_dir, extract_options)?;
    println!("✅ Extraction complete! {} frames saved to {}", count, output_dir.display());

    Ok(())
}

fn print_results(results: Vec<FileEntry>, total_scanned: usize) {
    println!("\n=== Found {} results (scanned {} files): ===", results.len(), total_scanned);

    if results.is_empty() {
        println!("No matches found.");
        return;
    }

    const MAX_DISPLAY: usize = 50;
    for (i, result) in results.iter().take(MAX_DISPLAY).enumerate() {
        println!("{:3}: {}", i + 1, result.path.display());
    }

    if results.len() > MAX_DISPLAY {
        println!("... and {} more (use --limit to see more)", results.len() - MAX_DISPLAY);
    }
}

fn configure_interactive(theme: &ColorfulTheme) -> Result<()> {
    println!("\n\x1b[1mConfiguration\x1b[0m");
    // TODO: Add configuration options

    let enable_feature: bool = Confirm::with_theme(theme)
        .with_prompt("Enable some feature?")
        .default(false)
        .interact()?;

    println!("Feature enabled: {}", enable_feature);

    Ok(())
}
