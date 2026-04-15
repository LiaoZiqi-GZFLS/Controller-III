//! Headless mode - non-interactive execution

use anyhow::Result;
use std::path::PathBuf;
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

fn print_results(results: Vec<crate::search::entry::FileEntry>, total_scanned: usize) {
    println!("Found {} results (scanned {} files):", results.len(), total_scanned);
    for (i, result) in results.iter().enumerate() {
        println!("{:4}: {}", i + 1, result.path.display());
    }
}

