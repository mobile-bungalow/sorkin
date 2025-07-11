use std::{ffi::c_void, mem::size_of, path::PathBuf};

use ffmpeg::encoder::Video;
use ffmpeg_next::{self as ffmpeg, encoder};
use godot::{
    engine::{audio_server::SpeakerMode, Engine, IMovieWriter, MovieWriter},
    global::Error as GodotError,
    prelude::*,
};

mod audio;
mod conversion;
mod settings;

use audio::OpusEncoder;
use conversion::ConversionContext;
use settings::EncoderConfig;

#[derive(Debug)]
pub enum Error {
    Ffmpeg(ffmpeg::Error),
    Conversion(String),
    Encoding(String),
    ConversionError(String),
}

impl From<ffmpeg::Error> for Error {
    fn from(err: ffmpeg::Error) -> Self {
        Error::Ffmpeg(err)
    }
}

#[derive(GodotClass)]
#[class(base=MovieWriter)]
pub struct SorkinWriter {
    base: Base<MovieWriter>,
    encoder: Option<VP9Encoder>,
    conversion_context: Option<ConversionContext>,
    frame_count: usize,
    fps: u32,
    output_path: Option<String>,
    config: EncoderConfig,
    total_frame_time: f64,
    recording_start_time: Option<std::time::Instant>,
    audio_buffer: Vec<f32>,
    audio_samples_per_video_frame: usize,
}

#[godot_api]
impl IMovieWriter for SorkinWriter {
    fn init(base: Base<MovieWriter>) -> Self {
        Self {
            base,
            encoder: None,
            conversion_context: None,
            frame_count: 0,
            fps: 30,
            output_path: None,
            config: EncoderConfig::from_project_settings(),
            total_frame_time: 0.0,
            recording_start_time: None,
            audio_buffer: Vec::new(),
            audio_samples_per_video_frame: 0,
        }
    }

    fn handles_file(&self, path: GString) -> bool {
        let path: PathBuf = path.to_string().into();
        let ext = path.extension().and_then(|s| s.to_str());
        if let Some("webm") = ext {
            godot_print!("using Sorkin writer for extension {:?}", ext.unwrap());
            true
        } else {
            false
        }
    }

    fn get_audio_mix_rate(&self) -> u32 {
        if self.config.enable_audio {
            audio::OPUS_SAMPLE_RATE
        } else {
            0
        }
    }

    fn get_audio_speaker_mode(&self) -> SpeakerMode {
        SpeakerMode::STEREO
    }

    fn write_begin(&mut self, movie_size: Vector2i, fps: u32, path: GString) -> GodotError {
        godot_print!(
            "Starting video recording: {}x{} at {} fps to {}",
            movie_size.x,
            movie_size.y,
            fps,
            path
        );
        ffmpeg::init().unwrap();

        self.fps = fps;
        self.frame_count = 0;
        self.output_path = Some(path.to_string());
        self.total_frame_time = 0.0;
        self.recording_start_time = Some(std::time::Instant::now());

        let audio_mix_rate = self.get_audio_mix_rate();
        if audio_mix_rate > 0 {
            self.audio_samples_per_video_frame =
                (audio_mix_rate / fps) as usize * audio::STEREO_CHANNELS as usize;
        } else {
            self.audio_samples_per_video_frame = 0;
        }
        self.audio_buffer.clear();

        GodotError::OK
    }
    unsafe fn write_frame(
        &mut self,
        frame_image: Gd<godot::classes::Image>,
        audio_frame_block: *const c_void,
    ) -> GodotError {
        let frame_start = std::time::Instant::now();
        let size = frame_image.get_size();

        if self.encoder.is_none() {
            if let Some(ref path) = self.output_path {
                let width = size.x as u32;
                let height = size.y as u32;

                match (
                    VP9Encoder::new(path.clone(), width, height, self.fps as f64, &self.config),
                    ConversionContext::new(
                        godot::classes::image::Format::RGBA8,
                        ffmpeg::format::Pixel::YUV420P,
                        width,
                        height,
                    ),
                ) {
                    (Ok(encoder), Ok(conversion_context)) => {
                        self.encoder = Some(encoder);
                        self.conversion_context = Some(conversion_context);
                    }
                    (Err(e), _) | (_, Err(e)) => {
                        godot_error!("Failed to initialize encoder: {:?}", e);
                        return GodotError::ERR_CANT_CREATE;
                    }
                }
            } else {
                return GodotError::ERR_UNCONFIGURED;
            }
        }

        if let (Some(encoder), Some(conversion_context)) =
            (&mut self.encoder, &mut self.conversion_context)
        {
            let mut frame = ffmpeg::frame::Video::new(
                ffmpeg::format::Pixel::YUV420P,
                conversion_context.width,
                conversion_context.height,
            );

            conversion_context.convert(frame_image, &mut frame);

            let pts = conversion::frame_to_pts(
                self.frame_count as i64,
                self.fps as i64,
                encoder.encoder.time_base().1 as i64,
            );
            frame.set_pts(Some(pts));

            match encoder.write_frame(&frame) {
                Ok(_) => {
                    if !audio_frame_block.is_null() && self.config.enable_audio {
                        let as_i32_samples = unsafe {
                            std::slice::from_raw_parts(
                                audio_frame_block as *const i32,
                                self.audio_samples_per_video_frame,
                            )
                        };

                        let audio_data: Vec<f32> = as_i32_samples
                            .iter()
                            .map(|&sample| {
                                // without this we get crazy clipping on edge cases
                                if sample == i32::MIN {
                                    -1.0f32
                                } else {
                                    sample as f32 / i32::MAX as f32
                                }
                            })
                            .collect();

                        self.audio_buffer.extend_from_slice(&audio_data);

                        let opus_frame_size_total =
                            audio::OPUS_FRAME_SIZE * audio::STEREO_CHANNELS as usize;

                        while self.audio_buffer.len() >= opus_frame_size_total {
                            let opus_frame_data: Vec<f32> =
                                self.audio_buffer.drain(0..opus_frame_size_total).collect();

                            let audio_block_size = opus_frame_data.len() * size_of::<f32>();

                            if let Err(e) = encoder.write_audio_data(
                                opus_frame_data.as_ptr() as *const c_void,
                                audio_block_size,
                            ) {
                                godot_error!("Failed to write audio data: {:?}", e);
                            }
                        }
                    }

                    self.frame_count += 1;
                    let frame_time = frame_start.elapsed();
                    self.total_frame_time += frame_time.as_secs_f64();
                    GodotError::OK
                }
                Err(_) => GodotError::ERR_FILE_CANT_WRITE,
            }
        } else {
            GodotError::ERR_UNCONFIGURED
        }
    }

    fn write_end(&mut self) {
        if let Some(mut encoder) = self.encoder.take() {
            if self.config.enable_audio && !self.audio_buffer.is_empty() {
                if let Some(audio_encoder) = encoder.audio_encoder.as_ref() {
                    let opus_frame_size_total = audio_encoder.encoder.frame_size() as usize
                        * audio_encoder.encoder.channels() as usize;

                    if self.audio_buffer.len() < opus_frame_size_total {
                        self.audio_buffer.resize(opus_frame_size_total, 0.0);
                    }

                    while self.audio_buffer.len() >= opus_frame_size_total {
                        let opus_frame_data: Vec<f32> =
                            self.audio_buffer.drain(0..opus_frame_size_total).collect();
                        let audio_block_size = opus_frame_data.len() * size_of::<f32>();

                        if let Err(e) = encoder.write_audio_data(
                            opus_frame_data.as_ptr() as *const c_void,
                            audio_block_size,
                        ) {
                            godot_error!("Failed to write final audio data: {:?}", e);
                        }
                    }
                }
            }

            self.conversion_context.take();
            match encoder.finish() {
                Ok(_) => {
                    let average_frame_time = if self.frame_count > 0 {
                        self.total_frame_time / self.frame_count as f64
                    } else {
                        0.0
                    };

                    let total_recording_time = self
                        .recording_start_time
                        .map(|start| start.elapsed().as_secs_f64())
                        .unwrap_or(0.0);

                    godot_print!(
                        "Video encoding completed successfully. Total frames: {}, Average frame time: {:.2}ms, Total recording time: {:.2}s",
                        self.frame_count,
                        average_frame_time * 1000.0,
                        total_recording_time
                    );
                }
                Err(e) => {
                    godot_error!("Failed to finish encoding: {:?}", e);
                }
            }
        }
    }
}

struct VP9Encoder {
    output_context: ffmpeg::format::context::Output,
    video_stream_index: usize,
    audio_stream_index: Option<usize>,
    encoder: Video,
    audio_encoder: Option<OpusEncoder>,
}

impl VP9Encoder {
    fn configure_encoder(
        codec: ffmpeg_next::Codec,
        width: u32,
        height: u32,
        fps: f64,
        global_header: bool,
        config: &EncoderConfig,
    ) -> Result<Video, Error> {
        let mut encoder = ffmpeg_next::codec::context::Context::new_with_codec(codec)
            .encoder()
            .video()
            .map_err(|e| Error::Encoding(format!("Could not create encoder context: {e}")))?;

        encoder.set_width(width);
        encoder.set_height(height);
        encoder.set_format(ffmpeg::format::Pixel::YUV420P);
        encoder.set_time_base((1, (fps as i32) * 1000));
        encoder.set_frame_rate(Some((fps as i32, 1)));

        if global_header {
            encoder.set_flags(ffmpeg::codec::Flags::GLOBAL_HEADER);
        }

        let mut dict = ffmpeg::Dictionary::new();
        dict.set("cpu-used", "5");
        dict.set("auto-alt-ref", "0");
        dict.set("lag-in-frames", "0");
        dict.set("row-mt", "1");
        dict.set("speed", "5");

        // Apply encoder config settings
        let thread_count_str = config.thread_count.to_string();
        dict.set("threads", &thread_count_str);

        let quality_str = match config.quality {
            settings::Quality::Realtime => "realtime",
            settings::Quality::Good => "good",
            settings::Quality::Best => "best",
        };
        dict.set("quality", quality_str);
        dict.set("deadline", quality_str);

        encoder
            .open_as_with(ffmpeg::codec::Id::VP9, dict)
            .map_err(|e| Error::Encoding(format!("Failed to open encoder: {e}")))
    }

    fn new(
        path: String,
        width: u32,
        height: u32,
        fps: f64,
        config: &EncoderConfig,
    ) -> Result<Self, Error> {
        let mut output_context = ffmpeg::format::output(&path)?;

        let global_header = output_context
            .format()
            .flags()
            .contains(ffmpeg::format::Flags::GLOBAL_HEADER);

        let codec = encoder::find(ffmpeg::codec::Id::VP9)
            .ok_or_else(|| Error::Encoding("Codec not found".to_string()))?;

        let video_stream_index = {
            let mut video_stream =
                output_context.add_stream(ffmpeg::encoder::find(ffmpeg::codec::Id::VP9))?;

            let encoder =
                Self::configure_encoder(codec, width, height, fps, global_header, config)?;
            video_stream.set_time_base((1, (fps as i32) * 1000));
            video_stream.set_parameters(&encoder);
            video_stream.index()
        };

        let (audio_stream_index, audio_encoder) = if path.ends_with(".webm") && config.enable_audio {
            let opus_encoder = OpusEncoder::new(
                audio::OPUS_SAMPLE_RATE,
                godot::engine::audio_server::SpeakerMode::STEREO,
                config,
            )
            .map_err(|e| Error::Encoding(format!("Failed to create Opus encoder: {e:?}")))?;

            let mut audio_stream = output_context.add_stream(opus_encoder.codec)?;
            audio_stream.set_time_base(opus_encoder.time_base());
            audio_stream.set_parameters(&opus_encoder.encoder);

            (Some(audio_stream.index()), Some(opus_encoder))
        } else {
            (None, None)
        };

        let encoder = Self::configure_encoder(codec, width, height, fps, global_header, config)?;

        output_context.write_header()?;

        Ok(VP9Encoder {
            output_context,
            video_stream_index,
            audio_stream_index,
            encoder,
            audio_encoder,
        })
    }

    fn write_frame(&mut self, frame: &ffmpeg::frame::Video) -> Result<(), ffmpeg::Error> {
        self.encoder.send_frame(frame)?;
        self.receive_and_write_video_packets()?;

        Ok(())
    }

    fn write_audio_data(
        &mut self,
        audio_data: *const c_void,
        data_size: usize,
    ) -> Result<(), Error> {
        if let (Some(ref mut audio_encoder), Some(audio_stream_index)) =
            (self.audio_encoder.as_mut(), self.audio_stream_index)
        {
            let packets = audio_encoder.encode_audio_data(audio_data, data_size)?;

            for mut packet in packets {
                packet.set_stream(audio_stream_index);
                packet.rescale_ts(
                    audio_encoder.time_base(),
                    self.output_context
                        .stream(audio_stream_index)
                        .unwrap()
                        .time_base(),
                );

                packet
                    .write_interleaved(&mut self.output_context)
                    .map_err(|e| Error::Encoding(format!("Failed to write audio packet: {e}")))?;
            }
        }
        Ok(())
    }

    fn finish(mut self) -> Result<(), ffmpeg::Error> {
        self.encoder.send_eof()?;
        self.receive_and_write_video_packets()?;

        if let (Some(ref mut audio_encoder), Some(audio_stream_index)) =
            (self.audio_encoder.as_mut(), self.audio_stream_index)
        {
            match audio_encoder.finish() {
                Ok(packets) => {
                    for mut packet in packets {
                        packet.set_stream(audio_stream_index);
                        packet.rescale_ts(
                            audio_encoder.time_base(),
                            self.output_context
                                .stream(audio_stream_index)
                                .unwrap()
                                .time_base(),
                        );
                        packet.write_interleaved(&mut self.output_context)?;
                    }
                }
                Err(e) => {
                    godot_error!("Failed to finish audio encoder: {:?}", e);
                }
            }
        }

        self.output_context.write_trailer()?;
        Ok(())
    }

    fn receive_and_write_video_packets(&mut self) -> Result<(), ffmpeg::Error> {
        let mut packet = ffmpeg::packet::Packet::empty();

        while self.encoder.receive_packet(&mut packet).is_ok() {
            packet.set_stream(self.video_stream_index);
            packet.rescale_ts(
                self.encoder.time_base(),
                self.output_context
                    .stream(self.video_stream_index)
                    .unwrap()
                    .time_base(),
            );
            packet.write_interleaved(&mut self.output_context)?;
        }

        Ok(())
    }
}

struct SorkinExtension;

#[gdextension]
unsafe impl ExtensionLibrary for SorkinExtension {
    fn min_level() -> InitLevel {
        InitLevel::Editor
    }

    fn on_level_init(level: InitLevel) {
        if level == InitLevel::Editor {
            EncoderConfig::register_project_settings();
        }

        if level == InitLevel::Scene {
            godot_print!("Registering Sorkin writer singleton.");
            let writer = SorkinWriter::new_alloc();
            Engine::singleton()
                .register_singleton(StringName::from("Sorkin"), writer.clone().upcast());
            MovieWriter::add_writer(writer.upcast());
        }
    }

    fn on_level_deinit(level: InitLevel) {
        if level == InitLevel::Scene {
            let mut engine = Engine::singleton();
            let singleton_name = StringName::from("Sorkin");

            let singleton = engine
                .get_singleton(singleton_name.clone())
                .expect("cannot retrieve the singleton");

            engine.unregister_singleton(singleton_name);
            singleton.free();
        }
    }
}
