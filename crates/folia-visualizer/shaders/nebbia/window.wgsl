// Night bus window from nebbiaSceneGlsl.ts, with the fog, breath and raindrop composite from nebbiaShaders.ts.
// Buildings are evaluated per pixel: the original cache is a pre-render of this same building().
// Colors are computed as the original's 8-bit canvas values; an sRGB target gets them decoded first.

struct VsOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

struct Uniforms {
    resolution: vec2<f32>,
    mask_size: vec2<f32>,
    time: f32,
    bus: f32,
    neon: f32,
    mist: f32,
    spot_i: f32,
    spot_r: f32,
    scale: f32,
    encode: f32,
    spot_tgt: vec2<f32>,
    spot_src: vec2<f32>,
    sky0: vec4<f32>,
    sky1: vec4<f32>,
    la: vec4<f32>,
    lb: vec4<f32>,
    lc: vec4<f32>,
    lobes: array<vec4<f32>, 8>,
    lobe_sd: array<vec4<f32>, 4>,
}

@group(0) @binding(0) var<uniform> u: Uniforms;
@group(0) @binding(1) var mask_tex: texture_2d<f32>;
@group(0) @binding(2) var samp: sampler;
@group(0) @binding(3) var drop_tex: texture_2d<f32>;
@group(0) @binding(4) var field_tex: texture_2d<f32>;

const HZ: f32 = 0.6;
const ROAD: f32 = 0.7;
const WIND: f32 = 0.6;
const SODIUM: vec3<f32> = vec3<f32>(1.0, 0.68, 0.32);

@vertex
fn vs(@builtin(vertex_index) index: u32) -> VsOut {
    var positions = array<vec2<f32>, 3>(vec2<f32>(-1.0, -1.0), vec2<f32>(3.0, -1.0), vec2<f32>(-1.0, 3.0));
    let p = positions[index];
    var out: VsOut;
    out.pos = vec4<f32>(p, 0.0, 1.0);
    out.uv = vec2<f32>(p.x * 0.5 + 0.5, 0.5 - p.y * 0.5);
    return out;
}

fn hash(p_in: vec2<f32>) -> f32 {
    var p = fract(p_in * vec2<f32>(123.34, 456.21));
    let d = dot(p, p + vec2<f32>(45.32, 45.32));
    p = p + vec2<f32>(d, d);
    return fract(p.x * p.y);
}

fn h1(n_in: f32) -> f32 {
    var n = fract(n_in * 0.1031);
    n = n * (n + 33.33);
    n = n * (n + n);
    return fract(n);
}

fn vnoise(p: vec2<f32>) -> f32 {
    let i = floor(p);
    var f = fract(p);
    f = f * f * (vec2<f32>(3.0, 3.0) - 2.0 * f);
    let a = mix(hash(i), hash(i + vec2<f32>(1.0, 0.0)), f.x);
    let b = mix(hash(i + vec2<f32>(0.0, 1.0)), hash(i + vec2<f32>(1.0, 1.0)), f.x);
    return mix(a, b, f.y);
}

fn pick(h: f32) -> vec3<f32> {
    if h < 0.34 { return u.la.xyz; }
    if h < 0.67 { return u.lb.xyz; }
    return u.lc.xyz;
}

fn glow_pt(d_in: vec2<f32>, r: f32, b: f32, streak: f32) -> f32 {
    var d = d_in;
    d.x = sign(d.x) * max(abs(d.x) - streak, 0.0);
    let s2 = r * r + b * b;
    return (r * r / s2) * exp(-dot(d, d) / s2 * 1.6);
}

fn reach(r: f32, b: f32) -> f32 {
    return 3.0 * sqrt(r * r + b * b);
}

fn lamps(q: vec2<f32>, b: f32, t: f32) -> vec3<f32> {
    var c = vec3<f32>(0.0, 0.0, 0.0);
    let xs = q.x + t * 0.42;
    let cw = 0.34;
    let streak = 0.012 * u.bus;
    let cs = floor(xs / cw);
    let sg = abs(q.y - 0.51) < 0.05 + reach(0.016, b + 0.002);
    let sh = abs(q.y - 0.645) < reach(0.026, b + 0.003);
    if sg || sh {
        for (var k: i32 = -1; k <= 1; k = k + 1) {
            let n = cs + f32(k);
            if sg && h1(n + 33.0) < 0.75 {
                let cen = vec2<f32>((n + 0.2 + 0.6 * h1(n)) * cw, 0.46 + 0.1 * h1(n + 17.0));
                c = c + pick(h1(n + 2.0)) * glow_pt(vec2<f32>(xs, q.y) - cen, 0.006 + 0.01 * h1(n + 5.0), b + 0.002, streak) * 1.6;
            }
            if sh && h1(n + 41.0) < 0.6 {
                let shop = vec2<f32>((n + 0.5) * cw, 0.645);
                let warm = mix(SODIUM, pick(h1(n + 8.0)), 0.3);
                c = c + warm * glow_pt(vec2<f32>(xs, q.y) - shop, 0.016 + 0.01 * h1(n + 3.0), b + 0.003, streak) * 0.9;
            }
        }
    }
    let xl = q.x + t * 0.22;
    let lw = 0.55;
    let cl = floor(xl / lw);
    if abs(q.y - 0.36) < reach(0.07, b) {
        for (var k: i32 = -1; k <= 1; k = k + 1) {
            let d = vec2<f32>(xl - (cl + f32(k) + 0.5) * lw, q.y - 0.36);
            c = c + SODIUM * (glow_pt(d, 0.007, b, 0.004 * u.bus) * 2.6 + glow_pt(d, 0.07, b, 0.0) * 0.3);
        }
    }
    return c;
}

fn building(q: vec2<f32>, b: f32, t: f32, layer: f32) -> vec4<f32> {
    let near = layer;
    let x = q.x + t * mix(0.02, 0.035, near) + layer * 7.7;
    let cw = mix(0.065, 0.12, near);
    let aa = max(b, 0.0012);
    let c0 = floor(x / cw);
    var res = vec4<f32>(0.0, 0.0, 0.0, 0.0);
    let fx = x - c0 * cw;
    let nb = aa + 0.0013;
    for (var k: i32 = -1; k <= 1; k = k + 1) {
        let kf = f32(k);
        if (kf < 0.0 && fx > nb) { continue; }
        if (kf > 0.0 && cw - fx > nb) { continue; }
        let n = c0 + kf + layer * 100.0;
        let w = cw * (0.72 + 0.26 * h1(n));
        let left = (c0 + kf) * cw + (cw - w) * h1(n + 1.0);
        let lx = x - left;
        let hgt = mix(0.07, 0.11, near) + mix(0.16, 0.27, near) * pow(h1(n + 2.0), 1.5);
        let base = HZ + near * 0.015;
        let top = base - hgt;
        let tier = h1(n + 3.0) < 0.5;
        let tw = w * (0.5 + 0.25 * h1(n + 4.0));
        let tl = (w - tw) * 0.5;
        let th = hgt * (0.12 + 0.15 * h1(n + 5.0));
        let body = smoothstep(-aa, aa, lx) * smoothstep(-aa, aa, w - lx) * smoothstep(-aa, aa, q.y - top);
        var up = 0.0;
        if tier {
            up = smoothstep(-aa, aa, lx - tl) * smoothstep(-aa, aa, tl + tw - lx) * smoothstep(-aa, aa, q.y - (top - th));
        }
        let roof = select(top, top - th, tier);
        var ant = 0.0;
        if h1(n + 6.0) < 0.3 {
            ant = smoothstep(0.0012 + aa, 0.0, abs(lx - w * 0.5)) * smoothstep(-aa, aa, q.y - (roof - 0.04 - 0.03 * h1(n + 7.0)));
        }
        let cov = max(max(body, up), ant);
        if cov <= 0.0 { continue; }
        var fac = mix(u.sky0.xyz, vec3<f32>(0.015, 0.018, 0.025), 0.55) * (0.7 + 0.6 * h1(n + 8.0));
        let low = pow(clamp((q.y - roof) / max(base - roof, 0.001), 0.0, 1.0), 2.5);
        fac = fac + mix(SODIUM, u.la.xyz, 0.5) * 0.1 * low;
        fac = fac + mix(u.sky1.xyz, u.lb.xyz, 0.3) * 0.12 * smoothstep(0.003 + aa, 0.0, lx) * body;
        let fh = mix(0.0075, 0.012, near);
        let wc = mix(0.0055, 0.0085, near);
        let g = vec2<f32>((lx - wc * 0.6) / wc, (q.y - roof - fh * 0.8) / fh);
        let id = floor(g);
        let f = fract(g);
        let dens = 0.15 + 0.5 * h1(n + 9.0);
        let floor_on = step(0.18, hash(vec2<f32>(id.y, n)));
        let on = step(hash(id + vec2<f32>(n * 3.7, 0.0)), dens) * floor_on;
        let inside = smoothstep(-aa, aa, lx - wc * 0.6) * smoothstep(-aa, aa, w - wc * 0.6 - lx) * smoothstep(-aa, aa, q.y - roof - fh * 0.8) * smoothstep(-aa, aa, base - fh - q.y);
        let pane = smoothstep(0.32, 0.22, abs(f.x - 0.5)) * smoothstep(0.3, 0.2, abs(f.y - 0.5));
        let blur_mix = clamp(b / (wc * 0.8), 0.0, 1.0);
        let win = mix(pane * on, 0.3 * dens * 0.82, blur_mix) * inside * max(body, up);
        let hc = hash(id + vec2<f32>(n * 1.3, 0.0));
        var wcol = vec3<f32>(1.0, 0.78, 0.5);
        if hc >= 0.5 && hc < 0.8 { wcol = vec3<f32>(0.78, 0.88, 1.0); }
        if hc >= 0.8 { wcol = pick(hash(id + vec2<f32>(5.0, 0.0))); }
        wcol = mix(wcol * (0.55 + 0.6 * hash(id + vec2<f32>(2.9, 0.0))), vec3<f32>(0.95, 0.82, 0.66) * 0.85, blur_mix);
        var bc = fac + wcol * win * mix(0.7, 1.0, near);
        bc = mix(bc, mix(u.sky0.xyz, u.sky1.xyz, 0.7), mix(0.45, 0.12, near));
        res = vec4<f32>(mix(res.xyz, bc, cov), max(res.w, cov));
    }
    return res;
}

fn neon(q: vec2<f32>, b: f32, t: f32) -> vec3<f32> {
    var c = vec3<f32>(0.0, 0.0, 0.0);
    let bb = b + 0.003;
    if abs(q.y - (HZ - 0.12)) < 0.1 + reach(0.012, bb) {
        for (var row: i32 = 0; row < 2; row = row + 1) {
            let rf = f32(row);
            let x = q.x + t * (0.06 + rf * 0.03) + rf * 7.3;
            let cw = 0.045 - rf * 0.012;
            let c0 = floor(x / cw);
            for (var k: i32 = -1; k <= 1; k = k + 1) {
                let n = c0 + f32(k) + rf * 300.0;
                if h1(n + 11.0) > 0.85 { continue; }
                let cen = vec2<f32>((c0 + f32(k) + 0.15 + 0.7 * h1(n)) * cw, HZ - 0.02 - 0.2 * pow(h1(n + 3.0), 1.6));
                let r = 0.004 + 0.008 * pow(h1(n + 6.0), 3.0);
                c = c + pick(h1(n + 9.0)) * glow_pt(vec2<f32>(x, q.y) - cen, r, bb, 0.0) * (1.0 + 1.8 * h1(n + 13.0));
            }
        }
    }
    let xb = q.x + t * 0.1;
    let bw = 0.75;
    let cb = floor(xb / bw);
    if abs(q.y - (HZ - 0.14)) < 0.06 + reach(0.13, b) {
        for (var k: i32 = -1; k <= 1; k = k + 1) {
            let n = cb + f32(k) + 500.0;
            let d = vec2<f32>(xb - (cb + f32(k) + 0.2 + 0.6 * h1(n)) * bw, q.y - (HZ - 0.08 - 0.12 * h1(n + 2.0)));
            let lc = pick(h1(n + 4.0));
            c = c + lc * (glow_pt(d, 0.014, bb, 0.0) * 2.0 + glow_pt(d, 0.13, b, 0.0) * 0.4);
        }
    }
    return c;
}

fn beam_from(src: vec2<f32>, q: vec2<f32>, b: f32) -> f32 {
    var d = u.spot_tgt - src;
    let len = length(d);
    if u.spot_i <= 0.0 || len < 0.001 { return 0.0; }
    d = d / len;
    let rel = q - src;
    let along = dot(rel, d);
    let perp = length(rel - d * clamp(along, 0.0, len));
    let k = clamp(along / len, 0.0, 1.0);
    let w = mix(0.004, u.spot_r * 0.9, k) + b;
    let body = exp(-perp * perp / (w * w)) * smoothstep(-0.01, 0.04, along) * (0.25 + 0.75 * k * k);
    let lamp = exp(-dot(rel, rel) / (0.00004 + b * b * 0.25)) * 2.5 / (1.0 + b * 50.0);
    return (body * 0.7 + lamp) * u.spot_i;
}

fn spot_beam(q: vec2<f32>, b: f32) -> f32 {
    return beam_from(vec2<f32>(u.spot_src.x, 0.595), q, b) + beam_from(vec2<f32>(u.spot_src.y, 0.595), q, b);
}

fn scene(q: vec2<f32>, b: f32) -> vec3<f32> {
    let t = u.time * u.bus;
    var col = mix(u.sky0.xyz, u.sky1.xyz, smoothstep(0.0, HZ, q.y));
    col = col + mix(u.la.xyz, SODIUM, 0.4) * 0.14 * exp(-abs(q.y - HZ + 0.12) * 6.0);
    let far = building(q, b, t, 0.0);
    col = mix(col, far.xyz, far.w);
    let near = building(q, b, t, 1.0);
    col = mix(col, near.xyz, near.w);
    col = col + (neon(q, b, t) + u.la.xyz * 0.12 * exp(-abs(q.y - HZ + 0.1) * 5.0)) * u.neon;
    col = col + lamps(q, b, t);
    let aa = max(b, 0.0015);
    let road = smoothstep(ROAD - aa, ROAD + aa, q.y);
    col = mix(col, u.sky0.xyz * 0.5, road * 0.85);
    if road > 0.0 {
        let qr = vec2<f32>(q.x, ROAD - (q.y - ROAD) * 0.55);
        col = col + lamps(qr, b + 0.012, t) * road * 0.35;
        let qn = vec2<f32>(q.x, HZ - (q.y - ROAD) * 0.22);
        col = col + neon(qn, b + 0.008, t) * road * 0.45 * u.neon;
    }
    for (var lane: i32 = 0; lane < 2; lane = lane + 1) {
        let sp = select(0.55, -1.1, lane == 1);
        let ly = select(0.76, 0.83, lane == 1);
        let ww = 0.9;
        let xc = q.x + t * sp;
        let cc = floor(xc / ww);
        let lc = select(vec3<f32>(1.0, 0.18, 0.12), vec3<f32>(1.0, 0.95, 0.85), lane == 1);
        let hd = abs(q.y - ly) < reach(0.004, b);
        let rf = road > 0.0 && abs((q.y - ly) * 3.0 - 0.03) < reach(0.02, b * 2.0);
        if !hd && !rf { continue; }
        for (var k: i32 = -1; k <= 1; k = k + 1) {
            let n = cc + f32(k) + f32(lane) * 50.0;
            if h1(n) > 0.6 { continue; }
            let cx = (cc + f32(k) + 0.2 + 0.6 * h1(n + 4.0)) * ww;
            let st = abs(sp) * 0.03 * u.bus;
            if hd {
                let c0 = glow_pt(vec2<f32>(xc - cx + 0.035, q.y - ly), 0.004, b, st) + glow_pt(vec2<f32>(xc - cx - 0.035, q.y - ly), 0.004, b, st);
                col = col + lc * c0 * 2.2;
            }
            if rf {
                col = col + lc * glow_pt(vec2<f32>(xc - cx, (q.y - ly) * 3.0 - 0.03), 0.02, b * 2.0, st) * road * 0.3;
            }
        }
    }
    let wind = max(WIND * u.bus, 0.001);
    let rd = normalize(vec2<f32>(-wind, 1.0));
    let r = vec2<f32>(dot(q, rd), dot(q, vec2<f32>(-rd.y, rd.x)));
    let col_id = floor(r.y / 0.007);
    let hr = h1(col_id);
    let seg = fract(r.x * (1.5 + hr) - u.time * (2.5 + hr * 2.0) * (1.0 + 0.4 * u.bus) + hr * 10.0);
    let line = smoothstep(0.0012 + b, 0.0, abs(fract(r.y / 0.007) - 0.5) * 0.007) * smoothstep(0.0, 0.1, seg) * smoothstep(0.3, 0.1, seg);
    // The rain shows only where the spotlights light it.
    let beam = spot_beam(q, b);
    col = col + u.la.xyz * beam + mix(u.la.xyz, vec3<f32>(1.0, 1.0, 1.0), 0.5) * line * beam * 1.6 * step(0.3, hr) * (1.0 - clamp(b * 40.0, 0.0, 1.0));
    return col;
}

fn to_linear(c: vec3<f32>) -> vec3<f32> {
    let lo = c / 12.92;
    let hi = pow((c + vec3<f32>(0.055, 0.055, 0.055)) / 1.055, vec3<f32>(2.4, 2.4, 2.4));
    return select(hi, lo, c <= vec3<f32>(0.04045, 0.04045, 0.04045));
}

@fragment
fn fs(in: VsOut) -> @location(0) vec4<f32> {
    let aspect = u.resolution.x / max(u.resolution.y, 1.0);
    // `sky0.w` is the bump slider. Without drum analysis the bus uses this idle sway, scaled by it.
    let sway = sin(u.time * 1.7) * 0.008 * u.sky0.w;
    var uv = in.uv;
    let res = u.sky1.w;
    if res < 0.99 || res > 1.01 {
        let cells = max(u.resolution * max(res, 0.5), vec2<f32>(1.0, 1.0));
        uv = (floor(in.uv * cells) + vec2<f32>(0.5, 0.5)) / cells;
    }
    let q = vec2<f32>(uv.x * aspect, uv.y + sway);
    // Noise runs in canvas pixels as the original; breath lobes are placed in CSS pixels.
    let px = in.uv * u.resolution * u.scale;
    let css = in.uv * u.resolution;
    let m = textureSample(mask_tex, samp, in.uv).r;
    let fine = vnoise(px / 6.0) * 0.6 + vnoise(px / 2.2) * 0.4;
    let blotch = vnoise(px / 55.0 + vec2<f32>(3.1, 3.1));
    let th = 0.3 + (fine - 0.5) * 0.22 * 0.6 + (blotch - 0.5) * 0.12;
    var clear = smoothstep(th - 0.05, th + 0.06, m);
    var breath = 0.0;
    let r1 = mat2x2(vec2<f32>(0.80, -0.60), vec2<f32>(0.60, 0.80)) * px;
    let r2 = mat2x2(vec2<f32>(0.36, 0.93), vec2<f32>(-0.93, 0.36)) * px;
    let bn = vnoise(r1 / 34.0 + vec2<f32>(7.0, 7.0)) * 0.55 + vnoise(r2 / 15.0 + vec2<f32>(2.3, 2.3)) * 0.3 + vnoise(px / 6.5) * 0.15;
    for (var i: i32 = 0; i < 8; i = i + 1) {
        let lobe = u.lobes[i];
        if lobe.z <= 0.0 { continue; }
        let lq = (css - lobe.xy) / max(lobe.zw, vec2<f32>(1.0, 1.0));
        if dot(lq, lq) > 1.0 { continue; }
        let sd = u.lobe_sd[i / 2];
        let pair = select(sd.zw, sd.xy, (i & 1) == 0);
        let bth = 0.3 + (bn - 0.5) * 0.5;
        breath = max(breath, smoothstep(bth - 0.2, bth + 0.25, pair.x) * pair.y * 0.7);
    }
    let dens = u.mist * (0.82 + 0.3 * vnoise(px / 380.0 + vec2<f32>(u.time * 0.015, u.time * 0.015))) * (0.9 + 0.2 * in.uv.y) + breath * 0.9;
    let haze = vec3<f32>(0.70, 0.74, 0.78);
    let blur = scene(q, 0.03);
    let glow = scene(q, 0.09);
    var fog = blur * max(0.08, 1.0 - 0.45 * dens - 0.25 * breath) + glow * 0.9 * dens + haze * (0.2 * dens + 0.25 * breath);
    fog = fog + vec3<f32>(hash(px) - 0.5) * 0.025;
    let sd = q - u.spot_tgt;
    fog = fog + u.la.xyz * u.spot_i * exp(-dot(sd, sd) / (u.spot_r * u.spot_r * 2.2)) * (0.18 + 0.4 * dens);
    fog = fog + vec3<f32>(smoothstep(0.78, 0.98, vnoise(px / 1.6)) * 0.05 * dens);
    // Drops at wipe edges and raindrops clear a ring of fog; drops condensed on the fog sit only on the fog.
    let rd = textureSample(drop_tex, samp, in.uv);
    let fd = textureSample(field_tex, samp, in.uv) * (1.0 - clear);
    clear = max(clear, smoothstep(0.93, 1.0, rd.a));
    var col = fog;
    if clear > 0.001 {
        let wiped = scene(q, 0.0012) * 1.05 + haze * 0.02 * dens;
        let rim = clear * (1.0 - clear) * 4.0;
        col = mix(fog, wiped, clear) + vec3<f32>(rim, rim, rim) * haze * 0.16 * dens;
    }
    let comp = vec4<f32>(rd.rgb + fd.rgb - 2.0 * rd.rgb * fd.rgb, max(rd.a, fd.a));
    let dmask = smoothstep(0.96, 0.99, comp.a);
    if dmask > 0.001 {
        let nn = comp.rg - vec2<f32>(0.5, 0.5);
        let ruv = in.uv + vec2<f32>(-nn.x, nn.y) * (comp.b * 0.6 + 0.4);
        var lens = scene(vec2<f32>(ruv.x * aspect, ruv.y), 0.006);
        let normal = normalize(vec3<f32>(nn * 2.0, 1.0));
        let light_dir = normalize(vec3<f32>(-1.0, 1.0, 2.0));
        let lambert = clamp(dot(light_dir, normal), 0.0, 1.0);
        let spec = pow(max(dot(normal, normalize(light_dir + vec3<f32>(0.0, 0.0, 1.0))), 0.0), 48.0);
        lens = lens + vec3<f32>(lambert - 0.8, lambert - 0.8, lambert - 0.8) * vec3<f32>(0.3, 0.3, 0.32) + vec3<f32>(spec, spec, spec) * vec3<f32>(0.3, 0.3, 0.33);
        lens = max(lens, fog * 0.55) + haze * 0.08 * dens;
        col = mix(col, lens, dmask);
    }
    col = clamp(col, vec3<f32>(0.0, 0.0, 0.0), vec3<f32>(1.0, 1.0, 1.0));
    if u.encode > 0.5 {
        col = to_linear(col);
    }
    return vec4<f32>(col, 1.0);
}
