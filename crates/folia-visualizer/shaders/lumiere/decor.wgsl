// Lines, stars, motes and bursts drawn over the lumiere light field. Positions are a function of time.

struct Light {
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

struct Decor {
    mote_drift: vec4<f32>,
    mote_look: vec4<f32>,
    front_drift: vec4<f32>,
    front_look: vec4<f32>,
    star: vec4<f32>,
    tint: vec4<f32>,
    bursts: array<vec4<f32>, 14>,
    colors: array<vec4<f32>, 14>,
}

@group(0) @binding(0) var<uniform> u: Light;
@group(1) @binding(1) var<uniform> d: Decor;

struct LineIn {
    @location(0) xy: vec2<f32>,
    @location(1) dir: vec2<f32>,
    @location(2) along_side: vec2<f32>,
    @location(3) width_alpha: vec2<f32>,
    @location(4) delay_span: vec2<f32>,
    @location(5) total_mid_x: vec2<f32>,
    @location(6) mid_y_gain: vec2<f32>,
    @location(7) begin_end: vec2<f32>,
}

struct VertOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) color: vec4<f32>,
    @location(1) local: vec2<f32>,
}

fn screen_of(p: vec2<f32>) -> vec2<f32> {
    let px = p * max(u.frame.y, 1.0);
    let pivot = u.frame.xy * 0.5;
    let rel = px - pivot;
    let s = max(u.camera.z, 1e-3);
    let co = cos(u.camera.w);
    let si = sin(u.camera.w);
    return u.camera.xy + vec2<f32>(co * rel.x - si * rel.y, si * rel.x + co * rel.y) * s;
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

fn beam_amount(p: vec2<f32>) -> f32 {
    var sum = 0.0;
    let count = i32(u.frame.w + 0.5);
    for (var i = 0; i < 6; i = i + 1) {
        if (i >= count) { break; }
        sum = sum + beam_mask(u.beam_a[i], u.beam_b[i], u.beam_c[i], u.beam_d[i].w, u.beam_e[i], u.beam_f[i].y, p).x;
    }
    return 1.0 - exp(-sum);
}

@vertex
fn vs_line(in: LineIn) -> VertOut {
    let height = max(u.frame.y, 1.0);
    let time = u.frame.z;
    let begin = in.begin_end.x;
    let end = in.begin_end.y;
    let draw = (time - begin) / 3.6;
    let local = clamp((draw - in.delay_span.x) / max(in.delay_span.y, 1e-3), 0.0, 1.0);
    let eased = 1.0 - pow(1.0 - local, 3.0);
    let amount = eased * in.total_mid_x.x;
    let along = in.along_side.x;
    let visible = select(0.0, 1.0, along <= amount + 1e-4);
    let out_k = select(1.0 - smoothstep(0.0, 1.0, (time - end) / 0.8), 1.0, end > 1e6);
    let fade = smoothstep(0.0, 1.0, (time - begin) / 1.2) * out_k * in.mid_y_gain.y;
    let lit = beam_amount(vec2<f32>(in.total_mid_x.y, in.mid_y_gain.x));
    let alpha = in.width_alpha.y * fade * (0.55 + 0.9 * lit) * visible;
    let a = screen_of(in.xy);
    let b = screen_of(in.xy + in.dir);
    let tangent = b - a;
    let len = max(length(tangent), 1e-4);
    let normal = vec2<f32>(-tangent.y, tangent.x) / len;
    let px = a + normal * in.along_side.y * in.width_alpha.x * height * max(u.camera.z, 1e-3) * 0.5;
    let ndc = vec2<f32>(px.x / max(u.frame.x, 1.0) * 2.0 - 1.0, 1.0 - px.y / max(u.frame.y, 1.0) * 2.0);
    var out: VertOut;
    out.pos = vec4<f32>(ndc, 0.0, 1.0);
    out.color = vec4<f32>(u.fog_color.rgb, clamp(alpha, 0.0, 1.0));
    out.local = vec2<f32>(0.0, 0.0);
    return out;
}

@fragment
fn fs_line(in: VertOut) -> @location(0) vec4<f32> {
    return in.color;
}

struct ParticleIn {
    @location(0) a: vec4<f32>,
    @location(1) b: vec4<f32>,
    @location(2) c: vec4<f32>,
}

fn corner(index: u32) -> vec2<f32> {
    let p = array<vec2<f32>, 6>(
        vec2<f32>(-1.0, -1.0), vec2<f32>(1.0, -1.0), vec2<f32>(1.0, 1.0),
        vec2<f32>(-1.0, -1.0), vec2<f32>(1.0, 1.0), vec2<f32>(-1.0, 1.0),
    );
    return p[index];
}

fn wrap_to(value: f32, low: f32, high: f32) -> f32 {
    let span = high - low;
    return low + ((value - low) % span + span) % span;
}

@vertex
fn vs_particle(@builtin(vertex_index) index: u32, in: ParticleIn) -> VertOut {
    let time = u.frame.z;
    let height = max(u.frame.y, 1.0);
    let mode = i32(in.c.y + 0.5);
    let local = d.star.w;
    var p = vec2<f32>(0.0);
    var size = in.c.x;
    var alpha = 0.0;
    var trail = false;
    let quad = corner(index);
    if (mode == 0 || mode == 5) {
        let progress = (local - in.a.w) / max(in.b.x, 1e-3);
        let shown = progress > 0.0;
        let eased = 1.0 - pow(1.0 - clamp(progress, 0.0, 1.0), 3.0);
        let y = in.a.z + (in.a.y - in.a.z) * eased;
        p = vec2<f32>(in.a.x, y);
        let speed = select(0.0, (in.a.y - in.a.z) * 3.0 * pow(1.0 - clamp(progress, 0.0, 1.0), 2.0) / max(in.b.x, 1e-3), progress < 1.0);
        let landed = in.a.w + in.b.x;
        let flash = select(0.0, exp(-(local - landed) / 0.25), local >= landed);
        let twinkle = 0.55 + 0.45 * sin(time * in.b.z * 6.2831853 + in.b.y);
        let base = select((select(0.45, 0.7, in.b.w > 0.5)) * twinkle, 0.9, progress < 1.0);
        let lit = beam_amount(p);
        alpha = clamp((base + lit * 0.8 + flash) * d.star.y, 0.0, 1.0) * select(0.0, 1.0, shown);
        size = in.c.x * (1.0 + flash * 1.5);
        trail = mode == 5 && speed > 0.05;
        if (mode == 5 && !trail) { alpha = 0.0; }
        if (trail) {
            let length = min(0.25, speed * 0.09);
            let along = (quad.x * 0.5 + 0.5) * length;
            p.y = y - along;
            size = max(1.5 / height, in.c.x * 0.7);
            alpha = alpha * 0.6;
        }
    } else if (mode == 1 || mode == 6) {
        let cycle = fract(time / max(in.a.y, 1e-3) + in.a.z);
        let y = -0.1 + cycle * 1.25;
        p = vec2<f32>(in.a.x + sin(time * 0.7 + in.a.z * 6.0) * in.a.w, y);
        let lit = beam_amount(p);
        let edge = clamp(cycle / 0.08, 0.0, 1.0) * clamp((1.0 - cycle) / 0.15, 0.0, 1.0);
        let rain_in = clamp((local - d.star.x * 0.6) / 1.5, 0.0, 1.0);
        alpha = clamp((0.18 + lit * 1.1) * edge * rain_in * d.star.y, 0.0, 1.0);
        size = in.b.x;
        let drop_speed = 1.25 / max(in.a.y, 1e-3);
        trail = mode == 6 && drop_speed > 0.05;
        if (mode == 6 && !trail) { alpha = 0.0; }
        if (trail) {
            let length = min(0.25, (1.25 / max(in.a.y, 1e-3)) * 0.09);
            p.y = y - (quad.x * 0.5 + 0.5) * length;
            size = max(1.5 / height, in.b.x * 0.7);
            alpha = alpha * 0.6;
        }
    } else if (mode == 2 || mode == 4) {
        let spec_d = select(d.mote_drift, d.front_drift, mode == 4);
        let spec_l = select(d.mote_look, d.front_look, mode == 4);
        let depth = in.a.z;
        let speed = 0.45 + depth * 0.9;
        let swirl = spec_d.z * (0.5 + depth);
        let omega = 6.2831853 / max(spec_d.w, 0.1);
        let aspect = max(u.frame.x, 1.0) / height;
        let margin = 0.08;
        let x = wrap_to(in.a.x + spec_d.x * speed * time + swirl * sin(time * omega * in.b.y + in.a.w), -margin, aspect + margin);
        let y = wrap_to(in.a.y + spec_d.y * speed * time + swirl * cos(time * omega * in.b.z + in.b.x), -margin, 1.0 + margin);
        p = vec2<f32>(x, y);
        let lit = beam_amount(p) * spec_l.z;
        let tw = 1.0 - spec_l.x + spec_l.x * (0.5 + 0.5 * sin(time * in.c.z * 6.2831853 + in.b.w));
        alpha = clamp((lit + spec_l.y) * tw, 0.0, 1.0) * spec_l.w;
        size = in.c.x;
    } else if (mode == 3) {
        p = in.a.xy;
        let begin = in.b.y;
        let end = in.b.z;
        let draw = (time - begin) / 3.6;
        let appear = clamp((draw - in.a.w) / 0.08, 0.0, 1.0);
        let out_k = select(1.0 - smoothstep(0.0, 1.0, (time - end) / 0.8), 1.0, end > 1e6);
        let fade = smoothstep(0.0, 1.0, (time - begin) / 1.2) * out_k * in.b.w;
        let lit = beam_amount(p);
        let tw = 0.55 + 0.45 * sin(time * 2.3 + in.b.x);
        alpha = appear * fade * (0.25 + 0.75 * lit) * tw;
        size = in.a.z * (1.0 + (1.0 - appear) * 1.5) * (0.8 + 0.2 * tw);
    } else {
        let slot = min(u32(in.c.x), 13u);
        let event = d.bursts[slot];
        let age = event.z;
        let length = event.w;
        let env = clamp(age / 0.03, 0.0, 1.0) * exp(-max(age - 0.03, 0.0) / 0.16);
        let spread = 1.0 + age * 2.0;
        let part = i32(in.c.z + 0.5);
        var center = vec2<f32>(event.x, event.y);
        var half = vec2<f32>(1.0);
        var part_alpha = 0.0;
        if (part == 0) {
            let vertical = length * (1.0 - pow(2.0, -10.0 * clamp(age / 0.08, 0.0, 1.0)));
            center.y = event.y - length * 0.2;
            half = vec2<f32>(length * 0.1 * spread * 0.5, vertical * 0.5);
            part_alpha = 0.6 * env;
        } else if (part == 1) {
            let vertical = length * (1.0 - pow(2.0, -10.0 * clamp(age / 0.08, 0.0, 1.0)));
            center.y = event.y - length * 0.2;
            half = vec2<f32>(length * 0.02 * spread * 0.5, vertical * 0.5);
            part_alpha = env;
        } else if (part == 2) {
            let horizontal = length * 0.62 * (1.0 - pow(2.0, -10.0 * clamp((age - 0.02) / 0.1, 0.0, 1.0)));
            center.y = event.y - length * 0.42;
            half = vec2<f32>(horizontal * 0.5, length * 0.09 * spread * 0.5);
            part_alpha = 0.55 * env * clamp((age - 0.02) / 0.03, 0.0, 1.0);
        } else if (part == 3) {
            let horizontal = length * 0.62 * (1.0 - pow(2.0, -10.0 * clamp((age - 0.02) / 0.1, 0.0, 1.0)));
            center.y = event.y - length * 0.42;
            half = vec2<f32>(horizontal * 0.5, length * 0.018 * spread * 0.5);
            part_alpha = 0.95 * env * clamp((age - 0.02) / 0.03, 0.0, 1.0);
        } else if (part == 4) {
            center.y = event.y - length * 0.42;
            let flash = length * (0.35 + age * 0.8);
            half = vec2<f32>(flash * 0.5);
            part_alpha = exp(-age / 0.07);
        } else {
            let ring = 1.0 - pow(1.0 - clamp(age / 0.4, 0.0, 1.0), 3.0);
            center.y = event.y - length * 0.42;
            let ring_size = length * 0.9 * ring;
            half = vec2<f32>(ring_size * 0.5);
            part_alpha = 0.35 * (1.0 - ring) * clamp(age / 0.03, 0.0, 1.0);
        }
        let px = center + quad * half;
        let screen = screen_of(px / height);
        let ndc = vec2<f32>(screen.x / max(u.frame.x, 1.0) * 2.0 - 1.0, 1.0 - screen.y / max(u.frame.y, 1.0) * 2.0);
        var out: VertOut;
        out.pos = vec4<f32>(ndc, 0.0, 1.0);
        let glow = d.colors[slot].rgb;
        let core = part == 1 || part == 3 || part == 4;
        out.color = vec4<f32>(mix(glow, vec3<f32>(1.0), vec3<f32>(select(0.0, 0.75, core))), part_alpha * select(0.0, 1.0, age >= 0.0 && age <= 0.6));
        out.local = quad;
        return out;
    }
    let screen = screen_of(p);
    let ndc_center = vec2<f32>(screen.x / max(u.frame.x, 1.0) * 2.0 - 1.0, 1.0 - screen.y / max(u.frame.y, 1.0) * 2.0);
    let pixel = size * height * max(u.camera.z, 1e-3);
    let ndc = ndc_center + quad * vec2<f32>(pixel / max(u.frame.x, 1.0), pixel / height);
    var out: VertOut;
    out.pos = vec4<f32>(ndc, 0.0, 1.0);
    out.color = vec4<f32>(u.fog_color.rgb, alpha);
    out.local = quad;
    return out;
}

@fragment
fn fs_particle(in: VertOut) -> @location(0) vec4<f32> {
    let disc = 1.0 - smoothstep(0.55, 1.0, length(in.local));
    return vec4<f32>(in.color.rgb, in.color.a * disc);
}
