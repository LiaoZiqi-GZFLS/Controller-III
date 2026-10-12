//! FFmpeg-based multimedia backend (full features)

#[cfg(feature = "ffmpeg")]
pub mod extract;
#[cfg(feature = "ffmpeg")]
pub mod extract_audio;
#[cfg(feature = "ffmpeg")]
pub mod info;
#[cfg(feature = "ffmpeg")]
pub mod transcode;
#[cfg(feature = "ffmpeg")]
pub mod trim;

#[cfg(feature = "ffmpeg")]
pub use self::extract::FfmpegFrameExtractor;
#[cfg(feature = "ffmpeg")]
pub use self::info::FfmpegMediaInfoProvider;
#[cfg(feature = "ffmpeg")]
pub use self::transcode::FfmpegTranscoder;

#[cfg(feature = "ffmpeg")]
pub use self::extract_audio::extract_audio;
#[cfg(feature = "ffmpeg")]
pub use self::trim::trim;

/// Check if FFmpeg is available (can be loaded)
#[cfg(feature = "ffmpeg")]
pub fn is_available() -> bool {
    // Try to initialize FFmpeg - if it fails, not available
    ffmpeg_the_third::init().is_ok()
}

#[cfg(not(feature = "ffmpeg"))]
pub fn is_available() -> bool {
    false
}
