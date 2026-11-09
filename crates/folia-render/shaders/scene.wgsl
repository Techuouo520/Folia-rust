// Meshes and images of an iced canvas, drawn into the glass blur source. Text uses glyph.wgsl.

struct Screen {
    size: vec4<f32>,
}

@group(0) @binding(0) var<uniform> screen: Screen;

fn project(p: vec2<f32>) -> vec4<f32> {
    let size = max(screen.size.xy, vec2<f32>(1.0));
    return vec4<f32>(p.x / size.x * 2.0 - 1.0, 1.0 - p.y / size.y * 2.0, 0.0, 1.0);
}

fn premultiply(color: vec4<f32>) -> vec4<f32> {
    return vec4<f32>(color.xyz * color.a, color.a);
}

struct SolidIn {
    @location(0) pos: vec2<f32>,
    @location(1) color: vec4<f32>,
}

struct SolidOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) color: vec4<f32>,
}

@vertex
fn solid_vs(in: SolidIn) -> SolidOut {
    var out: SolidOut;
    out.pos = project(in.pos);
    out.color = premultiply(in.color);
    return out;
}

@fragment
fn solid_fs(in: SolidOut) -> @location(0) vec4<f32> {
    return in.color;
}

fn unpack_u32(data: vec2<u32>) -> vec4<f32> {
    let rg = unpack2x16float(data.x);
    let ba = unpack2x16float(data.y);
    return vec4<f32>(rg.y, rg.x, ba.y, ba.x);
}

fn unpack_color(data: vec2<u32>) -> vec4<f32> {
    return premultiply(unpack_u32(data));
}

struct GradientIn {
    @location(0) pos: vec2<f32>,
    @location(1) @interpolate(flat) colors_1: vec4<u32>,
    @location(2) @interpolate(flat) colors_2: vec4<u32>,
    @location(3) @interpolate(flat) colors_3: vec4<u32>,
    @location(4) @interpolate(flat) colors_4: vec4<u32>,
    @location(5) @interpolate(flat) offsets: vec4<u32>,
    @location(6) direction: vec4<f32>,
}

struct GradientOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) raw: vec2<f32>,
    @location(1) @interpolate(flat) colors_1: vec4<u32>,
    @location(2) @interpolate(flat) colors_2: vec4<u32>,
    @location(3) @interpolate(flat) colors_3: vec4<u32>,
    @location(4) @interpolate(flat) colors_4: vec4<u32>,
    @location(5) @interpolate(flat) offsets: vec4<u32>,
    @location(6) direction: vec4<f32>,
}

@vertex
fn gradient_vs(in: GradientIn) -> GradientOut {
    var out: GradientOut;
    out.pos = project(in.pos);
    out.raw = in.pos;
    out.colors_1 = in.colors_1;
    out.colors_2 = in.colors_2;
    out.colors_3 = in.colors_3;
    out.colors_4 = in.colors_4;
    out.offsets = in.offsets;
    out.direction = in.direction;
    return out;
}

fn random(coords: vec2<f32>) -> f32 {
    return fract(sin(dot(coords, vec2<f32>(12.9898, 78.233))) * 43758.5453);
}

fn gradient_at(raw: vec2<f32>, direction: vec4<f32>, colors: array<vec4<f32>, 8>, offsets: array<f32, 8>, last: i32) -> vec4<f32> {
    let start = direction.xy;
    let end = direction.zw;
    let unit = normalize(end - start);
    let coord = dot(unit, raw - start) / length(end - start);
    var colors_arr = colors;
    var offsets_arr = offsets;
    var color = colors_arr[0];
    for (var i: i32 = 0; i < last; i++) {
        let a = offsets_arr[i];
        let b = offsets_arr[i + 1];
        if (a <= coord && coord <= b) {
            color = mix(colors_arr[i], colors_arr[i + 1], smoothstep(a, b, coord));
        }
    }
    if (coord >= offsets_arr[last]) {
        color = colors_arr[last];
    }
    let noise = 0.3 / 255.0;
    return color + mix(-noise, noise, random(raw));
}

@fragment
fn gradient_fs(in: GradientOut) -> @location(0) vec4<f32> {
    let colors = array<vec4<f32>, 8>(
        unpack_color(in.colors_1.xy),
        unpack_color(in.colors_1.zw),
        unpack_color(in.colors_2.xy),
        unpack_color(in.colors_2.zw),
        unpack_color(in.colors_3.xy),
        unpack_color(in.colors_3.zw),
        unpack_color(in.colors_4.xy),
        unpack_color(in.colors_4.zw),
    );
    let offsets_1 = unpack_u32(in.offsets.xy);
    let offsets_2 = unpack_u32(in.offsets.zw);
    var offsets = array<f32, 8>(
        offsets_1.x, offsets_1.y, offsets_1.z, offsets_1.w,
        offsets_2.x, offsets_2.y, offsets_2.z, offsets_2.w,
    );
    var last = 7;
    for (var i: i32 = 0; i <= 7; i++) {
        if (offsets[i] >= 1.0) {
            last = i;
            break;
        }
    }
    return gradient_at(in.raw, in.direction, colors, offsets, last);
}

struct ImageIn {
    @location(0) center: vec2<f32>,
    @location(1) tile: vec4<f32>,
    @location(2) clip: vec4<f32>,
    @location(3) radius: vec4<f32>,
    @location(4) rotation: f32,
    @location(5) opacity: f32,
}

struct ImageOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) @interpolate(flat) clip: vec4<f32>,
    @location(1) @interpolate(flat) radius: vec4<f32>,
    @location(2) @interpolate(flat) opacity: f32,
    @location(3) uv: vec2<f32>,
}

@group(1) @binding(0) var image: texture_2d<f32>;
@group(1) @binding(1) var image_sampler: sampler;

fn corner(index: u32) -> vec2<f32> {
    return vec2<f32>((vec2<u32>(1u, 2u) + index) % vec2<u32>(6u) < vec2<u32>(3u));
}

@vertex
fn image_vs(@builtin(vertex_index) index: u32, in: ImageIn) -> ImageOut {
    let tile = in.tile;
    let at = corner(index);
    let corners = array<vec2<f32>, 4>(
        tile.xy,
        tile.xy + vec2<f32>(tile.z, 0.0),
        tile.xy + vec2<f32>(0.0, tile.w),
        tile.xy + tile.zw,
    );
    let cos_r = cos(-in.rotation);
    let sin_r = sin(-in.rotation);
    var turned = array<vec2<f32>, 4>();
    for (var i = 0u; i < 4u; i++) {
        let c = corners[i] - in.center;
        turned[i] = vec2<f32>(c.x * cos_r - c.y * sin_r, c.x * sin_r + c.y * cos_r) + in.center;
    }
    var min_xy = turned[0];
    var max_xy = turned[0];
    for (var i = 1u; i < 4u; i++) {
        min_xy = min(min_xy, turned[i]);
        max_xy = max(max_xy, turned[i]);
    }
    let bounds = vec4<f32>(min_xy, max_xy - min_xy);
    let clip_min = max(bounds.xy, in.clip.xy);
    let clip_max = min(bounds.xy + bounds.zw, in.clip.xy + in.clip.zw);
    let clipped = vec4<f32>(clip_min, max(vec2<f32>(0.0), clip_max - clip_min));
    let p = clipped.xy + at * clipped.zw;
    var out: ImageOut;
    out.pos = project(p);
    out.clip = in.clip;
    out.radius = in.radius;
    out.opacity = in.opacity;
    let uv = (p - tile.xy) / max(tile.zw, vec2<f32>(0.0001));
    let uv_center = vec2<f32>(0.5);
    let d = uv - uv_center;
    out.uv = vec2<f32>(d.x * cos_r - d.y * sin_r, d.x * sin_r + d.y * cos_r) + uv_center;
    return out;
}

fn rounded_box_sdf(p: vec2<f32>, size: vec2<f32>, corners: vec4<f32>) -> f32 {
    let box_half = select(corners.yz, corners.xw, p.x > 0.0);
    let corner_radius = select(box_half.y, box_half.x, p.y > 0.0);
    let q = abs(p) - size + corner_radius;
    return min(max(q.x, q.y), 0.0) + length(max(q, vec2<f32>(0.0))) - corner_radius;
}

@fragment
fn image_fs(in: ImageOut) -> @location(0) vec4<f32> {
    let scale = max(screen.size.z, 0.0001);
    let clip = in.clip * scale;
    let radius = min(in.radius * scale, vec4<f32>(min(clip.z, clip.w) / 2.0));
    let d = rounded_box_sdf(2.0 * (in.pos.xy - clip.xy - clip.zw / 2.0), clip.zw, radius * 2.0) / 2.0;
    let antialias = clamp(1.0 - d, 0.0, 1.0);
    let sample = textureSample(image, image_sampler, in.uv);
    return sample * vec4<f32>(1.0, 1.0, 1.0, antialias * in.opacity);
}
