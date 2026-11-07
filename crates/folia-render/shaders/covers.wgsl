// Cover quads for the glass blur source. One atlas, one draw.

struct VsIn {
    @location(0) pos: vec2<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) local: vec2<f32>,
    @location(3) color0: vec4<f32>,
    @location(4) color1: vec4<f32>,
    @location(5) params: vec4<f32>,
}

struct VsOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) local: vec2<f32>,
    @location(2) color0: vec4<f32>,
    @location(3) color1: vec4<f32>,
    @location(4) params: vec4<f32>,
}

struct Frame {
    window: vec4<f32>,
    shade: vec4<f32>,
    tilt: vec4<f32>,
    encode: vec4<f32>,
}

@group(0) @binding(0) var<uniform> frame: Frame;
@group(1) @binding(0) var atlas: texture_2d<f32>;
@group(1) @binding(1) var atlas_sampler: sampler;

fn shade_a(t: f32, kind: f32) -> f32 {
    if (kind > 1.5) {
        return 0.0;
    }
    if (kind > 0.5) {
        if (t <= 0.55) { return 0.0; }
        if (t <= 0.74) { return (t - 0.55) / 0.19 * 0.12; }
        return 0.12 + (t - 0.74) / 0.26 * 0.28;
    }
    if (t <= 0.34) { return 0.05; }
    if (t <= 0.56) { return 0.05 + (t - 0.34) / 0.22 * 0.19; }
    return 0.24 + (t - 0.56) / 0.44 * 0.68;
}

fn to_linear(rgb: vec3<f32>) -> vec3<f32> {
    let hi = pow((rgb + vec3<f32>(0.055)) / 1.055, vec3<f32>(2.4));
    return select(hi, rgb / 12.92, rgb <= vec3<f32>(0.04045));
}

@vertex
fn vs(in: VsIn) -> VsOut {
    let pivot = frame.tilt.xy;
    let angle = frame.tilt.z;
    let d = in.pos - pivot;
    let turned = pivot + vec2<f32>(cos(angle) * d.x - sin(angle) * d.y, sin(angle) * d.x + cos(angle) * d.y);
    let window = max(frame.window.xy, vec2<f32>(1.0));
    let clip = vec2<f32>(turned.x / window.x * 2.0 - 1.0, 1.0 - turned.y / window.y * 2.0);
    var out: VsOut;
    out.pos = vec4<f32>(clip, 0.0, 1.0);
    out.uv = in.uv;
    out.local = in.local;
    out.color0 = in.color0;
    out.color1 = in.color1;
    out.params = in.params;
    return out;
}

@fragment
fn fs(in: VsOut) -> @location(0) vec4<f32> {
    var rgb = mix(in.color0.rgb, in.color1.rgb, (in.local.x + in.local.y) * 0.5);
    if (in.params.z > 0.5) {
        let sample = textureSample(atlas, atlas_sampler, in.uv);
        rgb = mix(in.color0.rgb, sample.rgb, sample.a);
    }
    rgb = mix(rgb, frame.shade.rgb, shade_a(in.local.y, in.params.x));
    if (frame.encode.x > 0.5) {
        rgb = to_linear(rgb);
    }
    return vec4<f32>(rgb, in.params.y);
}
