use egui::Vec2;
use hello_wgpu::{glass::GlassParams, renderer::GLASS_SHADER};
use wgpu::util::DeviceExt;

#[test]
#[ignore = "Requires a GPU; samples the actual WGSL surface"]
fn bevel_follows_rounded_contour_with_equal_width_and_continuous_normals() {
    let instance =
        wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
    let adapter = pollster::block_on(instance.request_adapter(&Default::default())).unwrap();
    let (device, queue) = pollster::block_on(adapter.request_device(&Default::default())).unwrap();
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("Actual liquid surface regression"),
        source: wgpu::ShaderSource::Wgsl(
            format!(
                "{GLASS_SHADER}\n{}",
                r#"
@group(1) @binding(0) var<storage, read_write> samples: array<vec4<f32>>;
@compute @workgroup_size(64)
fn probe(@builtin(global_invocation_id) id: vec3<u32>) {
    if id.x >= 1061u { return; }
    if id.x == 1060u {
        samples[id.x] = vec4<f32>(surface(vec2<f32>(100.0, 65.0)).height, 0.0, 0.0, 0.0);
        return;
    }
    if id.x >= 804u {
        let fixture = id.x - 804u;
        let quadrant = fixture / 64u;
        let angle = f32(fixture % 16u) / 15.0 * 1.5707963268;
        let depth = min(params.optics.x, params.geometry.z) * (0.1 + f32((fixture / 16u) % 4u) * 0.28);
        let signs = vec2<f32>(select(-1.0, 1.0, (quadrant & 1u) == 0u),
                              select(-1.0, 1.0, (quadrant & 2u) == 0u));
        let half = params.rect.zw * 0.5;
        let arc = (half - vec2<f32>(params.optics.x)
                  + (params.optics.x - depth) * vec2<f32>(cos(angle), sin(angle))) * signs;
        let side = vec2<f32>(half.x - depth, 0.0);
        let flat = vec2<f32>(half.x - params.geometry.z - 2.0, 0.0);
        samples[id.x] = vec4<f32>(surface(side).height, surface(arc).height, -sd_shape(arc), surface(flat).height);
        return;
    }
    let quadrant = id.x / 201u;
    let epsilon = (f32(id.x % 201u) - 100.0) * 0.05;
    let signs = vec2<f32>(select(-1.0, 1.0, (quadrant & 1u) == 0u),
                          select(-1.0, 1.0, (quadrant & 2u) == 0u));
    let p = (params.rect.zw * 0.5 - vec2<f32>(45.0) + vec2<f32>(epsilon, -epsilon)) * signs;
    let s = surface(p);
    samples[id.x] = vec4<f32>(s.height, s.gradient * signs, 0.0);
}
"#
            )
            .into(),
        ),
    });
    let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: Some("Probe wide bevel"),
        layout: None,
        module: &shader,
        entry_point: Some("probe"),
        compilation_options: Default::default(),
        cache: None,
    });
    let mut params = GlassParams {
        size: Vec2::new(300.0, 220.0),
        radius: 32.0,
        margin: 100.0,
        ..Default::default()
    };
    let uniform = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: None,
        contents: bytemuck::bytes_of(&params.uniform([1, 1], 1.0, 1.0, false)),
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
    });
    let output = device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: 1061 * 16,
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
        mapped_at_creation: false,
    });
    let readback = device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: output.size(),
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let groups = [(3, &uniform), (0, &output)]
        .into_iter()
        .enumerate()
        .map(|(index, (binding, buffer))| {
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: None,
                layout: &pipeline.get_bind_group_layout(index as u32),
                entries: &[wgpu::BindGroupEntry {
                    binding,
                    resource: buffer.as_entire_binding(),
                }],
            })
        })
        .collect::<Vec<_>>();
    let mut fixed_corner_heights = Vec::new();
    for radius in [0.0, 32.0, 80.0, 110.0] {
        for width in [12.0, 32.0, 64.0, 100.0] {
            params.radius = radius;
            params.margin = width;
            queue.write_buffer(
                &uniform,
                0,
                bytemuck::bytes_of(&params.uniform([1, 1], 1.0, 1.0, false)),
            );
            let mut encoder = device.create_command_encoder(&Default::default());
            {
                let mut pass = encoder.begin_compute_pass(&Default::default());
                pass.set_pipeline(&pipeline);
                for (i, group) in groups.iter().enumerate() {
                    pass.set_bind_group(i as u32, group, &[]);
                }
                pass.dispatch_workgroups(17, 1, 1);
            }
            encoder.copy_buffer_to_buffer(&output, 0, &readback, 0, output.size());
            queue.submit([encoder.finish()]);
            let (tx, rx) = std::sync::mpsc::channel();
            readback.map_async(wgpu::MapMode::Read, .., move |result| {
                tx.send(result).unwrap()
            });
            device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
            rx.recv().unwrap().unwrap();
            let bytes = readback.get_mapped_range(..).unwrap();
            let values: &[[f32; 4]] = bytemuck::cast_slice(&bytes);
            for quadrant in values[..804].as_chunks::<201>().0 {
                for pair in quadrant.windows(2) {
                    for component in 1..=2 {
                        assert!(pair[0][component].is_finite());
                        assert!(
                            (pair[0][component] - pair[1][component]).abs() < 0.002,
                            "normal jumps across a corner diagonal: {pair:?}"
                        );
                    }
                }
                for (sample, reflected) in quadrant.iter().zip(&values[..201]) {
                    for component in 0..3 {
                        assert!((sample[component] - reflected[component]).abs() < 0.0001);
                    }
                }
            }
            let flat_height = params.thickness + params.curvature * params.size.min_elem() * 0.1;
            for sample in &values[804..1060] {
                assert!(
                    (sample[0] - sample[1]).abs() < 0.0001,
                    "same inward distance must have same surface height: radius={radius}, width={width}, {sample:?}"
                );
                assert!(
                    (sample[3] - flat_height).abs() < 0.0001,
                    "bevel extends beyond its stated width"
                );
            }
            if width == 100.0 && radius > 0.0 {
                fixed_corner_heights.push(values[1060][0]);
            }
            drop(bytes);
            readback.unmap();
        }
    }
    assert!(
        fixed_corner_heights[0] > fixed_corner_heights[1] + 0.01
            && fixed_corner_heights[1] > fixed_corner_heights[2] + 0.01,
        "changing corner radius must change the surface at a fixed corner point: {fixed_corner_heights:?}"
    );
}
