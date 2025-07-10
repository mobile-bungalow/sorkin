use std::{ffi::c_void, path::PathBuf};

use ffmpeg::encoder::Video;
use ffmpeg_next::{self as ffmpeg, encoder};
use godot::{
    engine::{audio_server::SpeakerMode, Engine, IMovieWriter, MovieWriter, ProjectSettings},
    global::Error as GodotError,
    prelude::*,
};

mod conversion;
mod settings;

use conversion::ConversionContext;
use settings::{Codec, EncoderConfig, Quality};

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
}

const DEFAULT_MIX_RATE_HZ: u32 = 44_100;

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
        }
    }

    fn handles_file(&self, path: GString) -> bool {
        let path: PathBuf = path.to_string().into();
        let ext = path.extension().and_then(|s| s.to_str());
        if let Some("mp4") = ext {
            godot_print!("using Sorkin writer for extension {:?}", ext.unwrap());
            true
        } else {
            false
        }
    }

    fn get_audio_mix_rate(&self) -> u32 {
        DEFAULT_MIX_RATE_HZ
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

        // Store parameters for later initialization with actual frame size
        self.fps = fps;
        self.frame_count = 0;
        self.output_path = Some(path.to_string());
        self.total_frame_time = 0.0;
        self.recording_start_time = Some(std::time::Instant::now());

        // We'll create the encoder and conversion context on first frame
        // to use the actual frame dimensions
        GodotError::OK
    }
    unsafe fn write_frame(
        &mut self,
        frame_image: Gd<godot::classes::Image>,
        _audio_frame_block: *const c_void,
    ) -> GodotError {
        let frame_start = std::time::Instant::now();
        let size = frame_image.get_size();

        // Initialize encoder on first frame with actual frame dimensions
        if self.encoder.is_none() {
            if let Some(ref path) = self.output_path {
                let width = size.x as u32;
                let height = size.y as u32;

                match (
                    VP9Encoder::new(path.clone(), width, height, self.fps as f64),
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
            // Now frame dimensions will match conversion context dimensions
            let mut frame = ffmpeg::frame::Video::new(
                ffmpeg::format::Pixel::YUV420P,
                conversion_context.width,
                conversion_context.height,
            );

            conversion_context.convert(frame_image, &mut frame);

            // Use conversion utility to properly calculate PTS
            let pts = conversion::frame_to_pts(
                self.frame_count as i64,
                self.fps as i64,
                encoder.encoder.time_base().1 as i64,
            );
            frame.set_pts(Some(pts));

            match encoder.write_frame(&frame) {
                Ok(_) => {
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
        if let Some(encoder) = self.encoder.take() {
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
    encoder: Video,
}

impl VP9Encoder {
    fn configure_encoder(
        codec: ffmpeg_next::Codec,
        width: u32,
        height: u32,
        fps: f64,
        global_header: bool,
    ) -> Result<Video, Error> {
        let mut encoder = ffmpeg_next::codec::context::Context::new_with_codec(codec)
            .encoder()
            .video()
            .map_err(|e| Error::Encoding(format!("Could not create encoder context: {}", e)))?;

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
        dict.set("speed", "8");
        dict.set("threads", "0");
        dict.set("quality", "realtime");
        dict.set("deadline", "realtime");

        encoder
            .open_as_with(ffmpeg::codec::Id::VP9, dict)
            .map_err(|e| Error::Encoding(format!("Failed to open encoder: {}", e)))
    }

    fn new(path: String, width: u32, height: u32, fps: f64) -> Result<Self, Error> {
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

            let encoder = Self::configure_encoder(codec, width, height, fps, global_header)?;
            video_stream.set_time_base((1, (fps as i32) * 1000));
            video_stream.set_parameters(&encoder);
            video_stream.index()
        };

        let encoder = Self::configure_encoder(codec, width, height, fps, global_header)?;

        output_context.write_header()?;

        Ok(VP9Encoder {
            output_context,
            video_stream_index,
            encoder,
        })
    }

    fn write_frame(&mut self, frame: &ffmpeg::frame::Video) -> Result<(), ffmpeg::Error> {
        self.encoder.send_frame(frame)?;
        self.receive_and_write_packets()?;

        Ok(())
    }

    fn finish(mut self) -> Result<(), ffmpeg::Error> {
        self.encoder.send_eof()?;
        self.receive_and_write_packets()?;
        self.output_context.write_trailer()?;
        Ok(())
    }

    fn receive_and_write_packets(&mut self) -> Result<(), ffmpeg::Error> {
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
        InitLevel::Scene
    }

    fn on_level_init(level: InitLevel) {
        if level == InitLevel::Scene {
            // Register project settings first
            EncoderConfig::register_project_settings();

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
