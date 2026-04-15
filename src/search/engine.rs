//! Search engine trait and factory

use regex::Regex;
use std::path::Path;
use anyhow::Result;
use crate::search::entry::FileEntry;

/// Abstract search engine interface
pub trait SearchEngine {
    /// Perform search with given pattern and optional root directory
    fn search(&mut self, pattern: &Regex, root: Option<&Path>, limit: Option<usize>) -> Result<Vec<FileEntry>>;

    /// Get total number of files scanned
    fn count(&self) -> usize;

    /// Whether this engine is available for the given path
    fn is_available(&self, root: Option<&Path>) -> bool;
}

/// Create the best available search engine for the given parameters
pub fn create_search_engine(force_generic: bool) -> Box<dyn SearchEngine> {
    #[cfg(windows)]
    {
        if !force_generic {
            // Try to create NTFS engine - it will check availability
            match crate::search::ntfs::mft_reader::NtfsSearchEngine::new() {
                Ok(engine) => return Box::new(engine),
                Err(_) => {
                    // Fall back to generic
                }
            }
        }
    }

    // Use generic engine as fallback
    Box::new(crate::search::generic::walk_dir::GenericSearchEngine::new())
}
