struct Params {
    viewport: vec4<f32>, // physical width, height, pixels per point, stage width in points
    rect: vec4<f32>, // center and size in points
    optics: vec4<f32>, // radius, curvature, background distance, blur radius
    appearance: vec4<f32>, // IOR, highlight, enabled, mode
    geometry: vec4<f32>, // minimum thickness, shape, polished margin width, blur mode
    material: vec4<f32>, // roughness, reserved
    output: vec4<f32>, // target is sRGB
}
@group(0) @binding(0) var scene: texture_2d<f32>;
@group(0) @binding(1) var blurred: texture_2d<f32>;
@group(0) @binding(2) var filtering: sampler;
@group(0) @binding(3) var<uniform> params: Params;

@vertex
fn vs_main(@builtin(vertex_index) index: u32) -> @builtin(position) vec4<f32> {
    let x = f32((index << 1u) & 2u);
    let y = f32(index & 2u);
    return vec4<f32>(x * 2.0 - 1.0, 1.0 - y * 2.0, 0.0, 1.0);
}

fn sd_shape(p: vec2<f32>) -> f32 {
    let half_size = params.rect.zw * 0.5;
    if params.geometry.y > 0.5 {
        return length(p) - min(half_size.x, half_size.y);
    }
    let radius = min(params.optics.x, min(half_size.x, half_size.y));
    let q = abs(p) - half_size + vec2<f32>(radius);
    return length(max(q, vec2<f32>(0.0))) + min(max(q.x, q.y), 0.0) - radius;
}

// A rolled profile measured along the actual contour's inward distance.
// The sixth-power arc meets the flat center with continuous curvature; the
// small radicand floor keeps the rim slope finite. Return value and derivative.
fn rolled_edge(depth: f32) -> vec2<f32> {
    let t = 1.0 - clamp(depth, 0.0, 1.0);
    let t2 = t * t;
    let t5 = t2 * t2 * t;
    let root = sqrt(max(1.0 - 0.99 * t5 * t, 0.01));
    return vec2<f32>((root - 0.1) / 0.9, 3.3 * t5 / root);
}

// The exact contour distance defines the bevel footprint. Its gradient is
// undefined on interior medial axes when the bevel is wider than the radius.
// Regularize only the normal field there, rather than changing the footprint.
fn contour_gradient(p: vec2<f32>, bevel: f32) -> vec2<f32> {
    let half_size = params.rect.zw * 0.5;
    if params.geometry.y > 0.5 {
        return p / max(length(p), 0.0001);
    }
    let radius = min(params.optics.x, min(half_size.x, half_size.y));
    if bevel <= radius {
        // All non-flat samples have a unique nearest contour point.
        let q = abs(p) - half_size + vec2<f32>(radius);
        let outside = max(q, vec2<f32>(0.0));
        let radial = length(outside);
        if radial > 0.0001 { return sign(p) * outside / radial; }
        return vec2<f32>(0.0); // flat core; profile slope is zero here
    }
    let epsilon = max((bevel - radius) * 0.15, 0.5);
    let e2 = epsilon * epsilon;
    let absolute = sqrt(p * p + vec2<f32>(e2));
    let q = absolute - half_size + vec2<f32>(radius);
    let root = sqrt(q * q + vec2<f32>(e2));
    let positive = 0.5 * (q + root);
    let positive_gradient = 0.5 * (vec2<f32>(1.0) + q / root);
    let delta = q.x - q.y;
    let max_root = sqrt(delta * delta + e2);
    let maximum = 0.5 * (q.x + q.y + max_root);
    let max_gradient = 0.5 * (vec2<f32>(1.0) + vec2<f32>(delta, -delta) / max_root);
    let negative_gradient = 0.5 * (1.0 - maximum / sqrt(maximum * maximum + e2));
    let gradient = positive / max(length(positive), 0.0001) * positive_gradient
        + negative_gradient * max_gradient;
    return gradient * p / absolute;
}

struct Surface { height: f32, gradient: vec2<f32> }

fn surface(p: vec2<f32>) -> Surface {
    let half_size = max(params.rect.zw * 0.5, vec2<f32>(1.0));
    let base = max(params.geometry.x, 0.0);
    if params.appearance.w > 0.5 {
        let radius = select(length(half_size), min(half_size.x, half_size.y), params.geometry.y > 0.5);
        let radial = dot(p, p) / (radius * radius);
        let sag = params.optics.y * radius * 0.35;
        if params.appearance.w > 1.5 {
            return Surface(base + sag * radial, 2.0 * sag * p / (radius * radius));
        }
        return Surface(base + sag * max(1.0 - radial, 0.0), -2.0 * sag * p / (radius * radius));
    }
    let bevel = min(params.geometry.z, min(half_size.x, half_size.y));
    if bevel <= 0.001 { return Surface(base, vec2<f32>(0.0)); }
    let sag = params.optics.y * min(half_size.x, half_size.y) * 0.2;
    let edge = rolled_edge(-sd_shape(p) / bevel);
    let gradient = -sag * edge.y / bevel * contour_gradient(p, bevel);
    return Surface(base + sag * edge.x, gradient);
}

struct ExitHit { point: vec3<f32>, facing_normal: vec3<f32> }

fn exit_hit(p: vec2<f32>, front_height: f32, inside_ray: vec3<f32>) -> ExitHit {
    let start = vec3<f32>(p, front_height);
    let back = vec3<f32>(p + inside_ray.xy * front_height / max(-inside_ray.z, 0.0001), 0.0);
    // The subpixel AA fringe lies just outside the mathematical aperture. Do
    // not mistake those coverage samples for rays crossing a vertical wall.
    if sd_shape(back.xy) <= 0.0 || sd_shape(p) > 0.0 {
        return ExitHit(back, vec3<f32>(0.0, 0.0, 1.0));
    }
    // The glass is a convex aperture extruded to the flat back plane. A ray
    // leaving its aperture first hits a side wall, not the back interface.
    var lo = 0.0;
    var hi = 1.0;
    for (var i = 0; i < 14; i += 1) {
        let mid = (lo + hi) * 0.5;
        if sd_shape(mix(start.xy, back.xy, mid)) <= 0.0 { lo = mid; }
        else { hi = mid; }
    }
    let hit = mix(start, back, (lo + hi) * 0.5);
    let gradient = vec2<f32>(
        sd_shape(hit.xy + vec2<f32>(0.5, 0.0)) - sd_shape(hit.xy - vec2<f32>(0.5, 0.0)),
        sd_shape(hit.xy + vec2<f32>(0.0, 0.5)) - sd_shape(hit.xy - vec2<f32>(0.0, 0.5)));
    // refract's normal must face the incident medium (glass at this interface).
    return ExitHit(hit, -normalize(vec3<f32>(gradient, 0.0)));
}

fn micro_normal(base: vec3<f32>, slope: vec2<f32>) -> vec3<f32> {
    let reference = select(vec3<f32>(0.0, 0.0, 1.0), vec3<f32>(0.0, 1.0, 0.0), abs(base.z) > 0.9);
    let tangent = normalize(cross(reference, base));
    let bitangent = cross(base, tangent);
    return normalize(base + tangent * slope.x + bitangent * slope.y);
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

fn transmission_color(p: vec2<f32>, normal: vec3<f32>, back_slope: vec2<f32>) -> vec3<f32> {
    let point = params.rect.xy + p;
    if normal.z <= 0.0001 {
        // A rough microfacet facing away from the camera has no direct beam.
        return textureSampleLevel(scene, filtering, point * params.viewport.z / params.viewport.xy, 0.0).rgb * 0.65;
    }
    let inside_ray = refract(vec3<f32>(0.0, 0.0, -1.0), normal, 1.0 / params.appearance.x);
    let hit = exit_hit(p, surface(p).height, inside_ray);
    var facing = micro_normal(hit.facing_normal, back_slope);
    if dot(facing, inside_ray) > 0.0 { facing = -facing; }
    let outside_ray = refract(inside_ray, facing, params.appearance.x);
    if dot(outside_ray, outside_ray) <= 0.0001 || outside_ray.z >= -0.0001 {
        // TIR / ray missing the background: reflection approximation, never
        // normalize a zero refract result into a made-up transmission direction.
        return textureSampleLevel(scene, filtering, point * params.viewport.z / params.viewport.xy, 0.0).rgb * 0.65;
    }
    let air_length = (hit.point.z + params.optics.z) / -outside_ray.z;
    let sample_position = params.rect.xy + hit.point.xy + outside_ray.xy * air_length;
    let bounds = vec2<f32>(params.viewport.w * params.viewport.z, params.viewport.y);
    let pixel = sample_position * params.viewport.z;
    if params.geometry.w < 0.5 && params.optics.w > 0.0 {
        return black_border_sample(blurred, pixel, bounds);
    }
    return black_border_sample(scene, pixel, bounds);
}

fn hash(p: vec2<f32>) -> f32 {
    return fract(sin(dot(p, vec2<f32>(127.1, 311.7))) * 43758.5453);
}

fn frosted_color(p: vec2<f32>, normal: vec3<f32>) -> vec3<f32> {
    // 16 low-discrepancy samples of Gaussian microfacet slopes (Beckmann-like).
    // The two interfaces have independent azimuths. Fixed spatial seeds give a
    // fine, stable grain instead of time-dependent noise or a uniform Gaussian.
    let cell = floor(p / 0.8); // grain is attached to the glass, not the screen
    let angle_front = hash(cell) * 6.2831853;
    let angle_back = hash(cell + vec2<f32>(73.0, 19.0)) * 6.2831853;
    var color = vec3<f32>(0.0);
    for (var i = 0; i < 16; i += 1) {
        let u = (f32(i) + 0.5) / 16.0;
        let radius = params.material.x * sqrt(-log(1.0 - u));
        let phi = f32(i) * 2.3999632;
        let front = radius * vec2<f32>(cos(phi + angle_front), sin(phi + angle_front));
        let back = radius * vec2<f32>(cos(-phi + angle_back), sin(-phi + angle_back));
        color += transmission_color(p, micro_normal(normal, front), back);
    }
    return color / 16.0;
}

fn gamma_to_linear(c: vec3<f32>) -> vec3<f32> {
    return select(c / 12.92, pow((c + vec3<f32>(0.055)) / 1.055, vec3<f32>(2.4)), c > vec3<f32>(0.04045));
}

@fragment
fn fs_main(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
    let uv = position.xy / params.viewport.xy;
    let point = position.xy / params.viewport.z;
    let original = textureSampleLevel(scene, filtering, uv, 0.0).rgb;
    var color = original;
    let p = point - params.rect.xy;
    let distance = sd_shape(p);
    let coverage = 1.0 - smoothstep(-0.75 / params.viewport.z, 0.75 / params.viewport.z, distance);
    if params.appearance.z > 0.5 && coverage > 0.0 && point.x < params.viewport.w {
        let front = surface(p);
        let normal = normalize(vec3<f32>(-front.gradient, 1.0));
        var refracted = original;
        if params.geometry.w > 0.5 && params.material.x > 0.0 {
            refracted = frosted_color(p, normal);
        } else {
            refracted = transmission_color(p, normal, vec2<f32>(0.0));
        }
        let rim = exp(-abs(distance) / 1.5);
        let fresnel = pow(1.0 - normal.z, 3.0);
        let lighting = params.appearance.y * (rim * 0.3 + fresnel * 0.18);
        let glass_color = mix(refracted, vec3<f32>(0.90, 0.96, 1.0), clamp(lighting, 0.0, 1.0));
        color = mix(original, glass_color, coverage);
    }
    // Offscreen egui colors are gamma encoded. Only decode when the attachment
    // automatically encodes sRGB on write; non-sRGB targets need no conversion.
    if params.output.x > 0.5 {
        color = gamma_to_linear(color);
    }
    return vec4<f32>(color, 1.0);
}
