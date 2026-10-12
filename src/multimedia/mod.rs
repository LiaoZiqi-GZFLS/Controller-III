//! Multimedia processing module - format conversion, frame extraction, media info

pub mod ascii;
pub mod error;
pub mod extract;
pub mod extract_audio;
pub mod ffmpeg;
pub mod info;
pub mod media_info;
pub mod native;
pub mod traits;
pub mod transcode;
pub mod trim;

// Public exports
pub use ascii::gif_export::{GifExportOptions, export_ascii_gif};
pub use ascii::gif_renderer::GifStyle;
pub use ascii::play_ascii;
#[allow(unused_imports)]
pub use error::MultimediaError;
pub use extract::extract_frames;
pub use extract_audio::extract_audio;
#[allow(unused_imports)]
pub use info::{MediaInfo, StreamInfo};
pub use media_info::get_media_info;
#[allow(unused_imports)]
pub use traits::{
    AsciiColorMode, AsciiPlayOptions, AsciiPlayer, ExtractOptions, ExtractSelection,
    FrameExtractor, MediaInfoProvider, Transcoder, TranscoderOptions,
};
pub use transcode::transcode;
pub use trim::trim;
