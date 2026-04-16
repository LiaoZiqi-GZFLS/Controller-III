//! Native Rust multimedia backend (fallback, limited features)
//! Only supports MP4 + H.264 frame extraction when FFmpeg is not available

pub mod info;
pub mod extract;

pub use self::info::NativeMediaInfoProvider;
pub use self::extract::NativeFrameExtractor;
