use egui::{Pos2, Vec2};
use hello_wgpu::{
    glass::{BlurMode, GlassMode, GlassParams, GlassShape},
    renderer::{Effects, SCENE_FORMAT, Target},
};

struct Harness {
    device: wgpu::Device,
    queue: wgpu::Queue,
    effects: Effects,
    output: Target,
    size: [u32; 2],
}

impl Harness {
    fn new(format: wgpu::TextureFormat) -> Self {
        let instance =
            wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
        let adapter = pollster::block_on(instance.request_adapter(&Default::default()))
            .expect("GPU test requires a real adapter");
        eprintln!("GPU pixel tests: {:?}", adapter.get_info());
        let (device, queue) =
            pollster::block_on(adapter.request_device(&Default::default())).unwrap();
        let size = [256, 192];
        let effects = Effects::new(&device, size, format);
        let output = Target::new(&device, size, format, "GPU test output");
        Self {
            device,
            queue,
            effects,
            output,
            size,
        }
    }

    fn upload(&self, pixels: &[u8]) {
        self.queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &self.effects.scene.texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            pixels,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(self.size[0] * 4),
                rows_per_image: Some(self.size[1]),
            },
            wgpu::Extent3d {
                width: self.size[0],
                height: self.size[1],
                depth_or_array_layers: 1,
            },
        );
    }

    fn render(&self, params: &GlassParams, ppp: f32, stage: f32) -> Vec<u8> {
        let scope = self.device.push_error_scope(wgpu::ErrorFilter::Validation);
        let stride = (self.size[0] * 4).div_ceil(256) * 256;
        let readback = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Pixel readback"),
            size: u64::from(stride * self.size[1]),
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut encoder = self.device.create_command_encoder(&Default::default());
        self.effects.render(
            &self.queue,
            &mut encoder,
            &self.output.view,
            params,
            ppp,
            stage,
        );
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: &self.output.texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &readback,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(stride),
                    rows_per_image: Some(self.size[1]),
                },
            },
            wgpu::Extent3d {
                width: self.size[0],
                height: self.size[1],
                depth_or_array_layers: 1,
            },
        );
        self.queue.submit([encoder.finish()]);
        let (tx, rx) = std::sync::mpsc::channel();
        readback.map_async(wgpu::MapMode::Read, .., move |result| {
            tx.send(result).unwrap();
        });
        self.device
            .poll(wgpu::PollType::Wait {
                submission_index: None,
                timeout: Some(std::time::Duration::from_secs(20)),
            })
            .unwrap();
        rx.recv_timeout(std::time::Duration::from_secs(20))
            .unwrap()
            .unwrap();
        let view = readback.get_mapped_range(..).unwrap();
        let pixels = view
            .chunks(stride as usize)
            .flat_map(|row| row[..(self.size[0] * 4) as usize].iter().copied())
            .collect();
        drop(view);
        readback.unmap();
        assert!(
            pollster::block_on(scope.pop()).is_none(),
            "GPU validation error"
        );
        pixels
    }
}

fn params() -> GlassParams {
    GlassParams {
        center: Pos2::new(128.0, 96.0),
        size: Vec2::new(180.0, 130.0),
        radius: 22.0,
        blur: 0.0,
        highlight: 0.0,
        ..Default::default()
    }
}

fn gradient() -> Vec<u8> {
    (0..192)
        .flat_map(|y| (0..256).flat_map(move |x| [x as u8, y as u8, 100, 255]))
        .collect()
}

fn error(a: &[u8], b: &[u8]) -> f32 {
    a.iter()
        .zip(b)
        .map(|(&a, &b)| (f32::from(a) - f32::from(b)).abs())
        .sum::<f32>()
        / a.len() as f32
}

fn variation(pixels: &[u8]) -> f32 {
    let mut result = 0.0;
    for y in 65..125 {
        for x in 90..165 {
            let i = (y * 256 + x) * 4;
            result += (f32::from(pixels[i]) - f32::from(pixels[i + 4])).abs();
        }
    }
    result
}

#[test]
#[ignore = "Requires a GPU; run cargo test --test gpu -- --ignored --nocapture"]
fn gpu_pixels_verify_optics_blur_boundaries_and_color_space() {
    let mut h = Harness::new(SCENE_FORMAT);
    let source = gradient();
    h.upload(&source);
    let mut p = params();
    p.enabled = false;
    assert_eq!(
        h.render(&p, 1.0, 256.0),
        source,
        "disabled effect must preserve every pixel"
    );
    p.enabled = true;
    for mode in [GlassMode::Liquid, GlassMode::Lens, GlassMode::Concave] {
        p.mode = mode;
        p.curvature = 0.0;
        assert_eq!(
            h.render(&p, 1.0, 256.0),
            source,
            "flat glass must not refract"
        );
        p.curvature = 0.6;
        p.ior = 1.0;
        assert_eq!(h.render(&p, 1.0, 256.0), source, "IOR 1 must not refract");
        p.ior = 1.45;
        p.distance = 0.0;
        assert!(
            error(&h.render(&p, 1.0, 256.0), &source) > 0.01,
            "curved thick glass still displaces at zero background distance"
        );
        p.distance = 20.0;
        let near = h.render(&p, 1.0, 256.0);
        p.distance = 100.0;
        let far = h.render(&p, 1.0, 256.0);
        assert!(
            error(&far, &source) > error(&near, &source) * 1.1,
            "distance must increase displacement"
        );
        assert_eq!(
            &far[0..32 * 4],
            &source[0..32 * 4],
            "outside mask unchanged"
        );
    }

    // Rotational symmetry is required even for a rectangular aperture. The
    // 3-4-5 direction and horizontal direction have the same physical radius.
    p = params();
    p.center = Pos2::new(128.5, 96.5);
    p.mode = GlassMode::Lens;
    let pixels = h.render(&p, 1.0, 256.0);
    let pixel = |x: usize, y: usize| &pixels[(y * 256 + x) * 4..(y * 256 + x) * 4 + 3];
    let axis = 193.0 - f32::from(pixel(193, 96)[0]);
    let diag_x = 167.0 - f32::from(pixel(167, 148)[0]);
    let diag_y = 148.0 - f32::from(pixel(167, 148)[1]);
    assert!(
        (diag_x - axis * 0.6).abs() < 1.5 && (diag_y - axis * 0.8).abs() < 1.5,
        "lens must not turn radial circles into rounded squares"
    );

    let convex = h.render(&p, 1.0, 256.0);
    p.mode = GlassMode::Concave;
    let concave = h.render(&p, 1.0, 256.0);
    let at = (96 * 256 + 160) * 4;
    assert!(
        convex[at] < source[at] && concave[at] > source[at],
        "convex magnifies and concave shrinks around the axis"
    );
    p.mode = GlassMode::Lens;
    p.distance = 0.0;
    p.thickness = 1.0;
    let thin = h.render(&p, 1.0, 256.0);
    p.thickness = 100.0;
    let thick = h.render(&p, 1.0, 256.0);
    assert!(
        thick[at] < thin[at],
        "base thickness must affect internal propagation"
    );

    p = params();
    p.shape = GlassShape::Circle;
    p.size = Vec2::splat(140.0);
    let circle = h.render(&p, 1.0, 256.0);
    let outside = (40 * 256 + 70) * 4;
    assert_eq!(
        &circle[outside..outside + 4],
        &source[outside..outside + 4],
        "circle corners are transparent"
    );

    p = params();
    p.margin = 0.0;
    assert_eq!(
        h.render(&p, 1.0, 256.0),
        source,
        "zero liquid margin is a flat plate"
    );
    p.margin = 12.0;
    let narrow = h.render(&p, 1.0, 256.0);
    p.margin = 40.0;
    let wide = h.render(&p, 1.0, 256.0);
    assert!(
        error(&narrow, &wide) > 0.1,
        "polished margin must change the bevel independently of corner radius"
    );

    // Bright 'panel' pixels must never bleed into stage refraction or blur.
    let mut boundary_source = source.clone();
    for y in 0..192 {
        for x in 220..256 {
            boundary_source[(y * 256 + x) * 4..(y * 256 + x) * 4 + 4]
                .copy_from_slice(&[0, 0, 255, 255]);
        }
    }
    h.upload(&boundary_source);
    p.center.x = 185.0;
    p.blur = 24.0;
    p.distance = 300.0;
    p.curvature = 1.0;
    let boundary = h.render(&p, 1.0, 220.0);
    for y in 0..192 {
        for x in 0..220 {
            assert!(
                boundary[(y * 256 + x) * 4 + 2] <= 101,
                "sample escaped stage"
            );
        }
    }

    // Glass may overlap every stage edge. Out-of-stage optical samples are
    // black, including the internal boundary against the bright blue panel.
    let mut white = vec![255; 256 * 192 * 4];
    for y in 0..192 {
        for x in 220..256 {
            white[(y * 256 + x) * 4..(y * 256 + x) * 4 + 4].copy_from_slice(&[0, 0, 255, 255]);
        }
    }
    h.upload(&white);
    for (center, sample) in [
        (Pos2::new(30.0, 96.0), (10, 96)),
        (Pos2::new(190.0, 96.0), (210, 96)),
        (Pos2::new(128.0, 30.0), (128, 10)),
        (Pos2::new(128.0, 170.0), (128, 185)),
    ] {
        p = params();
        p.center = center;
        p.mode = GlassMode::Concave;
        p.curvature = 1.0;
        p.distance = 300.0;
        for ppp in [1.0, 2.0] {
            let mut scaled = p.clone();
            scaled.center = scaled.center / ppp;
            scaled.size /= ppp;
            scaled.radius /= ppp;
            scaled.thickness /= ppp;
            scaled.margin /= ppp;
            scaled.distance /= ppp;
            let pixels = h.render(&scaled, ppp, 220.0 / ppp);
            let at = (sample.1 * 256 + sample.0) * 4;
            assert_eq!(
                &pixels[at..at + 4],
                &[0, 0, 0, 255],
                "out-of-stage ray must be black: {center:?} at {ppp}x"
            );
            for y in 0..192 {
                let start = (y * 256 + 220) * 4;
                let end = (y * 256 + 256) * 4;
                assert_eq!(
                    &pixels[start..end],
                    &white[start..end],
                    "glass must be clipped before the panel"
                );
            }
        }
    }
    // A flat Gaussian-blurred plate straddling the right boundary must blend
    // white stage pixels with black padding, never duplicate the edge texel.
    p = params();
    p.center = Pos2::new(220.0, 96.0);
    p.curvature = 0.0;
    p.blur = 12.0;
    let pixels = h.render(&p, 1.0, 220.0);
    let edge = (96 * 256 + 219) * 4;
    assert!(pixels[edge] > 100 && pixels[edge] < 180);
    assert_eq!(
        pixels[edge],
        pixels[edge + 2],
        "blue panel leaked into blur"
    );

    let checker: Vec<u8> = (0..192)
        .flat_map(|y| {
            (0..256).flat_map(move |x| {
                let v = if (x / 2 + y / 2) % 2 == 0 { 0 } else { 255 };
                [v, v, v, 255]
            })
        })
        .collect();
    h.upload(&checker);
    p = params();
    p.curvature = 0.0;
    let sharp = h.render(&p, 1.0, 256.0);
    p.blur = 12.0;
    let soft = h.render(&p, 1.0, 256.0);
    assert!(
        variation(&soft) < variation(&sharp) * 0.2,
        "blur must reduce high-frequency detail"
    );

    // Same physical geometry expressed in half-size logical coordinates at 2x DPI.
    h.upload(&source);
    p = params();
    p.mode = GlassMode::Lens;
    p.blur = 4.0;
    let one_x = h.render(&p, 1.0, 256.0);
    p.center = Pos2::new(p.center.x * 0.5, p.center.y * 0.5);
    p.size *= 0.5;
    p.radius *= 0.5;
    p.distance *= 0.5;
    p.blur *= 0.5;
    p.thickness *= 0.5;
    p.margin *= 0.5;
    let two_x = h.render(&p, 2.0, 128.0);
    assert!(
        error(&one_x, &two_x) < 0.2,
        "DPI must preserve physical optics"
    );

    h.upload(&checker);
    p = params();
    p.curvature = 0.0;
    p.blur_mode = BlurMode::Frosted;
    p.roughness = 0.0;
    assert_eq!(
        h.render(&p, 1.0, 256.0),
        checker,
        "zero roughness is transparent"
    );
    p.roughness = 0.2;
    let frosted = h.render(&p, 1.0, 256.0);
    assert_eq!(
        h.render(&p, 1.0, 256.0),
        frosted,
        "grain must be stable across frames"
    );
    assert!(
        variation(&frosted) < variation(&checker) * 0.5,
        "rough surface must scatter fine detail"
    );
    p.ior = 1.0;
    assert_eq!(
        h.render(&p, 1.0, 256.0),
        checker,
        "no index contrast means no surface scattering"
    );
    p.ior = 1.45;
    p.distance = 0.0;
    p.thickness = 0.0;
    assert_eq!(
        h.render(&p, 1.0, 256.0),
        checker,
        "zero optical propagation has no blur footprint"
    );
    p.thickness = 90.0;
    let thick_frosted = h.render(&p, 1.0, 256.0);
    assert!(
        error(&thick_frosted, &checker) > 0.1,
        "thickness must grow surface-scattering footprint"
    );
    p.thickness = 0.0;
    p.distance = 150.0;
    assert!(
        error(&h.render(&p, 1.0, 256.0), &checker) > 0.1,
        "background distance must also grow the scattering footprint"
    );

    // Rebuild textures and their bindings, then verify the original result again.
    h.effects.resize(&h.device, [320, 200]);
    h.effects.resize(&h.device, h.size);
    h.upload(&source);
    p.enabled = false;
    assert_eq!(h.render(&p, 1.0, 256.0), source);

    let srgb = Harness::new(wgpu::TextureFormat::Rgba8UnormSrgb);
    srgb.upload(&source);
    let srgb_pixels = srgb.render(&p, 1.0, 256.0);
    assert!(
        error(&srgb_pixels, &source) < 0.5,
        "sRGB output must preserve displayed colors"
    );
    eprintln!(
        "Passed: convex/concave/liquid, radial symmetry, thickness, circle, margin, Gaussian/frosted, stable grain, DPI, bounds, resize, sRGB"
    );
}
