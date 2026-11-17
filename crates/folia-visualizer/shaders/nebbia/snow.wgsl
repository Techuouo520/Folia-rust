// Snowflake and frozen-bead atlas lighting, refracted ice and fixed dispersion from nebbiaSnowGl.ts.

struct View {
    size: vec2<f32>,
    shift: vec2<f32>,
    flags: vec4<f32>,
}
@group(0) @binding(0) var<uniform> view: View;
@group(0) @binding(1) var atlas: texture_2d<f32>;
@group(0) @binding(2) var glow: texture_2d<f32>;
@group(0) @binding(3) var behind: texture_2d<f32>;
@group(0) @binding(4) var linear: sampler;

struct Vertex {
    @builtin(position) position: vec4<f32>,
    @location(0) local: vec2<f32>,
    @location(1) halo: vec2<f32>,
    @location(2) uv: vec2<f32>,
    @location(3) @interpolate(flat) stage: vec4<f32>,
    @location(4) @interpolate(flat) radius: f32,
    @location(5) @interpolate(flat) seed: f32,
}

@vertex
fn vs(@builtin(vertex_index) index: u32, @location(0) a: vec4<f32>, @location(1) b: vec4<f32>, @location(2) c: vec2<f32>) -> Vertex {
    let corner = vec2<f32>(f32(index & 1u), f32(index >> 1u));
    let local = (corner * 2.0 - vec2<f32>(1.0)) * 2.6;
    let off = local * a.z;
    let cs = cos(a.w);
    let sn = sin(a.w);
    let point = a.xy + vec2<f32>(off.x * cs - off.y * sn, off.x * sn + off.y * cs) + view.shift;
    var out: Vertex;
    out.position = vec4<f32>(point.x / view.size.x * 2.0 - 1.0, 1.0 - point.y / view.size.y * 2.0, 0.0, 1.0);
    out.local = local / c;
    out.halo = local;
    out.uv = point / view.size;
    out.stage = b;
    out.radius = a.z;
    out.seed = fract(sin(a.w * 12.9898 + b.y * 78.233) * 43758.5453);
    return out;
}

fn fixed_fire(uv: vec2<f32>, normal: vec2<f32>, seed: f32) -> vec3<f32> {
    let weights = vec3<f32>(0.3, 0.59, 0.11);
    let step = vec2<f32>(0.025, 0.0);
    let gradient = vec2<f32>(
        dot(textureSampleLevel(glow, linear, uv + step.xy, 0.0).rgb - textureSampleLevel(glow, linear, uv - step.xy, 0.0).rgb, weights),
        dot(textureSampleLevel(glow, linear, uv + step.yx, 0.0).rgb - textureSampleLevel(glow, linear, uv - step.yx, 0.0).rgb, weights)
    );
    let light_length = length(gradient);
    let normal_length = length(normal);
    if light_length < 0.0001 || normal_length < 0.0001 { return vec3<f32>(0.0); }
    let angle = (dot(normal / normal_length, gradient / light_length) - mix(0.6, 0.95, seed)) / 0.05;
    if abs(angle) >= 1.0 { return vec3<f32>(0.0); }
    let hue = (angle * 0.5 + 0.5) * 0.75;
    let spectrum = clamp(abs(fract(vec3<f32>(hue) + vec3<f32>(0.0, 0.667, 0.333)) * 6.0 - vec3<f32>(3.0)) - vec3<f32>(1.0), vec3<f32>(0.0), vec3<f32>(1.0));
    return spectrum * (1.0 - angle * angle) * smoothstep(0.002, 0.03, light_length);
}

fn tile(stage: f32, row: f32, uv: vec2<f32>, bias: f32) -> vec4<f32> {
    return textureSampleBias(atlas, linear, (vec2<f32>(stage, row) + uv) / vec2<f32>(16.0, 15.0), bias);
}

@fragment
fn fs(input: Vertex) -> @location(0) vec4<f32> {
    let focus = input.stage.w;
    let uv = vec2<f32>(0.5) + input.local * 0.42;
    let clipped = clamp(uv, vec2<f32>(0.0), vec2<f32>(1.0));
    let stage = floor(input.stage.x);
    let fraction = input.stage.x - stage;
    let bias = 2.0 * (1.0 - focus);
    var color = mix(tile(stage, input.stage.y, clipped, bias), tile(min(stage + 1.0, 15.0), input.stage.y, clipped, bias), fraction);
    color *= input.stage.z * select(0.0, 1.0, all(uv == clipped));
    let sampled = textureSample(glow, linear, input.uv).rgb;
    let light = select(vec3<f32>(0.42), sampled, view.flags.x > 0.5);
    let luminance = dot(light, vec3<f32>(0.3, 0.59, 0.11));
    let hue = clamp(mix(vec3<f32>(1.0), light / max(luminance, 0.02), 0.8), vec3<f32>(0.0), vec3<f32>(2.2));
    let lit = hue * clamp(0.22 + 2.5 * luminance, 0.0, 1.4);
    var rgb = color.rgb * lit;
    let backlight = view.flags.x * smoothstep(0.08, 0.45, luminance) * focus * (1.0 - 0.7 * input.stage.x / 15.0);
    let alpha = color.a / max(input.stage.z, 0.001);
    let normal = vec2<f32>(dpdx(alpha), dpdy(alpha)) * input.radius;
    let edge = clamp(length(normal) * 0.5, 0.0, 1.0);
    let bend = normal * input.radius * 0.7 / view.size;
    let split = normal / max(length(normal), 0.001) * input.radius * 0.35 * edge / view.size;
    var through = vec3<f32>(textureSample(behind, linear, input.uv - bend * 0.8 - split).r, textureSample(behind, linear, input.uv - bend).g, textureSample(behind, linear, input.uv - bend * 1.25 + split).b);
    let gray = vec3<f32>(dot(through, vec3<f32>(0.3333)));
    through = gray + (through - gray) * (1.0 + 1.5 * edge);
    let ice = (through * (1.1 + 0.3 * edge) + min(light, vec3<f32>(1.2)) * edge * 0.3) * color.a + color.rgb * lit * 0.15;
    rgb = mix(rgb, ice, backlight * 0.85);
    let distance = length(input.halo);
    let halo = 0.35 * (1.0 - focus) * max(0.0, 1.0 - distance / (1.6 - 0.6 * focus));
    rgb = vec3<f32>(0.92, 0.95, 1.0) * lit * halo + rgb * (1.0 - halo);
    let opacity = halo + color.a * (1.0 - halo);
    let soft = view.flags.x * smoothstep(0.12, 0.6, luminance) * 0.45 * exp(-distance * distance * 2.4) * input.stage.z * (1.0 - input.stage.x / 15.0);
    let grid = clipped * 9.0;
    let cell = floor(grid);
    let offset = fract(grid) - vec2<f32>(0.5);
    let hash = fract(sin(dot(cell, vec2<f32>(12.9898, 78.233)) + input.seed * 91.7) * 43758.5453);
    let fire = fixed_fire(input.uv, normal, fract(hash * 7.31)) * step(hash, 0.4);
    let glint = backlight * exp(-dot(offset, offset) * 18.0) * edge * (0.6 + 2.0 * luminance) * input.stage.z;
    let emission = min(light, vec3<f32>(1.2)) * soft + fire * glint * 2.2;
    if view.flags.y > 0.5 {
        let straight = max(rgb, vec3<f32>(0.0)) / max(opacity, 0.00001);
        rgb = select(pow((straight + vec3<f32>(0.055)) / 1.055, vec3<f32>(2.4)), straight / 12.92, straight <= vec3<f32>(0.04045)) * opacity;
        let glow = max(emission, vec3<f32>(0.0));
        rgb += select(pow((glow + vec3<f32>(0.055)) / 1.055, vec3<f32>(2.4)), glow / 12.92, glow <= vec3<f32>(0.04045));
    } else { rgb += emission; }
    return vec4<f32>(rgb, opacity);
}
