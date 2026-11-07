// The remaining lumiere families: diffraction, botany, astral, the rest of stage and motes.

use super::beams::{Beam, DOWN, fan, shaft};
use super::catalog::make;
use super::profiles::{Mood, Profile, Type};

const NO_GLARE: [f32; 5] = [0.0; 5];
const SHIELD: f32 = 0.45;
const TAU: f32 = std::f32::consts::TAU;

macro_rules! over {
    ($start:expr, $($field:ident: $value:expr),* $(,)?) => {{
        let mut beam = $start;
        $(beam.$field = $value;)*
        beam
    }};
}

const fn wave_of(kind: &'static str, mood: Mood, beams: &'static [Beam]) -> Profile {
    let mut profile = make(kind, "wave", mood, beams);
    profile.wave_motion[3] = SHIELD;
    profile
}

const SLIT_BEAMS: [Beam; 1] = [over!(shaft(), spread: 0.05, width: 0.04, length: 0.6, intensity: 0.9)];
pub const WAVE_SLITS: Profile = {
    let mut profile = wave_of("wave-slits", Mood::Neutral, &SLIT_BEAMS);
    profile.fog_density = 0.9;
    profile.wave_mode = 2.0;
    profile.wave_center = [0.5, 0.72, 0.3, 0.0];
    profile.wave_motion = [5.0, 0.4, 0.55, SHIELD];
    profile.region_cy = 0.5;
    profile.region_h = 0.5;
    profile.typography = Type::Crossed;
    profile
};

const NEWTON_BEAMS: [Beam; 1] = [over!(shaft(), spread: 0.06, width: 0.04, length: 0.6, intensity: 0.6)];
pub const WAVE_NEWTON: Profile = {
    let mut profile = wave_of("wave-newton", Mood::Quiet, &NEWTON_BEAMS);
    profile.fog_density = 0.7;
    profile.glare = [0.5, 0.0, 0.12, 0.5, 0.45];
    profile.wave_mode = 1.0;
    profile.wave_center = [0.5, 0.5, 0.4, 0.0];
    profile.wave_motion = [5.0, 0.3, 0.5, 0.75];
    profile.region_w = 0.4;
    profile.region_cy = 0.5;
    profile.region_h = 0.62;
    profile.typography = Type::Vertical;
    profile.decay = 0.6;
    profile.decay_delay = 1.6;
    profile
};

const AIRY_BEAMS: [Beam; 1] =
    [over!(shaft(), x: 0.5, y: -0.05, spread: 0.02, width: 0.01, length: 0.6, intensity: 0.5, reach: 0.4)];
pub const WAVE_AIRY: Profile = {
    let mut profile = wave_of("wave-airy", Mood::Quiet, &AIRY_BEAMS);
    profile.fog_density = 0.6;
    profile.glare = [0.5, 0.4, 0.04, 1.0, 0.4];
    profile.wave_mode = 4.0;
    profile.wave_center = [0.5, 0.4, 0.3, 0.0];
    profile.wave_motion = [3.0, 0.0, 0.7, SHIELD];
    profile.region_cy = 0.72;
    profile.region_h = 0.36;
    profile.decay = 0.6;
    profile.decay_delay = 1.6;
    profile
};

const fn order_at(k: f32) -> Beam {
    over!(shaft(), y: 0.3, angle: DOWN + k * 0.32, spread: 0.02, width: 0.02, length: 1.2, intensity: 0.8 - k.abs() * 0.15, streaks: 0.2, core: 0.6, spectrum: if k == 0.0 { 0.0 } else { 0.8 }, sway: [0.0, 0.0, 0.0])
}
const GRATING_BEAMS: [Beam; 6] = [
    over!(shaft(), spread: 0.01, width: 0.03, length: 2.0, intensity: 0.9, reach: 0.32, sway: [0.0, 0.0, 0.0]),
    order_at(-2.0),
    order_at(-1.0),
    order_at(0.0),
    order_at(1.0),
    order_at(2.0),
];
pub const WAVE_GRATING: Profile = {
    let mut profile = wave_of("wave-grating", Mood::Neutral, &GRATING_BEAMS);
    profile.fog_density = 0.9;
    profile.glare = [0.5, 0.3, 0.05, 0.9, 0.6];
    profile.region_cy = 0.7;
    profile.region_h = 0.38;
    profile
};

const SOURCE_BEAMS: [Beam; 1] = [over!(shaft(), spread: 0.15, intensity: 0.4)];
pub const WAVE_SOURCES: Profile = {
    let mut profile = wave_of("wave-sources", Mood::Loud, &SOURCE_BEAMS);
    profile.fog_density = 0.8;
    profile.glare = [0.5, 0.0, 0.12, 0.5, 0.45];
    profile.wave_mode = 3.0;
    profile.wave_center = [0.5, 0.5, 0.46, 0.14];
    profile.wave_motion = [4.0, 1.2, 0.6, SHIELD];
    profile.typography = Type::Crossed;
    profile.burst = 0.3;
    profile.burst_mode = 1;
    profile.burst_every = 2;
    profile.burst_size = 3.2;
    profile.burst_tint = [0.85, 0.9, 1.0];
    profile.decay = 1.3;
    profile.decay_delay = 0.7;
    profile
};

const MOIRE_BEAMS: [Beam; 2] = [
    over!(shaft(), spread: 0.2, width: 0.3, length: 1.3, softness: 0.6, intensity: 0.6, streaks: 0.0, core: 0.2, gobo: 1, gobo_freq: 22.0, gobo_duty: 0.5, gobo_drift: 0.02, sway: [0.0, 0.0, 0.0]),
    over!(shaft(), angle: DOWN + 0.06, spread: 0.2, width: 0.3, length: 1.3, softness: 0.6, intensity: 0.6, streaks: 0.0, core: 0.2, gobo: 1, gobo_freq: 23.0, gobo_duty: 0.5, gobo_drift: -0.02, sway: [0.03, 14.0, 0.0]),
];
pub const WAVE_MOIRE: Profile = {
    let mut profile = wave_of("wave-moire", Mood::Neutral, &MOIRE_BEAMS);
    profile.fog_density = 0.8;
    profile.glare = [0.5, 0.0, 0.12, 0.5, 0.45];
    profile
};

const FILM_BEAMS: [Beam; 1] = [
    over!(shaft(), spread: 0.4, width: 0.3, length: 1.2, softness: 0.95, intensity: 0.45, streaks: 0.6, streak_freq: 3.0, streak_speed: 0.2, core: 0.2, spectrum: 1.0, sway: [0.15, 13.0, 0.0]),
];
pub const WAVE_FILM: Profile = {
    let mut profile = wave_of("wave-film", Mood::Quiet, &FILM_BEAMS);
    profile.fog_density = 0.7;
    profile.glare = NO_GLARE;
    profile.wave_mode = 1.0;
    profile.wave_center = [0.5, 0.4, 0.3, 0.0];
    profile.wave_motion = [1.6, 0.6, 0.25, SHIELD];
    profile.region_w = 0.4;
    profile.region_cy = 0.5;
    profile.region_h = 0.62;
    profile.typography = Type::Vertical;
    profile.decay = 0.6;
    profile.decay_delay = 1.6;
    profile
};

pub const WAVE_POLAR: Profile = {
    let mut profile = wave_of("wave-polar", Mood::Neutral, &[]);
    profile.fog_density = 0.8;
    profile.glare = NO_GLARE;
    profile.region_cy = 0.7;
    profile.region_h = 0.36;
    profile
};

const fn standing_at(y: f32, phase: f32) -> Beam {
    over!(shaft(), x: -0.02, y: y, angle: 0.0, spread: 0.004, width: 0.01, length: 3.0, intensity: 0.7, streaks: 0.6, streak_freq: 4.0, core: 0.8, sway: [0.0, 0.0, 0.0], pulse: [0.6, 1.2, phase])
}
const STANDING_BEAMS: [Beam; 3] = [standing_at(0.26, 0.0), standing_at(0.34, 1.0 / 3.0), standing_at(0.42, 2.0 / 3.0)];
pub const WAVE_STANDING: Profile = {
    let mut profile = wave_of("wave-standing", Mood::Loud, &STANDING_BEAMS);
    profile.fog_density = 0.9;
    profile.glare = NO_GLARE;
    profile.typography = Type::Crossed;
    profile.decay = 1.3;
    profile.decay_delay = 0.7;
    profile
};

const HOLO_BEAMS: [Beam; 1] = [
    over!(shaft(), spread: 0.3, width: 0.3, length: 1.3, softness: 0.8, intensity: 0.8, streaks: 0.3, core: 0.3, spectrum: 0.8, gobo: 3, gobo_freq: 24.0, gobo_duty: 0.7, gobo_drift: 0.15, sway: [0.1, 10.0, 0.0]),
];
pub const WAVE_HOLO: Profile = {
    let mut profile = wave_of("wave-holo", Mood::Loud, &HOLO_BEAMS);
    profile.fog_density = 0.8;
    profile.glare = [0.5, 0.0, 0.12, 0.6, 0.45];
    profile.wave_mode = 1.0;
    profile.wave_center = [0.5, 0.5, 0.5, 0.0];
    profile.wave_motion = [8.0, 1.0, 0.15, SHIELD];
    profile.region_w = 0.4;
    profile.region_cy = 0.5;
    profile.region_h = 0.62;
    profile.typography = Type::Vertical;
    profile.decay = 1.3;
    profile.decay_delay = 0.7;
    profile
};

const SPROUT_BEAMS: [Beam; 2] = [
    over!(shaft(), y: -0.05, spread: 0.075, width: 0.07, length: 1.2, softness: 0.7, intensity: 0.95, streaks: 0.5, streak_freq: 6.0, streak_speed: 0.06, core: 0.7, sway: [0.008, 15.0, 0.2]),
    over!(shaft(), y: -0.05, spread: 0.26, width: 0.0, length: 0.75, softness: 0.95, intensity: 0.42, streaks: 0.85, streak_freq: 30.0, streak_speed: 0.04, core: 0.3, sway: [0.0, 0.0, 0.0]),
];
pub const BOTANY_SPROUT: Profile = {
    let mut profile = make("botany-sprout", "botany", Mood::Quiet, &SPROUT_BEAMS);
    profile.mote_count = 200;
    profile.mote_gain = 1.2;
    profile.mote_ambient = 0.012;
    profile.mote_swirl = 0.018;
    profile.mote_size = [0.004, 0.011];
    profile.mote_drift = [-0.003, -0.012];
    profile.mote_period = 11.0;
    profile.mote_twinkle = 0.55;
    profile.front_count = 6;
    profile.front_size = [0.05, 0.12];
    profile.front_drift = [-0.004, -0.002];
    profile.front_swirl = 0.01;
    profile.front_period = 16.0;
    profile.front_ambient = 0.015;
    profile.front_gain = 0.25;
    profile.star_count = 300;
    profile.star_opening = 3.6;
    profile.starfall = true;
    profile.rain = 50;
    profile.rain_speed = 0.16;
    profile.rain_spread = 0.25;
    profile.star_brightness = 0.85;
    profile.fog_density = 0.9;
    profile.fog_tyndall = 0.14;
    profile.fog_scale = 2.0;
    profile.fog_drift_x = -0.006;
    profile.fog_drift_y = -0.03;
    profile.fog_warp = 1.1;
    profile.fog_ambient = 0.025;
    profile.glare = [0.5, -0.01, 0.1, 0.9, 0.35];
    profile.region_cy = 0.44;
    profile.region_h = 0.6;
    profile.hero = 0.08;
    profile.typography = Type::Vertical;
    profile.echo = 0.28;
    profile.decay = 0.75;
    profile.decay_delay = 1.2;
    profile.camera_push = 0.03;
    profile.camera_drift = [0.0, 0.004];
    profile
};

const VEIN_BEAMS: [Beam; 1] = [
    over!(shaft(), x: 0.15, y: -0.05, angle: 1.1, spread: 0.12, width: 0.12, length: 1.4, intensity: 0.9, streaks: 0.5, streak_freq: 6.0),
];
pub const BOTANY_VEINS: Profile = {
    let mut profile = make("botany-veins", "botany", Mood::Neutral, &VEIN_BEAMS);
    profile.fog_density = 0.9;
    profile.glare = [0.15, -0.02, 0.14, 0.8, 0.45];
    profile.region_cy = 0.72;
    profile.region_h = 0.36;
    profile
};

const fn canopy_at(x: f32, index: f32) -> Beam {
    over!(shaft(), x: x, y: -0.05, angle: 1.45 + index * 0.1, spread: 0.08, width: 0.08, length: 1.3, softness: 0.6, intensity: 0.85, streaks: 0.5, streak_freq: 6.0, gobo: 4, gobo_freq: 9.0, gobo_duty: 0.5, gobo_drift: 0.14, sway: [0.03, 6.0 + index, index * 0.3])
}
const CANOPY_BEAMS: [Beam; 3] = [canopy_at(0.25, 0.0), canopy_at(0.48, 1.0), canopy_at(0.7, 2.0)];
pub const BOTANY_CANOPY: Profile = {
    let mut profile = make("botany-canopy", "botany", Mood::Loud, &CANOPY_BEAMS);
    profile.fog_density = 1.1;
    profile.fog_drift_x = 0.008;
    profile.glare = NO_GLARE;
    profile.mote_count = 300;
    profile.mote_swirl = 0.03;
    profile.typography = Type::Crossed;
    profile.decay = 1.2;
    profile.decay_delay = 0.8;
    profile.rain = 90;
    profile
};

const FERN_BEAMS: [Beam; 2] =
    [over!(shaft(), x: 0.3, spread: 0.08, intensity: 0.85), over!(fan(), x: 0.3, intensity: 0.2)];
pub const BOTANY_FERN: Profile = {
    let mut profile = make("botany-fern", "botany", Mood::Neutral, &FERN_BEAMS);
    profile.region_cx = 0.62;
    profile.region_w = 0.5;
    profile.region_h = 0.62;
    profile.typography = Type::Vertical;
    profile.decay = 0.8;
    profile.decay_delay = 1.2;
    profile
};

const SEED_BEAMS: [Beam; 1] = [over!(shaft(), spread: 0.04, width: 0.02, length: 1.2, intensity: 0.8, core: 0.9)];
pub const BOTANY_SEED: Profile = {
    let mut profile = make("botany-seed", "botany", Mood::Quiet, &SEED_BEAMS);
    profile.fog_density = 0.7;
    profile.fog_ambient = 0.02;
    profile.glare = [0.5, 0.0, 0.12, 0.6, 0.45];
    profile.region_cy = 0.38;
    profile.region_h = 0.4;
    profile.decay = 0.6;
    profile.decay_delay = 1.6;
    profile
};

const fn bloom_at(k: f32) -> Beam {
    over!(shaft(), x: 0.5, y: 0.42, angle: (k / 6.0) * TAU + 0.26, spread: 0.12, width: 0.01, length: 0.6, softness: 0.7, intensity: 0.55, streaks: 0.5, streak_freq: 7.0, core: 0.4, reach: 0.45, sway: [0.05, 10.0, k / 6.0])
}
const BLOOM_BEAMS: [Beam; 6] =
    [bloom_at(0.0), bloom_at(1.0), bloom_at(2.0), bloom_at(3.0), bloom_at(4.0), bloom_at(5.0)];
pub const BOTANY_BLOOM: Profile = {
    let mut profile = make("botany-bloom", "botany", Mood::Loud, &BLOOM_BEAMS);
    profile.fog_density = 0.9;
    profile.glare = [0.5, 0.42, 0.1, 1.2, 0.4];
    profile.typography = Type::Crossed;
    profile.burst = 0.5;
    profile.burst_mode = 2;
    profile.burst_every = 3;
    profile.burst_offset = 1;
    profile.burst_size = 3.0;
    profile.burst_tint = [1.0, 0.8, 0.7];
    profile.decay = 1.2;
    profile.decay_delay = 0.8;
    profile
};

const VINE_BEAMS: [Beam; 2] =
    [over!(shaft(), x: 0.72, spread: 0.07, intensity: 0.9), over!(fan(), x: 0.72, intensity: 0.2)];
pub const BOTANY_VINE: Profile = {
    let mut profile = make("botany-vine", "botany", Mood::Neutral, &VINE_BEAMS);
    profile.glare = [0.72, 0.0, 0.12, 0.9, 0.45];
    profile.region_cx = 0.36;
    profile.region_w = 0.5;
    profile.region_h = 0.62;
    profile.typography = Type::Vertical;
    profile
};

const CHLORO_BEAMS: [Beam; 1] = [over!(shaft(), spread: 0.2, width: 0.3, softness: 0.9, intensity: 0.6, core: 0.3)];
pub const BOTANY_CHLORO: Profile = {
    let mut profile = make("botany-chloro", "botany", Mood::Quiet, &CHLORO_BEAMS);
    profile.fog_density = 0.7;
    profile.glare = NO_GLARE;
    profile.caustic_scale = 6.0;
    profile.caustic_speed = 0.12;
    profile.caustic_in_beam = 0.3;
    profile.caustic_floor = [0.5, 0.45, 0.2, 0.36, 0.35];
    profile.region_w = 0.5;
    profile.region_cy = 0.5;
    profile.region_h = 0.5;
    profile.typography = Type::Crossed;
    profile.decay = 0.6;
    profile.decay_delay = 1.6;
    profile
};

const FORMULA_BEAMS: [Beam; 2] = [shaft(), fan()];
pub const BOTANY_FORMULA: Profile = make("botany-formula", "botany", Mood::Neutral, &FORMULA_BEAMS);

const POLLEN_BEAMS: [Beam; 2] = [
    over!(shaft(), x: 0.2, y: -0.05, angle: 1.15, spread: 0.14, width: 0.16, length: 1.5, intensity: 1.0, streaks: 0.5),
    over!(fan(), x: 0.2, angle: 1.15, intensity: 0.3),
];
pub const BOTANY_POLLEN: Profile = {
    let mut profile = make("botany-pollen", "botany", Mood::Loud, &POLLEN_BEAMS);
    profile.fog_density = 1.0;
    profile.fog_drift_x = 0.012;
    profile.fog_drift_y = -0.01;
    profile.glare = [0.2, 0.0, 0.12, 0.9, 0.45];
    profile.mote_count = 360;
    profile.mote_gain = 1.6;
    profile.mote_swirl = 0.04;
    profile.mote_size = [0.006, 0.022];
    profile.mote_drift = [0.012, -0.006];
    profile.mote_period = 10.0;
    profile.mote_twinkle = 0.4;
    profile.mote_ambient = 0.02;
    profile.decay = 1.2;
    profile.decay_delay = 0.8;
    profile
};

const ORBIT_BEAMS: [Beam; 1] = [over!(shaft(), spread: 0.05, intensity: 0.45, reach: 0.4)];
pub const ASTRAL_ORBIT: Profile = {
    let mut profile = make("astral-orbit", "astral", Mood::Quiet, &ORBIT_BEAMS);
    profile.fog_density = 0.6;
    profile.fog_ambient = 0.015;
    profile.glare = [0.5, 0.42, 0.05, 1.0, 0.5];
    profile.region_cy = 0.74;
    profile.region_h = 0.32;
    profile.decay = 0.6;
    profile.decay_delay = 1.6;
    profile
};

const TRAIL_BEAMS: [Beam; 1] = [over!(shaft(), x: 0.72, spread: 0.04, intensity: 0.35)];
pub const ASTRAL_TRAIL: Profile = {
    let mut profile = make("astral-trail", "astral", Mood::Quiet, &TRAIL_BEAMS);
    profile.fog_density = 0.5;
    profile.fog_ambient = 0.01;
    profile.glare = NO_GLARE;
    profile.region_cx = 0.36;
    profile.region_w = 0.44;
    profile.region_h = 0.6;
    profile.typography = Type::Vertical;
    profile.decay = 0.5;
    profile.decay_delay = 1.8;
    profile
};

const SEXTANT_BEAMS: [Beam; 1] =
    [over!(shaft(), x: 0.85, y: -0.05, angle: 2.2, spread: 0.05, width: 0.03, length: 1.4, intensity: 0.9)];
pub const ASTRAL_SEXTANT: Profile = {
    let mut profile = make("astral-sextant", "astral", Mood::Neutral, &SEXTANT_BEAMS);
    profile.fog_density = 0.8;
    profile.glare = [0.85, -0.02, 0.12, 0.9, 0.45];
    profile.region_cx = 0.52;
    profile.region_cy = 0.66;
    profile.region_h = 0.4;
    profile
};

const CONSTELLATION_BEAMS: [Beam; 1] = [over!(shaft(), spread: 0.06, intensity: 0.4)];
pub const ASTRAL_CONSTELLATION: Profile = {
    let mut profile = make("astral-constellation", "astral", Mood::Neutral, &CONSTELLATION_BEAMS);
    profile.fog_density = 0.6;
    profile.fog_ambient = 0.015;
    profile.glare = [0.5, 0.0, 0.12, 0.4, 0.45];
    profile.region_cx = 0.42;
    profile.region_cy = 0.66;
    profile.region_w = 0.6;
    profile.region_h = 0.4;
    profile
};

pub const ASTRAL_ECLIPSE: Profile = {
    let mut profile = make("astral-eclipse", "astral", Mood::Loud, &[]);
    profile.fog_density = 0.8;
    profile.fog_tyndall = 0.2;
    profile.fog_drift_x = 0.004;
    profile.fog_drift_y = -0.01;
    profile.fog_ambient = 0.02;
    profile.region_cy = 0.7;
    profile.region_h = 0.4;
    profile.typography = Type::Crossed;
    profile.burst = 0.35;
    profile.burst_mode = 1;
    profile.burst_every = 1;
    profile.burst_size = 3.6;
    profile.burst_tint = [1.0, 0.6, 0.45];
    profile.decay = 1.3;
    profile.decay_delay = 0.7;
    profile
};

pub const ASTRAL_CORONA: Profile = {
    let mut profile = make("astral-corona", "astral", Mood::Loud, &[]);
    profile.fog_tyndall = 0.15;
    profile.fog_drift_x = 0.004;
    profile.fog_drift_y = -0.02;
    profile.glare = [0.5, 0.12, 0.16, 1.3, 0.7];
    profile.region_cy = 0.62;
    profile.region_h = 0.46;
    profile.typography = Type::Crossed;
    profile.burst = 0.3;
    profile.burst_mode = 1;
    profile.burst_every = 2;
    profile.burst_size = 3.4;
    profile.burst_tint = [1.0, 0.65, 0.4];
    profile.decay = 1.3;
    profile.decay_delay = 0.7;
    profile
};

const MOON_BEAMS: [Beam; 1] = [
    over!(shaft(), x: 0.82, y: 0.12, angle: 2.1, spread: 0.12, width: 0.08, length: 1.4, softness: 0.8, intensity: 0.75, tint: [0.72, 0.82, 1.0]),
];
pub const ASTRAL_MOON: Profile = {
    let mut profile = make("astral-moon", "astral", Mood::Quiet, &MOON_BEAMS);
    profile.fog_density = 1.2;
    profile.fog_drift_x = -0.01;
    profile.fog_drift_y = 0.0;
    profile.fog_ambient = 0.035;
    profile.fog_warp = 1.3;
    profile.glare = [0.82, 0.12, 0.1, 0.8, 0.2];
    profile.region_cx = 0.38;
    profile.region_w = 0.5;
    profile.region_h = 0.62;
    profile.typography = Type::Vertical;
    profile.decay = 0.6;
    profile.decay_delay = 1.6;
    profile
};

const NEBULA_BEAMS: [Beam; 2] = [
    over!(shaft(), x: 0.4, y: 0.45, angle: -0.6, spread: 0.6, width: 0.1, length: 0.8, softness: 1.0, intensity: 0.5, streaks: 0.6, streak_freq: 3.0, core: 0.2, spectrum: 0.7, sway: [0.2, 20.0, 0.0]),
    over!(shaft(), x: 0.6, y: 0.45, angle: 2.4, spread: 0.6, width: 0.1, length: 0.8, softness: 1.0, intensity: 0.4, streaks: 0.6, streak_freq: 3.0, core: 0.2, spectrum: 0.7, sway: [0.2, 23.0, 0.5]),
];
pub const ASTRAL_NEBULA: Profile = {
    let mut profile = make("astral-nebula", "astral", Mood::Neutral, &NEBULA_BEAMS);
    profile.fog_density = 1.4;
    profile.fog_ambient = 0.06;
    profile.fog_warp = 1.6;
    profile.fog_scale = 1.6;
    profile.fog_drift_x = 0.006;
    profile.fog_drift_y = 0.002;
    profile.glare = [0.5, 0.45, 0.08, 0.7, 0.2];
    profile
};

const ARMILLARY_BEAMS: [Beam; 2] = [shaft(), over!(fan(), intensity: 0.2)];
pub const ASTRAL_ARMILLARY: Profile = {
    let mut profile = make("astral-armillary", "astral", Mood::Neutral, &ARMILLARY_BEAMS);
    profile.region_w = 0.5;
    profile.region_cy = 0.5;
    profile.region_h = 0.62;
    profile.typography = Type::Vertical;
    profile
};

const fn meteor_at(index: f32) -> Beam {
    over!(shaft(), x: 0.95 - index * 0.2, y: -0.02 + index * 0.05, angle: 2.5, spread: 0.01, width: 0.006, length: 0.6, intensity: 0.9, streaks: 0.2, core: 0.8, reach: 0.5 + index * 0.1, pulse: [0.7, 2.2 + index * 0.7, index * 0.3], sway: [0.0, 0.0, 0.0])
}
const METEOR_BEAMS: [Beam; 3] = [meteor_at(0.0), meteor_at(1.0), meteor_at(2.0)];
pub const ASTRAL_METEOR: Profile = {
    let mut profile = make("astral-meteor", "astral", Mood::Loud, &METEOR_BEAMS);
    profile.fog_density = 0.7;
    profile.fog_ambient = 0.015;
    profile.glare = NO_GLARE;
    profile.typography = Type::Crossed;
    profile.decay = 1.3;
    profile.decay_delay = 0.7;
    profile.rain = 60;
    profile.rain_speed = 0.6;
    profile.rain_spread = 1.8;
    profile.star_brightness = 1.2;
    profile
};

const fn search_at(x: f32, index: f32) -> Beam {
    over!(shaft(), x: x, y: 1.02, angle: -DOWN, spread: 0.035, width: 0.03, length: 1.4, softness: 0.5, intensity: 0.8, streaks: 0.3, streak_freq: 5.0, core: 0.6, sway: [0.45, 7.0 + index * 1.3, index * 0.27])
}
const SEARCH_BEAMS: [Beam; 4] =
    [search_at(0.15, 0.0), search_at(0.38, 1.0), search_at(0.62, 2.0), search_at(0.85, 3.0)];
pub const STAGE_SEARCH: Profile = {
    let mut profile = make("stage-search", "stage", Mood::Loud, &SEARCH_BEAMS);
    profile.region_cy = 0.44;
    profile.region_h = 0.46;
    profile.decay = 1.3;
    profile.decay_delay = 0.7;
    profile
};

const fn foot_at(x: f32) -> Beam {
    over!(shaft(), x: x, y: 1.03, angle: -DOWN, spread: 0.22, width: 0.08, length: 0.7, softness: 0.9, intensity: 0.6, streaks: 0.4, streak_freq: 10.0, core: 0.3, tint: [1.0, 0.8, 0.6], sway: [0.0, 0.0, 0.0])
}
const FOOT_BEAMS: [Beam; 3] = [foot_at(0.25), foot_at(0.5), foot_at(0.75)];
pub const STAGE_FOOT: Profile = {
    let mut profile = make("stage-footlight", "stage", Mood::Quiet, &FOOT_BEAMS);
    profile.fog_drift_y = -0.03;
    profile.region_w = 0.5;
    profile.region_cy = 0.46;
    profile.region_h = 0.62;
    profile.typography = Type::Vertical;
    profile.decay = 0.6;
    profile.decay_delay = 1.6;
    profile
};

const fn back_at(k: f32) -> Beam {
    over!(shaft(), x: 0.5, y: 0.5, angle: (k / 6.0) * TAU + 0.5, spread: 0.12, width: 0.02, length: 0.8, softness: 0.7, intensity: 0.6, streaks: 0.6, streak_freq: 10.0, core: 0.4, sway: [0.05, 11.0, k / 6.0])
}
const BACK_BEAMS: [Beam; 6] = [back_at(0.0), back_at(1.0), back_at(2.0), back_at(3.0), back_at(4.0), back_at(5.0)];
pub const STAGE_BACK: Profile = {
    let mut profile = make("stage-backlight", "stage", Mood::Neutral, &BACK_BEAMS);
    profile.glare = [0.5, 0.5, 0.22, 1.2, 0.8];
    profile.region_cy = 0.5;
    profile.region_h = 0.5;
    profile.typography = Type::Crossed;
    profile
};

const PIN_BEAMS: [Beam; 1] = [
    over!(shaft(), spread: 0.03, width: 0.01, length: 1.3, softness: 0.55, intensity: 0.9, streaks: 0.4, streak_freq: 6.0, core: 0.9, sway: [0.0, 0.0, 0.0]),
];
const PIN_AIM: [([f32; 2], [f32; 2]); 1] = [([0.5, 0.02], [0.5, 0.56])];
pub const STAGE_PIN: Profile = {
    let mut profile = make("stage-pinspot", "stage", Mood::Quiet, &PIN_BEAMS);
    profile.aim = &PIN_AIM;
    profile.fog_ambient = 0.015;
    profile.decay = 0.6;
    profile.decay_delay = 1.6;
    profile
};

const fn strobe_at(phase: f32) -> Beam {
    over!(shaft(), spread: 0.08, width: 0.03, length: 1.3, softness: 0.55, intensity: 0.55, streaks: 0.4, streak_freq: 6.0, core: 0.6, sway: [0.0, 0.0, 0.0], pulse: [0.95, 0.5, phase])
}
const STROBE_BEAMS: [Beam; 4] = [strobe_at(0.0), strobe_at(0.25), strobe_at(0.5), strobe_at(0.75)];
const STROBE_AIM: [([f32; 2], [f32; 2]); 4] =
    [([0.1, 0.02], [0.5, 0.6]), ([0.36, 0.02], [0.5, 0.6]), ([0.64, 0.02], [0.5, 0.6]), ([0.9, 0.02], [0.5, 0.6])];
pub const STAGE_STROBE: Profile = {
    let mut profile = make("stage-strobe", "stage", Mood::Loud, &STROBE_BEAMS);
    profile.aim = &STROBE_AIM;
    profile.glare = [0.5, 0.02, 0.2, 0.6, 0.4];
    profile.typography = Type::Crossed;
    profile.burst = 0.5;
    profile.burst_mode = 2;
    profile.burst_every = 2;
    profile.burst_size = 3.0;
    profile.burst_tint = [1.0, 0.9, 0.8];
    profile.decay = 1.4;
    profile.decay_delay = 0.6;
    profile
};

const CURTAIN_BEAMS: [Beam; 1] = [
    over!(shaft(), spread: 0.01, width: 0.04, length: 1.6, softness: 0.5, intensity: 0.9, streaks: 0.3, core: 0.7, sway: [0.0, 0.0, 0.0]),
];
pub const STAGE_CURTAIN: Profile = {
    let mut profile = make("stage-curtain", "stage", Mood::Quiet, &CURTAIN_BEAMS);
    profile.glare = [0.5, -0.02, 0.08, 0.8, 0.5];
    profile.fog_ambient = 0.02;
    profile.region_w = 0.3;
    profile.region_cy = 0.5;
    profile.region_h = 0.66;
    profile.typography = Type::Vertical;
    profile.decay = 0.6;
    profile.decay_delay = 1.6;
    profile
};

const fn laser_at(k: f32) -> Beam {
    over!(shaft(), x: 0.5, y: 1.02, angle: -DOWN + (k - 2.5) * 0.22, spread: 0.002, width: 0.004, length: 2.0, softness: 0.3, intensity: 1.2, streaks: 0.0, core: 0.9, sway: [0.25, 4.5, k * 0.08])
}
const LASER_BEAMS: [Beam; 6] =
    [laser_at(0.0), laser_at(1.0), laser_at(2.0), laser_at(3.0), laser_at(4.0), laser_at(5.0)];
pub const STAGE_LASER: Profile = {
    let mut profile = make("stage-laser", "stage", Mood::Loud, &LASER_BEAMS);
    profile.glare = [0.5, 1.0, 0.06, 1.0, 0.6];
    profile.fog_density = 1.4;
    profile.region_cy = 0.4;
    profile.region_h = 0.46;
    profile.decay = 1.3;
    profile.decay_delay = 0.7;
    profile
};

const PROJECTOR_BEAMS: [Beam; 1] = [
    over!(shaft(), x: 0.12, y: 0.4, angle: 0.05, spread: 0.2, width: 0.02, length: 1.8, softness: 0.4, intensity: 1.0, streaks: 0.5, streak_freq: 9.0, core: 0.4, sway: [0.0, 0.0, 0.0], pulse: [0.05, 0.12, 0.0]),
];
pub const STAGE_PROJECTOR: Profile = {
    let mut profile = make("stage-projector", "stage", Mood::Neutral, &PROJECTOR_BEAMS);
    profile.glare = [0.12, 0.4, 0.05, 1.0, 0.4];
    profile.mote_count = 380;
    profile.mote_gain = 1.8;
    profile.mote_swirl = 0.03;
    profile.region_cx = 0.62;
    profile.region_w = 0.44;
    profile.region_h = 0.58;
    profile.typography = Type::Vertical;
    profile
};

const SMOKE_BEAMS: [Beam; 1] = [
    over!(shaft(), x: -0.05, y: 0.55, angle: -0.08, spread: 0.18, width: 0.3, length: 1.6, softness: 0.7, intensity: 0.42, streaks: 0.2, core: 0.3, sway: [0.0, 0.0, 0.0]),
];
pub const MOTES_SMOKE: Profile = {
    let mut profile = make("motes-smoke", "motes", Mood::Neutral, &SMOKE_BEAMS);
    profile.fog_density = 1.5;
    profile.fog_tyndall = 0.05;
    profile.fog_scale = 1.8;
    profile.fog_warp = 1.8;
    profile.fog_drift_x = 0.004;
    profile.fog_drift_y = -0.06;
    profile.fog_ambient = 0.05;
    profile.mote_count = 140;
    profile.typography = Type::Crossed;
    profile
};

const EMBER_BEAMS: [Beam; 1] = [
    over!(shaft(), x: 0.5, y: 1.05, angle: -DOWN, spread: 0.35, width: 0.3, length: 0.6, softness: 0.9, intensity: 0.8, streaks: 0.6, streak_freq: 8.0, core: 0.3, tint: [1.0, 0.55, 0.3], pulse: [0.2, 1.3, 0.0], sway: [0.0, 0.0, 0.0]),
];
pub const MOTES_EMBERS: Profile = {
    let mut profile = make("motes-embers", "motes", Mood::Loud, &EMBER_BEAMS);
    profile.fog_density = 1.1;
    profile.fog_drift_y = -0.05;
    profile.mote_count = 380;
    profile.mote_gain = 1.8;
    profile.mote_ambient = 0.1;
    profile.mote_swirl = 0.03;
    profile.mote_size = [0.003, 0.01];
    profile.mote_drift = [0.004, -0.07];
    profile.mote_period = 5.0;
    profile.mote_twinkle = 0.8;
    profile.region_cy = 0.44;
    profile.region_h = 0.46;
    profile.decay = 1.4;
    profile.decay_delay = 0.6;
    profile
};

const SNOW_BEAMS: [Beam; 1] = [over!(shaft(), spread: 0.2, width: 0.2, softness: 0.9, intensity: 0.5, core: 0.3)];
pub const MOTES_SNOW: Profile = {
    let mut profile = make("motes-snow", "motes", Mood::Quiet, &SNOW_BEAMS);
    profile.fog_density = 0.8;
    profile.fog_drift_y = 0.01;
    profile.mote_count = 460;
    profile.mote_ambient = 0.15;
    profile.mote_gain = 1.0;
    profile.mote_size = [0.003, 0.012];
    profile.mote_drift = [0.003, 0.035];
    profile.mote_period = 8.0;
    profile.mote_twinkle = 0.3;
    profile.rain_speed = 0.12;
    profile.rain_spread = 1.8;
    profile.star_brightness = 0.7;
    profile.region_w = 0.5;
    profile.region_cy = 0.5;
    profile.region_h = 0.62;
    profile.typography = Type::Vertical;
    profile.decay = 0.6;
    profile.decay_delay = 1.6;
    profile.rain = 60;
    profile
};

const BOKEH_BEAMS: [Beam; 1] = [over!(shaft(), intensity: 0.6)];
pub const MOTES_BOKEH: Profile = {
    let mut profile = make("motes-bokeh", "motes", Mood::Neutral, &BOKEH_BEAMS);
    profile.fog_density = 0.8;
    profile.mote_count = 120;
    profile.front_count = 34;
    profile.front_size = [0.06, 0.2];
    profile.front_ambient = 0.1;
    profile.front_gain = 0.5;
    profile.front_twinkle = 0.3;
    profile.front_drift = [0.008, -0.002];
    profile
};

const MIST_BEAMS: [Beam; 1] = [
    over!(shaft(), x: 0.7, y: 0.62, angle: -2.8, spread: 0.5, width: 0.3, length: 1.2, softness: 1.0, intensity: 0.55, streaks: 0.3, core: 0.2, sway: [0.0, 0.0, 0.0]),
];
pub const MOTES_MIST: Profile = {
    let mut profile = make("motes-mist", "motes", Mood::Quiet, &MIST_BEAMS);
    profile.fog_density = 1.5;
    profile.fog_tyndall = 0.1;
    profile.fog_scale = 1.2;
    profile.fog_warp = 0.6;
    profile.fog_drift_x = 0.02;
    profile.fog_drift_y = 0.0;
    profile.fog_ambient = 0.05;
    profile.glare = [0.7, 0.62, 0.16, 0.5, 0.08];
    profile.mote_count = 120;
    profile.mote_drift = [0.01, -0.01];
    profile.region_cx = 0.36;
    profile.region_cy = 0.42;
    profile.region_w = 0.44;
    profile.region_h = 0.5;
    profile.typography = Type::Vertical;
    profile.decay = 0.6;
    profile.decay_delay = 1.6;
    profile
};

const SWARM_BEAMS: [Beam; 2] = [
    over!(shaft(), intensity: 0.8),
    over!(shaft(), spread: 0.3, length: 0.6, softness: 0.9, intensity: 0.25, streaks: 0.8, streak_freq: 24.0),
];
pub const MOTES_SWARM: Profile = {
    let mut profile = make("motes-swarm", "motes", Mood::Loud, &SWARM_BEAMS);
    profile.mote_count = 460;
    profile.mote_gain = 1.6;
    profile.mote_ambient = 0.08;
    profile.mote_swirl = 0.14;
    profile.mote_size = [0.003, 0.012];
    profile.mote_drift = [0.0, 0.0];
    profile.mote_period = 6.0;
    profile.mote_twinkle = 0.6;
    profile.typography = Type::Crossed;
    profile.decay = 1.3;
    profile.decay_delay = 0.7;
    profile
};

const DISSOLVE_BEAMS: [Beam; 1] = [over!(shaft(), intensity: 0.7)];
pub const MOTES_DISSOLVE: Profile = {
    let mut profile = make("motes-dissolve", "motes", Mood::Neutral, &DISSOLVE_BEAMS);
    profile.mote_count = 380;
    profile.mote_gain = 1.5;
    profile.mote_swirl = 0.04;
    profile.mote_drift = [0.004, -0.02];
    profile.typography = Type::Crossed;
    profile.decay = 2.2;
    profile.decay_delay = 0.3;
    profile
};

const fn aurora_at(x: f32, index: f32) -> Beam {
    over!(shaft(), x: x, y: -0.1, angle: DOWN + 0.1, spread: 0.06, width: 0.16, length: 0.7, softness: 0.8, intensity: 0.7, streaks: 0.9, streak_freq: 30.0, streak_speed: 0.3, core: 0.2, spectrum: 0.5, tint: [0.6, 1.0, 0.8], sway: [0.12, 8.0 + index * 1.7, index * 0.2])
}
const AURORA_BEAMS: [Beam; 4] = [aurora_at(0.2, 0.0), aurora_at(0.4, 1.0), aurora_at(0.6, 2.0), aurora_at(0.8, 3.0)];
pub const MOTES_AURORA: Profile = {
    let mut profile = make("motes-aurora", "motes", Mood::Loud, &AURORA_BEAMS);
    profile.fog_density = 0.9;
    profile.fog_ambient = 0.03;
    profile.region_cy = 0.62;
    profile.region_h = 0.4;
    profile.decay = 1.2;
    profile.decay_delay = 0.8;
    profile.rain = 20;
    profile.star_count = 560;
    profile
};
