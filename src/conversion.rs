use ffmpeg_next::format::Pixel;
use godot::classes::image::Format;
use godot::engine::rendering_device::{
    DataFormat, SamplerFilter, ShaderStage, TextureUsageBits, UniformType,
};

use crate::Error;
use godot::classes::{Image, RenderingServer};
use godot::engine::{RdSamplerState, RdUniform, RenderingDevice};
use godot::prelude::*;

/// Converts frame index to a presentation time stamp
pub fn frame_to_pts(frame_idx: i64, fps: i64, ticks_per_second: i64) -> i64 {
    let seconds = frame_idx as f64 / fps as f64;
    (seconds * ticks_per_second as f64).round() as i64
}

/// Texture containers
/// The scratch buffer is used to store the rgb frame to be yuv'd
/// Note that this is really inefficient, as currently the
/// engine copies to cpu, and we upload back.
pub enum Channels {
    YUVA420p {
        scratch: Rid,
        y: Rid,
        u: Rid,
        v: Rid,
        a: Rid,
    },
}

pub struct ConversionContext {
    channels: Channels,
    sampler: Rid,
    pub width: u32,
    pub height: u32,
    shader: Rid,
    device: Gd<RenderingDevice>,
    uniforms: Rid,
    pipeline: Rid,
}

impl ConversionContext {
    fn copy_plane_data(&mut self, texture: Rid, buf: &mut [u8], line_size: usize, divisor: u32) {
        let tex = self.device.texture_get_data(texture, 0);
        let tex_slice = tex.as_slice();
        let plane_height = (self.height / divisor) as usize;
        let plane_width = (self.width / divisor) as usize;

        for row in 0..plane_height {
            let src_start = row * plane_width;
            let dst_start = row * line_size;

            let copy_len = plane_width
                .min(tex_slice.len().saturating_sub(src_start))
                .min(buf.len().saturating_sub(dst_start));

            if copy_len > 0 {
                buf[dst_start..dst_start + copy_len]
                    .copy_from_slice(&tex_slice[src_start..src_start + copy_len]);
            }
        }
    }
    pub fn new(from: Format, to: Pixel, width: u32, height: u32) -> Result<Self, crate::Error> {
        let render_server = RenderingServer::singleton();

        let mut rd = render_server
            .create_local_rendering_device()
            .ok_or(Error::Conversion(
                "Sorkin does not work in headless mode or with GL renderer, use Forward+".into(),
            ))?;

        let mut src = godot::classes::RdShaderSource::new_gd();

        src.set_stage_source(
            ShaderStage::COMPUTE,
            include_str!("./glsl/rgb_to_yuv420p.glsl").into(),
        );

        let spirv = rd
            .shader_compile_spirv_from_source(src)
            .ok_or(Error::Conversion("failed to compile source".into()))?;

        let shader = rd.shader_create_from_spirv(spirv);

        let pipeline = rd.compute_pipeline_create(shader);

        let data_tex_alloc = |rd: &mut Gd<RenderingDevice>, w, h, bind| {
            let mut view = godot::classes::RdTextureView::new_gd();
            view.set_format_override(DataFormat::R8_UNORM);

            let mut fmt = godot::classes::RdTextureFormat::new_gd();
            fmt.set_format(DataFormat::R8_UNORM);
            fmt.set_usage_bits(TextureUsageBits::STORAGE_BIT | TextureUsageBits::CAN_COPY_FROM_BIT);
            fmt.set_width(w);
            fmt.set_height(h);

            let tex = rd.texture_create(fmt, view);

            let mut tex_uniform = RdUniform::new_gd();
            tex_uniform.set_binding(bind);
            tex_uniform.set_uniform_type(UniformType::IMAGE);
            tex_uniform.add_id(tex);
            (tex_uniform, tex)
        };

        let scratch_tex_alloc = |rd: &mut Gd<RenderingDevice>, w, h, bind| {
            let default_view = godot::classes::RdTextureView::new_gd();

            let mut fmt = godot::classes::RdTextureFormat::new_gd();
            fmt.set_format(DataFormat::R8G8B8A8_UNORM);
            fmt.set_usage_bits(
                TextureUsageBits::CAN_UPDATE_BIT
                    | TextureUsageBits::SAMPLING_BIT
                    | TextureUsageBits::CAN_COPY_FROM_BIT,
            );
            fmt.set_width(w);
            fmt.set_height(h);

            let scratch_tex = rd.texture_create(fmt, default_view);

            let mut tex_uniform = RdUniform::new_gd();
            tex_uniform.set_binding(bind);
            tex_uniform.set_uniform_type(UniformType::TEXTURE);
            tex_uniform.add_id(scratch_tex);
            (tex_uniform, scratch_tex)
        };

        let mut state = RdSamplerState::new_gd();
        state.set_min_filter(SamplerFilter::NEAREST);
        state.set_mag_filter(SamplerFilter::NEAREST);

        let sampler = rd.sampler_create(state);
        let mut sampler_uni = RdUniform::new_gd();
        sampler_uni.add_id(sampler);
        sampler_uni.set_uniform_type(UniformType::SAMPLER);
        sampler_uni.set_binding(5);

        let (uniforms, channels) = match to {
            Pixel::YUVA420P | Pixel::YUV420P => {
                let (scratch_uni, scratch) = scratch_tex_alloc(&mut rd, width, height, 0);
                let (y_uni, y) = data_tex_alloc(&mut rd, width, height, 1);
                let (u_uni, u) = data_tex_alloc(&mut rd, width / 2, height / 2, 2);
                let (v_uni, v) = data_tex_alloc(&mut rd, width / 2, height / 2, 3);
                let (a_uni, a) = data_tex_alloc(&mut rd, width, height, 4);

                let uniforms = Array::from(&[
                    scratch_uni.clone(),
                    y_uni.clone(),
                    u_uni.clone(),
                    v_uni.clone(),
                    a_uni.clone(),
                    sampler_uni.clone(),
                ]);
                let uniforms = rd.uniform_set_create(uniforms, shader, 0);
                (
                    uniforms,
                    Channels::YUVA420p {
                        scratch,
                        y,
                        u,
                        v,
                        a,
                    },
                )
            }
            _ => {
                return Err(crate::Error::Conversion(format!(
                    "Unsupported Conversion {from:?} : {to:?}"
                )))
            }
        };

        Ok(Self {
            sampler,
            uniforms,
            device: rd,
            width,
            height,
            channels,
            shader,
            pipeline,
        })
    }

    pub fn convert(
        &mut self,
        mut input_image: Gd<Image>,
        frame: &mut ffmpeg_next::util::frame::Video,
        alpha_frame: Option<&mut ffmpeg_next::util::frame::Video>,
    ) {
        input_image.convert(Format::RGBA8);

        let Channels::YUVA420p { scratch, .. } = self.channels;

        self.device
            .texture_update(scratch, 0, input_image.get_data());

        let compute_list = self.device.compute_list_begin();

        self.device
            .compute_list_bind_compute_pipeline(compute_list, self.pipeline);

        self.device
            .compute_list_bind_uniform_set(compute_list, self.uniforms, 0);

        self.device.compute_list_dispatch(
            compute_list,
            self.width.div_ceil(16),
            self.height.div_ceil(16),
            1,
        );
        self.device.compute_list_end();

        self.device.submit();
        self.device.sync();

        let Channels::YUVA420p { y, u, v, a, .. } = self.channels;
        let planes = [(y, 1), (u, 2), (v, 2)];

        // Copy YUV channels to main frame
        for (plane_idx, (texture, divisor)) in planes.iter().enumerate() {
            let line_size = frame.stride(plane_idx);
            let buf = frame.data_mut(plane_idx);
            self.copy_plane_data(*texture, buf, line_size, *divisor);
        }

        if let Some(alpha_frame) = alpha_frame {
            let line_size = alpha_frame.stride(0);
            let buf = alpha_frame.data_mut(0);
            self.copy_plane_data(a, buf, line_size, 1);

            // Fill chroma planes with neutral gray for alpha
            (1..alpha_frame.planes()).for_each(|plane| {
                alpha_frame.data_mut(plane).fill(128);
            });
        }
    }
}

impl Drop for ConversionContext {
    fn drop(&mut self) {
        match &mut self.channels {
            Channels::YUVA420p {
                scratch,
                y,
                u,
                v,
                a,
            } => {
                self.device.free_rid(*y);
                self.device.free_rid(*u);
                self.device.free_rid(*v);
                self.device.free_rid(*a);
                self.device.free_rid(*scratch);
            }
        };

        self.device.free_rid(self.uniforms);
        self.device.free_rid(self.pipeline);
        self.device.free_rid(self.sampler);
        self.device.free_rid(self.shader);
    }
}
