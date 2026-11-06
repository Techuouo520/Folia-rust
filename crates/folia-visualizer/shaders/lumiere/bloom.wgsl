// Dual-filter bloom. The first downsample prefilters; later passes only blur. Output adds the glow onto the picture.

struct VsOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

struct Down {
    texel: vec4<f32>,
    params: vec4<f32>,
}

struct Up {
    texel: vec4<f32>,
    add: vec4<f32>,
}

@group(0) @binding(0) var tex: texture_2d<f32>;
@group(0) @binding(1) var tex_sampler: sampler;
@group(0) @binding(2) var<uniform> down: Down;

@group(1) @binding(0) var add_tex: texture_2d<f32>;
@group(1) @binding(1) var<uniform> up: Up;

@vertex
fn vs(@builtin(vertex_index) index: u32) -> VsOut {
    var corners = array<vec2<f32>, 3>(vec2<f32>(-1.0, -1.0), vec2<f32>(3.0, -1.0), vec2<f32>(-1.0, 3.0));
    let p = corners[index];
    var out: VsOut;
    out.pos = vec4<f32>(p, 0.0, 1.0);
    out.uv = vec2<f32>(p.x * 0.5 + 0.5, 0.5 - p.y * 0.5);
    return out;
}

fn tap(uv: vec2<f32>) -> vec4<f32> {
    return textureSample(tex, tex_sampler, uv);
}

@fragment
fn fs_down(in: VsOut) -> @location(0) vec4<f32> {
    let hp = down.texel.xy;
    var sum = tap(in.uv) * 4.0;
    sum = sum + tap(in.uv - hp);
    sum = sum + tap(in.uv + hp);
    sum = sum + tap(in.uv + vec2<f32>(hp.x, -hp.y));
    sum = sum + tap(in.uv - vec2<f32>(hp.x, -hp.y));
    var color = sum / 8.0;
    if (down.params.z > 0.5) {
        let br = max(color.r, max(color.g, color.b));
        let knee = max(down.params.y, 1e-4);
        let soft = clamp(br - down.params.x + knee, 0.0, 2.0 * knee);
        let shaped = soft * soft / (4.0 * knee);
        let contrib = max(shaped, br - down.params.x) / max(br, 1e-4);
        color = color * clamp(contrib, 0.0, 1.0);
    }
    return color;
}

@fragment
fn fs_up(in: VsOut) -> @location(0) vec4<f32> {
    let hp = up.texel.xy;
    var sum = tap(in.uv + vec2<f32>(-hp.x * 2.0, 0.0));
    sum = sum + tap(in.uv + vec2<f32>(-hp.x, hp.y)) * 2.0;
    sum = sum + tap(in.uv + vec2<f32>(0.0, hp.y * 2.0));
    sum = sum + tap(in.uv + vec2<f32>(hp.x, hp.y)) * 2.0;
    sum = sum + tap(in.uv + vec2<f32>(hp.x * 2.0, 0.0));
    sum = sum + tap(in.uv + vec2<f32>(hp.x, -hp.y)) * 2.0;
    sum = sum + tap(in.uv + vec2<f32>(0.0, -hp.y * 2.0));
    sum = sum + tap(in.uv + vec2<f32>(-hp.x, -hp.y)) * 2.0;
    let blurred = sum / 12.0;
    let same = textureSample(add_tex, tex_sampler, in.uv);
    return blurred + same * up.add.x;
}

@fragment
fn fs_combine(in: VsOut) -> @location(0) vec4<f32> {
    let base = tap(in.uv);
    let glow = textureSample(add_tex, tex_sampler, in.uv) * up.add.x;
    var rgb = base.rgb + glow.rgb * (1.0 - base.a * 0.5);
    let peak = max(max(rgb.r, rgb.g), rgb.b);
    if (peak > 1.0) {
        rgb = mix(rgb / peak, vec3<f32>(1.0), vec3<f32>(0.2 * (1.0 - 1.0 / peak)));
    }
    let a = min(max(base.a + max(max(glow.r, glow.g), glow.b) * (1.0 - base.a), max(max(rgb.r, rgb.g), rgb.b)), 1.0);
    return vec4<f32>(rgb, a);
}
