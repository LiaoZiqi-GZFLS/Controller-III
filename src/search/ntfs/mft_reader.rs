//! Direct NTFS MFT reading for extremely fast whole-drive search

use regex::Regex;
use std::io::{self, Seek, BufReader};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, Duration};
use anyhow::{Result, anyhow};
use mft::{MftParser, MftEntry};
use crate::search::{
    engine::SearchEngine,
    entry::FileEntry,
    filter::matches_pattern,
    sort::{sort_entries, apply_limit},
};
use std::ptr::null_mut;
use std::env;
use windows::Win32::Foundation::*;
use windows::Win32::Security::*;
use windows::Win32::System::Threading::*;
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::core::*;

/// Convert NTFS timestamp (jiff Timestamp) to SystemTime
fn ntfs_timestamp_to_system_time(ts: jiff::Timestamp) -> SystemTime {
    // jiff already gives nanoseconds since unix epoch
    SystemTime::UNIX_EPOCH + Duration::from_nanos(ts.as_nanosecond().try_into().unwrap())
}

/// NTFS search engine using direct MFT reading
/// Extremely fast for whole-drive searches (Everything-like speed)
#[cfg(windows)]
pub struct NtfsSearchEngine {
    last_count: usize,
}

#[cfg(windows)]
impl NtfsSearchEngine {
    pub fn new() -> Result<Self> {
        Ok(Self { last_count: 0 })
    }
}

#[cfg(windows)]
impl SearchEngine for NtfsSearchEngine {
    fn search(&mut self, pattern: &Regex, root: Option<&Path>, limit: Option<usize>) -> Result<Vec<FileEntry>> {
        let root = root.ok_or_else(|| anyhow!("NTFS search requires a root volume"))?;

        // Convert to absolute path to correctly get volume component
        let abs_root = std::env::current_dir()?.join(root);
        let volume_path = abs_root
            .components()
            .next()
            .ok_or_else(|| anyhow!("Could not determine volume from path: {}", root.display()))?;

        let mut volume_name = volume_path.as_os_str().to_string_lossy().to_string();
        // Trim trailing backslashes for device path formatting
        while volume_name.ends_with('\\') || volume_name.ends_with('/') {
            volume_name.pop();
        }

        // On Windows, we can open the volume directly
        let device_path = if volume_name.ends_with(':') {
            format!(r"\\.\{}", volume_name)
        } else {
            return Err(anyhow!("Could not get drive letter from path '{}'. NTFS search requires a full Windows volume path (e.g., C:).", root.display()));
        };

        println!("Opening NTFS volume: {}", device_path);
        let mut file = std::fs::OpenOptions::new()
            .read(true)
            .open(&device_path)?;

        // Reset to beginning - MftParser will handle the rest
        file.seek(io::SeekFrom::Start(0))?;
        let reader = BufReader::new(file);

        // MftParser::from_read_seek expects reader positioned at start of volume,
        // it will parse boot sector itself. Don't seek to MFT before passing it.
        let mut parser = match MftParser::from_read_seek(reader, None) {
            Ok(p) => p,
            Err(e) => {
                return Err(anyhow!("Failed to parse NTFS MFT: {}\nMake sure the volume is formatted as NTFS.", e));
            }
        };

        let mut entries = Vec::new();
        let mut candidates = Vec::new();

        // First pass: collect all candidate entries
        for entry_result in parser.iter_entries() {
            let entry: MftEntry = match entry_result {
                Ok(e) => e,
                Err(_) => continue,
            };

            // Get the file name attribute - iterate through attributes
            let mut found_name = None;
            for attr_result in entry.iter_attributes() {
                let attr = match attr_result {
                    Ok(a) => a,
                    Err(_) => continue,
                };
                found_name = attr.data.into_file_name();
                if found_name.is_some() {
                    break;
                }
            }

            let file_name_attr = match found_name {
                Some(f) => f,
                None => continue,
            };

            let name = file_name_attr.name.clone();

            // Skip system files
            if name.starts_with('$') {
                continue;
            }

            // Check directory flag - directory flag is bit 1
            if (entry.header.flags.bits() & 0x2) != 0 {
                continue;
            }

            // Store entry reference for second pass
            candidates.push((entry, file_name_attr));
        }

        // Second pass: get full paths and filter
        for (entry, file_name_attr) in candidates {
            // Get full path - MftParser can build it for us
            let full_path_str = match parser.get_full_path_for_entry(&entry) {
                Ok(Some(p)) => p,
                Ok(None) => continue,
                Err(_) => continue,
            };

            let full_path = PathBuf::from(&full_path_str);

            if full_path.components().count() == 0 {
                continue;
            }

            // Check if within search root
            if !full_path.starts_with(root) {
                continue;
            }

            let extension = full_path
                .extension()
                .and_then(|s| s.to_str())
                .map(|s| s.to_lowercase());

            // Get timestamps from file name attribute
            let modified = ntfs_timestamp_to_system_time(file_name_attr.modified);
            let created = ntfs_timestamp_to_system_time(file_name_attr.created);
            let size = entry.header.used_entry_size;

            let entry = FileEntry {
                path: full_path,
                file_name: file_name_attr.name.clone(),
                size: size as u64,
                modified: Some(modified),
                created: Some(created),
                is_dir: false,
                extension,
                relevance_score: 0,
                is_owned_by_current_user: false,
                is_system_owner: false,
            };

            if matches_pattern(&entry, pattern) {
                entries.push(entry);
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
        // We need admin privileges to open the physical drive
        is_running_as_admin().unwrap_or(false)
    }
}

#[cfg(windows)]
fn is_running_as_admin() -> Result<bool> {
    unsafe {
        let mut token = HANDLE(null_mut());
        OpenProcessToken(
            GetCurrentProcess(),
            TOKEN_QUERY,
            &mut token,
        ).map_err(|e| anyhow!("{:?}", e))?;

        let mut elevation: TOKEN_ELEVATION = Default::default();
        let mut return_len = 0;

        GetTokenInformation(
            token,
            TokenElevation,
            Some(std::mem::transmute(&mut elevation)),
            std::mem::size_of_val(&elevation) as u32,
            &mut return_len,
        ).map_err(|e| anyhow!("{:?}", e))?;

        Ok(elevation.TokenIsElevated != 0)
    }
}

/// Restart the current process with administrator privileges
#[cfg(windows)]
pub fn restart_as_admin() -> Result<()> {
    unsafe {
        // Get the current executable path
        let exe_path = std::env::current_exe()?;
        let exe_path_str = exe_path.to_string_lossy().to_string();

        // Get current command line arguments
        let args: Vec<String> = env::args().collect();

        // Build the argument string
        let args_str = args[1..].join(" ");

        // Use ShellExecute to run as administrator
        let ret = windows::Win32::UI::Shell::ShellExecuteW(
            None,
            PCWSTR("runas\0".encode_utf16().collect::<Vec<u16>>().as_ptr()),
            PCWSTR(widestring(&exe_path_str).as_ptr()),
            PCWSTR(if !args_str.is_empty() { widestring(&args_str).as_ptr() } else { std::ptr::null() }),
            PCWSTR::null(),
            SW_SHOWNORMAL,
        );

        if ret.0 as i32 <= 32 {
            return Err(anyhow!("Failed to elevate privileges"));
        }

        // Exit the current process
        std::process::exit(0);
    }
}

/// Helper to convert String to null-terminated wide string
#[cfg(windows)]
fn widestring(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

#[cfg(not(windows))]
pub struct NtfsSearchEngine;

#[cfg(not(windows))]
impl NtfsSearchEngine {
    pub fn new() -> Result<Self> {
        Err(anyhow!("NTFS search is only available on Windows"))
    }
}
