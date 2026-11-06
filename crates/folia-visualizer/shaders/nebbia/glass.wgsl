// What is on the glass, drawn before the window pass: wiped segments into the R8 mask (nebbiaMaskGl.ts)
// and raindrop sprites into the drop textures (the sprite program of nebbiaShaders.ts).

struct View {
    css: vec2<f32>,
    mask: vec2<f32>,
}

@group(0) @binding(0) var<uniform> view: View;
@group(0) @binding(1) var sprite_tex: texture_2d<f32>;
@group(0) @binding(2) var sprite_samp: sampler;

fn corner(index: u32) -> vec2<f32> {
    return vec2<f32>(f32(index & 1u), f32(index >> 1u));
}

fn clip(p: vec2<f32>, size: vec2<f32>) -> vec4<f32> {
    return vec4<f32>(p.x / size.x * 2.0 - 1.0, 1.0 - p.y / size.y * 2.0, 0.0, 1.0);
}

struct SegOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) p: vec2<f32>,
    @location(1) @interpolate(flat) seg: vec4<f32>,
    @location(2) @interpolate(flat) param: vec2<f32>,
}

@vertex
fn vs_mask(@builtin(vertex_index) index: u32, @location(0) seg: vec4<f32>, @location(1) param: vec2<f32>) -> SegOut {
    let c = corner(index) * 2.0 - vec2<f32>(1.0, 1.0);
    let a = seg.xy;
    let b = seg.zw;
    let d = b - a;
    let l = length(d);
    var t = vec2<f32>(1.0, 0.0);
    if l > 1e-5 {
        t = d / l;
    }
    let n = vec2<f32>(-t.y, t.x);
    // The widest layer's half width plus one pixel of antialiasing.
    let r = param.x * 0.65 + 1.0;
    let p = (a + b) * 0.5 + t * c.x * (l * 0.5 + r) + n * c.y * r;
    var out: SegOut;
    out.pos = clip(p, view.mask);
    out.p = p;
    out.seg = seg;
    out.param = param;
    return out;
}

fn cover(r: f32, d: f32) -> f32 {
    return clamp(r + 0.5 - d, 0.0, 1.0);
}

fn level(x: f32) -> f32 {
    return floor(x * 255.0 + 0.5) / 255.0;
}

@fragment
fn fs_mask(in: SegOut) -> @location(0) vec4<f32> {
    let a = in.seg.xy;
    let ba = in.seg.zw - a;
    let pa = in.p - a;
    let h = clamp(dot(pa, ba) / max(dot(ba, ba), 1e-6), 0.0, 1.0);
    let d = length(pa - ba * h);
    let w = in.param.x * 0.5;
    let v = in.param.y;
    // Three layers, wide and dim to narrow and bright, stacked as the canvas `lighten` did.
    var m = level(v * 0.38) * cover(w * 1.3, d);
    m = m + cover(w, d) * (level(v * 0.72) - m);
    m = m + cover(w * 0.62, d) * (level(v) - m);
    return vec4<f32>(m, 0.0, 0.0, 1.0);
}

struct DropOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) size: f32,
    @location(2) alpha: f32,
}

@vertex
fn vs_drop(@builtin(vertex_index) index: u32, @location(0) rect: vec4<f32>, @location(1) param: vec2<f32>) -> DropOut {
    let c = corner(index);
    let p = rect.xy + (c - vec2<f32>(0.5, 0.5)) * rect.zw;
    var out: DropOut;
    out.pos = clip(p, view.css);
    out.uv = c;
    out.size = param.x;
    out.alpha = param.y;
    return out;
}

@fragment
fn fs_drop(in: DropOut) -> @location(0) vec4<f32> {
    let c = textureSample(sprite_tex, sprite_samp, in.uv);
    let a = c.a * in.alpha;
    return vec4<f32>(c.rg * a, in.size * a, a);
}
