//! Native MP4 implementation of media information provider

use std::path::Path;
use std::fs::File;
use mp4::Mp4Reader;

use crate::multimedia::{
    error::MultimediaError,
    info::{MediaInfo, StreamInfo},
    traits::MediaInfoProvider,
};

/// Native MP4-based media information provider
pub struct NativeMediaInfoProvider;

impl NativeMediaInfoProvider {
    pub fn new() -> Self {
        Self
    }
}

impl MediaInfoProvider for NativeMediaInfoProvider {
    fn get_info(&mut self, path: &Path) -> Result<MediaInfo, MultimediaError> {
        let mut file = File::open(path)?;
        let file_size = file.metadata()?.len();
        let mp4 = Mp4Reader::read_header(&mut file, file_size)?;

        let duration = Some(mp4.moov.mvhd.duration as f64 / mp4.moov.mvhd.timescale as f64);

        let mut streams = Vec::new();

        for (i, (_track_id, track)) in mp4.tracks().into_iter().enumerate() {
            const SOUND: mp4::FourCC = mp4::FourCC { value: *b"soun" }; // "soun"
            const VIDEO: mp4::FourCC = mp4::FourCC { value: *b"vide" }; // "vide"
            let stream_type = match track.trak.mdia.hdlr.handler_type {
                hdlr if hdlr == SOUND => "audio",
                hdlr if hdlr == VIDEO => "video",
                _ => continue,
            }.to_string();

            let codec = match stream_type.as_str() {
                "video" => "h.264", // most common in MP4
                "audio" => "aac",  // most common in MP4
                _ => "unknown",
            }.to_string();

            let mut info = StreamInfo {
                index: i,
                stream_type: stream_type.clone(),
                codec,
                bitrate: None,
                width: None,
                height: None,
                frame_rate: None,
                sample_rate: None,
                channels: None,
                language: None,
            };

            if stream_type == "video" {
                if let Some(avc1) = &track.trak.mdia.minf.stbl.stsd.avc1 {
                    info.width = Some(avc1.width.into());
                    info.height = Some(avc1.height.into());
                }
                // Calculate frame rate
                let timescale = mp4.moov.mvhd.timescale;
                let track_duration = track.trak.tkhd.duration;
                if track_duration != 0 {
                    // Count total frames from stts entries
                    let frame_count: u32 = track.trak.mdia.minf.stbl.stts.entries
                        .iter()
                        .map(|entry| entry.sample_count)
                        .sum();
                    let duration_seconds = track_duration as f64 / timescale as f64;
                    if duration_seconds > 0.0 {
                        let fps = frame_count as f64 / duration_seconds;
                        info.frame_rate = Some(fps);
                    }
                }
            }

            if stream_type == "audio" {
                if let Some(mp4a) = &track.trak.mdia.minf.stbl.stsd.mp4a {
                    info.sample_rate = Some(mp4a.samplerate.value() as u32);
                    info.channels = Some(mp4a.channelcount as u32);
                }
            }

            streams.push(info);
        }

        let num_streams = mp4.tracks().len();
        Ok(MediaInfo {
            file_path: path.to_string_lossy().to_string(),
            file_size,
            duration,
            bitrate: None,
            num_streams,
            streams,
        })
    }
}
