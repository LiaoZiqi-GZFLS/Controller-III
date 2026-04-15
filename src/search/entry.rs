//! File entry - contains metadata for search results

use std::path::PathBuf;
use std::time::SystemTime;

/// File entry with metadata needed for sorting and display
#[derive(Debug, Clone)]
pub struct FileEntry {
    /// Full path to the file
    pub path: PathBuf,
    /// File name (extracted from path)
    pub file_name: String,
    /// File size in bytes
    pub size: u64,
    /// Last modification time
    pub modified: Option<SystemTime>,
    /// Creation time
    pub created: Option<SystemTime>,
    /// Is this a directory
    pub is_dir: bool,
    /// File extension (lowercase, without dot)
    pub extension: Option<String>,
    /// Calculated relevance score for "user files first" sorting (lower = more relevant)
    pub relevance_score: i32,
    /// Whether the file is owned by the current user (Windows only)
    #[cfg(windows)]
    pub is_owned_by_current_user: bool,
    /// Whether the owner is a system account (Windows only)
    #[cfg(windows)]
    pub is_system_owner: bool,
}

impl FileEntry {
    /// Create a new FileEntry from a path
    pub fn from_path(path: PathBuf) -> Self {
        let file_name = path
            .file_name()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default();

        let extension = path
            .extension()
            .and_then(|s| s.to_str())
            .map(|s| s.to_lowercase());

        let metadata = std::fs::metadata(&path).ok();

        Self {
            path,
            file_name,
            size: metadata.as_ref().map(|m| m.len()).unwrap_or(0),
            modified: metadata.as_ref().and_then(|m| m.modified().ok()),
            created: metadata.as_ref().and_then(|m| m.created().ok()),
            is_dir: metadata.as_ref().map(|m| m.is_dir()).unwrap_or(false),
            extension,
            relevance_score: 0,
            #[cfg(windows)]
            is_owned_by_current_user: false,
            #[cfg(windows)]
            is_system_owner: false,
        }
    }
}
