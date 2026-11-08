// Diagrams, stars, motes and crosses for one lumiere frame. Meshes are rebuilt when the shot changes, not per frame.

use folia_core::{Line, Lyrics};

use super::art::{self, Mesh, Rng};
use super::field::View;
use super::gpu::{DECOR_BYTES, MAX_PARTICLES};
use super::profiles::{PROFILES, Profile};
use super::program::{self, Program, Shot, Span};
use super::tuning::Tuning;

const TAU: f32 = std::f32::consts::TAU;
const MAX_BURSTS: usize = 14;
const STAR: f32 = 0.0;
const RAIN: f32 = 1.0;
const MOTE: f32 = 2.0;
const NODE: f32 = 3.0;
const FRONT: f32 = 4.0;
const STAR_TRAIL: f32 = 5.0;
const RAIN_TRAIL: f32 = 6.0;

#[derive(Clone, Copy, PartialEq)]
struct GeoKey {
    shot: u32,
    previous: u32,
    lead: u32,
    aspect: u32,
    seed: u32,
    amount: u32,
    opening: u32,
    flags: u32,
}

#[derive(Clone, Copy, PartialEq)]
struct FrontKey {
    lead: u32,
    aspect: u32,
    seed: u32,
    flags: u32,
}

/// A visible grapheme of a line and when it is sung (`glyphTimes`).
struct Glyph {
    grapheme: u16,
    start: f32,
}

struct Trigger {
    line: u16,
    grapheme: u16,
    time: f32,
    size: f32,
    dy: f32,
    tint: [f32; 3],
}

/// One cross the lyric layout can place. The age lives in the uniform, so the positions can move without a new mesh.
#[derive(Clone, Copy)]
pub(super) struct BurstRef {
    pub line: usize,
    pub grapheme: usize,
    pub time: f32,
    pub size: f32,
    pub dy: f32,
    pub tint: [f32; 3],
}

#[derive(Default)]
pub(super) struct Store {
    mesh: Mesh,
    lines: Vec<f32>,
    particles: Vec<f32>,
    front: Vec<f32>,
    scratch: Vec<Glyph>,
    triggers: Vec<Trigger>,
    key: Option<GeoKey>,
    front_key: Option<FrontKey>,
    burst_seed: Option<u32>,
    /// Where the particles change layer: starfall ends at `[0]`, the line-art nodes at `[1]`, motes follow.
    split: [u32; 2],
}

impl Store {
    pub(super) fn ensure(&mut self, lyric: u32, lyrics: &Lyrics, program: &Program) {
        let key = lyric ^ program::hash_text(2_166_136_261, &program.seed);
        if self.burst_seed == Some(key) {
            return;
        }
        self.triggers.clear();
        plan_bursts(&mut self.triggers, &mut self.scratch, lyrics, program);
        self.triggers.sort_by(|left, right| left.time.partial_cmp(&right.time).unwrap_or(std::cmp::Ordering::Equal));
        self.burst_seed = Some(key);
    }

    pub(super) fn active(&self, time: f64) -> [Option<BurstRef>; MAX_BURSTS] {
        let mut out = [None; MAX_BURSTS];
        let mut slot = 0;
        for trigger in &self.triggers {
            let age = time as f32 - trigger.time;
            if !(0.0..=0.6).contains(&age) {
                continue;
            }
            out[slot] = Some(BurstRef {
                line: usize::from(trigger.line),
                grapheme: usize::from(trigger.grapheme),
                time: trigger.time,
                size: trigger.size,
                dy: trigger.dy,
                tint: trigger.tint,
            });
            slot += 1;
            if slot == MAX_BURSTS {
                break;
            }
        }
        out
    }

    pub(super) fn rebuild(
        &mut self,
        program: &Program,
        view: &View,
        time: f64,
        tuning: Tuning,
    ) -> (Option<Vec<u8>>, Option<Vec<u8>>) {
        let key = geo_key(program, view, time, tuning);
        if self.key == Some(key) {
            return (None, None);
        }
        self.lines.clear();
        self.particles.clear();
        self.split = [0, 0];
        if !tuning.text_only {
            fill_stars(&mut self.particles, program, view, time, tuning);
            fill_rain(&mut self.particles, program, view, time);
            self.split[0] = (self.particles.len() / 12) as u32;
            if tuning.line_art {
                self.append_art(program, view, time, tuning);
            }
            self.split[1] = (self.particles.len() / 12) as u32;
            fill_dust(&mut self.particles, program, view, time, tuning, MOTE);
        }
        self.key = Some(key);
        (Some(pack(&self.lines)), Some(pack(&self.particles)))
    }

    /// Particle counts that end the starfall and the line-art nodes, in buffer order.
    pub(super) fn split(&self) -> [u32; 2] {
        self.split
    }

    pub(super) fn front(&mut self, program: &Program, view: &View, time: f64, tuning: Tuning) -> Option<Vec<u8>> {
        let paragraph = unit_paragraph(program, time);
        let lead = unit_lead(program, time);
        let key = FrontKey {
            lead: lead as u32,
            aspect: (view.width / view.height.max(1.0)).to_bits(),
            seed: program::hash_text(2_166_136_261, &program.seed) ^ u32::from(paragraph),
            flags: u32::from(tuning.front_bokeh) | (u32::from(tuning.text_only) << 1),
        };
        if self.front_key == Some(key) {
            return None;
        }
        self.front.clear();
        let profile = &PROFILES[lead];
        if tuning.front_bokeh && !tuning.text_only && profile.front && profile.front_count > 0 {
            fill_dust_profile(
                &mut self.front,
                profile,
                view.width / view.height.max(1.0),
                program::unit_stream(program, paragraph, b":front"),
                FRONT,
                usize::from(profile.front_count),
            );
        }
        self.front_key = Some(key);
        Some(pack(&self.front))
    }

    fn append_art(&mut self, program: &Program, view: &View, time: f64, tuning: Tuning) {
        let index = program.active(time);
        let aspect = view.width / view.height.max(1.0);
        self.append_shot(program, index, aspect, tuning);
        if let Some(previous) = index.checked_sub(1)
            && same_span(program, previous, index)
        {
            self.append_shot(program, previous, aspect, tuning);
        }
    }

    fn append_shot(&mut self, program: &Program, index: usize, aspect: f32, tuning: Tuning) {
        let Some(shot) = program.shots.get(index) else { return };
        let profile = &PROFILES[shot.profile];
        let Some(window) = art_window(program, index, tuning) else { return };
        self.mesh.clear();
        art::build(profile, aspect, art::art_seed(program, index, profile.kind), &mut self.mesh);
        art::write_lines(&self.mesh, window.begin, window.end, window.gain, &mut self.lines);
        for node in &self.mesh.nodes {
            if self.particles.len() / 12 >= MAX_PARTICLES as usize {
                break;
            }
            push(
                &mut self.particles,
                [node.x, node.y, node.size, node.delay],
                [node.phase, window.begin, window.end, window.gain],
                [0.0, NODE, 0.0, 0.0],
            );
        }
    }
}

pub(super) fn uniform(program: &Program, time: f64, tuning: Tuning, mote_lift: f32) -> [u8; DECOR_BYTES] {
    let mut out = [0_u8; DECOR_BYTES];
    let lead = &PROFILES[unit_lead(program, time)];
    write4(&mut out, 0, [lead.mote_drift[0], lead.mote_drift[1], lead.mote_swirl, lead.mote_period]);
    write4(&mut out, 1, [lead.mote_twinkle, lead.mote_ambient, lead.mote_gain, mote_lift]);
    write4(&mut out, 2, [lead.front_drift[0], lead.front_drift[1], lead.front_swirl, lead.front_period]);
    write4(&mut out, 3, [lead.front_twinkle, lead.front_ambient, lead.front_gain, 1.0]);
    let clock = star_clock(program, time, tuning);
    write4(&mut out, 4, [clock.opening, clock.brightness, lead.rain_speed, clock.local]);
    for slot in 0..MAX_BURSTS {
        write4(&mut out, 6 + slot, [0.0, 0.0, -1.0, 0.0]);
    }
    out
}

pub(super) fn stamp(out: &mut [u8; DECOR_BYTES], slot: usize, at: [f32; 3], age: f32, tint: [f32; 3], light: [f32; 3]) {
    if slot >= MAX_BURSTS {
        return;
    }
    let color = [
        light[0] + (tint[0] - light[0]) * 0.85,
        light[1] + (tint[1] - light[1]) * 0.85,
        light[2] + (tint[2] - light[2]) * 0.85,
    ];
    write4(out, 6 + slot, [at[0], at[1], age, at[2]]);
    write4(out, 20 + slot, [color[0], color[1], color[2], 1.0]);
}

struct Clock {
    local: f32,
    opening: f32,
    brightness: f32,
    on: bool,
}

fn star_clock(program: &Program, time: f64, tuning: Tuning) -> Clock {
    let lead = &PROFILES[unit_lead(program, time)];
    let Some((start, end, opening_on)) = clock_window(program, time) else {
        return Clock {
            local: time as f32,
            opening: lead.star_opening,
            brightness: lead.star_brightness,
            on: lead.starfall,
        };
    };
    let duration = (end - start) as f32;
    let can = opening_on && duration >= 2.0 && !tuning.text_only && lead.starfall;
    let opening = if can { lead.star_opening.min((duration * 0.4).max(0.8)) } else { lead.star_opening };
    let offset = if lead.starfall && !can { opening + 20.0 } else { 0.0 };
    Clock {
        local: (time - start) as f32 + offset,
        opening,
        brightness: lead.star_brightness,
        on: lead.starfall && !tuning.text_only,
    }
}

fn fill_stars(out: &mut Vec<f32>, program: &Program, view: &View, time: f64, tuning: Tuning) {
    let clock = star_clock(program, time, tuning);
    let lead = &PROFILES[unit_lead(program, time)];
    if !clock.on || lead.star_count == 0 {
        return;
    }
    let aspect = view.width / view.height.max(1.0);
    let mut rng = Rng(program::unit_stream(program, unit_paragraph(program, time), b":starfall"));
    let count = usize::from(lead.star_count).min(640);
    for _ in 0..count {
        if out.len() / 12 + 2 > MAX_PARTICLES as usize {
            break;
        }
        let bright = rng.next() < 0.08;
        let x = rng.next() * aspect;
        let y = 0.02 + rng.next().powf(1.7) * 0.9;
        let from_y = -0.05 - rng.next() * 0.25;
        let delay = clock.opening * (0.02 + 0.7 * rng.next().powf(1.4));
        let fall = 0.45 + rng.next() * 0.7;
        let size = if bright { 0.018 + rng.next() * 0.02 } else { 0.003 + rng.next() * 0.006 };
        let phase = rng.next() * TAU;
        let freq = 0.4 + rng.next() * 1.8;
        let mark = if bright { 1.0 } else { 0.0 };
        let body = [x, y, from_y, delay];
        let look = [fall, phase, freq, mark];
        push(out, body, look, [size, STAR, 0.0, 0.0]);
        push(out, body, look, [size, STAR_TRAIL, 0.0, 0.0]);
    }
}

fn fill_rain(out: &mut Vec<f32>, program: &Program, view: &View, time: f64) {
    let lead = &PROFILES[unit_lead(program, time)];
    if !lead.starfall || lead.rain == 0 {
        return;
    }
    let aspect = view.width / view.height.max(1.0);
    let mut rng = Rng(program::unit_stream(program, unit_paragraph(program, time), b":starfall"));
    // The rain stream continues after the stars, so consume the star draws first.
    for _ in 0..usize::from(lead.star_count).min(640) * 9 {
        let _ = rng.next();
    }
    let count = usize::from(lead.rain).min(256);
    for _ in 0..count {
        if out.len() / 12 + 2 > MAX_PARTICLES as usize {
            break;
        }
        let spread = (rng.next() + rng.next() + rng.next() - 1.5) / 1.5;
        let x = aspect * (0.5 + spread * lead.rain_spread * 0.5);
        let period = (1.25 / lead.rain_speed.max(0.01)) * (0.7 + rng.next() * 0.6);
        let phase = rng.next();
        let size = 0.003 + rng.next() * 0.005;
        let sway = (rng.next() - 0.5) * 0.02;
        let body = [x, period, phase, sway];
        push(out, body, [size, 0.0, 0.0, 0.0], [0.0, RAIN, 0.0, 0.0]);
        push(out, body, [size, 0.0, 0.0, 0.0], [0.0, RAIN_TRAIL, 0.0, 0.0]);
    }
}

fn fill_dust(out: &mut Vec<f32>, program: &Program, view: &View, time: f64, tuning: Tuning, mode: f32) {
    let lead = &PROFILES[unit_lead(program, time)];
    let count = (f32::from(lead.mote_count) * tuning.mote_amount * tuning.mote_scale()).round().max(0.0) as usize;
    fill_dust_profile(
        out,
        lead,
        view.width / view.height.max(1.0),
        program::unit_stream(program, unit_paragraph(program, time), b":motes"),
        mode,
        count.min(512),
    );
}

fn fill_dust_profile(out: &mut Vec<f32>, profile: &Profile, aspect: f32, seed: u32, mode: f32, count: usize) {
    let mut rng = Rng(seed);
    let margin = 0.08;
    let size = if mode == FRONT { profile.front_size } else { profile.mote_size };
    for _ in 0..count {
        if out.len() / 12 + 1 > MAX_PARTICLES as usize {
            break;
        }
        let depth = rng.next();
        let bx = rng.next() * (aspect + margin * 2.0) - margin;
        let by = rng.next() * (1.0 + margin * 2.0) - margin;
        let phase_a = rng.next() * TAU;
        let phase_b = rng.next() * TAU;
        let freq_a = 0.6 + rng.next() * 0.8;
        let freq_b = 0.5 + rng.next() * 0.9;
        let twinkle_phase = rng.next() * TAU;
        let twinkle_freq = 0.8 + rng.next() * 2.2;
        let radius = size[0] + (size[1] - size[0]) * depth * depth;
        push(
            out,
            [bx, by, depth, phase_a],
            [phase_b, freq_a, freq_b, twinkle_phase],
            [radius, mode, twinkle_freq, 0.0],
        );
    }
}

fn plan_bursts(out: &mut Vec<Trigger>, scratch: &mut Vec<Glyph>, lyrics: &Lyrics, program: &Program) {
    for (shot_index, shot) in program.shots.iter().enumerate() {
        let profile = &PROFILES[shot.profile];
        if profile.burst_mode == 0 {
            continue;
        }
        let covered = usize::from(shot.line_count).min(shot.lines.len());
        for line_index in 0..lyrics.lines.len() {
            let every = usize::from(profile.burst_every.max(1));
            let offset = usize::from(profile.burst_offset);
            if line_index < offset || (line_index - offset) % every != 0 {
                continue;
            }
            let covered_line = shot.lines[..covered].contains(&line_index);
            scratch.clear();
            if covered_line {
                collect_glyphs(&lyrics.lines[line_index], scratch);
            }
            if scratch.is_empty() {
                continue;
            }
            push_line(out, scratch, program, shot_index, line_index, profile);
        }
    }
}

fn push_line(
    out: &mut Vec<Trigger>,
    glyphs: &[Glyph],
    program: &Program,
    shot_index: usize,
    line: usize,
    profile: &Profile,
) {
    let mut rng = Rng(program::burst_state(program, shot_index, line));
    let mut picked = Vec::new();
    for (index, _) in glyphs.iter().enumerate() {
        if rng.next() < profile.burst {
            picked.push(index);
        }
    }
    if picked.is_empty() {
        let index = (rng.next() * glyphs.len() as f32).floor() as usize;
        picked.push(index.min(glyphs.len() - 1));
    }
    let sweep = glyphs[0].start;
    for (order, index) in picked.iter().copied().enumerate() {
        let glyph = &glyphs[index];
        let time = if profile.burst_mode == 2 { sweep + order as f32 * 0.07 } else { glyph.start };
        out.push(Trigger {
            line: line as u16,
            grapheme: glyph.grapheme,
            time,
            size: (0.7 + rng.next() * 0.6) * profile.burst_size,
            dy: (rng.next() - 0.5) * 0.9,
            tint: profile.burst_tint,
        });
    }
}

fn collect_glyphs(line: &Line, out: &mut Vec<Glyph>) {
    let timings = folia_lyrics::grapheme_timeline(line);
    for (index, (range, timing)) in folia_lyrics::graphemes(&line.text).into_iter().zip(timings).enumerate() {
        if !line.text[range].trim().is_empty() {
            out.push(Glyph { grapheme: index as u16, start: timing.start as f32 });
        }
    }
}

struct Window {
    begin: f32,
    end: f32,
    gain: f32,
}

struct Edge {
    start: f64,
    end: f64,
    opening: bool,
    first: bool,
    last: bool,
}

fn art_window(program: &Program, index: usize, tuning: Tuning) -> Option<Window> {
    let shot = program.shots.get(index)?;
    let profile = &PROFILES[shot.profile];
    let lead = if program.seamless { program.lead() } else { lead_profile(program, shot.paragraph) };
    let (begin, end) = if program.seamless {
        let start = program.spans.first().map(|span| span.start).unwrap_or(shot.start);
        let end_time = program.spans.last().map(|span| span.end).unwrap_or(shot.end);
        let opening = program.spans.first().map(|span| span.opening).unwrap_or(true);
        let edge = Edge { start, end: end_time, opening, first: index == 0, last: index + 1 == program.shots.len() };
        shot_window(shot, edge, lead, tuning)
    } else {
        match program.spans.get(span_of_shot(program, index)) {
            Some(span) => {
                let span_index = span_of_shot(program, index);
                let edge = Edge {
                    start: span.start,
                    end: span.end,
                    opening: span.opening,
                    first: is_edge(program, index, span_index, true),
                    last: is_edge(program, index, span_index, false),
                };
                shot_window(shot, edge, lead, tuning)
            }
            None => (shot.start as f32 - 0.6, shot.end as f32),
        }
    };
    Some(Window { begin, end, gain: profile.art_gain })
}

fn shot_window(shot: &Shot, edge: Edge, lead: usize, tuning: Tuning) -> (f32, f32) {
    let profile = &PROFILES[lead];
    let duration = (edge.end - edge.start) as f32;
    let can = edge.opening && duration >= 2.0 && !tuning.text_only && profile.starfall;
    let ignition = if can { profile.star_opening.min((duration * 0.4).max(0.8)) * 0.45 } else { 0.0 };
    let begin = if edge.first { edge.start as f32 + ignition - 0.6 } else { shot.start as f32 - 0.6 };
    let stop = if edge.last { 1.0e9 } else { shot.end as f32 };
    (begin, stop)
}

fn is_edge(program: &Program, index: usize, span_index: usize, first: bool) -> bool {
    let others = if first { 0..index } else { index + 1..program.shots.len() };
    others.into_iter().all(|other| span_of_shot(program, other) != span_index)
}

fn same_span(program: &Program, left: usize, right: usize) -> bool {
    program.seamless || span_of_shot(program, left) == span_of_shot(program, right)
}

fn span_of_shot(program: &Program, index: usize) -> usize {
    program.shots.get(index).map(|shot| span_index(program, shot.start)).unwrap_or(0)
}

fn span_at(program: &Program, time: f64) -> Option<&Span> {
    program.spans.get(span_index(program, time))
}

fn span_index(program: &Program, time: f64) -> usize {
    program.spans.iter().rposition(|span| span.start <= time).unwrap_or(0)
}

fn geo_key(program: &Program, view: &View, time: f64, tuning: Tuning) -> GeoKey {
    let shot = program.active(time) as u32;
    let previous = program
        .active(time)
        .checked_sub(1)
        .filter(|index| same_span(program, *index, shot as usize))
        .map(|index| index as u32)
        .unwrap_or(u32::MAX);
    let clock = star_clock(program, time, tuning);
    GeoKey {
        shot,
        previous,
        lead: unit_lead(program, time) as u32,
        aspect: (view.width / view.height.max(1.0)).to_bits(),
        seed: program::hash_text(2_166_136_261, &program.seed),
        amount: tuning.mote_amount.to_bits(),
        opening: clock.opening.to_bits(),
        flags: u32::from(tuning.line_art) | (u32::from(tuning.text_only) << 1) | (u32::from(clock.on) << 2),
    }
}

fn push(out: &mut Vec<f32>, a: [f32; 4], b: [f32; 4], c: [f32; 4]) {
    out.extend_from_slice(&a);
    out.extend_from_slice(&b);
    out.extend_from_slice(&c);
}

fn pack(floats: &[f32]) -> Vec<u8> {
    let mut out = Vec::with_capacity(floats.len() * 4);
    for value in floats {
        out.extend_from_slice(&value.to_le_bytes());
    }
    out
}

fn write4(out: &mut [u8], index: usize, value: [f32; 4]) {
    let start = index * 16;
    if start + 16 > out.len() {
        return;
    }
    for (slot, channel) in value.iter().enumerate() {
        out[start + slot * 4..start + slot * 4 + 4].copy_from_slice(&channel.to_le_bytes());
    }
}

fn clock_window(program: &Program, time: f64) -> Option<(f64, f64, bool)> {
    if program.seamless {
        let first = program.spans.first()?;
        let last = program.spans.last()?;
        return Some((first.start, last.end, first.opening));
    }
    let span = span_at(program, time)?;
    Some((span.start, span.end, span.opening))
}

pub(super) fn unit_lead(program: &Program, time: f64) -> usize {
    if program.seamless { program.lead() } else { lead_profile(program, paragraph_of(program, time)) }
}

pub(super) fn unit_paragraph(program: &Program, time: f64) -> u16 {
    if program.seamless { 0 } else { paragraph_of(program, time) }
}

fn paragraph_of(program: &Program, time: f64) -> u16 {
    program.shots.get(program.active(time)).map(|shot| shot.paragraph).unwrap_or(0)
}

fn lead_profile(program: &Program, paragraph: u16) -> usize {
    program
        .shots
        .iter()
        .find(|shot| shot.paragraph == paragraph && shot.line_count > 0)
        .or_else(|| program.shots.iter().find(|shot| shot.paragraph == paragraph))
        .map(|shot| shot.profile)
        .unwrap_or(0)
}
