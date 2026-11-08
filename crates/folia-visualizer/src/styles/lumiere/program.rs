// Shot program: split the lyric into paragraphs and shots, then cast a light position.

use folia_core::Lyrics;
use folia_lyrics::{
    ParagraphBoundary, ParagraphKind, ParagraphParams, StructureLine, classify, draft_paragraphs, gap_threshold,
    structure_lines,
};

use super::profiles::{BRIDGE_FAMILIES, Mood, PROFILES, Profile};

const MAX_SHOT: f64 = 7.0;
const MAX_LINES: usize = 2;
const LONG_LINE: f64 = 3.6;
const BRIDGE_GAP: f64 = 4.0;
const REOPEN_GAP: f64 = 2.5;
const RECENT: usize = 3;
/// Instrumental songs: the default length (folia passes no song duration) and the paragraph length, in seconds.
const INSTRUMENTAL_DURATION: f64 = 480.0;
const INSTRUMENTAL_PARAGRAPH: f64 = 32.0;
/// Bridge shots longer than `BRIDGE_SPLIT` are cut into pieces of about `BRIDGE_SHOT`.
const BRIDGE_SHOT: f64 = 10.0;
const BRIDGE_SPLIT: f64 = 16.0;

#[derive(Clone)]
pub struct Shot {
    pub profile: usize,
    pub start: f64,
    pub end: f64,
    pub lines: [usize; 2],
    pub line_count: u8,
    pub bridge: bool,
    /// Paragraph index before a seamless merge. The cast stream uses this.
    pub paragraph: u16,
    /// Shot index inside that paragraph. The cast stream uses this.
    pub local: u16,
}

impl Shot {
    /// The original id of the cast light position, such as `caustic-drift`.
    pub fn profile_kind(&self) -> &'static str {
        PROFILES[self.profile].kind
    }
}

/// One paragraph of the program, before any seamless merge.
#[derive(Clone, Copy)]
pub struct Span {
    pub start: f64,
    pub end: f64,
    pub lyric_end: f64,
    pub opening: bool,
    /// The paragraph's song lines, `[first, end)`.
    pub lines: [usize; 2],
    pub kind: ParagraphKind,
    pub boundary: ParagraphBoundary,
}

#[derive(Clone, Default)]
pub struct Program {
    pub shots: Vec<Shot>,
    pub spans: Vec<Span>,
    /// The original program seed: the song id, or `lumiere` when the song has none.
    pub seed: String,
    /// Two or more paragraphs were merged into the `lumiere-seamless` scene unit.
    pub seamless: bool,
}

impl Program {
    /// `compileLumiereProgram` without a song duration, as folia calls it.
    pub fn compile(lyrics: &Lyrics, seed: &str, seamless: bool) -> Program {
        let seed = if seed.is_empty() { "lumiere" } else { seed };
        let structure = structure_lines(&lyrics.lines);
        let plans = if structure.is_empty() {
            instrumental_plans(INSTRUMENTAL_DURATION)
        } else {
            lyric_plans(lyrics, &structure)
        };
        let mut shots = Vec::new();
        let mut spans: Vec<Span> = Vec::new();
        let mut recent = Vec::new();
        let mut family: Option<&str> = None;
        for (index, plan) in plans.iter().enumerate() {
            let opening = spans.last().is_none_or(|previous| plan.start - previous.lyric_end >= REOPEN_GAP);
            let lines = match (plan.lines.first(), plan.lines.last()) {
                (Some(first), Some(last)) => [first.source_index, last.source_index + 1],
                _ => [0, 0],
            };
            spans.push(Span {
                start: plan.start,
                end: plan.end,
                lyric_end: plan.lyric_end,
                opening,
                lines,
                kind: plan.kind,
                boundary: plan.boundary,
            });
            for (shot_index, planned) in split_long_bridges(plan_shots(plan)).iter().enumerate() {
                let profile = cast(seed, index, shot_index, plan.kind, planned.bridge, &recent, family);
                if recent.len() == RECENT {
                    recent.remove(0);
                }
                recent.push(profile);
                family = Some(PROFILES[profile].family);
                shots.push(Shot {
                    profile,
                    start: planned.start,
                    end: planned.end,
                    lines: planned.lines,
                    line_count: planned.count,
                    bridge: planned.bridge,
                    paragraph: index as u16,
                    local: shot_index as u16,
                });
            }
        }
        let seamless = seamless && spans.len() >= 2;
        Program { shots, spans, seed: seed.to_owned(), seamless }
    }

    pub fn active(&self, time: f64) -> usize {
        if let Some(index) = self.shots.iter().rposition(|shot| shot.start <= time && time < shot.end.max(shot.start)) {
            return index;
        }
        self.shots.iter().rposition(|shot| shot.start <= time).unwrap_or(0)
    }

    pub fn lead(&self) -> usize {
        self.shots.iter().find(|shot| shot.line_count > 0).map_or(0, |shot| shot.profile)
    }
}

/// A paragraph to cut into shots (`ParagraphPlan`).
struct Plan<'a> {
    lines: Vec<StructureLine<'a>>,
    kind: ParagraphKind,
    boundary: ParagraphBoundary,
    start: f64,
    end: f64,
    lyric_end: f64,
}

#[derive(Clone, Copy)]
struct Planned {
    start: f64,
    end: f64,
    lines: [usize; 2],
    count: u8,
    bridge: bool,
}

impl Planned {
    fn bridge(start: f64, end: f64) -> Planned {
        Planned { start, end, lines: [0, 0], count: 0, bridge: true }
    }
}

/// `planLyricParagraphs`: an intro paragraph when the first line comes after `BRIDGE_GAP`, then the drafted
/// paragraphs laid end to end from 0 to the last render end.
fn lyric_plans<'a>(lyrics: &Lyrics, structure: &[StructureLine<'a>]) -> Vec<Plan<'a>> {
    let params = ParagraphParams::DEFAULT;
    let drafts = draft_paragraphs(structure, gap_threshold(&lyrics.lines, &params), &params);
    let program_end = structure.iter().map(|line| line.render_end).fold(f64::NEG_INFINITY, f64::max).max(0.0);
    let first_start = structure.first().map_or(0.0, |line| line.line.start);
    let intro = first_start >= BRIDGE_GAP;
    let mut plans = Vec::with_capacity(drafts.len() + 1);
    if intro {
        plans.push(Plan {
            lines: Vec::new(),
            kind: ParagraphKind::Break,
            boundary: ParagraphBoundary::Intro,
            start: 0.0,
            end: first_start,
            lyric_end: first_start,
        });
    }
    for (index, draft) in drafts.iter().enumerate() {
        let (Some(first), Some(last)) = (draft.lines.first(), draft.lines.last()) else { continue };
        let next = drafts.get(index + 1).and_then(|next| next.lines.first()).map(|line| line.line.start);
        plans.push(Plan {
            lines: draft.lines.clone(),
            kind: classify(&draft.lines, index, drafts.len()),
            boundary: draft.boundary,
            start: if index == 0 && !intro { first_start.min(0.0) } else { first.line.start },
            end: next.unwrap_or(program_end),
            lyric_end: last.render_end,
        });
    }
    plans
}

/// `planInstrumentalParagraphs`: `duration` cut evenly into paragraphs of about `INSTRUMENTAL_PARAGRAPH`.
fn instrumental_plans<'a>(duration: f64) -> Vec<Plan<'a>> {
    let count = (duration / INSTRUMENTAL_PARAGRAPH).round().max(1.0) as usize;
    (0..count)
        .map(|index| {
            let start = duration * index as f64 / count as f64;
            let end = if index + 1 == count { duration } else { duration * (index + 1) as f64 / count as f64 };
            let boundary = if index == 0 { ParagraphBoundary::SongStart } else { ParagraphBoundary::Instrumental };
            Plan { lines: Vec::new(), kind: ParagraphKind::Break, boundary, start, end, lyric_end: end }
        })
        .collect()
}

/// `groupLines`: short lines pair up within `MAX_SHOT`, long lines stand alone.
fn group_lines<'a>(lines: &[StructureLine<'a>]) -> Vec<Vec<StructureLine<'a>>> {
    let mut groups: Vec<Vec<StructureLine<'a>>> = Vec::new();
    for line in lines {
        let duration = line.render_end - line.line.start;
        let attach = groups.last().is_some_and(|group| {
            let first = &group[0];
            let last_long = group.len() == 1 && first.render_end - first.line.start >= LONG_LINE;
            !last_long
                && duration < LONG_LINE
                && group.len() < MAX_LINES
                && line.render_end - first.line.start <= MAX_SHOT
        });
        match groups.last_mut() {
            Some(group) if attach => group.push(*line),
            _ => groups.push(vec![*line]),
        }
    }
    groups
}

/// `planShots`: each shot runs from its first line to the next shot; a gap of `BRIDGE_GAP` or more after the
/// lines closes the shot 0.6 s after them and leaves the gap to a bridge shot.
fn plan_shots(plan: &Plan<'_>) -> Vec<Planned> {
    let groups = group_lines(&plan.lines);
    if groups.is_empty() {
        return if plan.end > plan.start { vec![Planned::bridge(plan.start, plan.end)] } else { Vec::new() };
    }
    let mut shots = Vec::new();
    for (index, group) in groups.iter().enumerate() {
        let first = group[0].line.start;
        let start = if index == 0 { plan.start.min(first) } else { first };
        let lyric_end = group.iter().map(|line| line.render_end).fold(f64::NEG_INFINITY, f64::max);
        let next = groups.get(index + 1).map_or(plan.end, |group| group[0].line.start);
        let mut lines = [0, 0];
        for (slot, line) in group.iter().take(2).enumerate() {
            lines[slot] = line.source_index;
        }
        let count = u8::try_from(group.len().min(2)).unwrap_or(0);
        if next - lyric_end >= BRIDGE_GAP {
            let end = lyric_end + 0.6;
            shots.push(Planned { start, end, lines, count, bridge: false });
            shots.push(Planned::bridge(end, next));
        } else {
            shots.push(Planned { start, end: next.max(lyric_end), lines, count, bridge: false });
        }
    }
    shots
}

/// `splitLongBridges`: bridge shots over `BRIDGE_SPLIT` seconds are cut evenly into pieces of about `BRIDGE_SHOT`.
fn split_long_bridges(shots: Vec<Planned>) -> Vec<Planned> {
    let mut out = Vec::with_capacity(shots.len());
    for shot in shots {
        let length = shot.end - shot.start;
        if !shot.bridge || length <= BRIDGE_SPLIT {
            out.push(shot);
            continue;
        }
        let count = (length / BRIDGE_SHOT).round().max(2.0) as usize;
        out.extend((0..count).map(|index| {
            let start = shot.start + length * index as f64 / count as f64;
            let end =
                if index + 1 == count { shot.end } else { shot.start + length * (index + 1) as f64 / count as f64 };
            Planned::bridge(start, end)
        }));
    }
    out
}

fn cast(
    seed: &str,
    paragraph: usize,
    shot: usize,
    kind: ParagraphKind,
    bridge: bool,
    recent: &[usize],
    family: Option<&str>,
) -> usize {
    let mut ids = [0u16; 128];
    let mut count = 0;
    for index in 0..PROFILES.len().min(ids.len()) {
        ids[count] = index as u16;
        count += 1;
    }
    if bridge {
        narrow(&mut ids, &mut count, |index| bridge_family(&PROFILES[index]) && PROFILES[index].mood != Mood::Loud, 1);
    }
    if let Some(moods) = moods(kind) {
        narrow(&mut ids, &mut count, |index| moods.contains(&PROFILES[index].mood), 1);
    }
    narrow(&mut ids, &mut count, |index| !recent.contains(&index), 1);
    if let Some(family) = family {
        narrow(&mut ids, &mut count, |index| PROFILES[index].family != family, 3);
    }
    let mut rng = Rng(cast_state(seed, paragraph, shot));
    let pick = (rng.next() * count as f32).floor() as usize;
    ids[pick.min(count.saturating_sub(1))] as usize
}

fn bridge_family(profile: &Profile) -> bool {
    BRIDGE_FAMILIES.contains(&profile.family)
}

/// `MOODS_BY_KIND`.
fn moods(kind: ParagraphKind) -> Option<&'static [Mood]> {
    match kind {
        ParagraphKind::Chorus | ParagraphKind::Lift => Some(&[Mood::Loud, Mood::Neutral]),
        ParagraphKind::Verse | ParagraphKind::Breath => Some(&[Mood::Quiet, Mood::Neutral]),
        ParagraphKind::Outro => Some(&[Mood::Quiet]),
        ParagraphKind::Break => None,
    }
}

fn narrow(ids: &mut [u16], count: &mut usize, keep: impl Fn(usize) -> bool, at_least: usize) {
    let mut kept = [0u16; 128];
    let mut next = 0;
    for slot in ids.iter().take(*count) {
        if keep(*slot as usize) && next < kept.len() {
            kept[next] = *slot;
            next += 1;
        }
    }
    if next >= at_least {
        ids.iter_mut().zip(kept).take(next).for_each(|(slot, value)| *slot = value);
        *count = next;
    }
}

pub(super) fn song_seed(lyrics: &Lyrics) -> u32 {
    let mut hash = 2_166_136_261_u32;
    for line in &lyrics.lines {
        hash = absorb_dec(hash, line.start.to_bits() as u32);
        hash = absorb(hash, line.text.as_bytes());
    }
    hash
}

const FNV: u32 = 2_166_136_261;

/// FNV-1a over UTF-16 code units, matching `hashVisualizerSeed`.
pub(super) fn hash_text(mut hash: u32, text: &str) -> u32 {
    for unit in text.encode_utf16() {
        hash ^= u32::from(unit);
        hash = hash.wrapping_mul(16_777_619);
    }
    hash
}

fn hash_ascii(mut hash: u32, bytes: &[u8]) -> u32 {
    for byte in bytes {
        hash ^= u32::from(*byte);
        hash = hash.wrapping_mul(16_777_619);
    }
    hash
}

fn hash_dec(mut hash: u32, mut value: u32) -> u32 {
    if value == 0 {
        return hash_ascii(hash, b"0");
    }
    let mut buf = [0_u8; 10];
    let mut count = 0;
    while value > 0 {
        buf[count] = b'0' + (value % 10) as u8;
        count += 1;
        value /= 10;
    }
    while count > 0 {
        count -= 1;
        hash = hash_ascii(hash, &buf[count..count + 1]);
    }
    hash
}

/// RNG state for `{seed}:{paragraph}:{shot}:cast`.
pub(super) fn cast_state(seed: &str, paragraph: usize, shot: usize) -> u32 {
    let mut hash = hash_ascii(hash_text(FNV, seed), b":");
    hash = hash_ascii(hash_dec(hash, paragraph as u32), b":");
    hash_ascii(hash_dec(hash, shot as u32), b":cast")
}

/// RNG state for `{seed}:lumiere-seamless:{index}:{kind}{suffix}`, or the per-paragraph form when seamless is off.
pub(super) fn shot_stream(program: &Program, index: usize, kind: &str, suffix: &[u8]) -> u32 {
    let Some(shot) = program.shots.get(index) else { return 0 };
    let mut hash = hash_text(FNV, &program.seed);
    if program.seamless {
        hash = hash_ascii(hash, b":lumiere-seamless:");
        hash = hash_dec(hash, index as u32);
    } else {
        hash = hash_ascii(hash, b":lumiere-p");
        hash = hash_dec(hash, u32::from(shot.paragraph));
        hash = hash_ascii(hash, b":");
        hash = hash_dec(hash, u32::from(shot.local));
    }
    hash = hash_ascii(hash, b":");
    hash_ascii(hash_text(hash, kind), suffix)
}

/// RNG state for `{seed}:lumiere-seamless{suffix}`, or `{seed}:lumiere-p{paragraph}{suffix}`.
pub(super) fn unit_stream(program: &Program, paragraph: u16, suffix: &[u8]) -> u32 {
    if program.seamless {
        return hash_ascii(hash_ascii(hash_text(FNV, &program.seed), b":lumiere-seamless"), suffix);
    }
    let hash = hash_ascii(hash_text(FNV, &program.seed), b":lumiere-p");
    hash_ascii(hash_dec(hash, u32::from(paragraph)), suffix)
}

/// RNG state for `{scene}:{shot}:burst:{line}`. `shot` is the index inside the scene unit.
pub(super) fn burst_state(program: &Program, shot_index: usize, line: usize) -> u32 {
    let Some(shot) = program.shots.get(shot_index) else { return 0 };
    let mut hash = hash_text(FNV, &program.seed);
    if program.seamless {
        hash = hash_ascii(hash, b":lumiere-seamless:");
        hash = hash_dec(hash, shot_index as u32);
    } else {
        hash = hash_ascii(hash, b":lumiere-p");
        hash = hash_dec(hash, u32::from(shot.paragraph));
        hash = hash_ascii(hash, b":");
        hash = hash_dec(hash, u32::from(shot.local));
    }
    hash = hash_ascii(hash, b":burst:");
    hash_dec(hash, line as u32)
}

fn absorb(mut hash: u32, bytes: &[u8]) -> u32 {
    for byte in bytes {
        hash ^= u32::from(*byte);
        hash = hash.wrapping_mul(16_777_619);
    }
    hash
}

fn absorb_dec(mut hash: u32, mut value: u32) -> u32 {
    let mut buf = [b'0'; 10];
    let mut index = buf.len();
    if value == 0 {
        return absorb(hash, b"0");
    }
    while value > 0 {
        index -= 1;
        buf[index] = b'0' + (value % 10) as u8;
        value /= 10;
    }
    hash = absorb(hash, &buf[index..]);
    hash
}

pub struct Rng(pub u32);

impl Rng {
    pub fn next(&mut self) -> f32 {
        self.0 = self.0.wrapping_add(0x6d2b79f5);
        let state = self.0;
        let mut t = (state ^ (state >> 15)).wrapping_mul(state | 1);
        t = t.wrapping_add((t ^ (t >> 7)).wrapping_mul(t | 61)) ^ t;
        ((t ^ (t >> 14)) as f64 / 4_294_967_296.0) as f32
    }
}
