// Light positions transcribed from the original lumiere rigs. Beams that depend on aspect or a seed are built in `special`.

use super::beams::{Beam, DOWN, MAX_BEAMS, shaft};
use super::profiles::{Art, Mood, Profile, Type, base};

const NO_GLARE: [f32; 5] = [0.0; 5];
const TAU: f32 = std::f32::consts::TAU;

macro_rules! over {
    ($start:expr, $($field:ident: $value:expr),* $(,)?) => {{
        let mut beam = $start;
        $(beam.$field = $value;)*
        beam
    }};
}

const fn same(left: &str, right: &str) -> bool {
    let left = left.as_bytes();
    let right = right.as_bytes();
    if left.len() != right.len() {
        return false;
    }
    let mut index = 0;
    while index < left.len() {
        if left[index] != right[index] {
            return false;
        }
        index += 1;
    }
    true
}

pub(crate) const fn make(kind: &'static str, family: &'static str, mood: Mood, beams: &'static [Beam]) -> Profile {
    let mut profile = base(kind, family, mood, beams);
    if same(family, "lattice") {
        profile.region_cx = 0.56;
        profile.region_cy = 0.58;
        profile.region_w = 0.66;
        profile.region_h = 0.5;
        profile.mote_drift = [0.008, -0.006];
    } else if same(family, "prism") {
        profile.starfall = true;
        profile.rain = 50;
    } else if same(family, "caustic") {
        profile.mote_swirl = 0.03;
        profile.mote_drift[1] = -0.004;
        profile.mote_period = 12.0;
        profile.region_cy = 0.46;
        profile.region_h = 0.46;
    } else if same(family, "optics") {
        profile.region_cy = 0.74;
        profile.region_h = 0.3;
        profile.hero = 0.075;
        profile.mote_count = 180;
        profile.fog_density = 0.9;
        profile.fog_tyndall = 0.2;
        profile.fog_ambient = 0.02;
        profile.glare = NO_GLARE;
    } else if same(family, "wave") {
        profile.starfall = true;
        profile.rain = 40;
    } else if same(family, "botany") {
        profile.mote_count = 220;
        profile.mote_drift[1] = -0.012;
        profile.starfall = true;
        profile.rain = 40;
        profile.art_gain = 1.7;
    } else if same(family, "astral") {
        profile.starfall = true;
        profile.star_count = 620;
        profile.star_brightness = 1.1;
        profile.rain = 20;
        profile.mote_count = 160;
        profile.art_gain = 2.0;
    } else if same(family, "stage") {
        profile.mote_count = 200;
        profile.fog_density = 1.25;
        profile.fog_tyndall = 0.08;
        profile.fog_scale = 2.0;
        profile.fog_drift_x = 0.01;
        profile.fog_drift_y = -0.015;
        profile.fog_warp = 1.1;
        profile.fog_ambient = 0.035;
        profile.glare = NO_GLARE;
        profile.art = Art::Sparks;
    } else if same(family, "motes") {
        profile.starfall = true;
        profile.rain = 30;
        profile.art = Art::Sparks;
        profile.glare = NO_GLARE;
    }
    profile
}

pub fn glare(profile: &Profile, aspect: f32, seed: u32) -> [f32; 5] {
    if profile.kind == "astral-eclipse" {
        let aspect = aspect.max(1e-3);
        let x = 0.5 + 0.13 * (-0.8f32).cos() / aspect;
        let y = 0.34 + 0.13 * (-0.8f32).sin();
        return [x, y, 0.05, 1.3, 0.9];
    }
    if profile.kind == "optics-raytrace" {
        let y = 0.1 + Rng(seed).next() * 0.55;
        return [0.05, y, 0.05, 0.9, 0.6];
    }
    profile.glare
}

pub fn special(kind: &str, aspect: f32, seed: u32) -> Option<([Beam; MAX_BEAMS], usize)> {
    let built = match kind {
        "prism-shard" => prism_shard(seed),
        "prism-cube" => prism_cube(aspect),
        "optics-convex" => optics_rows(aspect, &[0.3, 0.4, 0.5]),
        "optics-concave" => optics_concave(aspect),
        "optics-mirror" => optics_mirror(aspect),
        "optics-refract" => optics_refract(aspect),
        "optics-bench" => optics_bench(aspect),
        "optics-periscope" => optics_periscope(aspect),
        "optics-pinhole" => optics_pinhole(aspect),
        "optics-telescope" => optics_telescope(aspect),
        "optics-raytrace" => optics_raytrace(aspect, seed),
        "wave-polar" => wave_polar(aspect),
        "astral-eclipse" => radial(0.5, 0.34, 0.13, aspect, 0.35, 0.7),
        "astral-corona" => radial(0.5, 0.12, 0.1, aspect, 0.6, 0.9),
        _ => return None,
    };
    Some(built)
}

fn ray(aspect: f32, from: [f32; 2], to: [f32; 2], open: bool) -> Beam {
    let dx = (to[0] - from[0]) * aspect;
    let dy = to[1] - from[1];
    let mut beam = over!(shaft(), x: from[0], y: from[1], angle: dy.atan2(dx), spread: 0.004, width: 0.008, length: 3.0, softness: 0.6, intensity: 0.9, streaks: 0.1, streak_freq: 3.0, streak_speed: 0.05, core: 0.8, sway: [0.0, 0.0, 0.0]);
    if !open {
        beam.reach = (dx * dx + dy * dy).sqrt() + 0.02;
    }
    beam
}

fn optics_rows(aspect: f32, rows: &[f32]) -> ([Beam; MAX_BEAMS], usize) {
    let mut out = [shaft(); MAX_BEAMS];
    for (index, y) in rows.iter().enumerate() {
        out[index * 2] = ray(aspect, [-0.02, *y], [0.42, *y], false);
        out[index * 2 + 1] = ray(aspect, [0.42, *y], [0.7, 0.4], true);
    }
    (out, rows.len() * 2)
}

fn optics_concave(aspect: f32) -> ([Beam; MAX_BEAMS], usize) {
    let mut out = [shaft(); MAX_BEAMS];
    for (index, y) in [0.32_f32, 0.4, 0.48].into_iter().enumerate() {
        let mut incoming = ray(aspect, [-0.02, y], [0.42, y], false);
        incoming.intensity = 0.7;
        let mut outgoing = ray(aspect, [0.42, y], [0.58, y + (y - 0.4) * 1.6], true);
        outgoing.intensity = 0.7;
        outgoing.spread = 0.01;
        out[index * 2] = incoming;
        out[index * 2 + 1] = outgoing;
    }
    (out, 6)
}

fn optics_mirror(aspect: f32) -> ([Beam; MAX_BEAMS], usize) {
    let mut out = [shaft(); MAX_BEAMS];
    out[0] = over!(ray(aspect, [0.12, 0.08], [0.5, 0.6], false), width: 0.02, intensity: 1.0);
    out[1] = over!(ray(aspect, [0.5, 0.6], [0.88, 0.08], true), width: 0.02, intensity: 0.8);
    (out, 2)
}

fn optics_refract(aspect: f32) -> ([Beam; MAX_BEAMS], usize) {
    let mut out = [shaft(); MAX_BEAMS];
    out[0] = over!(ray(aspect, [0.18, 0.05], [0.46, 0.5], false), width: 0.02, intensity: 1.0);
    out[1] = over!(ray(aspect, [0.46, 0.5], [0.58, 1.05], true), width: 0.02, intensity: 0.8);
    out[2] = over!(ray(aspect, [0.46, 0.5], [0.74, 0.05], true), width: 0.01, intensity: 0.3);
    (out, 3)
}

fn optics_bench(aspect: f32) -> ([Beam; MAX_BEAMS], usize) {
    let mut out = [shaft(); MAX_BEAMS];
    out[0] = over!(ray(aspect, [-0.02, 0.4], [0.3, 0.4], false), width: 0.05, spread: 0.01, intensity: 0.8);
    out[1] = over!(ray(aspect, [0.3, 0.4], [0.5, 0.4], false), width: 0.05, intensity: 0.9, spread: -0.06);
    out[2] = over!(ray(aspect, [0.5, 0.4], [0.7, 0.4], false), width: 0.02, intensity: 0.9, spread: 0.06);
    out[3] = over!(ray(aspect, [0.7, 0.4], [1.02, 0.4], true), width: 0.05, intensity: 0.7);
    (out, 4)
}

fn optics_periscope(aspect: f32) -> ([Beam; MAX_BEAMS], usize) {
    let mut out = [shaft(); MAX_BEAMS];
    out[0] = over!(ray(aspect, [1.02, 0.14], [0.3, 0.14], false), width: 0.02);
    out[1] = over!(ray(aspect, [0.3, 0.14], [0.3, 0.84], false), width: 0.02);
    out[2] = over!(ray(aspect, [0.3, 0.84], [1.02, 0.84], true), width: 0.02, intensity: 0.8);
    (out, 3)
}

fn optics_pinhole(aspect: f32) -> ([Beam; MAX_BEAMS], usize) {
    let distance = 0.47 * aspect;
    let mut out = [shaft(); MAX_BEAMS];
    out[0] = over!(
        shaft(),
        x: -0.02,
        y: 0.4,
        angle: 0.0,
        spread: -(0.15 / distance).atan(),
        width: 0.3,
        length: 3.0,
        softness: 0.4,
        intensity: 0.8,
        streaks: 0.3,
        streak_freq: 6.0,
        streak_speed: 0.05,
        core: 0.3,
        reach: distance + 0.3,
        sway: [0.0, 0.0, 0.0]
    );
    (out, 1)
}

fn optics_telescope(aspect: f32) -> ([Beam; MAX_BEAMS], usize) {
    let mut out = [shaft(); MAX_BEAMS];
    for (index, y) in [0.3_f32, 0.5].into_iter().enumerate() {
        let focus_y = 0.4 + (y - 0.4) * 0.3;
        out[index * 3] = over!(ray(aspect, [-0.02, y], [0.28, y], false), width: 0.02);
        out[index * 3 + 1] = ray(aspect, [0.28, y], [0.62, 0.4], false);
        out[index * 3 + 2] = over!(ray(aspect, [0.72, focus_y], [1.02, focus_y], true), width: 0.02, intensity: 1.1);
    }
    (out, 6)
}

fn optics_raytrace(aspect: f32, seed: u32) -> ([Beam; MAX_BEAMS], usize) {
    let mut rng = Rng(seed);
    let mut points = [[0.0; 2]; 7];
    for (index, point) in points.iter_mut().enumerate() {
        *point = [0.05 + (index as f32 / 6.0) * 0.9, 0.1 + rng.next() * 0.55];
    }
    let mut out = [shaft(); MAX_BEAMS];
    for (index, slot) in out.iter_mut().enumerate() {
        *slot = over!(ray(aspect, points[index], points[index + 1], false), intensity: 0.8, width: 0.012);
    }
    (out, 6)
}

fn wave_polar(aspect: f32) -> ([Beam; MAX_BEAMS], usize) {
    let second = aspect * 0.5 + 0.12;
    let mut out = [shaft(); MAX_BEAMS];
    out[0] = over!(
        shaft(),
        x: -0.02,
        y: 0.36,
        angle: 0.0,
        spread: 0.02,
        width: 0.14,
        length: 3.0,
        intensity: 0.8,
        streaks: 0.2,
        core: 0.4,
        reach: second + 0.02 * aspect,
        sway: [0.0, 0.0, 0.0]
    );
    out[1] = over!(
        shaft(),
        x: second / aspect.max(1e-3),
        y: 0.36,
        angle: 0.0,
        spread: 0.02,
        width: 0.14,
        length: 3.0,
        intensity: 0.7,
        streaks: 0.2,
        core: 0.4,
        sway: [0.0, 0.0, 0.0],
        pulse: [0.9, 6.0, 0.0]
    );
    (out, 2)
}

fn prism_cube(aspect: f32) -> ([Beam; MAX_BEAMS], usize) {
    let mut out = [shaft(); MAX_BEAMS];
    out[0] = over!(shaft(), x: -0.02, y: 0.34, angle: 0.0, spread: 0.01, width: 0.03, length: 3.0, intensity: 0.9, streaks: 0.2, sway: [0.0, 0.0, 0.0], reach: 0.44 * aspect);
    out[1] = over!(shaft(), x: 0.42, y: 0.34, angle: 0.0, spread: 0.01, width: 0.03, length: 3.0, intensity: 0.7, streaks: 0.2, sway: [0.0, 0.0, 0.0], spectrum: 0.3);
    out[2] = over!(shaft(), x: 0.42, y: 0.34, angle: DOWN, spread: 0.01, width: 0.03, length: 3.0, intensity: 0.6, streaks: 0.2, sway: [0.0, 0.0, 0.0], spectrum: 0.3);
    (out, 3)
}

fn prism_shard(seed: u32) -> ([Beam; MAX_BEAMS], usize) {
    let mut rng = Rng(seed);
    let mut out = [shaft(); MAX_BEAMS];
    for (index, slot) in out.iter_mut().take(5).enumerate() {
        *slot = over!(
            shaft(),
            x: 0.15 + rng.next() * 0.7,
            y: 0.15 + rng.next() * 0.5,
            angle: rng.next() * TAU,
            spread: 0.12,
            width: 0.01,
            length: 0.5,
            softness: 0.5,
            intensity: 0.8,
            streaks: 0.3,
            streak_freq: 6.0,
            core: 0.3,
            spectrum: 1.0,
            reach: 0.5 + rng.next() * 0.3,
            sway: [0.1, 9.0 + index as f32, rng.next()]
        );
    }
    (out, 5)
}

fn radial(cx: f32, cy: f32, radius: f32, aspect: f32, length: f32, intensity: f32) -> ([Beam; MAX_BEAMS], usize) {
    let mut out = [shaft(); MAX_BEAMS];
    let aspect = aspect.max(1e-3);
    for (index, slot) in out.iter_mut().enumerate() {
        let angle = (index as f32 / 6.0) * TAU + 0.3;
        *slot = over!(
            shaft(),
            x: cx + angle.cos() * radius / aspect,
            y: cy + angle.sin() * radius,
            angle: angle,
            spread: 0.14,
            width: 0.04,
            length: length,
            softness: 0.8,
            intensity: intensity,
            streaks: 0.7,
            streak_freq: 12.0,
            core: 0.3,
            sway: [0.04, 9.0 + index as f32, index as f32 / 6.0]
        );
    }
    (out, 6)
}

struct Rng(u32);

impl Rng {
    fn next(&mut self) -> f32 {
        self.0 = self.0.wrapping_add(0x6d2b79f5);
        let state = self.0;
        let mut mixed = (state ^ (state >> 15)).wrapping_mul(state | 1);
        mixed = mixed.wrapping_add((mixed ^ (mixed >> 7)).wrapping_mul(mixed | 61)) ^ mixed;
        ((mixed ^ (mixed >> 14)) as f64 / 4_294_967_296.0) as f32
    }
}

const fn window() -> Beam {
    over!(shaft(), x: 0.1, y: 0.06, angle: 0.62, spread: 0.1, width: 0.34, length: 1.3, softness: 0.35, intensity: 1.0, streaks: 0.25, streak_freq: 4.0, core: 0.3, sway: [0.01, 17.0, 0.0])
}

const fn tinted(tint: [f32; 3]) -> Beam {
    over!(shaft(), spread: 0.012, width: 0.012, length: 1.4, intensity: 0.7, streaks: 0.2, streak_freq: 3.0, core: 0.7, sway: [0.0, 0.0, 0.0], tint: tint)
}

const fn soft_top() -> Beam {
    over!(shaft(), spread: 0.22, width: 0.2, length: 1.4, softness: 0.8, intensity: 0.8, streaks: 0.3, core: 0.3)
}

const ARCH_BEAMS: [Beam; 2] = [
    over!(window(), x: 0.14, y: -0.02, angle: 0.95, width: 0.14, spread: 0.06, length: 1.6, intensity: 1.3, streaks: 0.55, streak_freq: 8.0, core: 0.6),
    over!(window(), x: 0.14, y: -0.02, angle: 0.95, width: 0.3, spread: 0.2, length: 0.9, softness: 0.9, intensity: 0.3, streaks: 0.8, streak_freq: 22.0),
];
pub const LATTICE_ARCH: Profile = {
    let mut profile = make("lattice-arch", "lattice", Mood::Loud, &ARCH_BEAMS);
    profile.fog_density = 1.1;
    profile.glare = [0.14, 0.0, 0.18, 1.1, 0.6];
    profile.typography = Type::Crossed;
    profile.decay = 1.2;
    profile.decay_delay = 0.8;
    profile.starfall = true;
    profile.rain = 90;
    profile.rain_spread = 0.8;
    profile
};

const fn rose_at(k: f32) -> Beam {
    over!(shaft(), y: 0.08, angle: DOWN + (k - 0.5) * 0.2, spread: 0.035, width: 0.03, length: 1.0, intensity: 0.65 - (k - 0.5).abs() * 0.06, streaks: 0.4, streak_freq: 5.0, sway: [0.015, 13.0, k * 0.1])
}
const ROSE_BEAMS: [Beam; 6] = [rose_at(-2.0), rose_at(-1.0), rose_at(0.0), rose_at(1.0), rose_at(2.0), rose_at(3.0)];
pub const LATTICE_ROSE: Profile = {
    let mut profile = make("lattice-rose", "lattice", Mood::Loud, &ROSE_BEAMS);
    profile.glare = [0.5, 0.08, 0.16, 1.1, 0.5];
    profile.region_cx = 0.5;
    profile.region_cy = 0.6;
    profile.region_w = 0.72;
    profile.region_h = 0.46;
    profile.decay = 1.2;
    profile.decay_delay = 0.8;
    profile.starfall = true;
    profile.rain = 70;
    profile
};

const SLIT_BEAMS: [Beam; 1] = [
    over!(shaft(), x: 0.1, y: 0.42, angle: 0.06, spread: 0.02, width: 0.012, length: 2.4, intensity: 1.0, streaks: 0.3, streak_freq: 3.0, core: 0.8, sway: [0.01, 15.0, 0.0]),
];
pub const LATTICE_SLIT: Profile = {
    let mut profile = make("lattice-slit", "lattice", Mood::Quiet, &SLIT_BEAMS);
    profile.fog_density = 0.8;
    profile.fog_ambient = 0.015;
    profile.glare = [0.1, 0.42, 0.08, 0.8, 0.9];
    profile.region_cx = 0.55;
    profile.region_cy = 0.5;
    profile.region_w = 0.6;
    profile.region_h = 0.64;
    profile.typography = Type::Vertical;
    profile.decay = 0.6;
    profile.decay_delay = 1.6;
    profile
};

const SHOJI_BEAMS: [Beam; 1] = [
    over!(window(), x: 0.12, y: 0.1, angle: 0.4, width: 0.5, spread: 0.18, softness: 0.95, intensity: 0.7, streaks: 0.1, core: 0.2, gobo: 1, gobo_freq: 10.0, gobo_duty: 0.86, gobo_drift: 0.0),
];
pub const LATTICE_SHOJI: Profile = {
    let mut profile = make("lattice-shoji", "lattice", Mood::Quiet, &SHOJI_BEAMS);
    profile.fog_density = 0.7;
    profile.fog_tyndall = 0.25;
    profile.glare = NO_GLARE;
    profile.decay = 0.6;
    profile.decay_delay = 1.6;
    profile
};

const GRILLE_BEAMS: [Beam; 1] = [
    over!(window(), width: 0.4, spread: 0.12, intensity: 1.0, gobo: 3, gobo_freq: 9.0, gobo_duty: 0.62, gobo_drift: 0.04),
];
pub const LATTICE_GRILLE: Profile = {
    let mut profile = make("lattice-grille", "lattice", Mood::Neutral, &GRILLE_BEAMS);
    profile.fog_drift_x = 0.01;
    profile.glare = [0.1, 0.06, 0.14, 0.7, 0.45];
    profile.typography = Type::Crossed;
    profile
};

const SWEEP_BEAMS: [Beam; 1] = [
    over!(window(), angle: 0.75, width: 0.3, intensity: 1.0, sway: [0.32, 10.0, 0.0], gobo: 1, gobo_freq: 11.0, gobo_duty: 0.55, gobo_drift: 0.05),
];
pub const LATTICE_SWEEP: Profile = {
    let mut profile = make("lattice-sweep", "lattice", Mood::Neutral, &SWEEP_BEAMS);
    profile.fog_drift_x = 0.01;
    profile.glare = [0.1, 0.06, 0.14, 0.7, 0.45];
    profile.camera_push = 0.02;
    profile.camera_drift = [0.012, 0.0];
    profile
};

const KOMOREBI_BEAMS: [Beam; 2] = [
    over!(shaft(), x: 0.35, y: -0.05, angle: 1.35, spread: 0.2, width: 0.2, length: 1.3, softness: 0.7, intensity: 1.0, streaks: 0.2, core: 0.3, gobo: 4, gobo_freq: 7.0, gobo_duty: 0.52, gobo_drift: 0.12, sway: [0.03, 7.0, 0.0]),
    over!(shaft(), x: 0.72, y: -0.05, angle: 1.8, spread: 0.14, width: 0.12, length: 1.1, softness: 0.7, intensity: 0.7, streaks: 0.2, core: 0.3, gobo: 4, gobo_freq: 8.0, gobo_duty: 0.55, gobo_drift: 0.1, sway: [0.03, 8.0, 0.4]),
];
pub const LATTICE_KOMOREBI: Profile = {
    let mut profile = make("lattice-komorebi", "lattice", Mood::Quiet, &KOMOREBI_BEAMS);
    profile.fog_density = 0.9;
    profile.fog_drift_x = 0.01;
    profile.glare = NO_GLARE;
    profile.mote_count = 220;
    profile.mote_swirl = 0.03;
    profile.mote_drift = [0.01, 0.004];
    profile.region_cx = 0.5;
    profile.region_cy = 0.54;
    profile.region_w = 0.72;
    profile.region_h = 0.6;
    profile.typography = Type::Vertical;
    profile.decay = 0.8;
    profile.decay_delay = 1.2;
    profile
};

const fn nave_at(x: f32, phase: f32) -> Beam {
    over!(window(), x: x, y: -0.02, angle: 1.1, width: 0.06, spread: 0.05, length: 1.5, softness: 0.5, intensity: 0.75, streaks: 0.5, streak_freq: 6.0, core: 0.5, sway: [0.008, 19.0, phase])
}
const NAVE_BEAMS: [Beam; 4] = [nave_at(0.12, 0.0), nave_at(0.34, 0.2), nave_at(0.56, 0.4), nave_at(0.78, 0.6)];
pub const LATTICE_NAVE: Profile = {
    let mut profile = make("lattice-nave", "lattice", Mood::Loud, &NAVE_BEAMS);
    profile.fog_density = 1.2;
    profile.fog_drift_x = 0.006;
    profile.fog_drift_y = -0.03;
    profile.glare = [0.45, 0.0, 0.25, 0.6, 0.3];
    profile.typography = Type::Crossed;
    profile.decay = 1.2;
    profile.decay_delay = 0.8;
    profile.starfall = true;
    profile.rain = 80;
    profile.rain_spread = 1.2;
    profile
};

const SPLIT_BEAMS: [Beam; 2] = [
    over!(shaft(), x: -0.02, y: 0.36, angle: 0.08, spread: 0.01, width: 0.02, length: 3.0, intensity: 0.9, streaks: 0.2, sway: [0.0, 0.0, 0.0], reach: 0.78),
    over!(shaft(), x: 0.42, y: 0.4, angle: 0.3, spread: 0.2, width: 0.02, length: 1.1, softness: 0.4, intensity: 1.1, streaks: 0.35, streak_freq: 9.0, core: 0.2, spectrum: 1.0, sway: [0.03, 11.0, 0.0]),
];
pub const PRISM_SPLIT: Profile = {
    let mut profile = make("prism-split", "prism", Mood::Loud, &SPLIT_BEAMS);
    profile.fog_density = 0.9;
    profile.glare = [0.42, 0.4, 0.06, 1.0, 0.3];
    profile.region_cx = 0.62;
    profile.region_cy = 0.66;
    profile.region_w = 0.6;
    profile.region_h = 0.44;
    profile.burst = 0.3;
    profile.burst_mode = 1;
    profile.burst_every = 2;
    profile.burst_offset = 1;
    profile.burst_size = 3.4;
    profile.burst_tint = [0.8, 0.9, 1.0];
    profile.decay = 1.2;
    profile.decay_delay = 0.8;
    profile
};

const BAND_BEAMS: [Beam; 1] = [
    over!(shaft(), x: -0.05, y: 0.4, angle: 0.12, spread: 0.05, width: 0.16, length: 2.5, softness: 0.4, intensity: 0.9, streaks: 0.25, streak_freq: 6.0, core: 0.3, spectrum: 1.0, sway: [0.02, 14.0, 0.0]),
];
pub const PRISM_BAND: Profile = {
    let mut profile = make("prism-band", "prism", Mood::Neutral, &BAND_BEAMS);
    profile.fog_density = 0.8;
    profile.glare = NO_GLARE;
    profile.region_cy = 0.56;
    profile.region_h = 0.46;
    profile
};

const FRINGE_BEAMS: [Beam; 2] = [
    over!(shaft(), spread: 0.1, softness: 0.3, spectrum: 0.35, intensity: 0.9),
    over!(shaft(), spread: 0.3, length: 0.6, softness: 0.9, intensity: 0.25, streaks: 0.8, streak_freq: 24.0, core: 0.4, spectrum: 0.5),
];
pub const PRISM_FRINGE: Profile = {
    let mut profile = make("prism-fringe", "prism", Mood::Quiet, &FRINGE_BEAMS);
    profile.region_cy = 0.5;
    profile.region_h = 0.62;
    profile.typography = Type::Vertical;
    profile.decay = 0.7;
    profile.decay_delay = 1.4;
    profile
};

pub const PRISM_SHARD: Profile = {
    let mut profile = make("prism-shard", "prism", Mood::Loud, &[]);
    profile.fog_density = 0.9;
    profile.fog_drift_y = -0.03;
    profile.glare = NO_GLARE;
    profile.typography = Type::Crossed;
    profile.burst = 0.4;
    profile.burst_mode = 2;
    profile.burst_every = 3;
    profile.burst_size = 3.0;
    profile.burst_tint = [0.85, 0.9, 1.0];
    profile.decay = 1.3;
    profile.decay_delay = 0.7;
    profile
};

const fn line_at(x: f32, tint: [f32; 3], intensity: f32) -> Beam {
    over!(tinted(tint), x: x, y: -0.05, angle: DOWN, intensity: intensity)
}
const LINE_BEAMS: [Beam; 5] = [
    line_at(0.22, [1.0, 0.35, 0.3], 0.7),
    line_at(0.36, [1.0, 0.8, 0.3], 0.5),
    line_at(0.47, [0.5, 1.0, 0.5], 0.8),
    line_at(0.61, [0.4, 0.8, 1.0], 0.6),
    line_at(0.78, [0.6, 0.45, 1.0], 0.45),
];
pub const PRISM_LINES: Profile = {
    let mut profile = make("prism-lines", "prism", Mood::Quiet, &LINE_BEAMS);
    profile.fog_density = 0.7;
    profile.fog_ambient = 0.015;
    profile.glare = NO_GLARE;
    profile.region_cy = 0.48;
    profile.region_h = 0.6;
    profile.typography = Type::Vertical;
    profile.decay = 0.6;
    profile.decay_delay = 1.6;
    profile
};

const MERGE_BEAMS: [Beam; 4] = [
    over!(tinted([1.0, 0.35, 0.3]), x: -0.02, y: 0.18, angle: 0.47, reach: 0.72, intensity: 0.8),
    over!(tinted([0.45, 1.0, 0.45]), x: -0.02, y: 0.4, angle: 0.1, reach: 0.62, intensity: 0.8),
    over!(tinted([0.4, 0.6, 1.0]), x: -0.02, y: 0.66, angle: -0.29, reach: 0.68, intensity: 0.8),
    over!(shaft(), x: 0.36, y: 0.46, angle: 0.02, spread: 0.02, width: 0.02, length: 2.0, intensity: 1.1, streaks: 0.2, streak_freq: 3.0, core: 0.7, sway: [0.0, 0.0, 0.0]),
];
pub const PRISM_MERGE: Profile = {
    let mut profile = make("prism-merge", "prism", Mood::Neutral, &MERGE_BEAMS);
    profile.fog_density = 0.9;
    profile.glare = [0.36, 0.46, 0.06, 1.1, 0.5];
    profile.typography = Type::Crossed;
    profile
};

pub const PRISM_CUBE: Profile = {
    let mut profile = make("prism-cube", "prism", Mood::Neutral, &[]);
    profile.fog_density = 0.9;
    profile.glare = [0.42, 0.34, 0.05, 0.9, 0.4];
    profile.region_cx = 0.6;
    profile.region_cy = 0.62;
    profile.region_w = 0.6;
    profile.region_h = 0.44;
    profile
};

const fn wheel_at(k: f32) -> Beam {
    over!(shaft(), x: 0.5, y: 0.08, angle: DOWN + (k - 1.0) * 0.7, spread: 0.18, width: 0.01, length: 1.1, softness: 0.4, intensity: 0.8, streaks: 0.3, streak_freq: 8.0, core: 0.2, spectrum: 1.0, sway: [0.5, 16.0, k / 3.0])
}
const WHEEL_BEAMS: [Beam; 3] = [wheel_at(0.0), wheel_at(1.0), wheel_at(2.0)];
pub const PRISM_WHEEL: Profile = {
    let mut profile = make("prism-wheel", "prism", Mood::Loud, &WHEEL_BEAMS);
    profile.fog_density = 0.9;
    profile.glare = [0.5, 0.08, 0.1, 1.1, 0.4];
    profile.typography = Type::Crossed;
    profile.decay = 1.2;
    profile.decay_delay = 0.8;
    profile
};

const DROPLET_BEAMS: [Beam; 1] = [
    over!(shaft(), x: 0.5, y: 1.2, angle: -DOWN, spread: 0.55, width: 0.1, length: 1.1, softness: 0.25, intensity: 0.55, streaks: 0.1, core: 0.0, spectrum: 1.0, sway: [0.0, 0.0, 0.0]),
];
pub const PRISM_DROPLET: Profile = {
    let mut profile = make("prism-droplet", "prism", Mood::Quiet, &DROPLET_BEAMS);
    profile.fog_density = 0.7;
    profile.fog_drift_y = 0.02;
    profile.glare = NO_GLARE;
    profile.rain = 160;
    profile.rain_speed = 0.5;
    profile.rain_spread = 1.6;
    profile.star_brightness = 0.8;
    profile.region_cy = 0.46;
    profile.region_h = 0.6;
    profile.typography = Type::Vertical;
    profile.decay = 0.6;
    profile.decay_delay = 1.6;
    profile
};

const PRISM_SWEEP_BEAMS: [Beam; 1] = [
    over!(shaft(), x: 0.5, y: -0.1, angle: DOWN, spread: 0.08, width: 0.1, length: 1.4, softness: 0.4, intensity: 0.95, streaks: 0.2, core: 0.3, spectrum: 1.0, sway: [0.35, 8.0, 0.0]),
];
pub const PRISM_SWEEP: Profile = {
    let mut profile = make("prism-sweep", "prism", Mood::Neutral, &PRISM_SWEEP_BEAMS);
    profile.fog_density = 0.9;
    profile.glare = [0.5, -0.05, 0.12, 0.6, 0.45];
    profile
};

const POOL_BEAMS: [Beam; 1] = [soft_top()];
pub const CAUSTIC_POOL: Profile = {
    let mut profile = make("caustic-pool", "caustic", Mood::Quiet, &POOL_BEAMS);
    profile.fog_density = 0.8;
    profile.fog_drift_y = -0.015;
    profile.fog_ambient = 0.02;
    profile.glare = [0.5, 0.0, 0.12, 0.6, 0.45];
    profile.caustic_scale = 2.2;
    profile.caustic_speed = 0.35;
    profile.caustic_in_beam = 0.8;
    profile.caustic_floor = [0.5, 0.82, 0.42, 0.14, 0.7];
    profile.decay = 0.7;
    profile.decay_delay = 1.4;
    profile
};

const CUP_BEAMS: [Beam; 1] = [
    over!(shaft(), x: -0.05, y: 0.3, angle: 0.35, spread: 0.08, width: 0.18, length: 1.6, softness: 0.5, intensity: 0.9, streaks: 0.3, core: 0.4, sway: [0.0, 0.0, 0.0]),
];
pub const CAUSTIC_CUP: Profile = {
    let mut profile = make("caustic-cup", "caustic", Mood::Neutral, &CUP_BEAMS);
    profile.fog_density = 0.8;
    profile.glare = NO_GLARE;
    profile.caustic_scale = 3.2;
    profile.caustic_speed = 0.25;
    profile.caustic_in_beam = 0.3;
    profile.caustic_floor = [0.36, 0.8, 0.14, 0.07, 1.1];
    profile.region_cx = 0.62;
    profile.region_cy = 0.48;
    profile.region_w = 0.56;
    profile.region_h = 0.64;
    profile.typography = Type::Vertical;
    profile
};

const RIPPLE_BEAMS: [Beam; 1] = [over!(soft_top(), intensity: 0.75)];
pub const CAUSTIC_RIPPLE: Profile = {
    let mut profile = make("caustic-ripple", "caustic", Mood::Neutral, &RIPPLE_BEAMS);
    profile.fog_density = 0.8;
    profile.glare = [0.5, 0.0, 0.12, 0.5, 0.45];
    profile.caustic_scale = 3.0;
    profile.caustic_speed = 0.5;
    profile.caustic_in_beam = 0.6;
    profile.caustic_floor = [0.5, 0.8, 0.34, 0.1, 0.5];
    profile.typography = Type::Crossed;
    profile
};

const fn surface_at(x: f32, index: f32) -> Beam {
    over!(shaft(), x: x, y: 0.1, angle: 1.35 + index * 0.06, spread: 0.04, width: 0.05, length: 1.2, softness: 0.6, intensity: 0.7, streaks: 0.5, streak_freq: 6.0, core: 0.5, sway: [0.05, 6.0 + index, index * 0.2])
}
const SURFACE_BEAMS: [Beam; 4] =
    [surface_at(0.2, 0.0), surface_at(0.42, 1.0), surface_at(0.6, 2.0), surface_at(0.8, 3.0)];
pub const CAUSTIC_SURFACE: Profile = {
    let mut profile = make("caustic-surface", "caustic", Mood::Loud, &SURFACE_BEAMS);
    profile.fog_density = 1.1;
    profile.fog_drift_y = -0.02;
    profile.glare = NO_GLARE;
    profile.caustic_scale = 2.6;
    profile.caustic_speed = 0.6;
    profile.caustic_in_beam = 0.9;
    profile.caustic_floor = [0.5, 0.06, 0.6, 0.07, 0.9];
    profile.mote_count = 300;
    profile.mote_drift = [0.004, -0.02];
    profile.region_cy = 0.58;
    profile.region_h = 0.46;
    profile.decay = 1.2;
    profile.decay_delay = 0.8;
    profile
};

const RING_BEAMS: [Beam; 1] = [over!(soft_top(), spread: 0.14, width: 0.1, intensity: 0.7)];
pub const CAUSTIC_RING: Profile = {
    let mut profile = make("caustic-ring", "caustic", Mood::Quiet, &RING_BEAMS);
    profile.fog_density = 0.7;
    profile.glare = [0.5, 0.0, 0.12, 0.5, 0.45];
    profile.caustic_scale = 4.0;
    profile.caustic_speed = 0.2;
    profile.caustic_in_beam = 0.2;
    profile.caustic_floor = [0.5, 0.5, 0.2, 0.36, 0.6];
    profile.region_w = 0.4;
    profile.region_cy = 0.5;
    profile.region_h = 0.62;
    profile.typography = Type::Vertical;
    profile.decay = 0.6;
    profile.decay_delay = 1.6;
    profile
};

const BURN_BEAMS: [Beam; 2] = [
    over!(shaft(), x: 0.5, y: 0.2, angle: DOWN, spread: -0.22, width: 0.2, length: 1.4, softness: 0.5, intensity: 1.2, streaks: 0.3, streak_freq: 5.0, core: 0.6, sway: [0.02, 9.0, 0.0]),
    over!(shaft(), x: 0.5, y: -0.1, angle: DOWN, spread: 0.03, width: 0.22, length: 1.0, intensity: 0.4, streaks: 0.4, reach: 0.3, sway: [0.0, 0.0, 0.0]),
];
pub const CAUSTIC_BURN: Profile = {
    let mut profile = make("caustic-burn", "caustic", Mood::Loud, &BURN_BEAMS);
    profile.fog_density = 1.0;
    profile.glare = [0.5, 0.65, 0.05, 1.1, 0.8];
    profile.region_cy = 0.66;
    profile.region_h = 0.4;
    profile.typography = Type::Crossed;
    profile.burst = 0.35;
    profile.burst_mode = 1;
    profile.burst_every = 1;
    profile.burst_size = 3.6;
    profile.burst_tint = [1.0, 0.7, 0.45];
    profile.decay = 1.3;
    profile.decay_delay = 0.7;
    profile
};

const SHIMMER_BEAMS: [Beam; 1] = [
    over!(shaft(), x: -0.05, y: 0.5, angle: 0.02, spread: 0.06, width: 0.5, length: 2.4, softness: 0.4, intensity: 0.8, streaks: 0.2, core: 0.2, gobo: 1, gobo_freq: 18.0, gobo_duty: 0.35, gobo_drift: 0.3, sway: [0.0, 0.0, 0.0]),
];
pub const CAUSTIC_SHIMMER: Profile = {
    let mut profile = make("caustic-shimmer", "caustic", Mood::Neutral, &SHIMMER_BEAMS);
    profile.fog_density = 0.8;
    profile.glare = NO_GLARE;
    profile.caustic_scale = 1.4;
    profile.caustic_speed = 0.8;
    profile.caustic_in_beam = 0.6;
    profile.region_cy = 0.5;
    profile.region_h = 0.5;
    profile
};

const fn crystal_at(k: f32) -> Beam {
    over!(shaft(), x: 0.5, y: 0.26, angle: DOWN + (k - 2.0) * 0.45, spread: 0.05, width: 0.01, length: 0.8, softness: 0.5, intensity: 0.6, streaks: 0.3, core: 0.4, spectrum: 0.7, sway: [0.05, 12.0, k * 0.15])
}
const CRYSTAL_BEAMS: [Beam; 5] = [crystal_at(0.0), crystal_at(1.0), crystal_at(2.0), crystal_at(3.0), crystal_at(4.0)];
pub const CAUSTIC_CRYSTAL: Profile = {
    let mut profile = make("caustic-crystal", "caustic", Mood::Neutral, &CRYSTAL_BEAMS);
    profile.fog_density = 0.9;
    profile.glare = [0.5, 0.26, 0.07, 1.0, 0.5];
    profile.caustic_scale = 3.4;
    profile.caustic_speed = 0.2;
    profile.caustic_in_beam = 0.4;
    profile.caustic_floor = [0.5, 0.8, 0.3, 0.09, 0.5];
    profile.region_cy = 0.58;
    profile.region_h = 0.44;
    profile.typography = Type::Crossed;
    profile
};

const DRIFT_BEAMS: [Beam; 1] = [over!(soft_top(), spread: 0.4, width: 0.4, intensity: 1.0, softness: 0.95)];
pub const CAUSTIC_DRIFT: Profile = {
    let mut profile = make("caustic-drift", "caustic", Mood::Quiet, &DRIFT_BEAMS);
    profile.fog_density = 0.8;
    profile.fog_drift_x = 0.004;
    profile.fog_drift_y = 0.0;
    profile.fog_ambient = 0.03;
    profile.glare = [0.5, 0.0, 0.12, 0.5, 0.45];
    profile.caustic_scale = 1.0;
    profile.caustic_speed = 0.08;
    profile.caustic_in_beam = 0.8;
    profile.caustic_floor = [0.5, 0.5, 0.5, 0.4, 0.4];
    profile.region_cy = 0.5;
    profile.region_h = 0.62;
    profile.typography = Type::Vertical;
    profile.decay = 0.5;
    profile.decay_delay = 1.8;
    profile
};

const STORM_BEAMS: [Beam; 1] = [over!(soft_top(), intensity: 1.0, pulse: [0.35, 0.9, 0.0])];
pub const CAUSTIC_STORM: Profile = {
    let mut profile = make("caustic-storm", "caustic", Mood::Loud, &STORM_BEAMS);
    profile.fog_density = 1.1;
    profile.glare = [0.5, 0.0, 0.12, 0.8, 0.45];
    profile.caustic_scale = 3.0;
    profile.caustic_speed = 1.3;
    profile.caustic_in_beam = 1.0;
    profile.caustic_floor = [0.5, 0.82, 0.5, 0.14, 0.8];
    profile.decay = 1.3;
    profile.decay_delay = 0.7;
    profile
};

const fn optic(kind: &'static str, mood: Mood) -> Profile {
    make(kind, "optics", mood, &[])
}

pub const OPTICS_CONVEX: Profile = {
    let mut profile = optic("optics-convex", Mood::Neutral);
    profile.glare = [0.7, 0.4, 0.05, 0.9, 0.6];
    profile
};
pub const OPTICS_CONCAVE: Profile = {
    let mut profile = optic("optics-concave", Mood::Quiet);
    profile.region_cx = 0.72;
    profile.region_cy = 0.56;
    profile.region_w = 0.4;
    profile.region_h = 0.72;
    profile.typography = Type::Vertical;
    profile.decay = 0.7;
    profile.decay_delay = 1.4;
    profile
};
pub const OPTICS_MIRROR: Profile = {
    let mut profile = optic("optics-mirror", Mood::Quiet);
    profile.glare = [0.5, 0.6, 0.05, 0.8, 0.7];
    profile.region_cx = 0.5;
    profile.region_cy = 0.34;
    profile.region_w = 0.5;
    profile.region_h = 0.36;
    profile.hero = 0.07;
    profile.typography = Type::Crossed;
    profile.decay = 0.7;
    profile.decay_delay = 1.4;
    profile
};
pub const OPTICS_REFRACT: Profile = {
    let mut profile = optic("optics-refract", Mood::Neutral);
    profile.glare = [0.46, 0.5, 0.05, 0.8, 0.8];
    profile.region_cx = 0.72;
    profile.region_cy = 0.3;
    profile.region_w = 0.44;
    profile.region_h = 0.3;
    profile.hero = 0.07;
    profile
};
pub const OPTICS_BENCH: Profile = {
    let mut profile = optic("optics-bench", Mood::Neutral);
    profile.glare = [0.5, 0.4, 0.04, 0.7, 0.6];
    profile
};
pub const OPTICS_PERISCOPE: Profile = {
    let mut profile = optic("optics-periscope", Mood::Neutral);
    profile.glare = [0.3, 0.14, 0.04, 0.7, 0.4];
    profile.region_cx = 0.62;
    profile.region_cy = 0.49;
    profile.region_w = 0.4;
    profile.region_h = 0.5;
    profile.typography = Type::Vertical;
    profile.decay = 0.8;
    profile.decay_delay = 1.2;
    profile
};

const FIBER_BEAMS: [Beam; 2] = [
    over!(shaft(), x: 0.84, y: 0.2, angle: -0.5, spread: 0.3, width: 0.02, length: 0.9, softness: 0.6, intensity: 1.2, streaks: 0.5, streak_freq: 9.0, streak_speed: 0.1, core: 0.6, pulse: [0.2, 2.3, 0.0], sway: [0.0, 0.0, 0.0]),
    over!(shaft(), x: 0.84, y: 0.2, angle: -0.5, spread: 0.08, width: 0.02, length: 1.2, softness: 0.5, intensity: 0.9, streaks: 0.2, streak_freq: 3.0, streak_speed: 0.0, core: 0.8, sway: [0.0, 0.0, 0.0]),
];
pub const OPTICS_FIBER: Profile = {
    let mut profile = make("optics-fiber", "optics", Mood::Loud, &FIBER_BEAMS);
    profile.glare = [0.84, 0.2, 0.07, 1.2, 0.8];
    profile.region_cx = 0.46;
    profile.region_cy = 0.52;
    profile.region_w = 0.6;
    profile.region_h = 0.5;
    profile.typography = Type::Crossed;
    profile.burst = 0.3;
    profile.burst_mode = 1;
    profile.burst_every = 2;
    profile.burst_size = 3.2;
    profile.burst_tint = [0.8, 0.9, 1.0];
    profile.rain_speed = 0.3;
    profile.rain_spread = 0.8;
    profile.decay = 1.3;
    profile.decay_delay = 0.7;
    profile.starfall = true;
    profile.rain = 60;
    profile.star_count = 380;
    profile.star_opening = 3.0;
    profile
};
pub const OPTICS_PINHOLE: Profile = {
    let mut profile = optic("optics-pinhole", Mood::Quiet);
    profile.glare = [0.45, 0.4, 0.03, 1.0, 0.3];
    profile.region_cx = 0.2;
    profile.region_cy = 0.52;
    profile.region_w = 0.3;
    profile.region_h = 0.7;
    profile.hero = 0.07;
    profile.typography = Type::Vertical;
    profile.decay = 0.6;
    profile.decay_delay = 1.6;
    profile
};
pub const OPTICS_TELESCOPE: Profile = {
    let mut profile = optic("optics-telescope", Mood::Loud);
    profile.glare = [0.62, 0.4, 0.05, 1.0, 0.8];
    profile.burst = 0.25;
    profile.burst_mode = 1;
    profile.burst_every = 2;
    profile.burst_offset = 1;
    profile.burst_size = 3.2;
    profile.burst_tint = [1.0, 0.8, 0.55];
    profile.decay = 1.2;
    profile.decay_delay = 0.8;
    profile
};
pub const OPTICS_RAYTRACE: Profile = {
    let mut profile = optic("optics-raytrace", Mood::Loud);
    profile.typography = Type::Crossed;
    profile.decay = 1.3;
    profile.decay_delay = 0.7;
    profile
};
