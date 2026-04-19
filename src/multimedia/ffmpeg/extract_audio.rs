//! FFmpeg implementation of audio extraction from video

use std::path::Path;
use ffmpeg_the_third as ffmpeg;
use ffmpeg::format;
use ffmpeg::media;
use ffmpeg::codec;
use ffmpeg::codec::{encoder, decoder};
use ffmpeg::software::resampling;

use crate::multimedia::error::MultimediaError;

enum AudioEncoder {
    Audio(encoder::Audio, decoder::Audio, Option<resampling::Context>),
}

/// Extract audio track from media (discard all video streams)
pub fn extract_audio(input: &Path, output: &Path, bitrate_kbps: Option<u32>, codec_name: Option<String>) -> Result<(), MultimediaError> {
    ffmpeg::init()?;

    // Open input
    let mut input_ctx = format::input(input)?;

    // Guess output format from extension
    let mut output_ctx = format::output(output)?;

    // Only process audio streams, fully decode-encode to handle sample format conversion
    let mut streams: Vec<(usize, usize, AudioEncoder)> = Vec::new();

    for (_enum_index, stream) in input_ctx.streams().enumerate() {
        let params = stream.parameters();
        if params.medium() != media::Type::Audio {
            continue; // skip video/subtitle
        }

        let actual_input_index = stream.index();

        // Find decoder
        if decoder::find(params.id()).is_none() {
            continue;
        }

        let context = codec::context::Context::from_parameters(params)?;
        let decode_ctx = context.decoder();
        let audio_dec = match decode_ctx.audio() {
            Ok(dec) => dec,
            Err(_) => continue,
        };

        // Find encoder
        let out_codec = if let Some(name) = &codec_name {
            codec::encoder::find_by_name(name)
        } else {
            let id = output_ctx.format().codec(output, media::Type::Audio);
            codec::encoder::find(id)
        };

        let out_codec = match out_codec {
            Some(c) => c,
            None => continue,
        };

        let mut out_stream = output_ctx.add_stream(out_codec)?;
        let encode_builder = codec::context::Context::new_with_codec(out_codec);
        let mut audio_encoder = encode_builder.encoder().audio()?;

        // Set parameters based on input
        audio_encoder.set_time_base(ffmpeg::Rational::new(1, audio_dec.rate() as i32));
        if let Some(bps) = bitrate_kbps {
            audio_encoder.set_bit_rate((bps * 1000) as usize);
        }

        // For MP3, most encoders (including Windows Media Foundation) only support 44100 Hz
        // For MP3, most encoders require s16 packed sample format
        let out_rate = if out_codec.name().contains("mp3") {
            44100
        } else {
            audio_dec.rate() as i32
        };
        audio_encoder.set_rate(out_rate);
        audio_encoder.set_ch_layout(audio_dec.ch_layout());

        let out_format = if out_codec.name().contains("mp3") {
            ffmpeg::format::Sample::I16(ffmpeg::format::sample::Type::Packed)
        } else {
            ffmpeg::format::Sample::F32(ffmpeg::format::sample::Type::Planar)
        };
        audio_encoder.set_format(out_format);

        let final_out_rate = audio_encoder.rate();
        let final_out_format = audio_encoder.format();

        // Get ch_layout before opening (no longer used after move, kept for documentation)
        let _final_out_ch_layout = audio_encoder.ch_layout();

        // Must set channel layout mask directly on the raw AVCodecContext - some encoders check this before opening
        unsafe {
            let ptr = audio_encoder.as_mut().as_mut_ptr();
            if let Some(mask) = audio_dec.ch_layout().mask() {
                (*ptr).ch_layout.u.mask = mask.bits();
            }
        }
        audio_encoder.set_ch_layout(audio_dec.ch_layout());

        let mut opened_audio_encoder = audio_encoder.open_with::<ffmpeg::Dictionary>(Default::default())?;
        out_stream.set_parameters(codec::Parameters::from(&opened_audio_encoder));

        // Create resampler if needed - convert from input format/rate/channels to output
        let mut resampler = None;
        let in_format = audio_dec.format();
        let in_rate = audio_dec.rate();
        let in_ch_layout = audio_dec.ch_layout();

        let final_out_ch_layout_opened = opened_audio_encoder.ch_layout();

        if in_format != final_out_format || in_rate != final_out_rate || in_ch_layout != final_out_ch_layout_opened {
            resampler = resampling::Context::get2(
                in_format, in_ch_layout, in_rate,
                final_out_format, final_out_ch_layout_opened, final_out_rate,
            ).ok();
        }

        streams.push((actual_input_index, out_stream.index(), AudioEncoder::Audio(opened_audio_encoder, audio_dec, resampler)));
    }

    if streams.is_empty() {
        return Err(MultimediaError::NoAudioStream);
    }

    // Write header
    output_ctx.write_header()?;

    let output_ctx_ptr = &mut output_ctx as *mut _;

    let mut frames_processed = 0;
    let mut packets_written = 0;

    // Process packets
    for (stream, mut packet) in input_ctx.packets().filter_map(Result::ok) {
        let input_index = stream.index();

        // Find matching stream info
        for (in_idx, out_idx, encoder) in &mut streams {
            if *in_idx != input_index {
                continue;
            }

            match encoder {
                AudioEncoder::Audio(audio_enc, audio_dec, resampler) => {
                    let out_stream = output_ctx.stream(*out_idx).unwrap();
                    let out_time_base = out_stream.time_base();

                    // Send original packet (original PTS based on input stream time_base) to decoder
                    // Don't rescale before decoding - rescale only happens on output packet
                    if let Err(e) = audio_dec.send_packet(&packet) {
                        eprintln!("Warning: Failed to send packet to audio decoder: {}", e);
                    }

                    let mut frame = ffmpeg::frame::Audio::empty();
                    while audio_dec.receive_frame(&mut frame).is_ok() {
                        frames_processed += 1;
                        if let Some(resampler) = resampler {
                            let mut converted = ffmpeg::frame::Audio::empty();
                            unsafe {
                                converted.alloc(
                                    audio_enc.format(),
                                    audio_enc.ch_layout().channels() as usize,
                                    audio_enc.ch_layout().mask().unwrap(),
                                );
                            }
                            if let Err(e) = resampler.run(&frame, &mut converted) {
                                eprintln!("Warning: resampling failed: {}", e);
                                continue;
                            }
                            if let Err(e) = audio_enc.send_frame(&converted) {
                                eprintln!("Warning: send frame failed: {}", e);
                                continue;
                            }
                        } else {
                            if let Err(e) = audio_enc.send_frame(&frame) {
                                eprintln!("Warning: send frame failed: {}", e);
                                continue;
                            }
                        }

                        let mut out_packet = ffmpeg::Packet::empty();
                        while audio_enc.receive_packet(&mut out_packet).is_ok() {
                            out_packet.set_stream(*out_idx);
                            out_packet.rescale_ts(audio_dec.time_base(), out_time_base);
                            unsafe { out_packet.write_interleaved(&mut *output_ctx_ptr)?; }
                        }
                    }
                }
            }
        }
    }

    // Flush all encoders
    for (_input_index, out_idx, encoder) in &mut streams {
        match encoder {
            AudioEncoder::Audio(audio_enc, audio_dec, _) => {
                audio_enc.send_eof()?;
                let out_stream = output_ctx.stream(*out_idx).unwrap();
                let out_time_base = out_stream.time_base();
                let mut out_packet = ffmpeg::Packet::empty();
                while audio_enc.receive_packet(&mut out_packet).is_ok() {
                    out_packet.set_stream(*out_idx);
                    out_packet.rescale_ts(audio_dec.time_base(), out_time_base);
                    unsafe { out_packet.write_interleaved(&mut *output_ctx_ptr)?; }
                    packets_written += 1;
                }
            }
        }
    }

    // Write trailer
    output_ctx.write_trailer()?;

    println!("Extracted {} audio stream(s), processed {} frames, wrote {} packets", streams.len(), frames_processed, packets_written);

    Ok(())
}
