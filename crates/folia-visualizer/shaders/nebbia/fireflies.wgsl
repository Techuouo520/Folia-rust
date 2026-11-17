// Dew firefly bodies, moving wings, focus blur and theme-coloured tails from nebbiaFireflyGl.ts.

struct View { size: vec2<f32>, shift: vec2<f32>, flags: vec4<f32> }
@group(0) @binding(0) var<uniform> view: View;
@group(0) @binding(2) var glow_texture: texture_2d<f32>;
@group(0) @binding(4) var linear: sampler;

struct Vertex {
    @builtin(position) position: vec4<f32>,
    @location(0) local: vec2<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) @interpolate(flat) parameters: vec4<f32>,
    @location(3) @interpolate(flat) glow: vec4<f32>,
}

@vertex
fn vs(@builtin(vertex_index) index: u32, @location(0) a: vec4<f32>, @location(1) b: vec4<f32>, @location(2) c: vec4<f32>) -> Vertex {
    let extent = max(2.6, 4.1 * view.flags.z);
    let local = (vec2<f32>(f32(index & 1u), f32(index >> 1u)) * 2.0 - vec2<f32>(1.0)) * extent;
    let off = local * a.z;
    let cs = cos(b.z);
    let sn = sin(b.z);
    let point = a.xy + vec2<f32>(off.x * cs - off.y * sn, off.x * sn + off.y * cs);
    var out: Vertex;
    out.position = vec4<f32>(point.x / view.size.x * 2.0 - 1.0, 1.0 - point.y / view.size.y * 2.0, 0.0, 1.0);
    out.local = local; out.uv = point / view.size;
    out.parameters = vec4<f32>(a.w, b.x, b.y, b.w); out.glow = c;
    return out;
}

fn blob(point: vec2<f32>, center: vec2<f32>, axes: vec2<f32>, angle: f32, edge: f32) -> f32 {
    let q = point - center;
    let cs = cos(angle); let sn = sin(angle);
    let local = vec2<f32>(q.x * cs + q.y * sn, -q.x * sn + q.y * cs) / axes;
    return 1.0 - smoothstep(1.0 - edge, 1.0 + edge, length(local));
}

fn to_linear(color: vec3<f32>) -> vec3<f32> {
    return select(pow((color + vec3<f32>(0.055)) / 1.055, vec3<f32>(2.4)), color / 12.92, color <= vec3<f32>(0.04045));
}

@fragment
fn fs(input: Vertex) -> @location(0) vec4<f32> {
    let focus = input.parameters.y;
    let flap = input.parameters.w;
    let q = input.local;
    let edge = 0.18 + 0.9 * (1.0 - focus);
    let body = max(max(blob(q, vec2<f32>(0.62, 0.0), vec2<f32>(0.17, 0.15), 0.0, edge), blob(q, vec2<f32>(0.3, 0.0), vec2<f32>(0.22, 0.2), 0.0, edge)), blob(q, vec2<f32>(-0.25, 0.0), vec2<f32>(0.48, 0.19), 0.0, edge));
    let angle = 0.12 + 1.2 * flap;
    let wing_edge = edge + 0.3 * flap;
    let wing = max(blob(q, vec2<f32>(-0.15 + 0.2 * flap, 0.08 + 0.45 * flap), vec2<f32>(0.62, 0.17), angle, wing_edge), blob(q, vec2<f32>(-0.15 + 0.2 * flap, -0.08 - 0.45 * flap), vec2<f32>(0.62, 0.17), -angle, wing_edge));
    let sampled = textureSample(glow_texture, linear, input.uv).rgb;
    let light = select(vec3<f32>(0.42), sampled, view.flags.x > 0.5);
    let luminance = dot(light, vec3<f32>(0.3, 0.59, 0.11));
    let hue = clamp(mix(vec3<f32>(1.0), light / max(luminance, 0.02), 0.8), vec3<f32>(0.0), vec3<f32>(2.2));
    let lit = hue * clamp(0.22 + 2.5 * luminance, 0.0, 1.4);
    let body_alpha = body * 0.5;
    let wing_alpha = wing * (0.3 - 0.1 * flap);
    let covered = (wing_alpha + body_alpha * (1.0 - wing_alpha)) * input.parameters.x;
    var rgb = (vec3<f32>(0.95, 0.97, 1.0) * wing_alpha + vec3<f32>(0.86, 0.88, 0.92) * body_alpha * (1.0 - wing_alpha)) * lit * input.parameters.x;
    let distance = length(q);
    let halo = 0.35 * (1.0 - focus) * max(0.0, 1.0 - distance / (1.6 - 0.6 * focus)) * input.parameters.x;
    rgb = vec3<f32>(0.92, 0.95, 1.0) * lit * halo + rgb * (1.0 - halo);
    let alpha = halo + covered * (1.0 - halo);
    let soft = view.flags.x * smoothstep(0.12, 0.6, luminance) * (1.0 - 0.75 * smoothstep(0.6, 1.2, luminance)) * 0.45 * exp(-distance * distance * 0.3 / (view.flags.z * view.flags.z)) * input.parameters.z;
    let tail_distance = length(q - vec2<f32>(-0.55, 0.0));
    let radius = 0.45 + 0.35 * (1.0 - focus);
    let lamp = 1.0 - smoothstep(0.0, radius, tail_distance) + 0.8 * exp(-tail_distance * tail_distance * 0.7);
    let tint = mix(input.glow.yzw, vec3<f32>(1.0), 0.35 * (1.0 - smoothstep(0.0, radius, tail_distance)));
    let emission = min(light, vec3<f32>(1.2)) * soft + tint * lamp * input.glow.x * input.parameters.x * mix(0.5, 1.0, focus);
    if view.flags.y > 0.5 {
        rgb = to_linear(max(rgb, vec3<f32>(0.0)) / max(alpha, 0.00001)) * alpha + to_linear(max(emission, vec3<f32>(0.0)));
    } else { rgb += emission; }
    return vec4<f32>(rgb, alpha);
}
