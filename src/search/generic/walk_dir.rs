//! Generic parallel directory walking implementation using jwalk

use regex::Regex;
use std::path::Path;
use anyhow::Result;
use jwalk::WalkDir;
use crate::search::{
    engine::SearchEngine,
    entry::FileEntry,
    filter::matches_pattern,
    sort::{sort_entries, apply_limit},
};

/// Generic search engine that uses parallel directory walking
/// Works cross-platform on any filesystem
#[derive(Debug, Default)]
pub struct GenericSearchEngine {
    last_count: usize,
}

impl GenericSearchEngine {
    pub fn new() -> Self {
        Self { last_count: 0 }
    }
}

impl SearchEngine for GenericSearchEngine {
    fn search(&mut self, pattern: &Regex, root: Option<&Path>, limit: Option<usize>) -> Result<Vec<FileEntry>> {
        let root_path = root.unwrap_or_else(|| Path::new("."));

        let mut entries = Vec::new();

        use jwalk::Parallelism;
        for entry_result in WalkDir::new(root_path)
            .follow_links(false)
            .parallelism(Parallelism::RayonNewPool(0)) {
            let entry = match entry_result {
                Ok(e) => e,
                Err(_) => continue,
            };

            let file_entry = FileEntry {
                path: entry.path().to_path_buf(),
                file_name: entry.file_name().to_string_lossy().into_owned(),
                size: entry.metadata().map(|m| m.len()).unwrap_or(0),
                modified: entry.metadata().ok().and_then(|m| m.modified().ok()),
                created: entry.metadata().ok().and_then(|m| m.created().ok()),
                is_dir: entry.metadata().ok().map(|m| m.is_dir()).unwrap_or(false),
                extension: entry
                    .path()
                    .extension()
                    .and_then(|s| s.to_str())
                    .map(|s| s.to_lowercase()),
                relevance_score: 0,
                #[cfg(windows)]
                is_owned_by_current_user: false,
                #[cfg(windows)]
                is_system_owner: false,
            };

            if matches_pattern(&file_entry, pattern) {
                entries.push(file_entry);
            }
        }

        self.last_count = entries.len();
        sort_entries(&mut entries);

        Ok(apply_limit(entries, limit))
    }

    fn count(&self) -> usize {
        self.last_count
    }

    fn is_available(&self, _root: Option<&Path>) -> bool {
        // Generic engine is always available
        true
    }
}
