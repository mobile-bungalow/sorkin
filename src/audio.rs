use crate::Error;
use ffmpeg_next as ffmpeg;
use godot::engine::audio_server::SpeakerMode;
use std::{ffi::c_void, mem::size_of};

pub struct OpusEncoder {
    pub encoder: ffmpeg::encoder::Audio,
    pub codec: ffmpeg::Codec,
    sample_rate: u32,
    channels: u16,
    frame_size: usize,
    frame_count: u64,
}

pub const OPUS_SAMPLE_RATE: u32 = 48_000;
pub const OPUS_FRAME_SIZE: usize = 960;
pub const STEREO_CHANNELS: u16 = 2;

impl OpusEncoder {
    pub fn new(sample_rate: u32, speaker_mode: SpeakerMode, config: &crate::settings::EncoderConfig) -> Result<Self, Error> {
        let channels = match speaker_mode {
            SpeakerMode::STEREO => 2,
            SpeakerMode::SURROUND_31 => 4,
            SpeakerMode::SURROUND_51 => 6,
            SpeakerMode::SURROUND_71 => 8,
            _ => 2,
        };

        let codec = ffmpeg::encoder::find(ffmpeg::codec::Id::OPUS)
            .ok_or_else(|| Error::Encoding("Opus codec not found".to_string()))?;

        let mut encoder = ffmpeg::codec::context::Context::new_with_codec(codec)
            .encoder()
            .audio()
            .map_err(|e| Error::Encoding(format!("Could not create audio encoder context: {e}")))?;

        encoder.set_rate(sample_rate as i32);
        encoder.set_format(ffmpeg::format::Sample::F32(
            ffmpeg::format::sample::Type::Packed,
        ));
        let channel_layout = match channels {
            1 => ffmpeg::channel_layout::ChannelLayout::MONO,
            2 => ffmpeg::channel_layout::ChannelLayout::STEREO,
            4 => ffmpeg::channel_layout::ChannelLayout::QUAD,
            6 => ffmpeg::channel_layout::ChannelLayout::_5POINT1,
            8 => ffmpeg::channel_layout::ChannelLayout::_7POINT1,
            _ => {
                return Err(Error::Encoding(
                    "Unsupported channel count for Opus".to_string(),
                ))
            }
        };
        encoder.set_channel_layout(channel_layout);

        let mut dict = ffmpeg::Dictionary::new();
        
        // Apply encoder config settings for audio quality
        let compression_level = match config.quality {
            crate::settings::Quality::Realtime => "10", // Fastest encoding
            crate::settings::Quality::Good => "5",      // Balanced quality/speed
            crate::settings::Quality::Best => "0",      // Highest quality
        };
        dict.set("compression_level", compression_level);
        dict.set("application", "audio");
        dict.set("vbr", "on");
        dict.set("bitrate", "128000");

        let encoder = encoder
            .open_as_with(ffmpeg::codec::Id::OPUS, dict)
            .map_err(|e| Error::Encoding(format!("Failed to open Opus encoder: {e}")))?;

        // if sample rate is unexpected
        let frame_size = OPUS_FRAME_SIZE * sample_rate as usize / OPUS_SAMPLE_RATE as usize;

        Ok(OpusEncoder {
            codec,
            encoder,
            sample_rate,
            channels,
            frame_size,
            frame_count: 0,
        })
    }

    pub fn encode_audio_data(
        &mut self,
        audio_data: *const c_void,
        data_size: usize,
    ) -> Result<Vec<ffmpeg::packet::Packet>, Error> {
        let samples_per_channel = data_size / (self.channels as usize * size_of::<f32>());

        if samples_per_channel != self.frame_size {
            return Err(Error::Encoding(format!(
                "Audio frame size mismatch: expected {} samples per channel, got {}",
                self.frame_size, samples_per_channel
            )));
        }

        let mut frame = ffmpeg::frame::Audio::new(
            ffmpeg::format::Sample::F32(ffmpeg::format::sample::Type::Packed),
            samples_per_channel,
            self.encoder.channel_layout(),
        );

        frame.set_rate(self.sample_rate);

        unsafe {
            let frame_data = frame.data_mut(0);
            let audio_bytes = std::slice::from_raw_parts(audio_data as *const u8, data_size);
            frame_data[..data_size].copy_from_slice(audio_bytes);
        }

        let pts = self.frame_count * self.frame_size as u64;
        frame.set_pts(Some(pts as i64));

        self.encoder
            .send_frame(&frame)
            .map_err(|e| Error::Encoding(format!("Failed to send audio frame: {e}")))?;

        self.frame_count += 1;

        let mut packets = Vec::new();
        let mut packet = ffmpeg::packet::Packet::empty();

        while self.encoder.receive_packet(&mut packet).is_ok() {
            packets.push(packet.clone());
        }

        Ok(packets)
    }

    pub fn finish(&mut self) -> Result<Vec<ffmpeg::packet::Packet>, Error> {
        self.encoder
            .send_eof()
            .map_err(|e| Error::Encoding(format!("Failed to send EOF to audio encoder: {e}")))?;

        let mut packets = Vec::new();
        let mut packet = ffmpeg::packet::Packet::empty();

        while self.encoder.receive_packet(&mut packet).is_ok() {
            packets.push(packet.clone());
        }

        Ok(packets)
    }

    pub fn time_base(&self) -> ffmpeg::Rational {
        self.encoder.time_base()
    }
}
