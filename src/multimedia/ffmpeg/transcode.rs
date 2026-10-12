//! FFmpeg implementation of media transcoder

use ffmpeg::codec;
use ffmpeg::format;
use ffmpeg::media;
use ffmpeg::software::scaling;
#[allow(unused_imports)]
use ffmpeg::{decoder, encoder};
use ffmpeg_the_third as ffmpeg;
use std::path::Path;

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

        use ffmpeg::codec::{decoder, encoder};

        // First pass: add all output streams and setup encoders
        // This ends the immutable borrow before we need mutable access for packet reading
        let output_ctx_ptr = &mut output_ctx as *mut _;

        enum Encoder {
            Video(
                encoder::Video,
                decoder::Video,
                Option<scaling::Context>,
                u32,
                u32,
            ),
            Audio(encoder::Audio, decoder::Audio),
        }

        let mut streams: Vec<(usize, usize, Encoder)> = Vec::new();
        let mut video_stream_index = None;

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
            if medium == media::Type::Video {
                video_stream_index = Some(out_index);
            }
            let encoder = match medium {
                media::Type::Video => {
                    if let Ok(mut video_dec) = decode_ctx.video() {
                        let width = options
                            .resolution
                            .map(|(w, _)| w)
                            .unwrap_or(video_dec.width());
                        let height = options
                            .resolution
                            .map(|(_, h)| h)
                            .unwrap_or(video_dec.height());
                        let mut video_encoder = encode_builder.encoder().video()?;
                        // Keep the same time base as input for compatibility
                        video_encoder.set_time_base(stream.time_base());
                        if let Some(bitrate_kbps) = options.bitrate {
                            video_encoder.set_bit_rate((bitrate_kbps * 1000) as usize);
                        }
                        video_encoder.set_width(width);
                        video_encoder.set_height(height);
                        video_encoder.set_frame_rate(Some(stream.avg_frame_rate()));
                        video_encoder.set_aspect_ratio(stream.sample_aspect_ratio());
                        // Must set pixel format - mpeg4 and many encoders require it explicitly
                        // YUV420P is universally supported and what almost all video decoders output
                        video_encoder.set_format(ffmpeg::format::Pixel::YUV420P);
                        let mut video_encoder =
                            video_encoder.open_with::<ffmpeg::Dictionary>(Default::default())?;
                        out_stream.set_parameters(ffmpeg::codec::Parameters::from(&video_encoder));

                        // Create swscale context for pixel format conversion and rescaling
                        // We always convert to YUV420P which is what the encoder expects
                        let src_format = video_dec.format();
                        let mut sws_ctx = None;

                        if src_format != ffmpeg::format::Pixel::YUV420P
                            || width != video_dec.width()
                            || height != video_dec.height()
                        {
                            // Need conversion/rescaler
                            // Parameter order for ffmpeg-the-third API: src_format, src_height, src_width, dst_format, dst_height, dst_width
                            // (it swaps them internally to match C libswscale signature)
                            sws_ctx = scaling::Context::get(
                                src_format,
                                video_dec.height(),
                                video_dec.width(),
                                ffmpeg::format::Pixel::YUV420P,
                                height,
                                width,
                                scaling::Flags::BILINEAR,
                            )
                            .ok();
                        }

                        Some(Encoder::Video(
                            video_encoder,
                            video_dec,
                            sws_ctx,
                            width,
                            height,
                        ))
                    } else {
                        None
                    }
                }
                media::Type::Audio => {
                    if let Ok(mut audio_dec) = decode_ctx.audio() {
                        let mut audio_encoder = encode_builder.encoder().audio()?;
                        audio_encoder
                            .set_time_base(ffmpeg::Rational::new(1, audio_dec.rate() as i32));
                        if let Some(bitrate_kbps) = options.bitrate {
                            audio_encoder.set_bit_rate((bitrate_kbps * 1000) as usize);
                        }
                        audio_encoder.set_rate(audio_dec.rate() as i32);
                        audio_encoder.set_ch_layout(audio_dec.ch_layout());
                        // Must set sample format - AAC specifically requires fltp = float planar
                        audio_encoder.set_format(ffmpeg::format::Sample::F32(
                            ffmpeg::format::sample::Type::Planar,
                        ));
                        let mut audio_encoder =
                            audio_encoder.open_with::<ffmpeg::Dictionary>(Default::default())?;
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

        // Write header after all streams added
        output_ctx.write_header()?;

        let mut video_frames_written = 0;
        let mut video_packets_processed = 0;

        // Second pass: process all packets
        'packet: for (stream, mut packet) in input_ctx.packets().filter_map(Result::ok) {
            let input_index = stream.index();

            // Check if this is a video packet for debugging
            if video_stream_index == Some(input_index) {
                video_packets_processed += 1;
            }

            // Find matching stream info
            for (in_idx, out_idx, encoder) in &mut streams {
                if *in_idx != input_index {
                    continue;
                }

                match encoder {
                    Encoder::Video(video_enc, video_dec, sws_ctx, out_w, out_h) => {
                        let out_stream = output_ctx.stream(*out_idx).unwrap();
                        let out_time_base = out_stream.time_base();
                        packet.set_stream(*out_idx);
                        // !! CRITICAL: Rescale packet PTS/DTS to output timebase here
                        // After decoding, frame inherits correct PTS/DTS - no double conversion!
                        packet.rescale_ts(stream.time_base(), out_time_base);

                        // Even if send_packet returns error, we still need to receive what's already decoded
                        let _ = video_dec.send_packet(&packet);

                        // Must create a new frame for each receive loop
                        loop {
                            let mut frame = ffmpeg::frame::Video::empty();
                            if !video_dec.receive_frame(&mut frame).is_ok() {
                                break;
                            }
                            if let Some(sws) = sws_ctx {
                                // Convert to YUV420P and/or scale
                                let mut converted = ffmpeg::frame::Video::empty();
                                converted.set_width(*out_w);
                                converted.set_height(*out_h);
                                converted.set_format(ffmpeg::format::Pixel::YUV420P);
                                // Must allocate buffer before swscale can write to it
                                unsafe {
                                    converted.alloc(ffmpeg::format::Pixel::YUV420P, *out_w, *out_h);
                                }
                                // PTS/DTS already correct from packet conversion above - just copy
                                converted.set_pts(frame.pts());
                                let _ = sws.run(&frame, &mut converted);
                                if video_enc.send_frame(&converted).is_err() {
                                    continue;
                                }
                            } else {
                                if video_enc.send_frame(&frame).is_err() {
                                    continue;
                                }
                            }

                            let mut out_packet = ffmpeg::Packet::empty();
                            while video_enc.receive_packet(&mut out_packet).is_ok() {
                                out_packet.set_stream(*out_idx);
                                unsafe {
                                    out_packet.write_interleaved(&mut *output_ctx_ptr)?;
                                }
                                video_frames_written += 1;
                            }
                        }
                    }
                    Encoder::Audio(audio_enc, audio_dec) => {
                        let out_stream = output_ctx.stream(*out_idx).unwrap();
                        let out_time_base = out_stream.time_base();
                        packet.set_stream(*out_idx);
                        // Rescale packet PTS/DTS to output timebase here - done once
                        packet.rescale_ts(audio_dec.time_base(), out_time_base);

                        // Even if send_packet returns error, we still need to receive what's already decoded
                        let _ = audio_dec.send_packet(&packet);

                        let mut frame = ffmpeg::frame::Audio::empty();
                        while audio_dec.receive_frame(&mut frame).is_ok() {
                            if audio_enc.send_frame(&frame).is_err() {
                                continue;
                            }

                            let mut out_packet = ffmpeg::Packet::empty();
                            while audio_enc.receive_packet(&mut out_packet).is_ok() {
                                out_packet.set_stream(*out_idx);
                                unsafe {
                                    out_packet.write_interleaved(&mut *output_ctx_ptr)?;
                                }
                            }
                        }
                    }
                }
            }
        }

        println!(
            "Processed {} video packets, wrote {} video frames",
            video_packets_processed, video_frames_written
        );

        // Flush all encoders
        for (_input_index, out_idx, encoder) in &mut streams {
            match encoder {
                Encoder::Video(video_enc, _, _, _, _) => {
                    video_enc.send_eof()?;
                    let mut out_packet = ffmpeg::Packet::empty();
                    while video_enc.receive_packet(&mut out_packet).is_ok() {
                        out_packet.set_stream(*out_idx);
                        unsafe {
                            out_packet.write_interleaved(&mut *output_ctx_ptr)?;
                        }
                        video_frames_written += 1;
                    }
                }
                Encoder::Audio(audio_enc, _) => {
                    audio_enc.send_eof()?;
                    let mut out_packet = ffmpeg::Packet::empty();
                    while audio_enc.receive_packet(&mut out_packet).is_ok() {
                        out_packet.set_stream(*out_idx);
                        unsafe {
                            out_packet.write_interleaved(&mut *output_ctx_ptr)?;
                        }
                    }
                }
            }
        }

        // Write trailer
        output_ctx.write_trailer()?;

        println!(
            "Total: {} video frames written to output",
            video_frames_written
        );

        Ok(())
    }

    fn is_available(&self) -> bool {
        ffmpeg::init().is_ok()
    }
}
