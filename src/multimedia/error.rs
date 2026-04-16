//! Multimedia error types

use thiserror::Error;

/// Multimedia processing error
#[derive(Debug, Error)]
pub enum MultimediaError {
    #[error("FFmpeg error: {0}")]
    FfmpegError(String),

    #[error("FFmpeg libraries not found. Please install FFmpeg development libraries for full multimedia features.")]
    FfmpegNotFound,

    #[error("I/O error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("File not found: {0}")]
    FileNotFound(String),

    #[error("Invalid media file: {0}")]
    InvalidMedia(String),

    #[error("No video stream found in file")]
    NoVideoStream,

    #[error("No audio stream found in file")]
    NoAudioStream,

    #[error("Invalid parameter: {0}")]
    InvalidParameter(String),

    #[error("Frame position out of bounds: requested {requested}, total {total}")]
    FrameOutOfBounds { requested: i64, total: i64 },

    #[error("Image encoding error: {0}")]
    ImageError(String),

    #[error("MP4 parsing error: {0}")]
    Mp4Error(String),

    #[error("H.264 decoding error: {0}")]
    OpenH264Error(String),

    #[error("Operation not supported: {0}")]
    Unsupported(String),

    #[error("Failed to create output directory: {0}")]
    CreateOutputDir(String),
}

#[cfg(feature = "ffmpeg")]
impl From<ffmpeg_the_third::Error> for MultimediaError {
    fn from(err: ffmpeg_the_third::Error) -> Self {
        MultimediaError::FfmpegError(err.to_string())
    }
}

impl From<image::ImageError> for MultimediaError {
    fn from(err: image::ImageError) -> Self {
        MultimediaError::ImageError(err.to_string())
    }
}

impl From<mp4::Error> for MultimediaError {
    fn from(err: mp4::Error) -> Self {
        MultimediaError::Mp4Error(err.to_string())
    }
}

impl From<openh264::Error> for MultimediaError {
    fn from(err: openh264::Error) -> Self {
        MultimediaError::OpenH264Error(err.to_string())
    }
}
