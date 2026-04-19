//! FFmpeg implementation of media trimming (cut by time range)

use std::path::Path;
use ffmpeg_the_third as ffmpeg;
use ffmpeg::format;
use ffmpeg::codec;

use crate::multimedia::error::MultimediaError;

/// Trim media to a time range using FFmpeg
pub fn trim(input: &Path, output: &Path, start_seconds: f64, duration_seconds: Option<f64>) -> Result<(), MultimediaError> {
    ffmpeg::init()?;

    // Open input
    let mut input_ctx = format::input(input)?;

    // Guess output format from extension
    let mut output_ctx = format::output(output)?;

    // Copy all streams as-is (stream copy) - no re-encoding, very fast
    for (_, stream) in input_ctx.streams().enumerate() {
        let params = stream.parameters();
        let id = params.id();
        if let Some(codec) = ffmpeg::codec::encoder::find(id) {
            let mut out_stream = output_ctx.add_stream(codec)?;
            out_stream.set_parameters(params);
        }
    }

    // Write header
    output_ctx.write_header()?;

    // Calculate end time if duration given
    let end_seconds = duration_seconds.map(|d| start_seconds + d);

    let mut packets_written = 0;

    'packet: for (stream, mut packet) in input_ctx.packets().filter_map(Result::ok) {
        // Get stream time base
        let tb = stream.time_base();

        // Calculate PTS in seconds
        let pts_seconds = match packet.pts() {
            Some(pts) => pts as f64 * tb.numerator() as f64 / tb.denominator() as f64,
            None => continue 'packet,
        };

        // Skip packets before start
        if pts_seconds < start_seconds {
            continue 'packet;
        }

        // Stop if we've reached end
        if let Some(end) = end_seconds {
            if pts_seconds > end {
                break 'packet;
            }
        }

        // Rescale PTS/DTS to output stream time base
        let out_stream = output_ctx.stream(stream.index()).unwrap();
        let out_tb = out_stream.time_base();
        packet.rescale_ts(tb, out_tb);
        packet.set_stream(stream.index());

        // Write packet
        unsafe {
            packet.write_interleaved(&mut output_ctx)?;
        }
        packets_written += 1;
    }

    // Write trailer
    output_ctx.write_trailer()?;

    println!("Trim complete: wrote {} packets", packets_written);

    Ok(())
}
