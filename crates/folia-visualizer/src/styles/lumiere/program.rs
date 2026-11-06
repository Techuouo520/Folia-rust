// Shot program: split the lyric into paragraphs and shots, then cast a light position. Audio energy stays 0.

use folia_core::Lyrics;
use folia_lyrics::RenderHints;

use super::profiles::{BRIDGE_FAMILIES, Mood, PROFILES, Profile};

const MAX_SHOT: f64 = 7.0;
const MAX_LINES: usize = 2;
const LONG_LINE: f64 = 3.6;
const BRIDGE_GAP: f64 = 4.0;
const REOPEN_GAP: f64 = 2.5;
const GAP_MIN: f64 = 1.25;
const GAP_MAX: f64 = 3.5;
const PARA_LINES: usize = 6;
const PARA_SECONDS: f64 = 18.0;
const RECENT: usize = 3;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Breath,
    Verse,
    Lift,
    Outro,
}

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

#[derive(Clone, Copy)]
pub struct Span {
    pub start: f64,
    pub end: f64,
    pub opening: bool,
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
    pub fn compile(lyrics: &Lyrics, seed: &str, seamless: bool) -> Program {
        let seed = if seed.is_empty() { "lumiere" } else { seed };
        let ends = render_ends(lyrics);
        if lyrics.lines.is_empty() {
            return Program {
                shots: vec![Shot {
                    profile: quiet_bridge(seed),
                    start: 0.0,
                    end: 8.0,
                    lines: [0, 0],
                    line_count: 0,
                    bridge: true,
                    paragraph: 0,
                    local: 0,
                }],
                spans: vec![Span { start: 0.0, end: 8.0, opening: true }],
                seed: seed.to_owned(),
                seamless: false,
            };
        }
        let paragraphs = paragraphs(lyrics, &ends);
        let mut shots = Vec::new();
        let mut spans = Vec::new();
        let mut recent = Vec::new();
        let mut family: Option<&str> = None;
        for (index, paragraph) in paragraphs.iter().enumerate() {
            let opening = match spans.last() {
                None => true,
                Some(_) => paragraph.start - paragraph.previous_lyric_end >= REOPEN_GAP,
            };
            spans.push(Span { start: paragraph.start, end: paragraph.end, opening });
            let planned = plan(paragraph, lyrics, &ends);
            for (shot_index, plan) in planned.iter().enumerate() {
                let profile = cast(seed, index, shot_index, paragraph.kind, plan.bridge, &recent, family);
                if recent.len() == RECENT {
                    recent.remove(0);
                }
                recent.push(profile);
                family = Some(PROFILES[profile].family);
                shots.push(Shot {
                    profile,
                    start: plan.start,
                    end: plan.end,
                    lines: plan.lines,
                    line_count: plan.count,
                    bridge: plan.bridge,
                    paragraph: index as u16,
                    local: shot_index as u16,
                });
            }
        }
        if shots.is_empty() {
            shots.push(Shot {
                profile: 0,
                start: 0.0,
                end: ends.first().copied().unwrap_or(8.0),
                lines: [0, 0],
                line_count: 0,
                bridge: true,
                paragraph: 0,
                local: 0,
            });
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

struct Paragraph {
    kind: Kind,
    start: f64,
    end: f64,
    previous_lyric_end: f64,
    lines: Vec<usize>,
}

struct Planned {
    start: f64,
    end: f64,
    lines: [usize; 2],
    count: u8,
    bridge: bool,
}

fn paragraphs(lyrics: &Lyrics, ends: &[f64]) -> Vec<Paragraph> {
    let threshold = gap_threshold(lyrics, ends);
    let mut drafts: Vec<Vec<usize>> = Vec::new();
    let mut current = Vec::new();
    for index in 0..lyrics.lines.len() {
        if index > 0 {
            let gap = lyrics.lines[index].start - ends[index - 1];
            if gap >= threshold && !current.is_empty() {
                drafts.push(std::mem::take(&mut current));
            }
        }
        current.push(index);
    }
    if !current.is_empty() {
        drafts.push(current);
    }
    let mut split = Vec::new();
    for draft in drafts {
        split.extend(split_draft(&draft, lyrics, ends));
    }
    let mut previous_end = 0.0;
    let last = split.len().saturating_sub(1);
    split
        .iter()
        .enumerate()
        .map(|(index, lines)| {
            let start = if index == 0 { 0.0 } else { lyrics.lines[lines[0]].start };
            let lyric_end = lines.last().and_then(|line| ends.get(*line)).copied().unwrap_or(start);
            let end =
                split.get(index + 1).and_then(|next| next.first()).map_or(lyric_end, |line| lyrics.lines[*line].start);
            let paragraph = Paragraph {
                kind: classify(lyrics, lines, index == last),
                start,
                end: end.max(lyric_end),
                previous_lyric_end: previous_end,
                lines: lines.clone(),
            };
            previous_end = lyric_end;
            paragraph
        })
        .collect()
}

fn split_draft(lines: &[usize], lyrics: &Lyrics, ends: &[f64]) -> Vec<Vec<usize>> {
    let mut pending = vec![lines.to_vec()];
    let mut out = Vec::new();
    let mut guard = 0;
    while let Some(chunk) = pending.pop() {
        guard += 1;
        let duration = span_of(&chunk, lyrics, ends);
        let small = chunk.len() <= PARA_LINES && (chunk.len() <= 1 || duration <= PARA_SECONDS);
        if small || chunk.len() < 2 || guard > 1000 {
            out.push(chunk);
            continue;
        }
        let mut best_at = 1;
        let mut best_gap = f64::MIN;
        for index in 1..chunk.len() {
            let gap = lyrics.lines[chunk[index]].start - ends[chunk[index - 1]];
            if gap > best_gap {
                best_gap = gap;
                best_at = index;
            }
        }
        let right = chunk[best_at..].to_vec();
        let left = chunk[..best_at].to_vec();
        pending.push(right);
        pending.push(left);
    }
    out
}

fn span_of(lines: &[usize], lyrics: &Lyrics, ends: &[f64]) -> f64 {
    let Some(first) = lines.first() else { return 0.0 };
    let Some(last) = lines.last() else { return 0.0 };
    ends[*last] - lyrics.lines[*first].start
}

fn classify(lyrics: &Lyrics, lines: &[usize], last: bool) -> Kind {
    if last {
        return Kind::Outro;
    }
    let duration = lines
        .last()
        .zip(lines.first())
        .map_or(0.0, |(last, first)| lyrics.lines[*last].end - lyrics.lines[*first].start);
    let words: usize = lines.iter().map(|index| lyrics.lines[*index].words.len().max(1)).sum();
    let marks: usize = lines
        .iter()
        .map(|index| lyrics.lines[*index].text.chars().filter(|ch| matches!(ch, '!' | '?' | '！' | '？' | '…')).count())
        .sum();
    if duration <= 3.5 || words <= 3 {
        Kind::Breath
    } else if marks >= 2 || words as f64 / duration.max(1.0) > 2.5 {
        Kind::Lift
    } else {
        Kind::Verse
    }
}

fn plan(paragraph: &Paragraph, lyrics: &Lyrics, ends: &[f64]) -> Vec<Planned> {
    if paragraph.lines.is_empty() {
        return vec![Planned { start: paragraph.start, end: paragraph.end, lines: [0, 0], count: 0, bridge: true }];
    }
    let started = |index: usize| lyrics.lines[index].start;
    let mut groups: Vec<Vec<usize>> = Vec::new();
    for &index in &paragraph.lines {
        let duration = ends[index] - started(index);
        let attach = groups.last().is_some_and(|group| {
            let first = group[0];
            let last_long = group.len() == 1 && ends[first] - started(first) >= LONG_LINE;
            !last_long && duration < LONG_LINE && group.len() < MAX_LINES && ends[index] - started(first) <= MAX_SHOT
        });
        if attach {
            if let Some(group) = groups.last_mut() {
                group.push(index);
            }
        } else {
            groups.push(vec![index]);
        }
    }
    let mut shots = Vec::new();
    for (index, group) in groups.iter().enumerate() {
        let start = if index == 0 { paragraph.start.min(started(group[0])) } else { started(group[0]) };
        let lyric_end = group.iter().map(|line| ends[*line]).fold(0.0, f64::max);
        let next = groups.get(index + 1).map_or(paragraph.end, |group| started(group[0]));
        let mut lines = [0, 0];
        for (slot, line) in group.iter().take(2).enumerate() {
            lines[slot] = *line;
        }
        let count = u8::try_from(group.len().min(2)).unwrap_or(0);
        if next - lyric_end >= BRIDGE_GAP {
            let end = lyric_end + 0.6;
            shots.push(Planned { start, end, lines, count, bridge: false });
            shots.push(Planned { start: end, end: next, lines: [0, 0], count: 0, bridge: true });
        } else {
            shots.push(Planned { start, end: next.max(lyric_end), lines, count, bridge: false });
        }
    }
    shots
}

fn render_ends(lyrics: &Lyrics) -> Vec<f64> {
    lyrics
        .lines
        .iter()
        .enumerate()
        .map(|(index, line)| {
            let hinted = RenderHints::of(line).render_end;
            let next = lyrics.lines.get(index + 1).map_or(f64::INFINITY, |line| line.start);
            line.start.max(hinted.min(next))
        })
        .collect()
}

fn gap_threshold(lyrics: &Lyrics, ends: &[f64]) -> f64 {
    let mut gaps: Vec<f64> = lyrics
        .lines
        .iter()
        .enumerate()
        .skip(1)
        .map(|(index, line)| line.start - ends[index - 1].min(line.start))
        .filter(|gap| *gap > 0.0)
        .collect();
    if gaps.is_empty() {
        return GAP_MIN;
    }
    gaps.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let mid = gaps.len() / 2;
    let median = if gaps.len().is_multiple_of(2) { (gaps[mid - 1] + gaps[mid]) * 0.5 } else { gaps[mid] };
    (median * 2.5).clamp(GAP_MIN, GAP_MAX)
}

fn cast(
    seed: &str,
    paragraph: usize,
    shot: usize,
    kind: Kind,
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

fn moods(kind: Kind) -> Option<[Mood; 2]> {
    match kind {
        Kind::Lift => Some([Mood::Loud, Mood::Neutral]),
        Kind::Verse | Kind::Breath => Some([Mood::Quiet, Mood::Neutral]),
        Kind::Outro => Some([Mood::Quiet, Mood::Quiet]),
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

fn quiet_bridge(seed: &str) -> usize {
    cast(seed, 0, 0, Kind::Breath, true, &[], None)
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
