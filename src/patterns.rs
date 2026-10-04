//! Stable, periodic height fields converted to tangent-space normal maps once.
//! RGB contains a unit normal; alpha preserves the scalar relief for inspection.
use crate::glass::GlassPattern;

pub const RESOLUTION: u32 = 512;

fn hash(x: i32, y: i32, seed: u32) -> f32 {
    let mut h = (x as u32).wrapping_mul(0x8da6_b343) ^ (y as u32).wrapping_mul(0xd816_3841) ^ seed;
    h ^= h >> 16;
    h = h.wrapping_mul(0x7feb_352d);
    h ^= h >> 15;
    (h & 0xffff) as f32 / 65535.0
}

fn noise(x: f32, y: f32, period: i32, seed: u32) -> f32 {
    let ix = x.floor() as i32;
    let iy = y.floor() as i32;
    let smooth = |v: f32| v * v * v * (v * (v * 6.0 - 15.0) + 10.0);
    let u = smooth(x.fract());
    let v = smooth(y.fract());
    let at = |dx: i32, dy: i32| {
        hash(
            (ix + dx).rem_euclid(period),
            (iy + dy).rem_euclid(period),
            seed,
        )
    };
    let mix = |a: f32, b: f32, t: f32| a + (b - a) * t;
    mix(mix(at(0, 0), at(1, 0), u), mix(at(0, 1), at(1, 1), u), v)
}

fn cellular(x: f32, y: f32, period: i32, crystal: bool) -> f32 {
    let mut nearest = f32::MAX;
    let mut second = f32::MAX;
    let mut facet = 0.0;
    for dy in -1..=1 {
        for dx in -1..=1 {
            let cx = x.floor() as i32 + dx;
            let cy = y.floor() as i32 + dy;
            let hx = cx.rem_euclid(period);
            let hy = cy.rem_euclid(period);
            let px = cx as f32 + 0.15 + hash(hx, hy, 17) * 0.7;
            let py = cy as f32 + 0.15 + hash(hx, hy, 93) * 0.7;
            let distance = (x - px).powi(2) + (y - py).powi(2);
            if distance < nearest {
                second = nearest;
                nearest = distance;
                // Tilted planes bounded by irregular cellular seams.
                facet = (x - px) * (hash(hx, hy, 123) - 0.5) + (y - py) * (hash(hx, hy, 331) - 0.5);
            } else if distance < second {
                second = distance;
            }
        }
    }
    if crystal {
        let seam = ((second.sqrt() - nearest.sqrt()) / 0.12).clamp(0.0, 1.0);
        let seam = seam * seam * (3.0 - 2.0 * seam);
        (0.45 + facet * 0.7) * seam
    } else {
        // Rounded dimples, separated by gently raised irregular rims.
        1.0 - (-nearest * 2.8).exp()
    }
}

pub fn height_field(pattern: GlassPattern) -> Vec<f32> {
    let bytes: Option<&[u8]> = match pattern {
        GlassPattern::Quatrefoil => {
            Some(include_bytes!("../assets/patterns/quatrefoil-height.png"))
        }
        GlassPattern::Fern => Some(include_bytes!("../assets/patterns/fern-height.png")),
        _ => None,
    };
    if let Some(bytes) = bytes {
        let image = image::load_from_memory(bytes)
            .expect("embedded pattern must decode")
            .to_luma8();
        let image = image::imageops::resize(
            &image,
            RESOLUTION,
            RESOLUTION,
            image::imageops::FilterType::Triangle,
        );
        let image = image::imageops::blur(&image, 0.8);
        let mut heights: Vec<f32> = image
            .into_raw()
            .into_iter()
            .map(|v| f32::from(v) / 255.0)
            .collect();
        // Generated artwork is only approximately tileable. Blend opposite edge
        // strips with a smooth taper before differentiating, so wrapping cannot
        // introduce a tall spurious ridge. The interior artwork is preserved.
        let n = RESOLUTION as usize;
        let band = n / 24;
        for horizontal in [true, false] {
            for line in 0..n {
                for i in 0..band {
                    let a = if horizontal {
                        line * n + i
                    } else {
                        i * n + line
                    };
                    let b = if horizontal {
                        line * n + n - 1 - i
                    } else {
                        (n - 1 - i) * n + line
                    };
                    let t = i as f32 / band as f32;
                    let weight = 1.0 - t * t * (3.0 - 2.0 * t);
                    let average = (heights[a] + heights[b]) * 0.5;
                    heights[a] += (average - heights[a]) * weight;
                    heights[b] += (average - heights[b]) * weight;
                }
            }
        }
        return heights;
    }
    let units = pattern.tile_units();
    (0..RESOLUTION)
        .flat_map(|y| {
            (0..RESOLUTION).map(move |x| {
                let x = x as f32 / RESOLUTION as f32 * units;
                let y = y as f32 / RESOLUTION as f32 * units;
                match pattern {
                    GlassPattern::OrangePeel => {
                        let warp_x = noise(x * 0.5, y * 0.5, 16, 5) * 0.6;
                        let warp_y = noise(x * 0.5, y * 0.5, 16, 67) * 0.6;
                        0.65 * noise(x + warp_x, y + warp_y, 32, 43)
                            + 0.25 * noise(x * 2.0, y * 2.0, 64, 83)
                            + 0.1 * noise(x * 4.0, y * 4.0, 128, 11)
                    }
                    GlassPattern::Hammered => cellular(x, y, 12, false),
                    GlassPattern::Crystal => cellular(x, y, 14, true),
                    _ => 0.5,
                }
            })
        })
        .collect()
}

pub fn normal_map(pattern: GlassPattern) -> Vec<u8> {
    let heights = height_field(pattern);
    let amplitude = match pattern {
        GlassPattern::OrangePeel => 0.75,
        GlassPattern::Hammered => 0.45,
        GlassPattern::Crystal => 0.7,
        GlassPattern::Quatrefoil => 0.07,
        GlassPattern::Fern => 0.09,
        GlassPattern::Frost => 0.0,
    };
    let n = RESOLUTION as usize;
    let factor = amplitude * RESOLUTION as f32 / (2.0 * pattern.tile_units());
    let mut pixels = Vec::with_capacity(n * n * 4);
    for y in 0..n {
        for x in 0..n {
            let dx = (heights[y * n + (x + 1) % n] - heights[y * n + (x + n - 1) % n]) * factor;
            let dy = (heights[((y + 1) % n) * n + x] - heights[((y + n - 1) % n) * n + x]) * factor;
            let length = (dx * dx + dy * dy + 1.0).sqrt();
            let encode = |v: f32| ((v * 0.5 + 0.5) * 255.0).round() as u8;
            pixels.extend_from_slice(&[
                encode(-dx / length),
                encode(-dy / length),
                encode(1.0 / length),
                (heights[y * n + x] * 255.0).round() as u8,
            ]);
        }
    }
    pixels
}

pub fn texture(device: &wgpu::Device, queue: &wgpu::Queue) -> wgpu::TextureView {
    use wgpu::util::DeviceExt;
    let pixels: Vec<u8> = GlassPattern::ALL.into_iter().flat_map(mip_chain).collect();
    let texture = device.create_texture_with_data(
        queue,
        &wgpu::TextureDescriptor {
            label: Some("Embossed glass normal atlas (linear data)"),
            size: wgpu::Extent3d {
                width: RESOLUTION,
                height: RESOLUTION,
                depth_or_array_layers: 6,
            },
            mip_level_count: RESOLUTION.ilog2() + 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        },
        wgpu::util::TextureDataOrder::LayerMajor,
        &pixels,
    );
    texture.create_view(&wgpu::TextureViewDescriptor {
        dimension: Some(wgpu::TextureViewDimension::D2Array),
        ..Default::default()
    })
}

fn mip_chain(pattern: GlassPattern) -> Vec<u8> {
    let mut current = normal_map(pattern);
    let mut result = current.clone();
    let mut width = RESOLUTION as usize;
    while width > 1 {
        let half = width / 2;
        let mut next = Vec::with_capacity(half * half * 4);
        for y in 0..half {
            for x in 0..half {
                let mut average = [0.0_f32; 4];
                for dy in 0..2 {
                    for dx in 0..2 {
                        let offset = ((y * 2 + dy) * width + x * 2 + dx) * 4;
                        for (channel, value) in average.iter_mut().enumerate() {
                            *value += f32::from(current[offset + channel]) / 4.0;
                        }
                    }
                }
                let n = [average[0], average[1], average[2]].map(|v| v / 255.0 * 2.0 - 1.0);
                let length = n.into_iter().map(|v| v * v).sum::<f32>().sqrt().max(0.001);
                for v in n {
                    next.push(((v / length * 0.5 + 0.5) * 255.0).round() as u8);
                }
                next.push(average[3].round() as u8);
            }
        }
        result.extend_from_slice(&next);
        current = next;
        width = half;
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn height_sources_and_normals_have_coherent_nonflat_relief() {
        for pattern in GlassPattern::ALL.into_iter().skip(1) {
            let heights = height_field(pattern);
            assert!(
                heights
                    .iter()
                    .all(|v| v.is_finite() && (0.0..=1.0).contains(v))
            );
            let normals = normal_map(pattern);
            let tilted = normals
                .as_chunks::<4>()
                .0
                .iter()
                .filter(|n| n[0].abs_diff(128) > 8 || n[1].abs_diff(128) > 8)
                .count();
            assert!(tilted > heights.len() / 20, "{pattern:?}: missing relief");
            for n in normals.as_chunks::<4>().0 {
                let v = [n[0], n[1], n[2]].map(|c| f32::from(c) / 255.0 * 2.0 - 1.0);
                assert!(v[2] > 0.0);
                assert!((v.into_iter().map(|c| c * c).sum::<f32>() - 1.0).abs() < 0.015);
            }
        }
    }
}
