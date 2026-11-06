// Beam specs, the height-unit mask shared with the light-field shader, and the deterministic hashes.

pub const MAX_BEAMS: usize = 6;
pub const DOWN: f32 = std::f32::consts::FRAC_PI_2;
const AUDIO_GAIN: f32 = 0.15;

#[derive(Clone, Copy)]
pub struct Beam {
    pub x: f32,
    pub y: f32,
    pub angle: f32,
    pub spread: f32,
    pub width: f32,
    pub length: f32,
    pub softness: f32,
    pub intensity: f32,
    pub streaks: f32,
    pub streak_freq: f32,
    pub streak_speed: f32,
    pub core: f32,
    pub sway: [f32; 3],
    pub pulse: [f32; 3],
    pub tint: [f32; 3],
    pub reveal: f32,
    pub gobo: u32,
    pub gobo_freq: f32,
    pub gobo_duty: f32,
    pub gobo_drift: f32,
    pub spectrum: f32,
    pub reach: f32,
}

#[derive(Clone, Copy)]
pub struct Resolved {
    pub ox: f32,
    pub oy: f32,
    pub dx: f32,
    pub dy: f32,
    pub half_width: f32,
    pub tan_spread: f32,
    pub softness: f32,
    pub length: f32,
    pub intensity: f32,
    pub streaks: f32,
    pub streak_freq: f32,
    pub streak_phase: f32,
    pub core: f32,
    pub rgb: [f32; 3],
    pub gobo: u32,
    pub gobo_freq: f32,
    pub gobo_duty: f32,
    pub gobo_phase: f32,
    pub spectrum: f32,
    pub reach: f32,
}

impl Resolved {
    pub const ZERO: Resolved = Resolved {
        ox: 0.0,
        oy: 0.0,
        dx: 1.0,
        dy: 0.0,
        half_width: 0.0,
        tan_spread: 0.0,
        softness: 0.0,
        length: 1.0,
        intensity: 0.0,
        streaks: 0.0,
        streak_freq: 0.0,
        streak_phase: 0.0,
        core: 0.0,
        rgb: [0.0, 0.0, 0.0],
        gobo: 0,
        gobo_freq: 0.0,
        gobo_duty: 1.0,
        gobo_phase: 0.0,
        spectrum: 0.0,
        reach: 0.0,
    };
}

#[derive(Clone, Copy)]
pub struct Drive {
    pub intensity: f32,
    pub bass: f32,
    pub color: [f32; 3],
    pub local: f32,
}

pub const fn shaft() -> Beam {
    Beam {
        x: 0.5,
        y: -0.04,
        angle: DOWN,
        spread: 0.09,
        width: 0.05,
        length: 0.8,
        softness: 0.6,
        intensity: 0.95,
        streaks: 0.6,
        streak_freq: 7.0,
        streak_speed: 0.08,
        core: 0.75,
        sway: [0.012, 13.0, 0.0],
        pulse: [0.0, 0.0, 0.0],
        tint: [1.0, 1.0, 1.0],
        reveal: 0.0,
        gobo: 0,
        gobo_freq: 0.0,
        gobo_duty: 1.0,
        gobo_drift: 0.0,
        spectrum: 0.0,
        reach: 0.0,
    }
}

pub const fn fan() -> Beam {
    Beam {
        x: 0.5,
        y: -0.04,
        angle: DOWN,
        spread: 0.3,
        width: 0.0,
        length: 0.6,
        softness: 0.9,
        intensity: 0.32,
        streaks: 0.85,
        streak_freq: 26.0,
        streak_speed: 0.05,
        core: 0.4,
        sway: [0.02, 17.0, 0.3],
        pulse: [0.0, 0.0, 0.0],
        tint: [1.0, 1.0, 1.0],
        reveal: 0.0,
        gobo: 0,
        gobo_freq: 0.0,
        gobo_duty: 1.0,
        gobo_drift: 0.0,
        spectrum: 0.0,
        reach: 0.0,
    }
}

pub fn resolve(beam: Beam, time: f32, aspect: f32, drive: Drive) -> Resolved {
    let angle = beam.angle + oscillate(beam.sway, time);
    let pulse = 1.0 + oscillate(beam.pulse, time);
    let (sin, cos) = angle.sin_cos();
    let reveal = if beam.reveal > 0.0 { 0.06 + 0.94 * smooth(0.0, 1.0, drive.local / beam.reveal) } else { 1.0 };
    Resolved {
        ox: beam.x * aspect,
        oy: beam.y,
        dx: cos,
        dy: sin,
        half_width: beam.width * 0.5,
        tan_spread: beam.spread.tan(),
        softness: beam.softness,
        length: beam.length * reveal,
        intensity: beam.intensity * pulse * drive.intensity * audio_lift(drive.bass),
        streaks: beam.streaks,
        streak_freq: beam.streak_freq,
        streak_phase: time * beam.streak_speed,
        core: beam.core,
        rgb: [beam.tint[0] * drive.color[0], beam.tint[1] * drive.color[1], beam.tint[2] * drive.color[2]],
        gobo: beam.gobo,
        gobo_freq: beam.gobo_freq,
        gobo_duty: beam.gobo_duty,
        gobo_phase: beam.gobo_drift * time,
        spectrum: beam.spectrum,
        reach: beam.reach,
    }
}

pub fn audio_lift(bass: f32) -> f32 {
    1.0 + AUDIO_GAIN * bass.max(0.0).powf(1.5)
}

pub fn light_at(beams: &[Resolved], x: f32, y: f32) -> f32 {
    beams.iter().fold(0.0, |sum, beam| sum + beam_sample(beam, x, y).0)
}

pub fn compress(value: f32) -> f32 {
    1.0 - (-value).exp()
}

/// Intensity and the signed cross-section coordinate, matching the shader's `beam_mask`.
pub fn beam_sample(beam: &Resolved, x: f32, y: f32) -> (f32, f32) {
    let px = x - beam.ox;
    let py = y - beam.oy;
    let along = px * beam.dx + py * beam.dy;
    if along <= 0.0 {
        return (0.0, 0.0);
    }
    let signed = px * -beam.dy + py * beam.dx;
    let half = (beam.half_width + along * beam.tan_spread).abs() + 1e-4;
    let s = signed.abs() / half;
    let edge = smooth(1.0, 1.0 - beam.softness.max(0.02), s);
    if edge <= 0.0 {
        return (0.0, 0.0);
    }
    let signed_s = s * signed.signum();
    let streak = 1.0 - beam.streaks + beam.streaks * value_noise1(signed_s * beam.streak_freq + beam.streak_phase);
    let core = 1.0 - beam.core + beam.core * (1.0 - s * s);
    let falloff = (-along / beam.length.max(0.001)).exp();
    let gobo = if beam.gobo > 0 { gobo_mask(beam, signed_s, along) } else { 1.0 };
    let reach =
        if beam.reach > 0.0 { 1.0 - smooth(beam.reach - 0.04_f32.max(half * 2.5), beam.reach, along) } else { 1.0 };
    let start = smooth(0.0, beam.half_width.abs() * 2.0 + 0.01, along);
    (edge * streak * core * falloff * gobo * reach * start * beam.intensity, signed_s)
}

fn gobo_mask(beam: &Resolved, u: f32, v: f32) -> f32 {
    0.12 + 0.88 * gobo_pattern(beam.gobo, beam.gobo_freq, beam.gobo_duty, beam.gobo_phase, u, v)
}

fn gobo_pattern(pattern: u32, frequency: f32, duty: f32, phase: f32, u: f32, v: f32) -> f32 {
    if pattern == 1 {
        return band(u * frequency * 0.5 + phase, duty).min(1.0);
    }
    if pattern == 2 {
        return smooth(duty * 0.5, duty * 0.5 + 0.08, u.abs());
    }
    if pattern == 3 {
        let a = band(u * frequency * 0.5 + v * frequency * 0.8 + phase, duty).min(1.0);
        let b = band(u * frequency * 0.5 - v * frequency * 0.8 - phase, duty).min(1.0);
        return a * b;
    }
    if pattern == 4 {
        return smooth(duty - 0.12, duty + 0.12, value_noise2(u * frequency, v * frequency * 0.6 + phase));
    }
    1.0
}

fn band(x: f32, duty: f32) -> f32 {
    let f = fract(x);
    1.0 - smooth(duty - 0.06, duty + 0.06, f) + smooth(0.94, 1.0, f)
}

fn oscillate(osc: [f32; 3], time: f32) -> f32 {
    if osc[1] <= 0.0 {
        return 0.0;
    }
    osc[0] * ((time / osc[1].max(0.001) + osc[2]) * std::f32::consts::TAU).sin()
}

pub fn fract(value: f32) -> f32 {
    value - value.floor()
}

pub fn hash11(input: f32) -> f32 {
    let mut p = fract(input * 0.1031);
    p *= p + 33.33;
    p *= p + p;
    fract(p)
}

pub fn value_noise1(x: f32) -> f32 {
    let i = x.floor();
    let f = x - i;
    let u = f * f * (3.0 - 2.0 * f);
    hash11(i) * (1.0 - u) + hash11(i + 1.0) * u
}

pub fn hash12(x: f32, y: f32) -> f32 {
    let mut a = fract(x * 0.1031);
    let mut b = fract(y * 0.1031);
    let mut c = fract(x * 0.1031);
    let d = a * (b + 33.33) + b * (c + 33.33) + c * (a + 33.33);
    a += d;
    b += d;
    c += d;
    fract((a + b) * c)
}

pub fn value_noise2(x: f32, y: f32) -> f32 {
    let ix = x.floor();
    let iy = y.floor();
    let fx = x - ix;
    let fy = y - iy;
    let ux = fx * fx * (3.0 - 2.0 * fx);
    let uy = fy * fy * (3.0 - 2.0 * fy);
    let a = hash12(ix, iy);
    let b = hash12(ix + 1.0, iy);
    let c = hash12(ix, iy + 1.0);
    let d = hash12(ix + 1.0, iy + 1.0);
    (a * (1.0 - ux) + b * ux) * (1.0 - uy) + (c * (1.0 - ux) + d * ux) * uy
}

pub fn smooth(edge0: f32, edge1: f32, x: f32) -> f32 {
    let span = edge1 - edge0;
    if span.abs() < 1e-6 {
        return if x >= edge1 { 1.0 } else { 0.0 };
    }
    let t = ((x - edge0) / span).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}
