//! FFmpeg implementation of media transcoder

use std::path::Path;
use ffmpeg_the_third as ffmpeg;
use ffmpeg::codec;
#[allow(unused_imports)]
use ffmpeg::{decoder, encoder};
use ffmpeg::format;
use ffmpeg::media;

use crate::multimedia::{
    error::MultimediaError,
    traits::{Transcoder, TranscoderOptions},
};

/// FFmpeg-based media transcoder
pub struct FfmpegTranscoder;

impl FfmpegTranscoder {
    pub fn new() -> Self {
        Self
    }
}

impl Transcoder for FfmpegTranscoder {
    fn transcode(
        &mut self,
        input: &Path,
        output: &Path,
        options: TranscoderOptions,
    ) -> Result<(), MultimediaError> {
        ffmpeg::init()?;

        // Open input
        let mut input_ctx = format::input(input)?;

        // Guess output format from extension
        let mut output_ctx = format::output(output)?;

        use ffmpeg::codec::{encoder, decoder};

        // First pass: add all output streams and setup encoders
        // This ends the immutable borrow before we need mutable access for packet reading
        let output_ctx_ptr = &mut output_ctx as *mut _;

        enum Encoder {
            Video(encoder::Video, decoder::Video),
            Audio(encoder::Audio, decoder::Audio),
        }

        let mut streams: Vec<(usize, usize, Encoder)> = Vec::new();

        for (input_index, stream) in input_ctx.streams().enumerate() {
            let in_codec_params = stream.parameters();
            let medium = in_codec_params.medium();

            // Find decoder
            if decoder::find(in_codec_params.id()).is_none() {
                continue; // skip unsupported streams
            }

            let context = codec::context::Context::from_parameters(in_codec_params)?;
            let decode_ctx = context.decoder();

            // Find encoder
            let out_codec = if let Some(codec_name) = &options.codec {
                encoder::find_by_name(codec_name)
            } else {
                // Auto-detect from output format
                let id = output_ctx.format().codec(output, medium);
                encoder::find(id)
            };

            let out_codec = match out_codec {
                Some(c) => c,
                None => continue,
            };

            let mut out_stream = output_ctx.add_stream(out_codec)?;
            let encode_builder = codec::context::Context::new_with_codec(out_codec);
            let out_index = out_stream.index();
            let encoder = match medium {
                media::Type::Video => {
                    if let Ok(mut video_dec) = decode_ctx.video() {
                        let width = options.resolution.map(|(w, _)| w).unwrap_or(video_dec.width());
                        let height = options.resolution.map(|(_, h)| h).unwrap_or(video_dec.height());
                        let mut video_encoder = encode_builder.encoder().video()?;
                        video_encoder.set_time_base(stream.time_base());
                        if let Some(bitrate_kbps) = options.bitrate {
                            video_encoder.set_bit_rate((bitrate_kbps * 1000) as usize);
                        }
                        video_encoder.set_width(width);
                        video_encoder.set_height(height);
                        video_encoder.set_frame_rate(Some(stream.avg_frame_rate()));
                        // Must set pixel format - mpeg4 and many encoders require it explicitly
                        // YUV420P is universally supported and what almost all video decoders output
                        video_encoder.set_format(ffmpeg::format::Pixel::YUV420P);
                        let mut video_encoder = video_encoder.open_with::<ffmpeg::Dictionary>(Default::default())?;
                        out_stream.set_parameters(ffmpeg::codec::Parameters::from(&video_encoder));
                        Some(Encoder::Video(video_encoder, video_dec))
                    } else {
                        None
                    }
                }
                media::Type::Audio => {
                    if let Ok(mut audio_dec) = decode_ctx.audio() {
                        let mut audio_encoder = encode_builder.encoder().audio()?;
                        audio_encoder.set_time_base(stream.time_base());
                        if let Some(bitrate_kbps) = options.bitrate {
                            audio_encoder.set_bit_rate((bitrate_kbps * 1000) as usize);
                        }
                        audio_encoder.set_rate(audio_dec.rate() as i32);
                        audio_encoder.set_ch_layout(audio_dec.ch_layout());
                        // Must set sample format - AAC specifically requires fltp = float planar
                        audio_encoder.set_format(ffmpeg::format::Sample::F32(ffmpeg::format::sample::Type::Planar));
                        let mut audio_encoder = audio_encoder.open_with::<ffmpeg::Dictionary>(Default::default())?;
                        out_stream.set_parameters(ffmpeg::codec::Parameters::from(&audio_encoder));
                        Some(Encoder::Audio(audio_encoder, audio_dec))
                    } else {
                        None
                    }
                }
                _ => None,
            };

            if let Some(encoder) = encoder {
                streams.push((input_index, out_index, encoder));
            }
        }

        // Write header after all streams are added
        output_ctx.write_header()?;

        // Second pass: process all packets
        'packet: for (stream, mut packet) in input_ctx.packets().filter_map(Result::ok) {
            let input_index = stream.index();

            // Find matching stream info
            for (in_idx, out_idx, encoder) in &mut streams {
                if *in_idx != input_index {
                    continue;
                }

                match encoder {
                    Encoder::Video(video_enc, video_dec) => {
                        packet.set_stream(*out_idx);
                        packet.rescale_ts(video_dec.time_base(), output_ctx.stream(*out_idx).unwrap().time_base());

                        if video_dec.send_packet(&packet).is_err() {
                            continue 'packet;
                        }

                        let mut frame = ffmpeg::frame::Video::empty();
                        while video_dec.receive_frame(&mut frame).is_ok() {
                            if video_enc.send_frame(&frame).is_err() {
                                continue;
                            }

                            let mut out_packet = ffmpeg::Packet::empty();
                            while video_enc.receive_packet(&mut out_packet).is_ok() {
                                out_packet.set_stream(*out_idx);
                                unsafe { out_packet.write_interleaved(&mut *output_ctx_ptr)?; }
                            }
                        }
                    }
                    Encoder::Audio(audio_enc, audio_dec) => {
                        packet.set_stream(*out_idx);
                        packet.rescale_ts(audio_dec.time_base(), output_ctx.stream(*out_idx).unwrap().time_base());

                        if audio_dec.send_packet(&packet).is_err() {
                            continue 'packet;
                        }

                        let mut frame = ffmpeg::frame::Audio::empty();
                        while audio_dec.receive_frame(&mut frame).is_ok() {
                            if audio_enc.send_frame(&frame).is_err() {
                                continue;
                            }

                            let mut out_packet = ffmpeg::Packet::empty();
                            while audio_enc.receive_packet(&mut out_packet).is_ok() {
                                out_packet.set_stream(*out_idx);
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
                Encoder::Video(video_enc, _) => {
                    video_enc.send_eof()?;
                    let mut out_packet = ffmpeg::Packet::empty();
                    while video_enc.receive_packet(&mut out_packet).is_ok() {
                        out_packet.set_stream(*out_idx);
                        unsafe { out_packet.write_interleaved(&mut *output_ctx_ptr)?; }
                    }
                }
                Encoder::Audio(audio_enc, _) => {
                    audio_enc.send_eof()?;
                    let mut out_packet = ffmpeg::Packet::empty();
                    while audio_enc.receive_packet(&mut out_packet).is_ok() {
                        out_packet.set_stream(*out_idx);
                        unsafe { out_packet.write_interleaved(&mut *output_ctx_ptr)?; }
                    }
                }
            }
        }

        // Write trailer
        output_ctx.write_trailer()?;

        Ok(())
    }

    fn is_available(&self) -> bool {
        ffmpeg::init().is_ok()
    }
}
