//! Interactive mode - guided user interaction

use anyhow::Result;
use dialoguer::{theme::ColorfulTheme, Select, Input, Confirm};
use std::path::PathBuf;
use crate::search::{create_search_engine, query_to_regex, FileEntry};

/// Run in interactive mode
pub fn run() -> Result<()> {
    let theme = ColorfulTheme::default();

    loop {
        println!("\n\x1b[1mWelcome to Controller III Interactive Mode!\x1b[0m");

        // Select operation
        let operations = vec![
            "🔍 File Search",
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
                configure_interactive(&theme)?;
            }
            2 => {
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
