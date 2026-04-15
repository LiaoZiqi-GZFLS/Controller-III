use anyhow::Result;
use clap::Parser;

mod cli;
mod modes;
mod search;

use cli::args::CliArgs;
use search::{create_search_engine, query_to_regex};

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
        if let Some(query) = &cli.search {
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
