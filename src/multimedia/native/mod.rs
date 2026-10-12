//! Native Rust multimedia backend (fallback, limited features)
//! Only supports MP4 + H.264 frame extraction when FFmpeg is not available

pub mod extract;
pub mod info;

pub use self::extract::NativeFrameExtractor;
pub use self::info::NativeMediaInfoProvider;
