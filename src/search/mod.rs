//! Fast file search module

pub mod engine;
pub mod entry;
pub mod sort;
pub mod filter;
pub mod generic;

#[cfg(windows)]
pub mod ntfs;

pub use engine::create_search_engine;
pub use entry::FileEntry;
pub use filter::query_to_regex;
#[cfg(windows)]
pub use ntfs::restart_as_admin;
