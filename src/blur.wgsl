struct BlurParams {
    direction_radius: vec4<f32>, // texel direction xy, radius in physical pixels, stage u max
}
@group(0) @binding(0) var input_image: texture_2d<f32>;
@group(0) @binding(1) var filtering: sampler;
@group(0) @binding(2) var<uniform> params: BlurParams;

@vertex
fn vs_main(@builtin(vertex_index) index: u32) -> @builtin(position) vec4<f32> {
    let x = f32((index << 1u) & 2u);
    let y = f32(index & 2u);
    return vec4<f32>(x * 2.0 - 1.0, 1.0 - y * 2.0, 0.0, 1.0);
}

fn bounded_texel(image: texture_2d<f32>, index: vec2<i32>, bounds: vec2<f32>) -> vec3<f32> {
    if any(index < vec2<i32>(0)) || any(vec2<f32>(index) + vec2<f32>(0.5) >= bounds) {
        return vec3<f32>(0.0);
    }
    return textureLoad(image, index, 0).rgb;
}

fn black_border_sample(image: texture_2d<f32>, pixel: vec2<f32>, bounds: vec2<f32>) -> vec3<f32> {
    if all(pixel >= vec2<f32>(0.5)) && all(pixel <= floor(bounds - vec2<f32>(0.5)) + vec2<f32>(0.5)) {
        return textureSampleLevel(image, filtering, pixel / vec2<f32>(textureDimensions(image)), 0.0).rgb;
    }
    let location = pixel - vec2<f32>(0.5);
    let index = vec2<i32>(floor(location));
    let fraction = fract(location);
    let a = mix(bounded_texel(image, index, bounds), bounded_texel(image, index + vec2<i32>(1, 0), bounds), fraction.x);
    let b = mix(bounded_texel(image, index + vec2<i32>(0, 1), bounds), bounded_texel(image, index + vec2<i32>(1, 1), bounds), fraction.x);
    return mix(a, b, fraction.y);
}

@fragment
fn fs_main(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
    let dims = vec2<f32>(textureDimensions(input_image));
    let uv = position.xy / dims;
    let bounds = vec2<f32>(params.direction_radius.w, 1.0) * dims;
    let radius = params.direction_radius.z;
    let sigma = max(radius / 3.0, 0.25);
    var sum = black_border_sample(input_image, position.xy, bounds);
    var total = 1.0;
    // Pair adjacent texels using bilinear filtering. This keeps a dense Gaussian
    // kernel at high DPI instead of sparse taps that cause stripes on text.
    for (var i = 1.0; i <= ceil(radius); i += 2.0) {
        let a = exp(-0.5 * i * i / (sigma * sigma));
        var b = 0.0;
        if i + 1.0 <= ceil(radius) {
            b = exp(-0.5 * (i + 1.0) * (i + 1.0) / (sigma * sigma));
        }
        let weight = a + b;
        let paired_offset = i + b / max(weight, 0.000001);
        let offset = params.direction_radius.xy * paired_offset;
        sum += (black_border_sample(input_image, (uv + offset) * dims, bounds)
             + black_border_sample(input_image, (uv - offset) * dims, bounds)) * weight;
        total += weight * 2.0;
    }
    return vec4<f32>(sum / total, 1.0);
}
