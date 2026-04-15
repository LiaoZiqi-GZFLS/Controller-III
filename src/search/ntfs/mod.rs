//! NTFS-specific fast search implementation

pub mod mft_reader;

pub use mft_reader::restart_as_admin;
