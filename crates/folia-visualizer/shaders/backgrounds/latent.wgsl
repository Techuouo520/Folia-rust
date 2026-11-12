// Latent background: Paper mesh-gradient under a warp 4x4 dither, then the theme veil.
// Mesh math follows paper-design/shaders mesh-gradient.ts (Apache-2.0). Dither is the warp shape
// with a 4x4 Bayer matrix, blended soft-light the way the CSS layer does.

struct VsOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

struct Uniforms {
    resolution: vec2<f32>,
    pixel_ratio: f32,
    mesh_time: f32,
    dither_time: f32,
    distortion: f32,
    swirl: f32,
    dither_size: f32,
    dither_opacity: f32,
    show_mesh: f32,
    show_dither: f32,
    overlay: f32,
    mesh_scale: f32,
    dither_scale: f32,
    saturate: f32,
    brightness: f32,
    _pad0: f32,
    _pad1: f32,
    _pad2: f32,
    _pad3: f32,
    dither_back: vec4<f32>,
    dither_front: vec4<f32>,
    overlay_color: vec4<f32>,
    mesh: array<vec4<f32>, 6>,
}

@group(0) @binding(0) var<uniform> u: Uniforms;

@vertex
fn vs(@builtin(vertex_index) index: u32) -> VsOut {
    var positions = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, -1.0),
        vec2<f32>(3.0, -1.0),
        vec2<f32>(-1.0, 3.0),
    );
    let p = positions[index];
    var out: VsOut;
    out.pos = vec4<f32>(p, 0.0, 1.0);
    out.uv = vec2<f32>(p.x * 0.5 + 0.5, 0.5 - p.y * 0.5);
    return out;
}

fn hash21(p_in: vec2<f32>) -> f32 {
    var p = fract(p_in * vec2<f32>(0.3183099, 0.3678794)) + vec2<f32>(0.1);
    p = p + vec2<f32>(dot(p, p + vec2<f32>(19.19)));
    return fract(p.x * p.y);
}

fn value_noise(st: vec2<f32>) -> f32 {
    let i = floor(st);
    let f = fract(st);
    let a = hash21(i);
    let b = hash21(i + vec2<f32>(1.0, 0.0));
    let c = hash21(i + vec2<f32>(0.0, 1.0));
    let d = hash21(i + vec2<f32>(1.0, 1.0));
    let uu = f * f * (vec2<f32>(3.0) - 2.0 * f);
    return mix(mix(a, b, uu.x), mix(c, d, uu.x), uu.y);
}

fn rotate(uv: vec2<f32>, th: f32) -> vec2<f32> {
    let c = cos(th);
    let s = sin(th);
    return vec2<f32>(c * uv.x - s * uv.y, s * uv.x + c * uv.y);
}

fn spot(i: i32, t: f32) -> vec2<f32> {
    let a = f32(i) * 0.37;
    let b = 0.6 + fract(f32(i) / 3.0) * 0.9;
    let c = 0.8 + fract(f32(i + 1) / 4.0);
    let x = sin(t * b + a);
    let y = cos(t * c + a * 1.5);
    return vec2<f32>(0.5) + vec2<f32>(0.5) * vec2<f32>(x, y);
}

fn mesh_color(uv_in: vec2<f32>) -> vec4<f32> {
    var uv = uv_in;
    let grain_uv = uv * 1000.0;
    let t = 0.5 * (u.mesh_time + 41.5);
    let radius = smoothstep(0.0, 1.0, length(uv - vec2<f32>(0.5)));
    let center = 1.0 - radius;
    for (var i = 1.0; i <= 2.0; i = i + 1.0) {
        let sy = smoothstep(0.0, 1.0, uv.y);
        let sx = smoothstep(0.0, 1.0, uv.x);
        uv.x = uv.x + u.distortion * center / i * sin(t + i * 0.4 * sy) * cos(0.2 * t + i * 2.4 * sy);
        uv.y = uv.y + u.distortion * center / i * cos(t + i * 2.0 * sx);
    }
    var turned = uv - vec2<f32>(0.5);
    turned = rotate(turned, -3.0 * u.swirl * radius);
    turned = turned + vec2<f32>(0.5);
    var color = vec3<f32>(0.0);
    var opacity = 0.0;
    var total = 0.0;
    for (var i = 0; i < 6; i = i + 1) {
        let pos = spot(i, t);
        let ink = u.mesh[i];
        let dist = pow(length(turned - pos), 3.5);
        let weight = 1.0 / (dist + 0.001);
        color = color + ink.rgb * ink.a * weight;
        opacity = opacity + ink.a * weight;
        total = total + weight;
    }
    let scale = max(0.0001, total);
    color = color / scale;
    opacity = opacity / scale;
    let grain = value_noise(rotate(grain_uv, 1.0) + vec2<f32>(3.0));
    let mixed = mix(grain, value_noise(rotate(grain_uv, 2.0) + vec2<f32>(-1.0)), 0.5);
    let shaped = pow(mixed, 1.3) * 2.0 - 1.0;
    let strength = pow(0.01 * abs(shaped), 0.8);
    let grain_color = vec3<f32>(step(0.0, shaped));
    color = mix(color, grain_color, 0.35 * strength);
    opacity = clamp(opacity + 0.5 * strength, 0.0, 1.0);
    return vec4<f32>(color, opacity);
}

fn bayer4(uv: vec2<f32>) -> f32 {
    let pos = vec2<i32>(fract(uv / 4.0) * 4.0);
    let x = clamp(pos.x, 0, 3);
    let y = clamp(pos.y, 0, 3);
    var m = array<f32, 16>(
        0.0, 8.0, 2.0, 10.0,
        12.0, 4.0, 14.0, 6.0,
        3.0, 11.0, 1.0, 9.0,
        15.0, 7.0, 13.0, 5.0,
    );
    return m[y * 4 + x] / 16.0;
}

fn grade(rgb: vec3<f32>) -> vec3<f32> {
    let gray = dot(rgb, vec3<f32>(0.2126, 0.7152, 0.0722));
    return mix(vec3<f32>(gray), rgb, u.saturate) * u.brightness;
}

fn zoom(uv: vec2<f32>, scale: f32) -> vec2<f32> {
    return vec2<f32>(0.5) + (uv - vec2<f32>(0.5)) / max(scale, 0.001);
}

/// Warp dither in the shader canvas's own pixels. `uv` is 0..1 inside this widget, not the window.
fn dither_ink(uv: vec2<f32>) -> vec3<f32> {
    let local = zoom(uv, u.dither_scale) * u.resolution;
    let px_size = max(u.dither_size, 0.5) * u.pixel_ratio;
    let px_uv = (local - 0.5 * u.resolution) / px_size;
    let snapped = (floor(px_uv) + 0.5) * px_size;
    let normalized = snapped / u.resolution;
    var shape_uv = normalized * u.resolution / u.pixel_ratio;
    shape_uv = shape_uv + vec2<f32>(0.5);
    let t = 0.5 * u.dither_time;
    shape_uv = shape_uv * 0.003;
    for (var i = 1.0; i < 6.0; i = i + 1.0) {
        shape_uv.x = shape_uv.x + 0.6 / i * cos(i * 2.5 * shape_uv.y + t);
        shape_uv.y = shape_uv.y + 0.6 / i * cos(i * 1.5 * shape_uv.x + t);
    }
    var shape = 0.15 / max(0.001, abs(sin(t - shape_uv.y - shape_uv.x)));
    shape = smoothstep(0.02, 1.0, shape);
    let threshold = bayer4(px_uv) - 0.5;
    let on = step(0.5, shape + threshold);
    return mix(u.dither_back.rgb, u.dither_front.rgb, on);
}

fn soft_channel(b: f32, s: f32) -> f32 {
    if (s <= 0.5) {
        return b - (1.0 - 2.0 * s) * b * (1.0 - b);
    }
    let d = select(sqrt(b), ((16.0 * b - 12.0) * b + 4.0) * b, b <= 0.25);
    return b + (2.0 * s - 1.0) * (d - b);
}

fn soft_light(base: vec3<f32>, blend: vec3<f32>) -> vec3<f32> {
    return vec3<f32>(soft_channel(base.r, blend.r), soft_channel(base.g, blend.g), soft_channel(base.b, blend.b));
}

@fragment
fn fragment(in: VsOut) -> @location(0) vec4<f32> {
    var rgb = u.overlay_color.rgb;
    if (u.show_mesh > 0.5) {
        let mesh = mesh_color(zoom(in.uv, u.mesh_scale));
        let covered = mesh.rgb * mesh.a + rgb * (1.0 - mesh.a);
        rgb = grade(covered);
    }
    if (u.show_dither > 0.5) {
        let ink = dither_ink(in.uv);
        let mixed = select(ink, soft_light(rgb, ink), u.show_mesh > 0.5);
        let opacity = select(1.0, clamp(u.dither_opacity, 0.0, 1.0), u.show_mesh > 0.5);
        rgb = mix(rgb, mixed, opacity);
    }
    rgb = mix(rgb, u.overlay_color.rgb, clamp(u.overlay, 0.0, 1.0));
    return vec4<f32>(rgb, 1.0);
}
