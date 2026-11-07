// Light positions. Zenith is the full family; lattice, motes and stage contribute the other moods the caster uses.

use super::beams::{Beam, DOWN, MAX_BEAMS, fan, shaft};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Mood {
    Quiet,
    Neutral,
    Loud,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub enum Type {
    Horizontal,
    Vertical,
    Crossed,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Art {
    Standard,
    Sparks,
    None,
}

#[derive(Clone, Copy)]
pub struct Profile {
    pub kind: &'static str,
    pub family: &'static str,
    pub mood: Mood,
    pub beams: &'static [Beam],
    pub aim: &'static [([f32; 2], [f32; 2])],
    pub fog_density: f32,
    pub fog_ambient: f32,
    pub fog_drift_x: f32,
    pub fog_drift_y: f32,
    pub fog_scale: f32,
    pub fog_tyndall: f32,
    pub fog_warp: f32,
    pub region_cx: f32,
    pub region_w: f32,
    pub region_h: f32,
    pub caustic_scale: f32,
    pub caustic_speed: f32,
    pub caustic_in_beam: f32,
    pub caustic_floor: [f32; 5],
    pub wave_mode: f32,
    pub wave_center: [f32; 4],
    pub wave_motion: [f32; 4],
    pub glare: [f32; 5],
    pub mote_count: u16,
    pub mote_gain: f32,
    pub mote_ambient: f32,
    pub mote_swirl: f32,
    pub mote_size: [f32; 2],
    pub mote_drift: [f32; 2],
    pub mote_period: f32,
    pub mote_twinkle: f32,
    pub front: bool,
    pub front_count: u16,
    pub front_size: [f32; 2],
    pub front_drift: [f32; 2],
    pub front_swirl: f32,
    pub front_period: f32,
    pub front_twinkle: f32,
    pub front_ambient: f32,
    pub front_gain: f32,
    pub hero: f32,
    pub region_cy: f32,
    pub typography: Type,
    /// Largest echo fragment size, as a fraction of the height (`echo.size`, default 0.34).
    pub echo: f32,
    pub decay: f32,
    pub decay_delay: f32,
    pub burst: f32,
    pub burst_mode: u8,
    pub burst_every: u8,
    pub burst_offset: u8,
    pub burst_size: f32,
    pub burst_tint: [f32; 3],
    pub starfall: bool,
    pub star_count: u16,
    pub star_opening: f32,
    pub rain: u16,
    pub rain_speed: f32,
    pub rain_spread: f32,
    pub star_brightness: f32,
    pub art_gain: f32,
    pub camera_push: f32,
    pub camera_drift: [f32; 2],
    pub art: Art,
}

pub(crate) const fn base(kind: &'static str, family: &'static str, mood: Mood, beams: &'static [Beam]) -> Profile {
    Profile {
        kind,
        family,
        mood,
        beams,
        aim: &[],
        fog_density: 1.0,
        fog_ambient: 0.03,
        fog_drift_x: 0.008,
        fog_drift_y: -0.04,
        fog_scale: 2.4,
        fog_tyndall: 0.12,
        fog_warp: 0.9,
        region_cx: 0.5,
        region_w: 0.72,
        region_h: 0.5,
        caustic_scale: 0.0,
        caustic_speed: 0.0,
        caustic_in_beam: 0.0,
        caustic_floor: [0.0, 0.0, 0.0, 0.0, 0.0],
        wave_mode: 0.0,
        wave_center: [0.0, 0.0, 0.0, 0.0],
        wave_motion: [0.0, 0.0, 0.0, 0.0],
        glare: [0.5, 0.0, 0.12, 0.9, 0.45],
        mote_count: 260,
        mote_gain: 1.3,
        mote_ambient: 0.015,
        mote_swirl: 0.02,
        mote_size: [0.004, 0.013],
        mote_drift: [0.004, -0.01],
        mote_period: 9.0,
        mote_twinkle: 0.5,
        front: true,
        front_count: 9,
        front_size: [0.05, 0.15],
        front_drift: [0.006, -0.002],
        front_swirl: 0.012,
        front_period: 14.0,
        front_twinkle: 0.2,
        front_ambient: 0.02,
        front_gain: 0.3,
        hero: 0.085,
        region_cy: 0.54,
        typography: Type::Horizontal,
        echo: 0.34,
        decay: 1.0,
        decay_delay: 1.0,
        burst: 0.0,
        burst_mode: 0,
        burst_every: 1,
        burst_offset: 0,
        burst_size: 3.2,
        burst_tint: [1.0, 1.0, 1.0],
        starfall: false,
        star_count: 420,
        star_opening: 3.2,
        rain: 70,
        rain_speed: 0.22,
        rain_spread: 0.35,
        star_brightness: 1.0,
        art_gain: 1.0,
        camera_push: 0.035,
        camera_drift: [0.0, -0.006],
        art: Art::Standard,
    }
}

const fn wide_fan() -> Beam {
    let mut beam = fan();
    beam.spread = 0.55;
    beam.length = 0.42;
    beam.softness = 1.0;
    beam.intensity = 0.16;
    beam.streaks = 0.9;
    beam.streak_freq = 40.0;
    beam.streak_speed = -0.03;
    beam.core = 0.2;
    beam.sway = [0.0, 0.0, 0.0];
    beam
}

const SHAFT_BEAMS: [Beam; 3] = [shaft(), fan(), wide_fan()];
const SHAFT: Profile = {
    let mut profile = base("zenith-shaft", "zenith", Mood::Neutral, &SHAFT_BEAMS);
    profile.typography = Type::Crossed;
    profile.starfall = true;
    profile
};

const fn fan_ray(k: f32) -> Beam {
    let mut beam = shaft();
    beam.angle = DOWN + k * 0.24;
    beam.spread = 0.035;
    beam.width = 0.012;
    beam.length = 0.9;
    beam.intensity = 0.7 - k.abs() * 0.08;
    beam.streaks = 0.4;
    beam.streak_freq = 5.0;
    beam.sway = [0.05, 11.0, k * 0.1];
    beam
}

const FAN_BEAMS: [Beam; 5] = [fan_ray(-2.0), fan_ray(-1.0), fan_ray(0.0), fan_ray(1.0), fan_ray(2.0)];
const FAN_PROFILE: Profile = {
    let mut profile = base("zenith-fan", "zenith", Mood::Loud, &FAN_BEAMS);
    profile.decay = 1.2;
    profile.decay_delay = 0.8;
    profile.starfall = true;
    profile.rain = 110;
    profile.rain_spread = 0.6;
    profile
};

const fn halo_shaft() -> Beam {
    let mut beam = shaft();
    beam.spread = 0.07;
    beam
}

const fn halo_fan() -> Beam {
    let mut beam = fan();
    beam.spread = 0.42;
    beam.intensity = 0.24;
    beam
}

const HALO_BEAMS: [Beam; 2] = [halo_shaft(), halo_fan()];
const HALO: Profile = {
    let mut profile = base("zenith-halo", "zenith", Mood::Neutral, &HALO_BEAMS);
    profile.glare = [0.5, 0.0, 0.2, 1.2, 0.7];
    profile.typography = Type::Vertical;
    profile.region_cy = 0.5;
    profile.region_h = 0.62;
    profile.decay = 0.8;
    profile.decay_delay = 1.2;
    profile.starfall = true;
    profile
};

const fn drift_shaft() -> Beam {
    let mut beam = shaft();
    beam.sway = [0.2, 9.0, 0.0];
    beam.intensity = 0.85;
    beam
}

const fn drift_fan() -> Beam {
    let mut beam = fan();
    beam.intensity = 0.2;
    beam.sway = [0.2, 9.0, 0.0];
    beam
}

const DRIFT_BEAMS: [Beam; 2] = [drift_shaft(), drift_fan()];
const DRIFT: Profile = {
    let mut profile = base("zenith-drift", "zenith", Mood::Quiet, &DRIFT_BEAMS);
    profile.fog_density = 0.9;
    profile.decay = 0.7;
    profile.decay_delay = 1.4;
    profile.camera_push = 0.02;
    profile.camera_drift = [0.01, 0.0];
    profile
};

const fn twin(x: f32, phase: f32) -> Beam {
    let mut beam = shaft();
    beam.x = x;
    beam.spread = 0.06;
    beam.sway = [0.06, 12.0, phase];
    beam
}

const TWIN_BEAMS: [Beam; 2] = [twin(0.33, 0.0), twin(0.67, 0.5)];
const TWIN: Profile = {
    let mut profile = base("zenith-twin", "zenith", Mood::Neutral, &TWIN_BEAMS);
    profile.glare = [0.0, 0.0, 0.0, 0.0, 0.0];
    profile.typography = Type::Crossed;
    profile.starfall = false;
    profile
};

const fn rain_at(x: f32, index: f32) -> Beam {
    let mut beam = shaft();
    beam.x = x;
    beam.spread = 0.012;
    beam.width = 0.01;
    beam.length = 1.1;
    beam.intensity = 0.45;
    beam.streaks = 0.7;
    beam.streak_freq = 3.0;
    beam.pulse = [0.35, 1.7 + index * 0.37, index * 0.21];
    beam.sway = [0.0, 0.0, 0.0];
    beam
}

const RAIN_BEAMS: [Beam; 6] = [
    rain_at(0.18, 0.0),
    rain_at(0.34, 1.0),
    rain_at(0.5, 2.0),
    rain_at(0.66, 3.0),
    rain_at(0.82, 4.0),
    rain_at(0.42, 5.0),
];
const RAIN: Profile = {
    let mut profile = base("zenith-rain", "zenith", Mood::Loud, &RAIN_BEAMS);
    profile.fog_drift_y = 0.05;
    profile.glare = [0.5, 0.0, 0.12, 0.5, 0.45];
    profile.typography = Type::Vertical;
    profile.decay = 1.3;
    profile.decay_delay = 0.6;
    profile.starfall = true;
    profile.rain = 220;
    profile.rain_speed = 0.45;
    profile.rain_spread = 1.4;
    profile.star_brightness = 1.1;
    profile
};

const fn well_shaft() -> Beam {
    let mut beam = shaft();
    beam.spread = 0.05;
    beam.width = 0.02;
    beam.length = 1.4;
    beam.intensity = 0.8;
    beam.core = 0.9;
    beam
}

const WELL_BEAMS: [Beam; 1] = [well_shaft()];
const WELL: Profile = {
    let mut profile = base("zenith-well", "zenith", Mood::Quiet, &WELL_BEAMS);
    profile.fog_density = 0.7;
    profile.fog_ambient = 0.015;
    profile.glare = [0.5, 0.0, 0.12, 0.6, 0.45];
    profile.mote_count = 160;
    profile.decay = 0.6;
    profile.decay_delay = 1.6;
    profile.art = Art::None;
    profile
};

const CROWN: Profile = {
    let mut profile = base("zenith-crown", "zenith", Mood::Loud, &SHAFT_BEAMS);
    profile.glare = [0.5, 0.0, 0.18, 1.3, 0.8];
    profile.burst = 0.4;
    profile.burst_mode = 1;
    profile.burst_every = 1;
    profile.burst_size = 4.0;
    profile.burst_tint = [1.0, 0.6, 0.4];
    profile.star_count = 520;
    profile.star_opening = 2.6;
    profile.rain = 120;
    profile.rain_speed = 0.32;
    profile.rain_spread = 0.5;
    profile.star_brightness = 1.1;
    profile.decay = 1.25;
    profile.decay_delay = 0.7;
    profile.starfall = true;
    profile.rain = 120;
    profile.camera_push = 0.05;
    profile.camera_drift = [0.0, -0.01];
    profile
};

const fn descent_shaft() -> Beam {
    let mut beam = shaft();
    beam.reveal = 3.0;
    beam
}

const fn descent_fan() -> Beam {
    let mut beam = fan();
    beam.reveal = 3.5;
    beam
}

const DESCENT_BEAMS: [Beam; 2] = [descent_shaft(), descent_fan()];
const DESCENT: Profile = {
    let mut profile = base("zenith-descent", "zenith", Mood::Neutral, &DESCENT_BEAMS);
    profile.typography = Type::Crossed;
    profile.starfall = true;
    profile.rain = 40;
    profile
};

const fn ember_shaft() -> Beam {
    let mut beam = shaft();
    beam.intensity = 0.45;
    beam.tint = [1.0, 0.7, 0.5];
    beam.pulse = [0.2, 5.0, 0.0];
    beam
}

const fn ember_fan() -> Beam {
    let mut beam = fan();
    beam.intensity = 0.12;
    beam.tint = [1.0, 0.6, 0.4];
    beam
}

const EMBER_BEAMS: [Beam; 2] = [ember_shaft(), ember_fan()];
const EMBER: Profile = {
    let mut profile = base("zenith-ember", "zenith", Mood::Quiet, &EMBER_BEAMS);
    profile.fog_density = 1.2;
    profile.fog_ambient = 0.04;
    profile.fog_drift_y = -0.05;
    profile.glare = [0.5, 0.0, 0.12, 0.4, 0.45];
    profile.mote_count = 320;
    profile.mote_gain = 1.6;
    profile.mote_ambient = 0.04;
    profile.mote_swirl = 0.03;
    profile.mote_drift = [0.004, -0.035];
    profile.region_cy = 0.5;
    profile.region_h = 0.62;
    profile.typography = Type::Vertical;
    profile.decay = 1.4;
    profile.decay_delay = 0.8;
    profile.camera_push = 0.02;
    profile.camera_drift = [0.0, -0.012];
    profile
};

const fn window_beam() -> Beam {
    let mut beam = shaft();
    beam.x = 0.1;
    beam.y = 0.06;
    beam.angle = 0.62;
    beam.spread = 0.1;
    beam.width = 0.34;
    beam.length = 1.3;
    beam.softness = 0.35;
    beam.intensity = 1.0;
    beam.streaks = 0.25;
    beam.streak_freq = 4.0;
    beam.core = 0.3;
    beam.sway = [0.01, 17.0, 0.0];
    beam.gobo = 1;
    beam.gobo_freq = 13.0;
    beam.gobo_duty = 0.52;
    beam.gobo_drift = 0.02;
    beam
}

const BLINDS_BEAMS: [Beam; 1] = [window_beam()];
const BLINDS: Profile = {
    let mut profile = base("lattice-blinds", "lattice", Mood::Neutral, &BLINDS_BEAMS);
    profile.fog_drift_x = 0.012;
    profile.fog_drift_y = -0.02;
    profile.glare = [0.1, 0.06, 0.16, 0.8, 0.45];
    profile.region_cx = 0.56;
    profile.region_cy = 0.58;
    profile.region_w = 0.66;
    profile
};

const fn cross_beam() -> Beam {
    let mut beam = window_beam();
    beam.x = 0.9;
    beam.angle = std::f32::consts::PI - 0.7;
    beam.width = 0.3;
    beam.intensity = 0.9;
    beam.gobo = 2;
    beam.gobo_freq = 0.0;
    beam.gobo_duty = 0.16;
    beam.gobo_drift = 0.0;
    beam
}

const CROSS_BEAMS: [Beam; 1] = [cross_beam()];
const CROSS: Profile = {
    let mut profile = base("lattice-cross", "lattice", Mood::Quiet, &CROSS_BEAMS);
    profile.fog_drift_x = -0.008;
    profile.glare = [0.9, 0.06, 0.14, 0.7, 0.45];
    profile.region_cx = 0.4;
    profile.region_cy = 0.52;
    profile.region_w = 0.6;
    profile.region_h = 0.62;
    profile.art = Art::Sparks;
    profile
};

const fn dust_shaft() -> Beam {
    let mut beam = shaft();
    beam.x = 0.15;
    beam.y = -0.05;
    beam.angle = 1.05;
    beam.spread = 0.1;
    beam.width = 0.1;
    beam.length = 1.5;
    beam.intensity = 0.8;
    beam.streaks = 0.4;
    beam
}

const DUST_BEAMS: [Beam; 1] = [dust_shaft()];
const DUST: Profile = {
    let mut profile = base("motes-dust", "motes", Mood::Quiet, &DUST_BEAMS);
    profile.fog_density = 0.8;
    profile.fog_ambient = 0.015;
    profile.glare = [0.0, 0.0, 0.0, 0.0, 0.0];
    profile.mote_count = 420;
    profile.mote_gain = 1.6;
    profile.mote_swirl = 0.035;
    profile.mote_size = [0.003, 0.012];
    profile.mote_drift = [0.002, -0.004];
    profile.mote_period = 14.0;
    profile.decay = 0.7;
    profile.decay_delay = 1.4;
    profile.starfall = true;
    profile.rain = 30;
    profile.art = Art::Sparks;
    profile
};

const fn firefly_shaft() -> Beam {
    let mut beam = shaft();
    beam.spread = 0.05;
    beam.intensity = 0.15;
    beam
}

const FIREFLY_BEAMS: [Beam; 1] = [firefly_shaft()];
const FIREFLY: Profile = {
    let mut profile = base("motes-firefly", "motes", Mood::Quiet, &FIREFLY_BEAMS);
    profile.fog_density = 0.6;
    profile.fog_ambient = 0.02;
    profile.glare = [0.0, 0.0, 0.0, 0.0, 0.0];
    profile.mote_count = 90;
    profile.mote_gain = 0.8;
    profile.mote_ambient = 0.55;
    profile.mote_swirl = 0.06;
    profile.mote_size = [0.008, 0.022];
    profile.mote_drift = [0.003, -0.003];
    profile.mote_period = 11.0;
    profile.mote_twinkle = 0.9;
    profile.front_count = 14;
    profile.front_ambient = 0.12;
    profile.front_twinkle = 0.7;
    profile.region_cy = 0.5;
    profile.region_w = 0.5;
    profile.region_h = 0.62;
    profile.typography = Type::Vertical;
    profile.decay = 0.6;
    profile.decay_delay = 1.6;
    profile.art = Art::Sparks;
    profile.starfall = true;
    profile.rain = 30;
    profile
};

const fn lamp() -> Beam {
    let mut beam = shaft();
    beam.spread = 0.08;
    beam.width = 0.03;
    beam.length = 1.3;
    beam.softness = 0.55;
    beam.intensity = 1.0;
    beam.streaks = 0.4;
    beam.streak_freq = 6.0;
    beam.core = 0.6;
    beam.sway = [0.02, 12.0, 0.0];
    beam
}

const fn lamp_pair(phase: f32) -> Beam {
    let mut beam = lamp();
    beam.sway = [0.03, 9.0, phase];
    beam
}

const SPOT_BEAMS: [Beam; 1] = [lamp()];
const SPOT_AIM: [([f32; 2], [f32; 2]); 1] = [([0.14, 0.02], [0.5, 0.56])];
const SPOT: Profile = {
    let mut profile = base("stage-spot", "stage", Mood::Neutral, &SPOT_BEAMS);
    profile.aim = &SPOT_AIM;
    profile.fog_density = 1.25;
    profile.fog_tyndall = 0.08;
    profile.fog_scale = 2.0;
    profile.fog_drift_x = 0.01;
    profile.fog_drift_y = -0.015;
    profile.fog_warp = 1.1;
    profile.fog_ambient = 0.035;
    profile.glare = [0.14, 0.02, 0.08, 0.9, 0.5];
    profile.mote_count = 200;
    profile.art = Art::Sparks;
    profile
};

const STAGE_CROSS_BEAMS: [Beam; 2] = [lamp_pair(0.0), lamp_pair(0.5)];
const STAGE_CROSS_AIM: [([f32; 2], [f32; 2]); 2] = [([0.06, 0.02], [0.5, 0.56]), ([0.94, 0.02], [0.5, 0.56])];
const STAGE_CROSS: Profile = {
    let mut profile = base("stage-cross", "stage", Mood::Loud, &STAGE_CROSS_BEAMS);
    profile.aim = &STAGE_CROSS_AIM;
    profile.fog_density = 1.25;
    profile.fog_tyndall = 0.08;
    profile.fog_scale = 2.0;
    profile.fog_drift_x = 0.01;
    profile.fog_drift_y = -0.015;
    profile.fog_warp = 1.1;
    profile.fog_ambient = 0.035;
    profile.glare = [0.5, 0.56, 0.06, 0.6, 0.6];
    profile.mote_count = 200;
    profile.typography = Type::Crossed;
    profile.burst = 0.3;
    profile.burst_mode = 1;
    profile.burst_every = 2;
    profile.burst_offset = 1;
    profile.burst_size = 3.4;
    profile.burst_tint = [1.0, 0.75, 0.5];
    profile.decay = 1.3;
    profile.decay_delay = 0.7;
    profile.art = Art::Sparks;
    profile
};

pub const PROFILES: &[Profile] = &[
    SHAFT,
    FAN_PROFILE,
    HALO,
    DRIFT,
    TWIN,
    RAIN,
    WELL,
    CROWN,
    DESCENT,
    EMBER,
    BLINDS,
    CROSS,
    super::catalog::LATTICE_ARCH,
    super::catalog::LATTICE_ROSE,
    super::catalog::LATTICE_SLIT,
    super::catalog::LATTICE_SHOJI,
    super::catalog::LATTICE_GRILLE,
    super::catalog::LATTICE_SWEEP,
    super::catalog::LATTICE_KOMOREBI,
    super::catalog::LATTICE_NAVE,
    super::catalog::PRISM_SPLIT,
    super::catalog::PRISM_BAND,
    super::catalog::PRISM_FRINGE,
    super::catalog::PRISM_SHARD,
    super::catalog::PRISM_LINES,
    super::catalog::PRISM_MERGE,
    super::catalog::PRISM_CUBE,
    super::catalog::PRISM_WHEEL,
    super::catalog::PRISM_DROPLET,
    super::catalog::PRISM_SWEEP,
    super::catalog::CAUSTIC_POOL,
    super::catalog::CAUSTIC_CUP,
    super::catalog::CAUSTIC_RIPPLE,
    super::catalog::CAUSTIC_SURFACE,
    super::catalog::CAUSTIC_RING,
    super::catalog::CAUSTIC_BURN,
    super::catalog::CAUSTIC_SHIMMER,
    super::catalog::CAUSTIC_CRYSTAL,
    super::catalog::CAUSTIC_DRIFT,
    super::catalog::CAUSTIC_STORM,
    super::catalog::OPTICS_CONVEX,
    super::catalog::OPTICS_CONCAVE,
    super::catalog::OPTICS_MIRROR,
    super::catalog::OPTICS_REFRACT,
    super::catalog::OPTICS_BENCH,
    super::catalog::OPTICS_PERISCOPE,
    super::catalog::OPTICS_FIBER,
    super::catalog::OPTICS_PINHOLE,
    super::catalog::OPTICS_TELESCOPE,
    super::catalog::OPTICS_RAYTRACE,
    super::more::WAVE_SLITS,
    super::more::WAVE_NEWTON,
    super::more::WAVE_AIRY,
    super::more::WAVE_GRATING,
    super::more::WAVE_SOURCES,
    super::more::WAVE_MOIRE,
    super::more::WAVE_FILM,
    super::more::WAVE_POLAR,
    super::more::WAVE_STANDING,
    super::more::WAVE_HOLO,
    super::more::BOTANY_SPROUT,
    super::more::BOTANY_VEINS,
    super::more::BOTANY_CANOPY,
    super::more::BOTANY_FERN,
    super::more::BOTANY_SEED,
    super::more::BOTANY_BLOOM,
    super::more::BOTANY_VINE,
    super::more::BOTANY_CHLORO,
    super::more::BOTANY_FORMULA,
    super::more::BOTANY_POLLEN,
    super::more::ASTRAL_ECLIPSE,
    super::more::ASTRAL_ORBIT,
    super::more::ASTRAL_TRAIL,
    super::more::ASTRAL_SEXTANT,
    super::more::ASTRAL_CONSTELLATION,
    super::more::ASTRAL_CORONA,
    super::more::ASTRAL_MOON,
    super::more::ASTRAL_NEBULA,
    super::more::ASTRAL_ARMILLARY,
    super::more::ASTRAL_METEOR,
    SPOT,
    STAGE_CROSS,
    super::more::STAGE_SEARCH,
    super::more::STAGE_FOOT,
    super::more::STAGE_BACK,
    super::more::STAGE_PIN,
    super::more::STAGE_STROBE,
    super::more::STAGE_CURTAIN,
    super::more::STAGE_LASER,
    super::more::STAGE_PROJECTOR,
    DUST,
    FIREFLY,
    super::more::MOTES_SMOKE,
    super::more::MOTES_EMBERS,
    super::more::MOTES_SNOW,
    super::more::MOTES_BOKEH,
    super::more::MOTES_MIST,
    super::more::MOTES_SWARM,
    super::more::MOTES_DISSOLVE,
    super::more::MOTES_AURORA,
];

const _: [(); 100] = [(); PROFILES.len()];

pub fn beams_for(profile: &Profile, aspect: f32, seed: u32) -> ([Beam; MAX_BEAMS], usize) {
    if let Some(built) = super::catalog::special(profile.kind, aspect, seed) {
        return built;
    }
    let mut out = [shaft(); MAX_BEAMS];
    let count = profile.beams.len().min(MAX_BEAMS);
    for (index, beam) in profile.beams.iter().take(count).enumerate() {
        let mut next = *beam;
        if let Some(aim) = profile.aim.get(index) {
            next.x = aim.0[0];
            next.y = aim.0[1];
            let dx = (aim.1[0] - aim.0[0]) * aspect;
            let dy = aim.1[1] - aim.0[1];
            next.angle = dy.atan2(dx);
        }
        out[index] = next;
    }
    (out, count)
}

pub const BRIDGE_FAMILIES: &[&str] = &["motes", "astral", "zenith"];
