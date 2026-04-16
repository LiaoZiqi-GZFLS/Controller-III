//! FFmpeg implementation of media information provider

use std::path::Path;
#[cfg(feature = "ffmpeg")]
use ffmpeg_the_third as ffmpeg;
// Parameters accessed via method calls, no direct import needed

use crate::multimedia::{
    error::MultimediaError,
    info::{MediaInfo, StreamInfo},
    traits::MediaInfoProvider,
};

/// FFmpeg-based media information provider
pub struct FfmpegMediaInfoProvider;

impl FfmpegMediaInfoProvider {
    pub fn new() -> Self {
        Self
    }
}

impl MediaInfoProvider for FfmpegMediaInfoProvider {
    fn get_info(&mut self, path: &Path) -> Result<MediaInfo, MultimediaError> {
        ffmpeg::init()?;

        let file_size = std::fs::metadata(path)?.len();
        let input_context = ffmpeg::format::input(path)?;

        let duration = input_context.duration();
        let duration = if duration != ffmpeg::ffi::AV_NOPTS_VALUE {
            Some(duration as f64 / ffmpeg::ffi::AV_TIME_BASE as f64)
        } else {
            None
        };

        let bitrate = input_context.bit_rate();
        let bitrate = if bitrate > 0 { Some(bitrate as u64) } else { None };

        let mut streams = Vec::new();

        for (i, stream) in input_context.streams().enumerate() {
            let codec_params = stream.parameters();

            let stream_type = match codec_params.medium() {
                ffmpeg::media::Type::Video => "video",
                ffmpeg::media::Type::Audio => "audio",
                _ => continue,
            }
            .to_string();

            let codec = codec_params.id().name().to_string();

            let mut info = StreamInfo {
                index: i,
                stream_type,
                codec,
                bitrate: if codec_params.bit_rate() > 0 {
                    Some(codec_params.bit_rate() as u64)
                } else {
                    None
                },
                width: None,
                height: None,
                frame_rate: None,
                sample_rate: None,
                channels: None,
                language: None,
            };

            match codec_params.medium() {
                ffmpeg::media::Type::Video => {
                    if let Ok(context) = ffmpeg::codec::context::Context::from_parameters(codec_params) {
                        if let Ok(decoder) = context.decoder().video() {
                            info.width = Some(decoder.width());
                            info.height = Some(decoder.height());
                            let avg_fps = stream.avg_frame_rate();
                            if avg_fps.numerator() != 0 {
                                let fps = avg_fps.numerator() as f64 / avg_fps.denominator() as f64;
                                info.frame_rate = Some(fps);
                            }
                        }
                    }
                }
                ffmpeg::media::Type::Audio => {
                    if let Ok(context) = ffmpeg::codec::context::Context::from_parameters(codec_params) {
                        if let Ok(decoder) = context.decoder().audio() {
                            info.sample_rate = Some(decoder.rate());
                            info.channels = Some(decoder.ch_layout().channels());
                        }
                    }
                }
                _ => {}
            }

            // Try to get language from metadata
            let tags = stream.metadata();
            for (key, value) in tags.iter() {
                if key.eq_ignore_ascii_case("language") {
                    info.language = Some(value.to_string());
                    break;
                }
            }

            streams.push(info);
        }

        Ok(MediaInfo {
            file_path: path.to_string_lossy().to_string(),
            file_size,
            duration,
            bitrate,
            num_streams: input_context.nb_streams() as usize,
            streams,
        })
    }
}
