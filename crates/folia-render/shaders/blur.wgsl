// Dual Kawase blur. No brightness threshold: every pixel of the picture is blurred.

struct VsOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

struct Tap {
    texel: vec4<f32>,
}

@group(0) @binding(0) var tex: texture_2d<f32>;
@group(0) @binding(1) var tex_sampler: sampler;
@group(0) @binding(2) var<uniform> tap: Tap;

@vertex
fn vs(@builtin(vertex_index) index: u32) -> VsOut {
    var corners = array<vec2<f32>, 3>(vec2<f32>(-1.0, -1.0), vec2<f32>(3.0, -1.0), vec2<f32>(-1.0, 3.0));
    let p = corners[index];
    var out: VsOut;
    out.pos = vec4<f32>(p, 0.0, 1.0);
    out.uv = vec2<f32>(p.x * 0.5 + 0.5, 0.5 - p.y * 0.5);
    return out;
}

@fragment
fn fs_down(in: VsOut) -> @location(0) vec4<f32> {
    let hp = tap.texel.xy;
    var sum = textureSample(tex, tex_sampler, in.uv) * 4.0;
    sum = sum + textureSample(tex, tex_sampler, in.uv - hp);
    sum = sum + textureSample(tex, tex_sampler, in.uv + hp);
    sum = sum + textureSample(tex, tex_sampler, in.uv + vec2<f32>(hp.x, -hp.y));
    sum = sum + textureSample(tex, tex_sampler, in.uv - vec2<f32>(hp.x, -hp.y));
    return sum / 8.0;
}

@fragment
fn fs_up(in: VsOut) -> @location(0) vec4<f32> {
    let hp = tap.texel.xy;
    var sum = textureSample(tex, tex_sampler, in.uv + vec2<f32>(-hp.x * 2.0, 0.0));
    sum = sum + textureSample(tex, tex_sampler, in.uv + vec2<f32>(-hp.x, hp.y)) * 2.0;
    sum = sum + textureSample(tex, tex_sampler, in.uv + vec2<f32>(0.0, hp.y * 2.0));
    sum = sum + textureSample(tex, tex_sampler, in.uv + vec2<f32>(hp.x, hp.y)) * 2.0;
    sum = sum + textureSample(tex, tex_sampler, in.uv + vec2<f32>(hp.x * 2.0, 0.0));
    sum = sum + textureSample(tex, tex_sampler, in.uv + vec2<f32>(hp.x, -hp.y)) * 2.0;
    sum = sum + textureSample(tex, tex_sampler, in.uv + vec2<f32>(0.0, -hp.y * 2.0));
    sum = sum + textureSample(tex, tex_sampler, in.uv + vec2<f32>(-hp.x, -hp.y)) * 2.0;
    return sum / 12.0;
}

@fragment
fn fs_copy(in: VsOut) -> @location(0) vec4<f32> {
    return textureSample(tex, tex_sampler, in.uv);
}
