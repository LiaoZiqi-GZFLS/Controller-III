//! Native Rust frame extractor (MP4 + H.264 only)
//! Used as fallback when FFmpeg is not available

use std::path::{Path, PathBuf};
use std::fs;
use std::fs::File;
use mp4::Mp4Reader;
use openh264::{decoder::Decoder, decoder::DecodedYUV, OpenH264API};
use openh264::formats::YUVSource;
use image::{ImageBuffer, RgbImage, ExtendedColorType};

use crate::multimedia::{
    error::MultimediaError,
    traits::{FrameExtractor, ExtractOptions},
};

/// Native MP4+H.264 frame extractor
pub struct NativeFrameExtractor;

impl NativeFrameExtractor {
    pub fn new() -> Self {
        Self
    }

    /// Check if input is likely MP4 with H.264
    fn is_mp4_h264(&self, input: &Path) -> bool {
        // Check extension
        if let Some(ext) = input.extension() {
            let ext = ext.to_string_lossy().to_lowercase();
            if ext != "mp4" {
                return false;
            }
        }

        // Try to open - if it reads, it's MP4
        if let Ok(mut file) = File::open(input) {
            if let Ok(file_size) = file.metadata() {
                if let Ok(mp4) = Mp4Reader::read_header(&mut file, file_size.len()) {
                    // Check if there's a video track
                    const VIDEO: mp4::FourCC = mp4::FourCC { value: *b"vide" }; // "vide"
                    mp4.tracks().values().any(|track| {
                        track.trak.mdia.hdlr.handler_type == VIDEO
                    })
                } else {
                    false
                }
            } else {
                false
            }
        } else {
            false
        }
    }
}

impl FrameExtractor for NativeFrameExtractor {
    fn extract_frames(
        &mut self,
        input: &Path,
        output_dir: &Path,
        options: ExtractOptions,
    ) -> Result<usize, MultimediaError> {
        // Create output directory if it doesn't exist
        if !output_dir.exists() {
            fs::create_dir_all(output_dir)
                .map_err(|_| MultimediaError::CreateOutputDir(output_dir.to_string_lossy().to_string()))?;
        }

        let mut file = File::open(input)?;
        let file_size = file.metadata()?.len();
        let mut mp4 = Mp4Reader::read_header(&mut file, file_size)?;

        // Find video track
        const VIDEO: mp4::FourCC = mp4::FourCC { value: *b"vide" }; // "vide"
        let (video_track_id, video_track) = match mp4.tracks().iter()
            .find(|(_, track)| track.trak.mdia.hdlr.handler_type == VIDEO) {
                Some((id, track)) => (*id, track),
                None => return Err(MultimediaError::InvalidMedia("No video track found in MP4".into())),
            };

        // Get video dimensions
        let (width, height) = match &video_track.trak.mdia.minf.stbl.stsd.avc1 {
            Some(avc1) => (avc1.width.into(), avc1.height.into()),
            None => return Err(MultimediaError::InvalidMedia("Could not get video dimensions".into())),
        };

        // Get timescale and duration
        let timescale = mp4.moov.mvhd.timescale;

        // Collect sample indices to extract
        let samples = collect_samples(video_track_id, &options, timescale, video_track);

        if samples.is_empty() {
            return Ok(0);
        }

        // Create H.264 decoder
        let api = OpenH264API::from_source();
        let mut decoder = Decoder::new(api)?;

        let input_stem = input.file_stem()
            .unwrap_or_default()
            .to_string_lossy();

        let output_format = match options.format.as_str() {
            "png" => "png",
            "jpeg" | "jpg" => "jpeg",
            _ => "png",
        };

        let mut extracted = 0;

        for (i, (sample_id, _)) in samples.iter().enumerate() {
            // Get sample data
            if let Ok(Some(sample_data)) = mp4.read_sample(video_track_id, *sample_id) {
                let data = sample_data.bytes.clone();

                // Decode NAL unit
                match decoder.decode(&data) {
                    Ok(Some(yuv)) => {
                        // Convert YUV to RGB and save
                        if let Ok(rgb) = yuv_to_rgb(yuv, width, height) {
                            let output_path: PathBuf = output_dir.join(
                                format!("{}_frame_{:06}.{}", input_stem, i + 1, output_format)
                            );
                            match output_format {
                                "png" => rgb.save(output_path)?,
                                "jpeg" | "jpg" => {
                                    let mut out_file = std::fs::File::create(output_path)?;
                                    let mut encoder = image::codecs::jpeg::JpegEncoder::new(&mut out_file);
                                    encoder.encode(&rgb, width, height, ExtendedColorType::Rgb8)?;
                                }
                                _ => unreachable!(),
                            }
                            extracted += 1;
                        }
                    }
                    Ok(None) => continue, // need more data
                    Err(e) => {
                        eprintln!("Warning: decoding error: {}", e);
                        continue;
                    }
                }
            }
        }

        Ok(extracted)
    }

    fn is_available(&self, input: &Path) -> bool {
        self.is_mp4_h264(input)
    }
}

fn collect_samples<'a>(
    _track_id: u32,
    options: &ExtractOptions,
    timescale: u32,
    track: &mp4::Mp4Track,
) -> Vec<(u32, u64)> {
    use crate::multimedia::ExtractSelection;
    let mut track_samples = Vec::new();
    let mut sample_id = 0;
    let mut timestamp = 0;

    // Manually iterate through sample entries from stts box
    for entry in &track.trak.mdia.minf.stbl.stts.entries {
        for _ in 0..entry.sample_count {
            // Convert delta from timescale units to timestamp
            let abs_timestamp = timestamp + entry.sample_delta as u64;
            track_samples.push((sample_id, abs_timestamp));
            sample_id += 1;
            timestamp = abs_timestamp;
        }
    }

    match &options.selection {
        ExtractSelection::Times(times) => {
            let mut result = Vec::new();
            for &time in times {
                let ts: u64 = (time * timescale as f64).round() as u64;
                // Find closest sample to this timestamp
                if let Some((sample_id, timestamp)) = track_samples.iter()
                    .min_by_key(|(_, ts_sample)| (*ts_sample as i64 - ts as i64).abs()) {
                    result.push((*sample_id, *timestamp));
                }
            }
            result
        }

        ExtractSelection::Frames(frame_nums) => {
            let mut result = Vec::new();
            for &frame_num in frame_nums {
                if (frame_num as usize) <= track_samples.len() {
                    if let Some((sample_id, timestamp)) = track_samples.get(frame_num as usize - 1) {
                        result.push((*sample_id, *timestamp));
                    }
                }
            }
            result
        }

        ExtractSelection::EveryNth(every) => {
            let mut result = Vec::new();
            for (i, (sample_id, timestamp)) in track_samples.iter().enumerate() {
                if i % (*every as usize) == 0 {
                    result.push((*sample_id, *timestamp));
                }
            }
            result
        }
    }
}

fn yuv_to_rgb(yuv: DecodedYUV, width: u32, height: u32) -> Result<RgbImage, MultimediaError> {
    let mut rgb_frame: RgbImage = ImageBuffer::new(width, height);
    let y_data = yuv.y();
    let u_data = yuv.u();
    let v_data = yuv.v();
    let (stride_y, stride_u, stride_v) = yuv.strides_yuv();

    for y_row in 0..height {
        for x in 0..width {
            let y = y_data[(y_row as usize * stride_y) + x as usize];
            let uv_row = (y_row / 2) as usize;
            let uv_x = (x / 2) as usize;
            let u = u_data[uv_row * stride_u + uv_x];
            let v = v_data[uv_row * stride_v + uv_x];

            // Convert YUV to RGB (BT.601)
            let yf = y as f32;
            let uf = u as f32 - 128.0;
            let vf = v as f32 - 128.0;

            let r = (yf + 1.402 * vf).clamp(0.0, 255.0) as u8;
            let g = (yf - 0.34414 * uf - 0.71414 * vf).clamp(0.0, 255.0) as u8;
            let b = (yf + 1.772 * uf).clamp(0.0, 255.0) as u8;

            rgb_frame.put_pixel(x, y_row, image::Rgb([r, g, b]));
        }
    }

    Ok(rgb_frame)
}
