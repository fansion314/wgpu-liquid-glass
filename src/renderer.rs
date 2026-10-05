use crate::glass::{BlurMode, GlassParams, GlassUniform};
use wgpu::util::DeviceExt;

pub const SCENE_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;
pub const GLASS_SHADER: &str = include_str!("glass.wgsl");
pub const BLUR_SHADER: &str = include_str!("blur.wgsl");

#[repr(C)]
#[derive(Clone, Copy, Default, bytemuck::Pod, bytemuck::Zeroable)]
pub struct ImageMapping {
    /// Image minimum and size in logical points. A zero size disables it.
    pub rect: [f32; 4],
    /// Canvas minimum and maximum in logical points.
    pub clip: [f32; 4],
}

impl ImageMapping {
    pub fn new(rect: egui::Rect, clip: egui::Rect) -> Self {
        Self {
            rect: [rect.min.x, rect.min.y, rect.width(), rect.height()],
            clip: [clip.min.x, clip.min.y, clip.max.x, clip.max.y],
        }
    }
}

pub struct Target {
    pub texture: wgpu::Texture,
    pub view: wgpu::TextureView,
}

impl Target {
    pub fn new(
        device: &wgpu::Device,
        size: [u32; 2],
        format: wgpu::TextureFormat,
        label: &str,
    ) -> Self {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some(label),
            size: wgpu::Extent3d {
                width: size[0],
                height: size[1],
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_SRC
                | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let view = texture.create_view(&Default::default());
        Self { texture, view }
    }
}

pub struct Effects {
    pub scene: Target,
    blur_x: Target,
    blur_y: Target,
    size: [u32; 2],
    format: wgpu::TextureFormat,
    sampler: wgpu::Sampler,
    normal_atlas: wgpu::TextureView,
    pattern_sampler: wgpu::Sampler,
    glass_pipeline: wgpu::RenderPipeline,
    blur_pipeline: wgpu::RenderPipeline,
    glass_buffer: wgpu::Buffer,
    image_buffer: wgpu::Buffer,
    original_image: wgpu::TextureView,
    image_texture: Option<wgpu::Texture>,
    blur_buffers: [wgpu::Buffer; 2],
    glass_bind: wgpu::BindGroup,
    blur_binds: [wgpu::BindGroup; 2],
}

impl Effects {
    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        size: [u32; 2],
        format: wgpu::TextureFormat,
    ) -> Self {
        let glass_pipeline = pipeline(device, "Glass composite", GLASS_SHADER, format);
        let blur_pipeline = pipeline(device, "Separable Gaussian blur", BLUR_SHADER, SCENE_FORMAT);
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Linear clamp"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let normal_atlas = crate::patterns::texture(device, queue);
        let pattern_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Repeating glass relief"),
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::Repeat,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Linear,
            ..Default::default()
        });
        let glass_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Glass parameters"),
            contents: bytemuck::bytes_of(&GlassUniform::default_zero()),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let image_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Original image mapping"),
            contents: bytemuck::bytes_of(&ImageMapping::default()),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let blur_buffers = std::array::from_fn(|_| {
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Blur parameters"),
                contents: bytemuck::cast_slice(&[0.0_f32; 4]),
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            })
        });
        let scene = Target::new(device, size, SCENE_FORMAT, "Background");
        let blur_x = Target::new(device, size, SCENE_FORMAT, "Horizontal blur");
        let blur_y = Target::new(device, size, SCENE_FORMAT, "Vertical blur");
        let original_image = scene.texture.create_view(&Default::default());
        let glass_bind = glass_binding(
            device,
            &glass_pipeline,
            (&scene.view, &blur_y.view),
            &sampler,
            &glass_buffer,
            (&normal_atlas, &pattern_sampler),
            (&original_image, &image_buffer),
        );
        let blur_binds = [
            blur_binding(
                device,
                &blur_pipeline,
                &scene.view,
                &sampler,
                &blur_buffers[0],
            ),
            blur_binding(
                device,
                &blur_pipeline,
                &blur_x.view,
                &sampler,
                &blur_buffers[1],
            ),
        ];
        Self {
            scene,
            blur_x,
            blur_y,
            size,
            format,
            sampler,
            normal_atlas,
            pattern_sampler,
            glass_pipeline,
            blur_pipeline,
            glass_buffer,
            image_buffer,
            original_image,
            image_texture: None,
            blur_buffers,
            glass_bind,
            blur_binds,
        }
    }

    pub fn resize(&mut self, device: &wgpu::Device, size: [u32; 2]) {
        if size == self.size {
            return;
        }
        self.size = size;
        self.scene = Target::new(device, size, SCENE_FORMAT, "Background");
        self.blur_x = Target::new(device, size, SCENE_FORMAT, "Horizontal blur");
        self.blur_y = Target::new(device, size, SCENE_FORMAT, "Vertical blur");
        self.glass_bind = glass_binding(
            device,
            &self.glass_pipeline,
            (&self.scene.view, &self.blur_y.view),
            &self.sampler,
            &self.glass_buffer,
            (&self.normal_atlas, &self.pattern_sampler),
            (&self.original_image, &self.image_buffer),
        );
        self.blur_binds = [
            blur_binding(
                device,
                &self.blur_pipeline,
                &self.scene.view,
                &self.sampler,
                &self.blur_buffers[0],
            ),
            blur_binding(
                device,
                &self.blur_pipeline,
                &self.blur_x.view,
                &self.sampler,
                &self.blur_buffers[1],
            ),
        ];
    }

    /// Reuse egui's full-resolution GPU image; no second image upload/copy.
    pub fn set_image(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        texture: &wgpu::Texture,
        mapping: ImageMapping,
    ) {
        queue.write_buffer(&self.image_buffer, 0, bytemuck::bytes_of(&mapping));
        if self.image_texture.as_ref() == Some(texture) {
            return;
        }
        self.image_texture = Some(texture.clone());
        self.original_image = texture.create_view(&Default::default());
        self.glass_bind = glass_binding(
            device,
            &self.glass_pipeline,
            (&self.scene.view, &self.blur_y.view),
            &self.sampler,
            &self.glass_buffer,
            (&self.normal_atlas, &self.pattern_sampler),
            (&self.original_image, &self.image_buffer),
        );
    }

    pub fn render(
        &self,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        output: &wgpu::TextureView,
        glass: &GlassParams,
        pixels_per_point: f32,
        stage_width: f32,
    ) {
        let uniform = glass.uniform(
            self.size,
            pixels_per_point,
            stage_width,
            self.format.is_srgb(),
        );
        queue.write_buffer(&self.glass_buffer, 0, bytemuck::bytes_of(&uniform));
        if glass.enabled && glass.blur_mode == BlurMode::Gaussian && glass.blur > 0.0 {
            let radius = glass.blur * pixels_per_point;
            let stage_u = stage_width * pixels_per_point / self.size[0] as f32;
            let horizontal = [1.0 / self.size[0] as f32, 0.0, radius, stage_u];
            let vertical = [0.0, 1.0 / self.size[1] as f32, radius, stage_u];
            queue.write_buffer(&self.blur_buffers[0], 0, bytemuck::cast_slice(&horizontal));
            queue.write_buffer(&self.blur_buffers[1], 0, bytemuck::cast_slice(&vertical));
            draw(
                encoder,
                &self.blur_x.view,
                &self.blur_pipeline,
                &self.blur_binds[0],
                "Horizontal blur",
            );
            draw(
                encoder,
                &self.blur_y.view,
                &self.blur_pipeline,
                &self.blur_binds[1],
                "Vertical blur",
            );
        }
        draw(
            encoder,
            output,
            &self.glass_pipeline,
            &self.glass_bind,
            "Glass composite",
        );
    }
}

impl GlassUniform {
    fn default_zero() -> Self {
        bytemuck::Zeroable::zeroed()
    }
}

fn pipeline(
    device: &wgpu::Device,
    label: &str,
    source: &str,
    format: wgpu::TextureFormat,
) -> wgpu::RenderPipeline {
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some(label),
        source: wgpu::ShaderSource::Wgsl(source.into()),
    });
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some(label),
        layout: None,
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some("vs_main"),
            compilation_options: Default::default(),
            buffers: &[],
        },
        primitive: Default::default(),
        depth_stencil: None,
        multisample: Default::default(),
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: Some("fs_main"),
            compilation_options: Default::default(),
            targets: &[Some(wgpu::ColorTargetState {
                format,
                blend: None,
                write_mask: wgpu::ColorWrites::ALL,
            })],
        }),
        multiview_mask: None,
        cache: None,
    })
}

fn glass_binding(
    device: &wgpu::Device,
    pipeline: &wgpu::RenderPipeline,
    background: (&wgpu::TextureView, &wgpu::TextureView),
    sampler: &wgpu::Sampler,
    buffer: &wgpu::Buffer,
    pattern: (&wgpu::TextureView, &wgpu::Sampler),
    image: (&wgpu::TextureView, &wgpu::Buffer),
) -> wgpu::BindGroup {
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("Glass inputs"),
        layout: &pipeline.get_bind_group_layout(0),
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(background.0),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::TextureView(background.1),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: wgpu::BindingResource::Sampler(sampler),
            },
            wgpu::BindGroupEntry {
                binding: 3,
                resource: buffer.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 4,
                resource: wgpu::BindingResource::TextureView(pattern.0),
            },
            wgpu::BindGroupEntry {
                binding: 5,
                resource: wgpu::BindingResource::Sampler(pattern.1),
            },
            wgpu::BindGroupEntry {
                binding: 6,
                resource: wgpu::BindingResource::TextureView(image.0),
            },
            wgpu::BindGroupEntry {
                binding: 7,
                resource: image.1.as_entire_binding(),
            },
        ],
    })
}

fn blur_binding(
    device: &wgpu::Device,
    pipeline: &wgpu::RenderPipeline,
    input: &wgpu::TextureView,
    sampler: &wgpu::Sampler,
    buffer: &wgpu::Buffer,
) -> wgpu::BindGroup {
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("Blur inputs"),
        layout: &pipeline.get_bind_group_layout(0),
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(input),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::Sampler(sampler),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: buffer.as_entire_binding(),
            },
        ],
    })
}

fn draw(
    encoder: &mut wgpu::CommandEncoder,
    target: &wgpu::TextureView,
    pipeline: &wgpu::RenderPipeline,
    bind: &wgpu::BindGroup,
    label: &str,
) {
    let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some(label),
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
            view: target,
            depth_slice: None,
            resolve_target: None,
            ops: wgpu::Operations {
                load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                store: wgpu::StoreOp::Store,
            },
        })],
        ..Default::default()
    });
    pass.set_pipeline(pipeline);
    pass.set_bind_group(0, bind, &[]);
    pass.draw(0..3, 0..1);
}
