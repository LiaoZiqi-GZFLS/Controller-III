//! Sorting logic - "user files first" heuristic

use once_cell::sync::Lazy;
use rayon::prelude::*;
use crate::search::entry::FileEntry;

/// Common user document extensions that should get priority
static USER_EXTENSIONS: Lazy<Vec<&'static str>> = Lazy::new(|| {
    vec![
        // Documents
        "txt", "md", "doc", "docx", "pdf", "odt", "rtf",
        "xls", "xlsx", "csv", "ppt", "pptx",
        // Images
        "jpg", "jpeg", "png", "gif", "bmp", "svg", "webp", "ico",
        "tiff", "psd", "raw",
        // Videos
        "mp4", "mkv", "avi", "mov", "wmv", "flv", "webm",
        // Audio
        "mp3", "wav", "flac", "ogg", "aac",
        // Archives
        "zip", "rar", "7z", "tar", "gz", "bz2",
        // Source code
        "rs", "c", "cpp", "h", "py", "js", "ts", "html", "css",
        "go", "java", "rb", "php", "json", "yaml", "yml", "toml",
        "xml", "sql", "sh", "bat", "ps1",
        // Bookmarks/Notes
        "epub", "mobi", "pdf", "djvu",
    ]
});

/// System file extensions that should be deprioritized
static SYSTEM_EXTENSIONS: Lazy<Vec<&'static str>> = Lazy::new(|| {
    vec![
        "sys", "dll", "exe", "bin", "obj", "lib", "a", "o",
        "pdb", "ilk", "exp", "res", "rc", "manifest",
    ]
});

/// System directories that should be deprioritized
static SYSTEM_DIRS: Lazy<Vec<&'static str>> = Lazy::new(|| {
    vec![
        r"\Windows\", r"\Program Files\", r"\Program Files (x86)\",
        r"\ProgramData\", r"\AppData\Local\", r"\AppData\Roaming\",
        r"\System Volume Information\", r"\$Recycle.Bin\",
    ]
});

/// User directories that should get priority
static USER_DIRS: Lazy<Vec<&'static str>> = Lazy::new(|| {
    vec![
        r"\Users\", r"\Documents\", r"\Desktop\", r"\Downloads\",
        r"\Pictures\", r"\Music\", r"\Videos\", r"\OneDrive\",
    ]
});

/// Calculate relevance score - lower score = more relevant (appears first)
pub fn calculate_relevance_score(entry: &mut FileEntry) {
    let mut score = 0;

    // Factor 1: Directory location - biggest weight
    let path_str = entry.path.to_string_lossy();

    for user_dir in USER_DIRS.iter() {
        if path_str.contains(user_dir) {
            score -= 50;
            break;
        }
    }

    for sys_dir in SYSTEM_DIRS.iter() {
        if path_str.contains(sys_dir) {
            score += 50;
            break;
        }
    }

    // Factor 2: File extension
    if let Some(ext) = &entry.extension {
        if USER_EXTENSIONS.iter().any(|e| e == ext) {
            score -= 10;
        }
        if SYSTEM_EXTENSIONS.iter().any(|e| e == ext) {
            score += 10;
        }
    }

    // Factor 3: Windows-specific owner check
    #[cfg(windows)]
    {
        if entry.is_owned_by_current_user {
            score -= 30;
        }
        if entry.is_system_owner {
            score += 30;
        }
    }

    entry.relevance_score = score;
}

/// Sort entries by relevance (user files first), then by modified time (newest first)
pub fn sort_entries(entries: &mut Vec<FileEntry>) {
    // Calculate scores in parallel
    entries.par_iter_mut().for_each(calculate_relevance_score);

    // Sort: by relevance score ascending, then by modified time descending
    entries.par_sort_by(|a, b| {
        let score_cmp = a.relevance_score.cmp(&b.relevance_score);
        if score_cmp != std::cmp::Ordering::Equal {
            return score_cmp;
        }

        // If same score, newer modified first
        match (a.modified, b.modified) {
            (Some(a_time), Some(b_time)) => b_time.cmp(&a_time),
            (Some(_), None) => std::cmp::Ordering::Less,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (None, None) => a.file_name.cmp(&b.file_name),
        }
    });
}

/// Apply result limit
pub fn apply_limit(entries: Vec<FileEntry>, limit: Option<usize>) -> Vec<FileEntry> {
    match limit {
        Some(limit) if limit < entries.len() => entries.into_iter().take(limit).collect(),
        _ => entries,
    }
}
