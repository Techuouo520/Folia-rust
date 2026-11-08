// One lumiere frame: resolved beams and the uniform block the light-field pass uploads.

use folia_core::{AnimationIntensity, Theme};
use iced::Size;

use super::beams::{Drive, MAX_BEAMS, Resolved, smooth};
use super::palette::{self, Palette};
use super::profiles::{PROFILES, beams_for};
use super::program::Program;
use super::tuning::Tuning;

pub const UNIFORM_BYTES: usize = 832;
const HANDOFF: f64 = 0.9;
const GLARE_MOVE: f64 = 1.6;
const IGNITION: f32 = 3.2 * 0.45;

pub struct View {
    pub beams: [Resolved; MAX_BEAMS],
    pub count: u32,
    pub width: f32,
    pub height: f32,
    pub time: f32,
    pub fog_density: f32,
    pub fog_tyndall: f32,
    pub fog_scale: f32,
    pub fog_warp: f32,
    pub fog_drift: [f32; 2],
    pub fog_ambient: f32,
    pub octaves: f32,
    pub color: [f32; 3],
    pub glare: [f32; 4],
    pub glare_streak: f32,
    pub palette: Palette,
    /// Theme background with the dark field already over it. The shader treats this as opaque.
    pub base: [f32; 3],
    pub camera: Camera,
    pub profile: usize,
    pub previous: Option<usize>,
    pub lead: usize,
    pub handoff: f32,
    pub enter: f32,
}

pub struct Camera {
    pub x: f32,
    pub y: f32,
    pub pivot_x: f32,
    pub pivot_y: f32,
    pub scale: f32,
    pub rotation: f32,
}

pub fn prepare(
    program: &Program,
    time: f64,
    size: Size,
    theme: &Theme,
    tuning: Tuning,
    heard: super::audio::Heard,
) -> View {
    let width = size.width.max(1.0);
    let height = size.height.max(1.0);
    let aspect = width / height;
    let index = program.active(time);
    let shot = program.shots.get(index);
    let profile_index = shot.map_or(0, |shot| shot.profile);
    let since = shot.map_or(0.0, |shot| time - shot.start);
    let handoff = if index > 0 { smooth(0.0, 1.0, (since / HANDOFF) as f32) } else { 1.0 };
    let glare_k = if index > 0 { ease_cubic((since / GLARE_MOVE) as f32) } else { 1.0 };
    let enter = enter(program, time);
    let colors = palette::palette(theme, tuning.theme_color_mix);
    let bass = heard.bass.min(1.0) * tuning.audio_response;
    let drive = |weight: f32, local: f32| Drive {
        intensity: enter * tuning.light_intensity * weight,
        bass,
        color: colors.light,
        local,
    };
    let mut gathered = [Resolved::ZERO; MAX_BEAMS * 2];
    let mut count = 0;
    if shot.is_some() {
        count = push_beams(
            &mut gathered,
            count,
            profile_index,
            light_seed(program, index),
            time,
            aspect,
            drive(handoff, since as f32),
        );
    }
    if handoff < 1.0
        && index > 0
        && let Some(previous) = program.shots.get(index - 1)
    {
        let local = (time - previous.start) as f32;
        count = push_beams(
            &mut gathered,
            count,
            previous.profile,
            light_seed(program, index - 1),
            time,
            aspect,
            drive(1.0 - handoff, local),
        );
    }
    gathered[..count].sort_by(|a, b| b.intensity.partial_cmp(&a.intensity).unwrap_or(std::cmp::Ordering::Equal));
    let kept = count.min(MAX_BEAMS);
    let mut beams = [Resolved::ZERO; MAX_BEAMS];
    beams[..kept].copy_from_slice(&gathered[..kept]);
    let current = &PROFILES[profile_index];
    let previous = index.checked_sub(1).and_then(|index| program.shots.get(index)).map(|shot| &PROFILES[shot.profile]);
    let bridge = shot.is_some_and(|shot| shot.bridge);
    let fog = blend_fog(previous, current, handoff, tuning, bridge);
    let current_seed = light_seed(program, index);
    let previous_seed = if index > 0 { light_seed(program, index - 1) } else { 0 };
    let glare =
        blend_glare(previous, current, glare_k, aspect, enter * tuning.light_intensity, previous_seed, current_seed);
    View {
        beams,
        count: kept as u32,
        width,
        height,
        time: time as f32,
        fog_density: fog[0],
        fog_tyndall: fog[1],
        fog_scale: fog[2] * (1.0 + 0.2 * heard.power.min(1.0) * tuning.audio_response),
        fog_warp: fog[3],
        fog_drift: [fog[4], fog[5]],
        fog_ambient: fog[6],
        octaves: tuning.octaves(),
        color: colors.light,
        glare: [glare[0], glare[1], glare[2], glare[3]],
        glare_streak: glare[4],
        palette: colors,
        base: base_color(theme, tuning.dark_field),
        camera: camera(program, time, size, theme),
        profile: profile_index,
        previous: index.checked_sub(1).and_then(|slot| program.shots.get(slot)).map(|shot| shot.profile),
        lead: program.lead(),
        handoff,
        enter,
    }
}

pub fn pack(view: &View) -> [u8; UNIFORM_BYTES] {
    let mut out = [0u8; UNIFORM_BYTES];
    let mut cursor = 0;
    let component = |beam: Resolved, slot: usize| -> [f32; 4] {
        match slot {
            0 => [beam.ox, beam.oy, beam.dx, beam.dy],
            1 => [beam.half_width, beam.tan_spread, beam.softness, beam.length],
            2 => [beam.intensity, beam.streaks, beam.streak_freq, beam.streak_phase],
            3 => [beam.rgb[0], beam.rgb[1], beam.rgb[2], beam.core],
            4 => [beam.gobo as f32, beam.gobo_freq, beam.gobo_duty, beam.gobo_phase],
            _ => [beam.spectrum, beam.reach, 0.0, 0.0],
        }
    };
    for slot in 0..6 {
        for beam in view.beams {
            push(&mut out, &mut cursor, component(beam, slot));
        }
    }
    push(&mut out, &mut cursor, [view.width, view.height, view.time, view.count as f32]);
    push(&mut out, &mut cursor, [view.fog_density, view.fog_tyndall, view.fog_scale, view.fog_warp]);
    push(&mut out, &mut cursor, [view.fog_drift[0], view.fog_drift[1], view.fog_ambient, view.octaves]);
    push(&mut out, &mut cursor, [view.color[0], view.color[1], view.color[2], 0.0]);
    push(&mut out, &mut cursor, view.glare);
    push(&mut out, &mut cursor, [view.glare_streak, 0.0, 0.0, 0.0]);
    push(&mut out, &mut cursor, [view.color[0], view.color[1], view.color[2], 0.0]);
    push(&mut out, &mut cursor, backdrop(view));
    let aspect = view.width / view.height.max(1.0);
    let (source, fade) = module_source(view);
    let enabled = if source.caustic_scale > 0.0 { 1.0 } else { 0.0 };
    push(&mut out, &mut cursor, [source.caustic_scale, source.caustic_speed, source.caustic_in_beam * fade, enabled]);
    push(
        &mut out,
        &mut cursor,
        [
            source.caustic_floor[0] * aspect,
            source.caustic_floor[1],
            source.caustic_floor[2] * aspect,
            source.caustic_floor[3],
        ],
    );
    push(&mut out, &mut cursor, [source.caustic_floor[4] * fade * view.enter, 0.0, 0.0, 0.0]);
    let wave = if source.wave_mode > 0.5 { source.wave_motion[2] * fade * view.enter } else { 0.0 };
    push(&mut out, &mut cursor, [source.wave_mode, source.wave_motion[0], source.wave_motion[1], wave]);
    push(
        &mut out,
        &mut cursor,
        [source.wave_center[0] * aspect, source.wave_center[1], source.wave_center[2], source.wave_center[3]],
    );
    let lead = &PROFILES[view.lead];
    push(
        &mut out,
        &mut cursor,
        [lead.region_cx * aspect, lead.region_cy, lead.region_w * aspect * 0.5, lead.region_h * 0.5],
    );
    let shield = if source.wave_mode > 0.5 { source.wave_motion[3] } else { 0.0 };
    push(&mut out, &mut cursor, [shield, 0.0, 0.0, 0.0]);
    push(&mut out, &mut cursor, [view.camera.x, view.camera.y, view.camera.scale, view.camera.rotation]);
    let _ = cursor;
    out
}

fn backdrop(view: &View) -> [f32; 4] {
    [view.base[0], view.base[1], view.base[2], 1.0]
}

fn base_color(theme: &Theme, amount: f32) -> [f32; 3] {
    let dark = palette::dark_field(theme, amount);
    let bg = [
        f32::from(theme.background.r) / 255.0,
        f32::from(theme.background.g) / 255.0,
        f32::from(theme.background.b) / 255.0,
    ];
    [
        dark.rgb[0] * dark.alpha + bg[0] * (1.0 - dark.alpha),
        dark.rgb[1] * dark.alpha + bg[1] * (1.0 - dark.alpha),
        dark.rgb[2] * dark.alpha + bg[2] * (1.0 - dark.alpha),
    ]
}

fn push_beams(
    out: &mut [Resolved],
    mut count: usize,
    profile: usize,
    seed: u32,
    time: f64,
    aspect: f32,
    drive: Drive,
) -> usize {
    let profile = &PROFILES[profile];
    let (beams, n) = beams_for(profile, aspect, seed);
    for beam in beams.iter().take(n) {
        if count >= out.len() {
            break;
        }
        out[count] = super::beams::resolve(*beam, time as f32, aspect, drive);
        count += 1;
    }
    count
}

fn blend_fog(
    previous: Option<&super::profiles::Profile>,
    current: &super::profiles::Profile,
    handoff: f32,
    tuning: Tuning,
    bridge: bool,
) -> [f32; 7] {
    let sample = |profile: &super::profiles::Profile| {
        [
            profile.fog_density,
            profile.fog_tyndall,
            profile.fog_scale,
            profile.fog_warp,
            profile.fog_drift_x,
            profile.fog_drift_y,
            profile.fog_ambient,
        ]
    };
    let to = sample(current);
    let from = previous.map(sample).unwrap_or(to);
    let mut mixed = [0.0; 7];
    for (slot, value) in mixed.iter_mut().enumerate() {
        *value = from[slot] + (to[slot] - from[slot]) * handoff;
    }
    mixed[0] *= tuning.fog_density;
    mixed[6] *= tuning.fog_density * if bridge { 1.15 } else { 1.0 };
    mixed
}

fn blend_glare(
    previous: Option<&super::profiles::Profile>,
    current: &super::profiles::Profile,
    glare_k: f32,
    aspect: f32,
    scale: f32,
    previous_seed: u32,
    current_seed: u32,
) -> [f32; 5] {
    let place = |profile: &super::profiles::Profile, seed: u32| {
        let glare = super::catalog::glare(profile, aspect, seed);
        [glare[0] * aspect, glare[1], glare[2], glare[3], glare[4]]
    };
    let to = place(current, current_seed);
    let from = previous.map(|profile| place(profile, previous_seed)).unwrap_or(to);
    let mut mixed = [0.0; 5];
    for (slot, value) in mixed.iter_mut().enumerate() {
        *value = from[slot] + (to[slot] - from[slot]) * glare_k;
    }
    mixed[3] *= scale;
    mixed
}

fn camera(program: &Program, time: f64, size: Size, theme: &Theme) -> Camera {
    let profile = &PROFILES[program.lead()];
    let motion = match theme.intensity {
        AnimationIntensity::Calm => 0.6,
        AnimationIntensity::Chaotic => 1.4,
        AnimationIntensity::Normal => 1.0,
    };
    let progress = section_progress(program, time);
    let t = time as f32;
    let float_x = ((t * 0.21 + 0.4).sin() * 0.6 + (t * 0.53 + 1.9).sin() * 0.4) * 0.006 * motion;
    let float_y = ((t * 0.17 + 1.1).cos() * 0.6 + (t * 0.47 + 0.3).sin() * 0.4) * 0.006 * motion;
    Camera {
        x: size.width * (0.5 + profile.camera_drift[0] * progress + float_x),
        y: size.height * (0.5 + profile.camera_drift[1] * progress + float_y),
        pivot_x: size.width * 0.5,
        pivot_y: size.height * 0.5,
        scale: (1.0 + profile.camera_push * progress) * (1.0 + 0.008 * motion * (t * 0.31 + 0.7).sin()),
        rotation: 0.004 * motion * (t * 0.13 + 2.1).sin(),
    }
}

fn section_progress(program: &Program, time: f64) -> f32 {
    let mut index = 0;
    for (slot, span) in program.spans.iter().enumerate() {
        if span.start <= time {
            index = slot;
        }
    }
    let Some(span) = program.spans.get(index) else { return 0.0 };
    let linear = ((time - span.start) / (span.end - span.start).max(0.001)).clamp(0.0, 1.0) as f32;
    let eased = (1.0 - (linear * std::f32::consts::PI).cos()) * 0.5;
    if index.is_multiple_of(2) { eased } else { 1.0 - eased }
}

fn enter(program: &Program, time: f64) -> f32 {
    let span = program.spans.iter().rev().find(|span| span.start <= time);
    let Some(span) = span else { return 1.0 };
    if !span.opening {
        return 1.0;
    }
    let local = (time - span.start) as f32;
    smooth(0.0, 1.0, (local - IGNITION + 0.4) / 1.6)
}

fn ease_cubic(value: f32) -> f32 {
    let t = value.clamp(0.0, 1.0);
    if t < 0.5 { 4.0 * t * t * t } else { 1.0 - (-2.0 * t + 2.0).powi(3) / 2.0 }
}

fn push(out: &mut [u8], cursor: &mut usize, value: [f32; 4]) {
    for channel in value {
        let bytes = channel.to_le_bytes();
        let end = (*cursor + 4).min(out.len());
        let len = end - *cursor;
        out[*cursor..end].copy_from_slice(&bytes[..len]);
        *cursor = end;
    }
}

fn module_source(view: &View) -> (&super::profiles::Profile, f32) {
    let current = &PROFILES[view.profile];
    let previous = view.previous.map(|index| &PROFILES[index]).unwrap_or(current);
    if view.handoff < 0.5 { (previous, 1.0 - view.handoff * 2.0) } else { (current, view.handoff * 2.0 - 1.0) }
}

fn light_seed(program: &Program, index: usize) -> u32 {
    let Some(shot) = program.shots.get(index) else { return 0 };
    super::program::shot_stream(program, index, PROFILES[shot.profile].kind, b":light")
}
