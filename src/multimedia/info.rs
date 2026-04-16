//! Media information types

/// Stream information
#[derive(Debug, Clone)]
pub struct StreamInfo {
    /// Stream index
    pub index: usize,
    /// Stream type ("video" or "audio")
    pub stream_type: String,
    /// Codec name
    pub codec: String,
    /// Bitrate in bits per second (if known)
    pub bitrate: Option<u64>,
    /// Video: width in pixels
    pub width: Option<u32>,
    /// Video: height in pixels
    pub height: Option<u32>,
    /// Video: frame rate (fps)
    pub frame_rate: Option<f64>,
    /// Audio: sample rate
    pub sample_rate: Option<u32>,
    /// Audio: channels
    pub channels: Option<u32>,
    /// Language (if tagged)
    pub language: Option<String>,
}

/// Complete media information
#[derive(Debug, Clone)]
pub struct MediaInfo {
    /// File path
    pub file_path: String,
    /// File size in bytes
    pub file_size: u64,
    /// Duration in seconds
    pub duration: Option<f64>,
    /// Overall bitrate in bits per second
    pub bitrate: Option<u64>,
    /// Number of streams
    pub num_streams: usize,
    /// Information for each stream
    pub streams: Vec<StreamInfo>,
}

impl MediaInfo {
    /// Format as human-readable string
    pub fn format(&self) -> String {
        let mut output = String::new();

        output.push_str(&format!("File: {}\n", self.file_path));
        output.push_str(&format!("Size: {:.2} MB\n", self.file_size as f64 / (1024.0 * 1024.0)));

        if let Some(dur) = self.duration {
            let minutes = (dur / 60.0).floor();
            let seconds = dur % 60.0;
            output.push_str(&format!("Duration: {:.0}m {:.1}s\n", minutes, seconds));
        }

        if let Some(b) = self.bitrate {
            output.push_str(&format!("Overall bitrate: {:.1} Mbps\n", b as f64 / 1_000_000.0));
        }

        output.push_str(&format!("\nStreams ({}):\n", self.num_streams));

        for (i, stream) in self.streams.iter().enumerate() {
            output.push_str(&format!("\n[{}] {}: codec {}\n", i, stream.stream_type, stream.codec));

            if let Some(b) = stream.bitrate {
                output.push_str(&format!("       Bitrate: {:.1} Mbps\n", b as f64 / 1_000_000.0));
            }

            if let (Some(w), Some(h)) = (stream.width, stream.height) {
                output.push_str(&format!("       Resolution: {}x{}\n", w, h));
            }

            if let Some(fps) = stream.frame_rate {
                output.push_str(&format!("       Frame rate: {:.2} fps\n", fps));
            }

            if let Some(sr) = stream.sample_rate {
                output.push_str(&format!("       Sample rate: {} Hz\n", sr));
            }

            if let Some(ch) = stream.channels {
                output.push_str(&format!("       Channels: {}\n", ch));
            }

            if let Some(lang) = &stream.language {
                output.push_str(&format!("       Language: {}\n", lang));
            }
        }

        output
    }
}
