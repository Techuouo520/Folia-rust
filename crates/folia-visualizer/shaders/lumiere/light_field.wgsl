// Lumiere light field. Beams, fog, glare, caustics and interference in height units, y down.
// The beam formula matches `beams.rs`. Output is premultiplied: rgb is the light, alpha is its peak.

struct VsOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) px: vec2<f32>,
}

struct Uniforms {
    beam_a: array<vec4<f32>, 6>,
    beam_b: array<vec4<f32>, 6>,
    beam_c: array<vec4<f32>, 6>,
    beam_d: array<vec4<f32>, 6>,
    beam_e: array<vec4<f32>, 6>,
    beam_f: array<vec4<f32>, 6>,
    frame: vec4<f32>,
    fog_a: vec4<f32>,
    fog_b: vec4<f32>,
    fog_color: vec4<f32>,
    glare: vec4<f32>,
    glare_b: vec4<f32>,
    glare_color: vec4<f32>,
    dark: vec4<f32>,
    caustic: vec4<f32>,
    caustic_floor: vec4<f32>,
    caustic_b: vec4<f32>,
    wave: vec4<f32>,
    wave_b: vec4<f32>,
    shield: vec4<f32>,
    shield_b: vec4<f32>,
    camera: vec4<f32>,
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
    let uv = vec2<f32>(p.x * 0.5 + 0.5, 0.5 - p.y * 0.5);
    out.px = uv * u.frame.xy;
    return out;
}

fn hash11(x: f32) -> f32 {
    var p = fract(x * 0.1031);
    p = p * (p + 33.33);
    p = p * (p + p);
    return fract(p);
}

fn hash12(p: vec2<f32>) -> f32 {
    var p3 = fract(vec3<f32>(p.x, p.y, p.x) * 0.1031);
    let d = dot(p3, p3.yzx + vec3<f32>(33.33));
    p3 = p3 + vec3<f32>(d);
    return fract((p3.x + p3.y) * p3.z);
}

fn value_noise1(x: f32) -> f32 {
    let i = floor(x);
    let f = x - i;
    let u = f * f * (3.0 - 2.0 * f);
    return mix(hash11(i), hash11(i + 1.0), u);
}

fn value_noise2(p: vec2<f32>) -> f32 {
    let i = floor(p);
    let f = p - i;
    let t = f * f * (3.0 - 2.0 * f);
    let a = hash12(i);
    let b = hash12(i + vec2<f32>(1.0, 0.0));
    let c = hash12(i + vec2<f32>(0.0, 1.0));
    let d = hash12(i + vec2<f32>(1.0, 1.0));
    return mix(mix(a, b, t.x), mix(c, d, t.x), t.y);
}

fn fbm(p_in: vec2<f32>, octaves: f32) -> f32 {
    var p = p_in;
    var sum = 0.0;
    var amp = 0.5;
    var norm = 0.0;
    let rot = mat2x2<f32>(vec2<f32>(0.8, -0.6), vec2<f32>(0.6, 0.8));
    for (var i = 0; i < 6; i = i + 1) {
        if (f32(i) >= octaves) {
            break;
        }
        sum = sum + amp * value_noise2(p);
        norm = norm + amp;
        p = rot * p * 2.03 + vec2<f32>(17.1, 9.2);
        amp = amp * 0.5;
    }
    return sum / max(norm, 1e-4);
}

fn fog(p: vec2<f32>, t: f32) -> f32 {
    let drift = u.fog_b.xy * t;
    let q = (p - drift) * u.fog_a.z;
    let octaves = u.fog_b.w;
    let w = vec2<f32>(
        fbm(q + vec2<f32>(0.0, t * 0.07), 3.0),
        fbm(q + vec2<f32>(5.2, 1.3) - vec2<f32>(t * 0.05, 0.0), 3.0),
    );
    let n = fbm(q + u.fog_a.w * (w - vec2<f32>(0.5)) * 2.0, octaves);
    return smoothstep(0.28, 0.82, n);
}

fn band(x: f32, duty: f32) -> f32 {
    let f = fract(x);
    return min(1.0, 1.0 - smoothstep(duty - 0.06, duty + 0.06, f) + smoothstep(0.94, 1.0, f));
}

fn gobo_pattern(e: vec4<f32>, s: f32, v: f32) -> f32 {
    let pattern = e.x;
    if (pattern < 1.5) {
        return band(s * e.y * 0.5 + e.w, e.z);
    }
    if (pattern < 2.5) {
        return smoothstep(e.z * 0.5, e.z * 0.5 + 0.08, abs(s));
    }
    if (pattern < 3.5) {
        return band(s * e.y * 0.5 + v * e.y * 0.8 + e.w, e.z) * band(s * e.y * 0.5 - v * e.y * 0.8 - e.w, e.z);
    }
    return smoothstep(e.z - 0.12, e.z + 0.12, value_noise2(vec2<f32>(s * e.y, v * e.y * 0.6 + e.w)));
}

fn gobo_mask(e: vec4<f32>, s: f32, v: f32) -> f32 {
    if (e.x < 0.5) {
        return 1.0;
    }
    return 0.12 + 0.88 * gobo_pattern(e, s, v);
}

fn beam_mask(a: vec4<f32>, b: vec4<f32>, c: vec4<f32>, core_amount: f32, e: vec4<f32>, reach: f32, p: vec2<f32>) -> vec2<f32> {
    let d = p - a.xy;
    let along = dot(d, a.zw);
    if (along <= 0.0) {
        return vec2<f32>(0.0);
    }
    let signed_perp = d.x * -a.w + d.y * a.z;
    let half_ = abs(b.x + along * b.y) + 1e-4;
    let s = abs(signed_perp) / half_;
    let edge = smoothstep(1.0, 1.0 - max(b.z, 0.02), s);
    if (edge <= 0.0) {
        return vec2<f32>(0.0);
    }
    let signed_s = s * sign(signed_perp);
    let streak = 1.0 - c.y + c.y * value_noise1(signed_s * c.z + c.w);
    let core = 1.0 - core_amount + core_amount * (1.0 - s * s);
    let falloff = exp(-along / max(b.w, 0.001));
    let reach_mask = select(1.0, 1.0 - smoothstep(reach - max(0.04, half_ * 2.5), reach, along), reach > 0.0);
    let start = smoothstep(0.0, abs(b.x) * 2.0 + 0.01, along);
    let intensity = edge * streak * core * falloff * gobo_mask(e, signed_s, along) * reach_mask * start * c.x;
    return vec2<f32>(intensity, signed_s);
}

fn spectrum_color(s: f32) -> vec3<f32> {
    let h = clamp(0.5 + 0.5 * s, 0.0, 1.0) * 0.8;
    let k = clamp(abs(fract(vec3<f32>(h) + vec3<f32>(0.0, 2.0 / 3.0, 1.0 / 3.0)) * 6.0 - vec3<f32>(3.0)) - vec3<f32>(1.0), vec3<f32>(0.0), vec3<f32>(1.0));
    return mix(vec3<f32>(1.0), k, vec3<f32>(0.85));
}

fn caustic_field(p: vec2<f32>, t: f32) -> f32 {
    let q = p * u.caustic.x * 6.2831853 - vec2<f32>(250.0);
    var i = q;
    var c = 1.0;
    let inten = 0.005;
    for (var n = 0; n < 4; n = n + 1) {
        let tt = t * (1.0 - (3.5 / f32(n + 1)));
        i = q + vec2<f32>(cos(tt - i.x) + sin(tt + i.y), sin(tt - i.y) + cos(tt + i.x));
        c = c + 1.0 / length(vec2<f32>(q.x / (sin(i.x + tt) / inten), q.y / (cos(i.y + tt) / inten)));
    }
    c = c / 4.0;
    c = 1.17 - pow(c, 1.4);
    return clamp(pow(abs(c), 8.0), 0.0, 3.0);
}

fn wave_pattern(p: vec2<f32>, t: f32) -> f32 {
    let d = p - u.wave_b.xy;
    let radius = max(u.wave_b.z, 1e-3);
    let r = length(d) / radius;
    var mask = smoothstep(1.0, 0.55, r);
    let mode = u.wave.x;
    let f = u.wave.y;
    let phase = t * u.wave.z;
    var v = 0.0;
    if (mode < 1.5) {
        v = 0.5 + 0.5 * cos(r * r * f * 6.0 - phase);
    } else if (mode < 2.5) {
        let x = d.x / radius;
        let y = d.y / radius;
        let xc = x * (1.0 + 0.35 * y * y);
        v = pow(cos(xc * f * 3.0 - phase * 0.3), 2.0) * exp(-xc * xc * 2.5);
        mask = smoothstep(1.0, 0.25, length(vec2<f32>(x * 0.85, y * 1.5)));
    } else if (mode < 3.5) {
        let s1 = vec2<f32>(-u.wave_b.w * 0.5, 0.0);
        let d1 = length(d - s1);
        let d2 = length(d + s1);
        v = (0.5 + 0.5 * cos((d1 - d2) * f * 20.0)) * (0.5 + 0.5 * cos((d1 + d2) * f * 6.0 - phase));
    } else {
        let x = r * f * 4.0 + 1e-3;
        let sc = sin(x) / x;
        v = min(sc * sc * 4.0, 1.5);
    }
    let q = (p - u.shield.xy) / max(u.shield.zw, vec2<f32>(1e-3));
    let shield = 1.0 - u.shield_b.x * smoothstep(1.0, 0.35, length(q));
    return v * mask * shield * u.wave.w;
}

@fragment
fn fragment(in: VsOut) -> @location(0) vec4<f32> {
    let h = max(u.frame.y, 1.0);
    let t = u.frame.z;
    let pivot = u.frame.xy * 0.5;
    let cam = u.camera;
    let rel = in.px - cam.xy;
    let scale = max(cam.z, 1e-3);
    let co = cos(cam.w);
    let si = sin(cam.w);
    let stage = pivot + vec2<f32>(co * rel.x + si * rel.y, -si * rel.x + co * rel.y) / scale;
    let p = stage / h;
    let density = fog(p, t);
    let tyndall = u.fog_a.y + (1.0 - u.fog_a.y) * density * u.fog_a.x;
    let caustic_on = u.caustic.w > 0.5;
    let caustic = select(0.0, caustic_field(p, t * u.caustic.y + 23.0), caustic_on);
    let caustic_mix = select(0.0, u.caustic.z, caustic_on);
    let caustic_lift = mix(1.0, 0.25 + 1.5 * caustic, caustic_mix);
    var light = vec3<f32>(0.0);
    let count = i32(u.frame.w + 0.5);
    for (var i = 0; i < 6; i = i + 1) {
        if (i >= count) {
            break;
        }
        let color_core = u.beam_d[i];
        let sample = beam_mask(u.beam_a[i], u.beam_b[i], u.beam_c[i], color_core.w, u.beam_e[i], u.beam_f[i].y, p);
        var color = color_core.rgb;
        let spectrum = u.beam_f[i].x;
        if (spectrum > 0.0) {
            let peak = max(max(color.r, color.g), color.b);
            color = mix(color, spectrum_color(sample.y) * peak, vec3<f32>(spectrum));
        }
        light = light + color * sample.x;
    }
    light = light * tyndall * caustic_lift;
    if (caustic_on && u.caustic_b.x > 0.0) {
        let e = (p - u.caustic_floor.xy) / max(u.caustic_floor.zw, vec2<f32>(1e-3));
        light = light + u.fog_color.rgb * caustic * u.caustic_b.x * smoothstep(1.0, 0.6, length(e));
    }
    if (u.wave.x > 0.5) {
        light = light + u.fog_color.rgb * wave_pattern(p, t);
    }
    light = light + u.fog_color.rgb * density * u.fog_b.z;
    if (u.glare.w > 0.0) {
        let g = p - u.glare.xy;
        let r = max(u.glare.z, 1e-3);
        let dist = length(g);
        let glare = exp(-dist / r) * 0.55 + exp(-(dist * dist) / (r * r * 0.03));
        let streak = exp(-abs(g.y) / (r * 0.035)) * exp(-abs(g.x) / (r * 7.0)) * u.glare_b.x;
        light = light + u.glare_color.rgb * (glare + streak) * u.glare.w;
    }
    let peak = max(max(light.r, light.g), light.b);
    let mapped = 1.0 - exp(-peak);
    var c = select(vec3<f32>(0.0), light / peak * mapped, peak > 1e-5);
    c = mix(c, vec3<f32>(mapped), vec3<f32>(0.3 * mapped * mapped * mapped));
    c = c + vec3<f32>(hash12(in.pos.xy) - 0.5) / 255.0;
    c = clamp(c, vec3<f32>(0.0), vec3<f32>(1.0));
    let a = max(max(c.r, c.g), c.b);
    let rgb = c + u.dark.rgb * (1.0 - a);
    let alpha = a + u.dark.a * (1.0 - a);
    return vec4<f32>(rgb, alpha);
}
