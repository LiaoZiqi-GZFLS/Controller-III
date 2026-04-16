//! Multimedia processing module - format conversion, frame extraction, media info

pub mod error;
pub mod traits;
pub mod info;
pub mod media_info;
pub mod transcode;
pub mod extract;
pub mod ffmpeg;
pub mod native;

// Public exports
#[allow(unused_imports)]
pub use error::MultimediaError;
#[allow(unused_imports)]
pub use traits::{
    MediaInfoProvider, Transcoder, FrameExtractor,
    TranscoderOptions, ExtractOptions, ExtractSelection
};
#[allow(unused_imports)]
pub use info::{MediaInfo, StreamInfo};
pub use media_info::get_media_info;
pub use transcode::transcode;
pub use extract::extract_frames;
