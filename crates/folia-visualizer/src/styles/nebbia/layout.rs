// Where a line sits on the glass. Words, blocks, the nine compositions and the on-screen pick are the
// original nebbiaChars.ts, nebbiaBlocks.ts, nebbiaCompositions.ts and nebbiaLayout.ts. A frame does not move them.

use std::sync::{Arc, Mutex};

use folia_core::{Line, Lyrics};

use crate::tuning::{Manifest, Values};

use super::Rng;
use super::tuning::manifest;

const FLOW: f32 = 0.3;
const STACK: f32 = 0.32;
const HERO: f32 = 2.0;
const SEMI: f32 = 1.35;
const SUPPORT: f32 = 1.0;
const PARTICLE: f32 = 0.78;
const HERO_FAST: f32 = 1.55;
const SEMI_FAST: f32 = 1.18;
const PARTICLE_FAST: f32 = 0.85;

/// One character after the whole song has been placed. `x`/`y` are screen pixels, origin top-left;
/// `t`/`d` are when it is sung.
pub struct Laid {
    pub line: usize,
    pub ch: char,
    pub x: f32,
    pub y: f32,
    pub s: f32,
    pub rot: f32,
    pub t: f64,
    pub d: f64,
    /// First character of a word: big raindrops land on these.
    pub lead: bool,
}

/// One source line. `next` is the next line's start, infinite for the last one.
#[derive(Clone)]
pub struct Span {
    pub start: f64,
    pub end: f64,
    pub next: f64,
    pub fingers: bool,
    pub credits: bool,
}

pub struct Song {
    pub chars: Vec<Laid>,
    pub spans: Vec<Span>,
    pub timing: Timing,
    pub seed: u32,
}

struct Source {
    chars: Vec<char>,
    times: Vec<(f64, f64)>,
    words: Vec<Vec<usize>>,
    start: f64,
    end: f64,
    next: f64,
    credits: bool,
}

/// Fog timing of the original `buildNebbiaScene`, shared by the layout and the scene.
#[derive(Clone, Copy)]
pub struct Timing {
    pub hold: f64,
    pub refog: f64,
    pub credits_at: f64,
}

/// A line keeps its writing this much longer while the glass around it is still empty.
const LINGER_MAX: f64 = 16.0;
const LINGER_LINES: usize = 3;

impl Timing {
    pub fn fade_of(self, fingers: bool) -> f64 {
        self.refog + if fingers { 2.5 } else { 0.0 }
    }

    fn before_credits(self, span: &Span) -> f64 {
        (span.end + 0.5).max(self.credits_at - self.fade_of(span.fingers) * 0.6)
    }

    /// When line `i` starts to fog over: the next line's start (at most six seconds after its end) plus `hold`.
    pub fn fog_at(self, spans: &[Span], i: usize) -> f64 {
        let span = &spans[i];
        if span.credits {
            return f64::INFINITY;
        }
        (span.next.min(span.end + 6.0) + self.hold).min(self.before_credits(span))
    }

    /// The latest line `i` may fog over while nothing needs its place.
    pub fn linger_at(self, spans: &[Span], i: usize) -> f64 {
        let fog = self.fog_at(spans, i);
        let later = spans.get(i + LINGER_LINES).map_or(f64::INFINITY, |line| line.start) + self.hold;
        fog.max((fog + LINGER_MAX).min(later).min(self.before_credits(&spans[i])))
    }
}

struct Word {
    idx: Vec<usize>,
    scale: f32,
    cjk: bool,
}

struct Block {
    role: u8,
    words: Vec<Word>,
}

struct Measured {
    vertical: bool,
    items: Vec<MeasureItem>,
    w: f32,
    h: f32,
}

struct MeasureItem {
    k: usize,
    off: f32,
    s: f32,
    rot: f32,
    punct: bool,
}

struct Placed {
    k: usize,
    x: f32,
    y: f32,
    s: f32,
    rot: f32,
}

struct Composed {
    items: Vec<Placed>,
    turned: i32,
}

#[derive(Clone, Copy)]
struct Box {
    x0: f32,
    x1: f32,
    y0: f32,
    y1: f32,
}

struct Candidate {
    score: f32,
    name: &'static str,
    items: Vec<Placed>,
    cx: f32,
    cy: f32,
    rect: Box,
}

/// Places every line. The same lyrics, size and tuning always return the same picture.
pub fn song(lyrics: &Lyrics, width: f32, height: f32, font_scale: f32, tuning: &Values) -> Arc<Song> {
    let key = key_of(lyrics, width, height, font_scale, tuning);
    let mut slot = cache().lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    if let Some((held, song)) = slot.as_ref()
        && *held == key
    {
        return Arc::clone(song);
    }
    let built = Arc::new(build(lyrics, width, height, font_scale, tuning));
    *slot = Some((key, Arc::clone(&built)));
    built
}

fn cache() -> &'static Mutex<Option<(u64, Arc<Song>)>> {
    static CACHE: Mutex<Option<(u64, Arc<Song>)>> = Mutex::new(None);
    &CACHE
}

fn key_of(lyrics: &Lyrics, width: f32, height: f32, font_scale: f32, tuning: &Values) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    let mix = |hash: &mut u64, value: u64| {
        *hash = hash.wrapping_mul(0x0000_0100_0000_01b3).wrapping_add(value);
    };
    mix(&mut hash, width.to_bits().into());
    mix(&mut hash, height.to_bits().into());
    mix(&mut hash, font_scale.to_bits().into());
    mix(&mut hash, tuning.fingerprint());
    mix(&mut hash, super::ink::strokes_epoch());
    for line in &lyrics.lines {
        mix(&mut hash, line.start.to_bits());
        mix(&mut hash, line.end.to_bits());
        for unit in line.text.encode_utf16() {
            mix(&mut hash, u64::from(unit));
        }
    }
    let title = lyrics.title.as_deref().unwrap_or("");
    let artist = lyrics.artist.as_deref().unwrap_or("");
    for unit in title.encode_utf16().chain(artist.encode_utf16()) {
        mix(&mut hash, u64::from(unit));
    }
    hash
}

fn build(lyrics: &Lyrics, width: f32, height: f32, font_scale: f32, tuning: &Values) -> Song {
    let list = manifest();
    let sources = sources(lyrics, tuning, list);
    let layouts = members(tuning, list, "layouts");
    let jitter = tuning.number(list, "jitter");
    let fades = members(tuning, list, "fade");
    let seed = song_seed(lyrics);
    let spans: Vec<Span> = sources
        .iter()
        .enumerate()
        .map(|(index, source)| Span {
            start: source.start,
            end: source.end,
            next: source.next,
            fingers: !source.credits && fingers_line(&fades, index, seed),
            credits: source.credits,
        })
        .collect();
    let timing = Timing {
        hold: f64::from(tuning.number(list, "hold")),
        refog: f64::from(tuning.number(list, "refog")).max(0.5),
        credits_at: spans.iter().find(|span| span.credits).map_or(f64::INFINITY, |span| span.start),
    };
    let mut placed_busy: Vec<(Box, f64, f64, f64)> = Vec::new();
    let mut recent: Vec<&'static str> = Vec::new();
    let mut prev: Option<(f32, f32)> = None;
    let mut chars = Vec::new();
    let mut title_box: Option<Box> = None;
    for (index, source) in sources.iter().enumerate() {
        let chosen = choose(Choose {
            source,
            index,
            width,
            height,
            font_scale,
            jitter,
            layouts: &layouts,
            busy: &placed_busy,
            recent: &recent,
            prev,
            title_box,
        });
        let Some(chosen) = chosen else { continue };
        let fade = timing.fade_of(spans[index].fingers);
        let fog = timing.fog_at(&spans, index);
        let until = fog + fade;
        placed_busy.push((chosen.rect, until, fog, until.max(timing.linger_at(&spans, index) + fade)));
        recent.push(chosen.name);
        if recent.len() > 3 {
            recent.remove(0);
        }
        prev = Some((chosen.cx, chosen.cy));
        if source.credits && title_box.is_none() {
            title_box = Some(chosen.rect);
        }
        let mut items: Vec<&Placed> = chosen.items.iter().collect();
        items.sort_by_key(|item| item.k);
        for item in items {
            let Some(ch) = source.chars.get(item.k).copied() else { continue };
            let (t, d) = source.times.get(item.k).copied().unwrap_or((source.start, 0.0));
            let s = item.s * if wide(ch) { 1.0 } else { 1.15 };
            let lead = source.words.iter().any(|word| word.first() == Some(&item.k));
            chars.push(Laid { line: index, ch, x: item.x, y: item.y, s, rot: item.rot, t, d, lead });
        }
    }
    Song { chars, spans, timing, seed }
}

/// The original `lineModeOf`: the only fade style, or a coin per line when both are on.
fn fingers_line(fades: &[String], index: usize, seed: u32) -> bool {
    match fades {
        [only] => only == "fingers",
        _ => Rng(seed.wrapping_add((index as u32).wrapping_mul(977)).wrapping_add(3)).next() >= 0.5,
    }
}

fn song_seed(lyrics: &Lyrics) -> u32 {
    let text: Vec<char> = lyrics.lines.iter().flat_map(|line| line.text.chars()).collect();
    hash_text(&text)
}

struct Choose<'a> {
    source: &'a Source,
    index: usize,
    width: f32,
    height: f32,
    font_scale: f32,
    jitter: f32,
    layouts: &'a [String],
    busy: &'a [(Box, f64, f64, f64)],
    recent: &'a [&'static str],
    prev: Option<(f32, f32)>,
    title_box: Option<Box>,
}

fn choose(job: Choose<'_>) -> Option<Candidate> {
    let source = job.source;
    let w = job.width;
    let h = job.height;
    if source.words.is_empty() {
        return None;
    }
    let margin_x = w * 0.05;
    let margin_y = h * 0.07;
    let s_min = (h * 0.065).max(26.0) * job.font_scale;
    let s_max = h * 0.14 * job.font_scale;
    let mut rng = Rng(hash_text(&source.chars).wrapping_add((job.index as u32).wrapping_mul(7919)));
    let visible = source.chars.iter().filter(|ch| !ch.is_whitespace()).count();
    let fast = visible > 0 && (source.end - source.start) / (visible as f64) < 0.22;
    let roles = roles_of(source);
    if source.credits {
        let screen = Screen { w, h, s_min, s_max, margin_x, margin_y };
        return credits_candidate(source, &roles, screen, job.title_box);
    }
    let scales = if fast { [HERO_FAST, SEMI_FAST, SUPPORT, PARTICLE_FAST] } else { [HERO, SEMI, SUPPORT, PARTICLE] };
    let mut best: Option<Candidate> = None;
    for max_blocks in [5, 7] {
        let blocks = make_blocks(source, &roles, scales, max_blocks);
        let hero = blocks.iter().position(|block| block.role == 0).unwrap_or(0);
        for name in job.layouts {
            let Some(composed) = compose(name, &blocks, source, hero) else { continue };
            if composed.items.is_empty() {
                continue;
            }
            let (bb, bb_w, bb_h) = bounds(&composed.items);
            let simple = matches!(name.as_str(), "ribbon" | "lines2" | "lines3");
            let max_h = h * if name == "ribbon" { 0.28 } else { 0.5 };
            let max_w = (w - margin_x * 2.0) * if simple { 0.94 } else { 0.68 };
            let fit = (max_w / bb_w).min(max_h / bb_h).min(s_max);
            if fit < s_min * 0.75 {
                continue;
            }
            for (step, factor) in [1.0, 0.8, 0.64, 0.5].into_iter().enumerate() {
                let s = fit * factor;
                if step > 0 && s < s_min * 0.75 {
                    continue;
                }
                consider_places(
                    &mut best,
                    Place {
                        name,
                        composed: &composed,
                        bb,
                        bb_w,
                        bb_h,
                        s,
                        s_min,
                        w,
                        h,
                        margin_x,
                        margin_y,
                        job: &job,
                        fast,
                        max_blocks,
                        blocks_len: blocks.len(),
                        rng: &mut rng,
                    },
                );
            }
        }
    }
    let screen = Screen { w, h, s_min, s_max, margin_x, margin_y };
    best.or_else(|| fallback(source, &roles, screen))
}

struct Place<'a> {
    name: &'a str,
    composed: &'a Composed,
    bb: Box,
    bb_w: f32,
    bb_h: f32,
    s: f32,
    s_min: f32,
    w: f32,
    h: f32,
    margin_x: f32,
    margin_y: f32,
    job: &'a Choose<'a>,
    fast: bool,
    max_blocks: usize,
    blocks_len: usize,
    rng: &'a mut Rng,
}

fn consider_places(best: &mut Option<Candidate>, place: Place<'_>) {
    let bw = place.bb_w * place.s;
    let bh = place.bb_h * place.s;
    let source = place.job.source;
    for fx in [0.5_f32, 0.3, 0.7] {
        for fy in [0.25_f32, 0.42, 0.58, 0.75] {
            let cx = (fx * place.w).clamp(place.margin_x + bw / 2.0, place.w - place.margin_x - bw / 2.0);
            let cy = (fy * place.h).clamp(place.margin_y + bh / 2.0, place.h - place.margin_y - bh / 2.0);
            let rect = Box { x0: cx - bw / 2.0, x1: cx + bw / 2.0, y0: cy - bh / 2.0, y1: cy + bh / 2.0 };
            let pad = place.s * 0.3;
            let padded = Box { x0: rect.x0 - pad, x1: rect.x1 + pad, y0: rect.y0 - pad, y1: rect.y1 + pad };
            let mut score = overlap_score(place.job, padded, bw * bh, source.start);
            score -= place.s / place.h * 100.0;
            if place.s < place.s_min {
                score += 40.0 + (place.s_min - place.s) / place.s_min * 200.0;
            }
            score += bias(place.name);
            score += place.composed.turned as f32 * if place.fast { 10.0 } else { 4.0 };
            if place.fast && !matches!(place.name, "ribbon" | "lines2" | "lines3") {
                score += 6.0;
            }
            if place.max_blocks == 7 && place.blocks_len <= 5 {
                score += 100.0;
            }
            if let Some(rep) = place.job.recent.iter().rposition(|name| *name == place.name) {
                let age = place.job.recent.len() - 1 - rep;
                score += [18.0, 9.0, 4.0][age.min(2)];
            }
            if let Some((px, py)) = place.job.prev
                && (px - cx).abs() < place.w * 0.08
                && (py - cy).abs() < place.h * 0.08
            {
                score += 8.0;
            }
            score += (cy - place.h * 0.5).abs() / place.h * 8.0;
            score += place.rng.next() as f32 * 8.0;
            if best.as_ref().is_none_or(|held| score < held.score) {
                *best = Some(Candidate {
                    score,
                    name: name_of(place.name),
                    items: screen_items(&place.composed.items, place.bb, place.s, cx, cy, place.job.jitter, place.rng),
                    cx,
                    cy,
                    rect,
                });
            }
        }
    }
}

fn overlap_score(job: &Choose<'_>, padded: Box, area: f32, start: f64) -> f32 {
    let mut score = 0.0;
    for (rect, until, fog_at, linger) in job.busy {
        if *linger <= start - 0.3 {
            continue;
        }
        let weight = if *until <= start - 0.3 {
            0.03
        } else if start <= *fog_at {
            1.0
        } else {
            let span = (*until - *fog_at).max(1e-3);
            0.6 + 0.4 * (((*until - start) / span) as f32).clamp(0.0, 1.0)
        };
        score += overlap(padded, *rect) / area.max(1.0) * 1000.0 * weight;
    }
    score
}

fn screen_items(items: &[Placed], bb: Box, s: f32, cx: f32, cy: f32, jitter: f32, rng: &mut Rng) -> Vec<Placed> {
    let mid_x = (bb.x0 + bb.x1) / 2.0;
    let mid_y = (bb.y0 + bb.y1) / 2.0;
    let tilt = (rng.next() as f32 - 0.5) * 0.04 * jitter;
    items
        .iter()
        .map(|item| {
            let x = cx + (item.x - mid_x) * s;
            let y = cy + (item.y - mid_y) * s;
            let size = item.s * s * (1.0 + (rng.next() as f32 - 0.5) * 0.08 * jitter);
            Placed {
                k: item.k,
                x: x + (rng.next() as f32 - 0.5) * s * 0.04 * jitter,
                y: y + (x - cx) * tilt + (rng.next() as f32 - 0.5) * s * 0.05 * jitter,
                s: size,
                rot: item.rot + (rng.next() as f32 - 0.5) * 0.08 * jitter + tilt,
            }
        })
        .collect()
}

struct Screen {
    w: f32,
    h: f32,
    s_min: f32,
    s_max: f32,
    margin_x: f32,
    margin_y: f32,
}

fn fallback(source: &Source, roles: &[u8], screen: Screen) -> Option<Candidate> {
    let Screen { w, h, s_min: _, s_max, margin_x, margin_y } = screen;
    let scales = [HERO_FAST, SEMI_FAST, SUPPORT, PARTICLE_FAST];
    let blocks = make_blocks(source, roles, scales, 9);
    let composed = compose("lines3", &blocks, source, 0)
        .or_else(|| compose("lines2", &blocks, source, 0))
        .or_else(|| compose("ribbon", &blocks, source, 0))?;
    let (bb, bb_w, bb_h) = bounds(&composed.items);
    let s = s_max.min((w - margin_x * 2.0) / bb_w).min((h - margin_y * 2.0) / bb_h);
    let mut rng = Rng(hash_text(&source.chars));
    Some(Candidate {
        score: 0.0,
        name: "lines3",
        items: screen_items(&composed.items, bb, s, w / 2.0, h / 2.0, 0.0, &mut rng),
        cx: w / 2.0,
        cy: h / 2.0,
        rect: Box {
            x0: w / 2.0 - bb_w * s / 2.0,
            x1: w / 2.0 + bb_w * s / 2.0,
            y0: h / 2.0 - bb_h * s / 2.0,
            y1: h / 2.0 + bb_h * s / 2.0,
        },
    })
}

fn credits_candidate(source: &Source, roles: &[u8], screen: Screen, title_box: Option<Box>) -> Option<Candidate> {
    let Screen { w, h, s_min, s_max, margin_x, margin_y: _ } = screen;
    let scales = [SUPPORT, SUPPORT, SUPPORT, SUPPORT];
    let blocks = make_blocks(source, roles, scales, 9);
    let wide = compose("ribbon", &blocks, source, 0)?;
    let (_one_bb, one_w, _) = bounds(&wide.items);
    let fits = (s_max * 1.15).min((w - margin_x * 2.0) * 0.8 / one_w) >= s_min;
    let composed = if fits { wide } else { compose("lines2", &blocks, source, 0).unwrap_or(wide) };
    let (bb, bb_w, bb_h) = bounds(&composed.items);
    let cap = if title_box.is_none() { s_max * 1.15 } else { s_max * 0.55 };
    let s = cap.min((w - margin_x * 2.0) * 0.8 / bb_w).min(h * 0.32 / bb_h);
    let bw = bb_w * s;
    let bh = bb_h * s;
    let cy = match title_box {
        Some(title) => title.y1 + bh / 2.0 + s * 0.6,
        None => h * 0.42,
    };
    let mut rng = Rng(hash_text(&source.chars));
    Some(Candidate {
        score: 0.0,
        name: "ribbon",
        items: screen_items(&composed.items, bb, s, w / 2.0, cy, 0.2, &mut rng),
        cx: w / 2.0,
        cy,
        rect: Box { x0: w / 2.0 - bw / 2.0, x1: w / 2.0 + bw / 2.0, y0: cy - bh / 2.0, y1: cy + bh / 2.0 },
    })
}

fn bias(name: &str) -> f32 {
    match name {
        "ribbon" => 12.0,
        "lines2" => 10.0,
        "lines3" => 14.0,
        "cross" => 0.0,
        "pillar" => 1.0,
        "header" => 1.0,
        "badge" => 4.0,
        "columns" => 2.0,
        "orbit" => 4.0,
        _ => 20.0,
    }
}

fn name_of(name: &str) -> &'static str {
    match name {
        "lines2" => "lines2",
        "lines3" => "lines3",
        "cross" => "cross",
        "pillar" => "pillar",
        "header" => "header",
        "badge" => "badge",
        "columns" => "columns",
        "orbit" => "orbit",
        _ => "ribbon",
    }
}

fn compose(name: &str, blocks: &[Block], source: &Source, hero: usize) -> Option<Composed> {
    let ctx = Ctx { blocks, chars: &source.chars, hero };
    match name {
        "lines2" => lines2(&ctx),
        "lines3" => lines3(&ctx),
        "cross" => cross(&ctx),
        "pillar" => pillar(&ctx),
        "header" => header(&ctx),
        "badge" => badge(&ctx),
        "columns" => columns(&ctx),
        "orbit" => orbit(&ctx),
        _ => Some(flow_rows(&ctx, &[idx_all(blocks.len())], &[0.0])),
    }
}

struct Ctx<'a> {
    blocks: &'a [Block],
    chars: &'a [char],
    hero: usize,
}

fn lines2(ctx: &Ctx<'_>) -> Option<Composed> {
    let n = ctx.blocks.len();
    if n < 2 {
        return None;
    }
    let widths: Vec<f32> = (0..n).map(|i| measure(ctx, i, false).w).collect();
    let total: f32 = widths.iter().sum();
    let mut acc = 0.0;
    let mut cut = 1;
    let mut best = f32::INFINITY;
    for (i, width) in widths.iter().enumerate() {
        acc += width;
        if i + 1 < n && (acc - total / 2.0).abs() < best {
            best = (acc - total / 2.0).abs();
            cut = i + 1;
        }
    }
    let top: Vec<usize> = (0..cut).collect();
    let bottom: Vec<usize> = (cut..n).collect();
    Some(flow_rows(ctx, &[top, bottom], &[-0.12, 0.12]))
}

fn lines3(ctx: &Ctx<'_>) -> Option<Composed> {
    let n = ctx.blocks.len();
    if n < 3 {
        return None;
    }
    let widths: Vec<f32> = (0..n).map(|i| measure(ctx, i, false).w).collect();
    let total: f32 = widths.iter().sum();
    let mut rows: Vec<Vec<usize>> = vec![Vec::new()];
    let mut acc = 0.0;
    for (i, width) in widths.iter().enumerate() {
        let last = rows.last().is_some_and(|row| !row.is_empty());
        let need =
            last && rows.len() < 3 && (acc + width / 2.0 > total / 3.0 * rows.len() as f32 || n - i <= 3 - rows.len());
        if need {
            rows.push(Vec::new());
        }
        if let Some(row) = rows.last_mut() {
            row.push(i);
        }
        acc += width;
    }
    if rows.len() == 3 { Some(flow_rows(ctx, &rows, &[-0.14, 0.1, -0.04])) } else { None }
}

fn cross(ctx: &Ctx<'_>) -> Option<Composed> {
    let n = ctx.blocks.len();
    let h = ctx.hero;
    if !(3..=7).contains(&n) {
        return None;
    }
    let top = h / 2;
    let right = (n - 1 - h).div_ceil(2);
    if top == 0 && h + right + 1 >= n {
        return None;
    }
    let mut out = Vec::new();
    let mut turned = 0;
    let hero = measure(ctx, h, false);
    emit(&hero, 0.0, 0.0, &mut out);
    let mut x = -hero.w / 2.0 - STACK;
    for i in (top..h).rev() {
        let block = measure(ctx, i, false);
        emit(&block, x - block.w / 2.0, (hero.h - block.h) / 2.0, &mut out);
        x -= block.w + FLOW;
    }
    let mut y = -hero.h / 2.0 - STACK;
    for i in (0..top).rev() {
        let block = measure(ctx, i, true);
        turned += i32::from(block.items.iter().any(|item| item.rot != 0.0));
        emit(&block, 0.0, y - block.h / 2.0, &mut out);
        y -= block.h + STACK;
    }
    x = hero.w / 2.0 + STACK;
    for i in (h + 1)..=(h + right) {
        let block = measure(ctx, i, false);
        emit(&block, x + block.w / 2.0, (hero.h - block.h) / 2.0, &mut out);
        x += block.w + FLOW;
    }
    y = hero.h / 2.0 + STACK;
    for i in (h + right + 1)..n {
        let block = measure(ctx, i, true);
        turned += i32::from(block.items.iter().any(|item| item.rot != 0.0));
        emit(&block, 0.0, y + block.h / 2.0, &mut out);
        y += block.h + STACK;
    }
    Some(Composed { items: out, turned })
}

fn pillar(ctx: &Ctx<'_>) -> Option<Composed> {
    let n = ctx.blocks.len();
    let h = ctx.hero;
    if !(2..=6).contains(&n) {
        return None;
    }
    let mut out = Vec::new();
    let hero = measure(ctx, h, true);
    let mut turned = i32::from(hero.items.iter().any(|item| item.rot != 0.0));
    emit(&hero, 0.0, 0.0, &mut out);
    turned += side(ctx, 0, h, -1.0, &hero, &mut out);
    turned += side(ctx, h + 1, n, 1.0, &hero, &mut out);
    Some(Composed { items: out, turned })
}

fn side(ctx: &Ctx<'_>, from: usize, to: usize, dir: f32, hero: &Measured, out: &mut Vec<Placed>) -> i32 {
    let mut turned = 0;
    let mut cols: Vec<Vec<Measured>> = vec![Vec::new()];
    let top = -hero.h / 2.0 + 0.15;
    let mut y = top;
    let step = if from < to { 1 } else { return 0 };
    let mut i = from;
    while i != to {
        let block = measure(ctx, i, true);
        turned += i32::from(block.items.iter().any(|item| item.rot != 0.0));
        if y + block.h > hero.h / 2.0 + 0.6 && cols.last().is_some_and(|col| !col.is_empty()) {
            cols.push(Vec::new());
            y = top;
        }
        if let Some(col) = cols.last_mut() {
            col.push(block);
        }
        y += cols.last().and_then(|col| col.last()).map_or(0.0, |block| block.h) + STACK;
        i += step;
    }
    let order: Vec<usize> = if dir > 0.0 { (0..cols.len()).collect() } else { (0..cols.len()).rev().collect() };
    let mut col_x = dir * (hero.w / 2.0 + FLOW);
    for index in order {
        y = top;
        let width = cols[index].iter().map(|block| block.w).fold(0.0, f32::max);
        for block in &cols[index] {
            emit(block, col_x + dir * block.w / 2.0, y + block.h / 2.0, out);
            y += block.h + STACK;
        }
        col_x += dir * (width + FLOW);
    }
    turned
}

fn header(ctx: &Ctx<'_>) -> Option<Composed> {
    let n = ctx.blocks.len();
    let h = ctx.hero;
    if n < 3 || n - 1 - h < 2 {
        return None;
    }
    let mut out = Vec::new();
    let hero = measure(ctx, h, false);
    emit(&hero, 0.0, 0.0, &mut out);
    if h > 0 {
        let row: Vec<Measured> = (0..h).map(|i| measure(ctx, i, false)).collect();
        let row_w = row.iter().map(|block| block.w).sum::<f32>() + FLOW * (h - 1) as f32;
        let row_h = row.iter().map(|block| block.h).fold(0.0, f32::max);
        let mut x = -row_w / 2.0;
        let y = -hero.h / 2.0 - STACK - row_h / 2.0;
        for block in &row {
            emit(block, x + block.w / 2.0, y + (row_h - block.h) / 2.0, &mut out);
            x += block.w + FLOW;
        }
    }
    let mut y = hero.h / 2.0 + STACK;
    let left = -hero.w * 0.22 - FLOW / 2.0;
    let right = hero.w * 0.22 + FLOW / 2.0;
    let mut i = h + 1;
    while i < n {
        let a = measure(ctx, i, false);
        let b = if i + 1 < n { Some(measure(ctx, i + 1, false)) } else { None };
        let rh = a.h.max(b.as_ref().map_or(0.0, |block| block.h));
        emit(&a, left - a.w / 2.0, y + rh - a.h / 2.0, &mut out);
        if let Some(block) = &b {
            emit(block, right + block.w / 2.0, y + rh - block.h / 2.0, &mut out);
        }
        y += rh + STACK;
        i += 2;
    }
    Some(Composed { items: out, turned: 0 })
}

fn badge(ctx: &Ctx<'_>) -> Option<Composed> {
    let n = ctx.blocks.len();
    if !(3..=7).contains(&n) || ctx.hero != 0 {
        return None;
    }
    let mut out = Vec::new();
    let hero = measure(ctx, 0, true);
    let rest: Vec<Measured> = (1..n).map(|i| measure(ctx, i, false)).collect();
    let col_w = rest.iter().map(|block| block.w).fold(0.0, f32::max).max(hero.h * 0.9);
    emit(&hero, hero.w / 2.0, hero.h / 2.0, &mut out);
    let mut x = hero.w + FLOW;
    let mut y = 0.0;
    let mut row_h = 0.0;
    for block in &rest {
        let left = if y < hero.h { hero.w + FLOW } else { 0.0 };
        if x > left && x + block.w > hero.w + FLOW + col_w {
            y += row_h + STACK;
            x = if y < hero.h { hero.w + FLOW } else { 0.0 };
            row_h = 0.0;
        }
        emit(block, x + block.w / 2.0, y + block.h / 2.0, &mut out);
        x += block.w + FLOW;
        row_h = row_h.max(block.h);
    }
    Some(Composed { items: out, turned: i32::from(hero.items.iter().any(|item| item.rot != 0.0)) })
}

fn columns(ctx: &Ctx<'_>) -> Option<Composed> {
    let n = ctx.blocks.len();
    if !(2..=5).contains(&n) || ctx.blocks.iter().any(|block| block.words.iter().any(|word| !word.cjk)) {
        return None;
    }
    let mut out = Vec::new();
    let mut x = 0.0;
    for i in 0..n {
        let block = measure(ctx, i, true);
        emit(&block, x + block.w / 2.0, block.h / 2.0 + (i % 2) as f32 * 0.5, &mut out);
        x += block.w + STACK;
    }
    Some(Composed { items: out, turned: 0 })
}

fn orbit(ctx: &Ctx<'_>) -> Option<Composed> {
    let n = ctx.blocks.len();
    let h = ctx.hero;
    if !(4..=7).contains(&n) {
        return None;
    }
    let mut out = Vec::new();
    let hero = measure(ctx, h, false);
    emit(&hero, 0.0, 0.0, &mut out);
    let mut rects = vec![Box { x0: -hero.w / 2.0, x1: hero.w / 2.0, y0: -hero.h / 2.0, y1: hero.h / 2.0 }];
    let mut angle = -std::f32::consts::PI * 0.75;
    let mut turned = 0;
    let base = (hero.w * hero.w + hero.h * hero.h).sqrt() / 2.0 + STACK;
    for i in 0..n {
        if i == h {
            continue;
        }
        let mut radius = base;
        let mut aim = angle;
        let mut placed = None;
        for tries in 0..600 {
            let vertical = aim.cos().abs() > 0.75;
            let block = measure(ctx, i, vertical);
            let cx = aim.cos() * radius * 1.25;
            let cy = aim.sin() * radius * 0.8;
            let rect =
                Box { x0: cx - block.w / 2.0, x1: cx + block.w / 2.0, y0: cy - block.h / 2.0, y1: cy + block.h / 2.0 };
            if rects.iter().all(|held| separate(*held, rect) >= FLOW) {
                placed = Some((block, rect));
                break;
            }
            aim += 0.08;
            if tries % 80 == 79 {
                radius += 0.5;
            }
        }
        let (block, rect) = placed?;
        rects.push(rect);
        turned += i32::from(block.items.iter().any(|item| item.rot != 0.0));
        emit(&block, (rect.x0 + rect.x1) / 2.0, (rect.y0 + rect.y1) / 2.0, &mut out);
        angle = aim + 0.5;
    }
    Some(Composed { items: out, turned })
}

fn flow_rows(ctx: &Ctx<'_>, rows: &[Vec<usize>], shifts: &[f32]) -> Composed {
    let measured: Vec<Vec<Measured>> =
        rows.iter().map(|row| row.iter().map(|i| measure(ctx, *i, false)).collect()).collect();
    let widths: Vec<f32> = measured
        .iter()
        .map(|row| row.iter().map(|block| block.w).sum::<f32>() + FLOW * row.len().saturating_sub(1) as f32)
        .collect();
    let widest = widths.iter().copied().fold(0.0, f32::max);
    let mut out = Vec::new();
    let mut y = 0.0;
    for (ri, row) in measured.iter().enumerate() {
        let rh = row.iter().map(|block| block.h).fold(0.0, f32::max);
        let mut x = -widths[ri] / 2.0 + shifts.get(ri).copied().unwrap_or(0.0) * widest;
        for block in row {
            emit(block, x + block.w / 2.0, y + rh - block.h / 2.0, &mut out);
            x += block.w + FLOW;
        }
        y += rh + STACK;
    }
    Composed { items: out, turned: 0 }
}

fn measure(ctx: &Ctx<'_>, index: usize, vertical: bool) -> Measured {
    let Some(block) = ctx.blocks.get(index) else {
        return Measured { vertical, items: Vec::new(), w: 0.0, h: 0.0 };
    };
    let mut items = Vec::new();
    let mut pen = 0.0;
    let mut thick = 0.0_f32;
    for (wi, word) in block.words.iter().enumerate() {
        if wi > 0 {
            let prev = &block.words[wi - 1];
            let gap = if prev.cjk && word.cjk { 0.04 } else { 0.24 };
            pen += gap * if vertical && word.cjk { 0.0 } else { 1.0 };
        }
        for &k in &word.idx {
            let ch = ctx.chars.get(k).copied().unwrap_or(' ');
            let vert_cjk = vertical && word.cjk;
            let advance = if vert_cjk { if punct(ch) { 0.55 } else { 1.02 } } else { advance_of(ch) };
            let rot = if vertical && (!word.cjk || turns(ch)) { std::f32::consts::FRAC_PI_2 } else { 0.0 };
            let s = word.scale * if word.cjk { complex_scale(ch, word.scale) } else { 1.0 };
            items.push(MeasureItem { k, off: pen + advance * s / 2.0, s, rot, punct: vert_cjk && punct(ch) });
            pen += advance * s;
            thick = thick.max(s);
        }
    }
    if vertical {
        Measured { vertical, items, w: thick, h: pen }
    } else {
        Measured { vertical, items, w: pen, h: thick }
    }
}

fn emit(block: &Measured, cx: f32, cy: f32, out: &mut Vec<Placed>) {
    for item in &block.items {
        if block.vertical {
            let x = cx + if item.punct { item.s * 0.3 } else { 0.0 };
            let y = cy - block.h / 2.0 + item.off - if item.punct { item.s * 0.2 } else { 0.0 };
            out.push(Placed { k: item.k, x, y, s: item.s, rot: item.rot });
        } else {
            out.push(Placed {
                k: item.k,
                x: cx - block.w / 2.0 + item.off,
                y: cy + block.h / 2.0 - item.s / 2.0,
                s: item.s,
                rot: 0.0,
            });
        }
    }
}

fn make_blocks(source: &Source, roles: &[u8], scales: [f32; 4], max_blocks: usize) -> Vec<Block> {
    let mut blocks: Vec<Block> = Vec::new();
    for (i, word) in source.words.iter().enumerate() {
        let role = roles.get(i).copied().unwrap_or(2);
        let cjk = word.iter().all(|k| source.chars.get(*k).is_some_and(|ch| cjk(*ch) || punct(*ch)));
        let owned = Word { idx: word.clone(), scale: scales.get(role as usize).copied().unwrap_or(SUPPORT), cjk };
        let merge = role == 3 && blocks.last().is_some_and(|block| block.role != 0 && block.role != 1);
        if merge {
            if let Some(last) = blocks.last_mut() {
                last.words.push(owned);
            }
        } else {
            blocks.push(Block { role, words: vec![owned] });
        }
    }
    while blocks.len() > max_blocks {
        let mut at = None;
        let mut shortest = f32::INFINITY;
        for i in 0..blocks.len().saturating_sub(1) {
            if blocks[i].role <= 1 || blocks[i + 1].role <= 1 {
                continue;
            }
            let len = (block_len(&blocks[i]) + block_len(&blocks[i + 1])) as f32;
            if len < shortest {
                shortest = len;
                at = Some(i);
            }
        }
        let Some(i) = at else { break };
        let mut words: Vec<Word> = Vec::new();
        words.append(&mut blocks[i].words);
        words.append(&mut blocks[i + 1].words);
        blocks[i] = Block { role: 2, words };
        blocks.remove(i + 1);
    }
    blocks
}

fn block_len(block: &Block) -> usize {
    block.words.iter().map(|word| word.idx.len()).sum()
}

fn roles_of(source: &Source) -> Vec<u8> {
    let scores: Vec<f32> = source.words.iter().map(|word| score(source, word)).collect();
    let mut hero = 0;
    for (i, score) in scores.iter().enumerate() {
        if *score > scores[hero] {
            hero = i;
        }
    }
    let mut semi = None;
    if source.words.len() >= 4 {
        for (i, word) in source.words.iter().enumerate() {
            if i == hero || i.abs_diff(hero) < 2 || word.len() < 2 || scores[i] < scores[hero] * 0.35 {
                continue;
            }
            if semi.is_none_or(|held: usize| scores[i] > scores[held]) {
                semi = Some(i);
            }
        }
    }
    source
        .words
        .iter()
        .enumerate()
        .map(|(i, word)| {
            if i == hero && source.words.len() > 1 {
                0
            } else if semi == Some(i) {
                1
            } else if word.len() == 1 && source.chars.get(word[0]).is_some_and(|ch| particle(*ch)) {
                3
            } else {
                2
            }
        })
        .collect()
}

/// Sonnet's score: characters (at most 8) x 14 plus how long the word is sung (at most 2.5 s) x 18.
fn score(source: &Source, word: &[usize]) -> f32 {
    let (Some(first), Some(last)) = (word.first(), word.last()) else { return 0.0 };
    let t0 = source.times.get(*first).map_or(source.start, |time| time.0);
    let t1 = source.times.get(*last).map_or(source.end, |time| time.0 + time.1);
    word.len().min(8) as f32 * 14.0 + ((t1 - t0).clamp(0.0, 2.5) as f32) * 18.0
}

/// Characters with many strokes grow on their own (from 10 strokes, 1.25x at 19 and up); big words grow less.
/// Only the stroke-order data counts, so characters without it keep their size.
fn complex_scale(ch: char, scale: f32) -> f32 {
    let n = super::ink::stroke_count(ch).unwrap_or(0) as f32;
    1.0 + 0.25 * ((n - 9.0) / 10.0).clamp(0.0, 1.0) / scale.max(1.0)
}

/// The title is written this long after the last line ends (the original `CREDITS_DELAY`).
const CREDITS_DELAY: f64 = 2.5;

fn sources(lyrics: &Lyrics, tuning: &Values, list: Manifest) -> Vec<Source> {
    let mut out: Vec<Source> = lyrics
        .lines
        .iter()
        .enumerate()
        .map(|(i, line)| {
            source_line(
                line,
                lyrics.lines.get(i + 1).map_or(f64::INFINITY, |next| next.start),
                false,
                tuning.number(list, "doodle"),
            )
        })
        .collect();
    let title = lyrics.title.as_deref().map(str::trim).filter(|text| !text.is_empty());
    if !tuning.flag(list, "credits") || out.is_empty() {
        return out;
    }
    let Some(title) = title else { return out };
    let at = out.iter().map(|line| line.end).fold(f64::NEG_INFINITY, f64::max) + CREDITS_DELAY;
    if let Some(last) = out.last_mut() {
        last.next = last.next.min(at);
    }
    let named = credit_line(title, at);
    let after = named.end + 0.2;
    out.push(named);
    if let Some(artist) = lyrics.artist.as_deref().map(str::trim).filter(|text| !text.is_empty()) {
        out.push(credit_line(artist, after));
    }
    out
}

/// Words come from the shared word segmenter, as the original `segmentLyricWords`; the characters of
/// one timed word split its time evenly.
fn source_line(line: &Line, next: f64, credits: bool, doodle: f32) -> Source {
    let mut chars = Vec::new();
    let mut times = Vec::new();
    let mut owner = Vec::new();
    for (w, word) in line.words.iter().enumerate() {
        let step = (word.end - word.start).max(0.0) / word.text.chars().count().max(1) as f64;
        for (i, ch) in word.text.chars().enumerate() {
            if ch.is_whitespace() {
                continue;
            }
            chars.push(ch);
            times.push((word.start + step * i as f64, step));
            owner.push(w);
        }
    }
    if !credits && !chars.is_empty() && doodle > 0.0 {
        let roll = {
            let x = (line.start + 17.0).sin() * 10000.0;
            x - x.floor()
        };
        if roll < f64::from(doodle) {
            chars.push(super::ink::DOODLE);
            times.push((line.end, 0.45));
            owner.push(owner.last().copied().unwrap_or(0));
        }
    }
    let mut words: Vec<Vec<usize>> = Vec::new();
    for unit in folia_lyrics::layout_units(line, true) {
        let idx: Vec<usize> = (0..chars.len()).filter(|k| unit.words.contains(&owner[*k])).collect();
        if idx.is_empty() {
            continue;
        }
        match words.last_mut() {
            Some(prev) if idx.iter().all(|k| punct(chars[*k])) => prev.extend(idx),
            _ => words.push(idx),
        }
    }
    Source { chars, times, words, start: line.start, end: line.end.max(line.start), next, credits }
}

/// The title or artist as one more line: a character every 0.1 s, each sung for 0.5 s.
fn credit_line(text: &str, at: f64) -> Source {
    let words: Vec<folia_core::Word> = text
        .chars()
        .enumerate()
        .map(|(i, ch)| folia_core::Word {
            text: ch.to_string(),
            start: at + i as f64 * 0.1,
            end: at + i as f64 * 0.1 + 0.5,
        })
        .collect();
    let end = words.last().map_or(at, |word| word.end);
    source_line(&Line { words, start: at, end, text: text.to_owned(), ..Line::default() }, f64::INFINITY, true, 0.0)
}

fn members(tuning: &Values, list: Manifest, key: &str) -> Vec<String> {
    let options: Vec<&str> = list
        .items()
        .filter_map(|item| match item {
            crate::tuning::Item::Member { key: item_key, option, .. } if item_key == key => Some(option),
            _ => None,
        })
        .collect();
    let on: Vec<String> =
        options.iter().copied().filter(|option| tuning.member(list, key, option)).map(str::to_owned).collect();
    if on.is_empty() { options.into_iter().map(str::to_owned).collect() } else { on }
}

fn idx_all(n: usize) -> Vec<usize> {
    (0..n).collect()
}

fn bounds(items: &[Placed]) -> (Box, f32, f32) {
    let mut x0 = f32::INFINITY;
    let mut x1 = f32::NEG_INFINITY;
    let mut y0 = f32::INFINITY;
    let mut y1 = f32::NEG_INFINITY;
    for item in items {
        x0 = x0.min(item.x - item.s / 2.0);
        x1 = x1.max(item.x + item.s / 2.0);
        y0 = y0.min(item.y - item.s / 2.0);
        y1 = y1.max(item.y + item.s / 2.0);
    }
    if items.is_empty() {
        return (Box { x0: 0.0, x1: 1.0, y0: 0.0, y1: 1.0 }, 1.0, 1.0);
    }
    (Box { x0, x1, y0, y1 }, (x1 - x0).max(0.01), (y1 - y0).max(0.01))
}

fn overlap(a: Box, b: Box) -> f32 {
    (a.x1.min(b.x1) - a.x0.max(b.x0)).max(0.0) * (a.y1.min(b.y1) - a.y0.max(b.y0)).max(0.0)
}

fn separate(a: Box, b: Box) -> f32 {
    (a.x0 - b.x1).max(b.x0 - a.x1).max(a.y0 - b.y1).max(b.y0 - a.y1)
}

fn hash_text(chars: &[char]) -> u32 {
    let mut hash = 2_166_136_261_u32;
    for ch in chars {
        let mut buf = [0_u16; 2];
        for unit in ch.encode_utf16(&mut buf) {
            hash ^= u32::from(*unit);
            hash = hash.wrapping_mul(16_777_619);
        }
    }
    hash
}

fn advance_of(ch: char) -> f32 {
    if ch == ' ' {
        0.32
    } else if matches!(ch, '，' | '。' | '、' | '・' | '．') {
        0.5
    } else if wide(ch) {
        1.0
    } else if matches!(ch, 'i' | 'l' | '.' | ',' | '\'' | '!' | '|' | ':' | ';') {
        0.3
    } else if matches!(ch, 'm' | 'w' | 'M' | 'W') {
        0.72
    } else {
        0.5
    }
}

fn cjk(ch: char) -> bool {
    ('\u{3040}'..='\u{30FF}').contains(&ch)
        || ('\u{3400}'..='\u{9FFF}').contains(&ch)
        || ('\u{F900}'..='\u{FAFF}').contains(&ch)
        || ('\u{AC00}'..='\u{D7A3}').contains(&ch)
}

fn wide(ch: char) -> bool {
    ('\u{3000}'..='\u{9FFF}').contains(&ch) || ('\u{FF00}'..='\u{FFEF}').contains(&ch)
}

fn punct(ch: char) -> bool {
    matches!(ch, '，' | '。' | '、' | '！' | '？' | '!' | '?' | ',' | '.' | '・' | '…')
}

fn turns(ch: char) -> bool {
    matches!(ch, 'ー' | '—' | '～' | '〜' | '…' | '-')
}

fn particle(ch: char) -> bool {
    "的了着过吗呢吧啊呀嘛之与和是也都就还又被把给のがをにはへとでやもねよなかしてたっ".contains(ch)
}
