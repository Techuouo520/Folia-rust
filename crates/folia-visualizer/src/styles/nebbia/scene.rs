// The whole song on the glass, as the original nebbiaScene.ts, nebbiaWipe.ts and the big drops of
// nebbiaHits.ts: placed characters with warped skeletons, edge beads and drips, the writing schedule,
// and when each line fogs over, yields its place or is breathed away. Built once per layout.

use std::sync::{Arc, Mutex};

use folia_core::Lyrics;

use crate::tuning::Values;

use super::ink::{self, Ink};
use super::layout::{self, Song, Timing};
use super::tuning::manifest;
use super::{Rng, smooth};

/// Edge beads per stroke, drips and end beads; the original `NEBBIA_FIXED`.
const BEADS: f32 = 0.35;
const DRIP: f32 = 0.25;
const BREATH_SPREAD: f64 = 0.3;
const BREATH_HOLD: f64 = 1.0;

pub struct Bead {
    pub s: f32,
    pub x: f32,
    pub y: f32,
    pub nx: f32,
    pub ny: f32,
    pub r: f32,
    pub j: f32,
}

pub struct Drip {
    pub delay: f64,
    pub len: f32,
    pub dur: f64,
    pub r: f32,
    pub seed: f32,
}

pub struct Char {
    pub ch: char,
    pub x: f32,
    pub y: f32,
    pub s: f32,
    pub rot: f32,
    pub t: f64,
    pub d: f64,
    /// Finger writing starts at `wt` and takes `wd`; a fading character takes `appear_dur` from `t`.
    pub wt: f64,
    pub wd: f64,
    pub appear_dur: f64,
    pub glyph: Option<Ink>,
    pub beads: Vec<Vec<Bead>>,
    pub drips: Vec<Option<Drip>>,
    pub ends: Vec<(f32, f32)>,
}

#[derive(Clone, Copy)]
pub struct Rect {
    pub x0: f32,
    pub x1: f32,
    pub y0: f32,
    pub y1: f32,
    pub s: f32,
}

pub struct Streak {
    pub pts: Vec<(f32, f32)>,
    pub w: f32,
    pub a: f32,
    pub b: f32,
}

pub struct Wipe {
    pub streaks: Vec<Streak>,
    pub beads: Vec<(f32, f32, f32)>,
    pub dur: f64,
}

pub struct Line {
    pub chars: Vec<Char>,
    pub t: f64,
    pub end: f64,
    pub fingers: bool,
    pub fog_at: f64,
    pub yield_at: f64,
    pub breath_at: f64,
    pub breath_until: f64,
    pub appear_breath_at: f64,
    pub appear_breath_until: f64,
    pub rect: Rect,
    pub wipe: Option<Wipe>,
    /// Every character has appeared, never before the line ends (the spotlight goes out after it).
    pub done: f64,
}

pub struct Hit {
    pub t: f64,
    pub x: f32,
    pub y: f32,
    pub r: f32,
    pub sat: Vec<[f32; 3]>,
    pub seed: f32,
}

pub struct Scene {
    pub lines: Vec<Line>,
    pub big: Vec<Hit>,

    pub seed: u32,
}

impl Line {
    /// The original `yieldFactor`: a line making room fogs fast before the next one is written, or vanishes
    /// in the breath that covers it.
    pub fn yield_factor(&self, time: f64) -> f32 {
        let gone = if self.breath_at.is_finite() {
            smooth((time - self.breath_at) / BREATH_SPREAD)
        } else {
            smooth(time - self.yield_at + 0.6)
        };
        (1.0 - gone) as f32
    }
}

impl Char {
    /// When the character starts to appear: written from `wt`, faded in from `t`.
    pub fn start(&self) -> f64 {
        if self.appear_dur > 0.0 { self.t } else { self.wt }
    }
}

/// The scene for this song, size and tuning; rebuilt when a skeleton finishes in the background.
pub fn scene(lyrics: &Lyrics, width: f32, height: f32, font_scale: f32, tuning: &Values) -> Arc<Scene> {
    let song = layout::song(lyrics, width, height, font_scale, tuning);
    let epoch = ink::epoch();
    let mut slot = cache().lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    if let Some((held, at, scene)) = slot.as_ref()
        && Arc::ptr_eq(held, &song)
        && *at == epoch
    {
        return Arc::clone(scene);
    }
    let built = Arc::new(build(&song, width, height, tuning));
    *slot = Some((song, epoch, Arc::clone(&built)));
    built
}

type Cached = Option<(Arc<Song>, u64, Arc<Scene>)>;

fn cache() -> &'static Mutex<Cached> {
    static CACHE: Mutex<Cached> = Mutex::new(None);
    &CACHE
}

fn build(song: &Song, width: f32, height: f32, tuning: &Values) -> Scene {
    let list = manifest();
    let warp = tuning.number(list, "warp");
    let speed = f64::from(tuning.number(list, "writeSpeed")).max(0.4);
    let appear: Vec<&str> =
        ["write", "fade", "breath"].into_iter().filter(|option| tuning.member(list, "appear", option)).collect();
    let timing = song.timing;
    let mut lines: Vec<Line> = song
        .spans
        .iter()
        .enumerate()
        .map(|(li, span)| {
            let mut chars: Vec<Char> = song
                .chars
                .iter()
                .filter(|laid| laid.line == li)
                .enumerate()
                .map(|(i, laid)| decorate(laid, li, i, warp))
                .collect();
            let mut line = Line {
                t: span.start,
                end: span.end,
                fingers: span.fingers,
                fog_at: timing.fog_at(&song.spans, li),
                yield_at: f64::INFINITY,
                breath_at: f64::INFINITY,
                breath_until: f64::INFINITY,
                appear_breath_at: f64::INFINITY,
                appear_breath_until: f64::INFINITY,
                rect: rect_of(&chars),
                wipe: None,
                done: span.end,
                chars: Vec::new(),
            };
            apply_appear(&mut line, &mut chars, span.next, &appear, speed, song.seed, li);
            line.chars = chars;
            line
        })
        .collect();
    align_credits(&mut lines, &song.spans);
    yield_places(&mut lines, &song.spans, timing);
    for (li, line) in lines.iter_mut().enumerate() {
        line.done = line
            .chars
            .iter()
            .filter(|c| c.glyph.is_some())
            .fold(line.end, |end, c| end.max(c.start() + if c.appear_dur > 0.0 { c.appear_dur } else { c.wd }));
        if line.fingers {
            line.wipe = wipe_of(line.rect, li);
        }
    }
    let big = big_hits(song, &lines, width, height);
    Scene { lines, big, seed: song.seed }
}

/// One character with its warped skeleton, edge beads, drips and end beads (the original `decorate`).
fn decorate(laid: &layout::Laid, li: usize, i: usize, warp: f32) -> Char {
    let mut c = Char {
        ch: laid.ch,
        x: laid.x,
        y: laid.y,
        s: laid.s,
        rot: laid.rot,
        t: laid.t,
        d: laid.d,
        wt: laid.t,
        wd: 0.0,
        appear_dur: 0.0,
        glyph: None,
        beads: Vec::new(),
        drips: Vec::new(),
        ends: Vec::new(),
    };
    let Some(base) = ink::glyph(laid.ch) else { return c };
    let mut rnd = Rng((li as u32).wrapping_mul(131).wrapping_add((i as u32).wrapping_mul(17)).wrapping_add(5));
    let mut shape = Rng((rnd.next() * 1e9) as u32);
    let g = warp_glyph(&base, &mut shape, warp);
    c.beads = g
        .strokes
        .iter()
        .map(|st| {
            let mut out = Vec::new();
            let step = 7.0 / c.s;
            let mut s = step * rnd.unit();
            while s < st.len {
                if rnd.unit() <= BEADS {
                    let [x, y, nx, ny] = st.point_at(s);
                    let side = if rnd.unit() < 0.5 { -1.0 } else { 1.0 };
                    let r = 1.6 + rnd.unit().powf(2.2) * 3.6;
                    out.push(Bead { s, x, y, nx: nx * side, ny: ny * side, r, j: rnd.unit() });
                }
                s += step * (0.5 + rnd.unit());
            }
            out
        })
        .collect();
    c.drips = g
        .strokes
        .iter()
        .map(|_| {
            (rnd.unit() < DRIP * 0.22).then(|| Drip {
                delay: 0.3 + rnd.next() * 1.6,
                len: 30.0 + rnd.unit() * 130.0,
                dur: 2.0 + rnd.next() * 3.5,
                r: 3.2 + rnd.unit() * 2.6,
                seed: rnd.unit() * 100.0,
            })
        })
        .collect();
    c.ends = g.strokes.iter().map(|_| (rnd.unit(), rnd.unit())).collect();
    c.glyph = Some(g);
    c
}

/// Handwriting: the whole character sheared, squashed and bent a little; each stroke moved, turned and
/// lengthened a little, with a tremor along it (the original `warpGlyph`).
fn warp_glyph(g: &Ink, rnd: &mut Rng, j: f32) -> Ink {
    if j <= 0.0 {
        return g.clone();
    }
    let mut r = || rnd.unit() * 2.0 - 1.0;
    let shear = r() * 0.16 * j;
    let sx = 1.0 + r() * 0.1 * j;
    let sy = 1.0 + r() * 0.1 * j;
    let bx = r() * 0.035 * j;
    let by = r() * 0.035 * j;
    let px = rnd.unit() * std::f32::consts::TAU;
    let py = rnd.unit() * std::f32::consts::TAU;
    let whole = |x: f32, y: f32| {
        let mut u = (x - 0.5) * sx;
        let mut v = (y - 0.5) * sy;
        u += v * shear;
        u += bx * (v * std::f32::consts::PI * 1.6 + px).sin();
        v += by * (u * std::f32::consts::PI * 1.6 + py).sin();
        (u + 0.5, v + 0.5)
    };
    let sj = (8.0 / g.strokes.len() as f32).clamp(0.35, 1.0);
    let polys = g
        .strokes
        .iter()
        .map(|st| {
            let n = st.pts.len();
            let cx = st.pts.iter().map(|p| p.0).sum::<f32>() / n as f32;
            let cy = st.pts.iter().map(|p| p.1).sum::<f32>() / n as f32;
            let mut r = || rnd.unit() * 2.0 - 1.0;
            let rot = r() * 0.09 * j * sj;
            let len = 1.0 + r() * 0.1 * j * sj;
            let ox = r() * 0.022 * j * sj;
            let oy = r() * 0.022 * j * sj;
            let (sr, cr) = rot.sin_cos();
            let (x0, y0) = st.pts[0];
            let tremor = 0.006 * j;
            let tf = 18.0 + rnd.unit() * 14.0;
            let tp = rnd.unit() * std::f32::consts::TAU;
            (0..n)
                .map(|i| {
                    let (x, y) = st.pts[i];
                    let u = x0 + (x - x0) * len;
                    let v = y0 + (y - y0) * len;
                    let (du, dv) = (u - cx, v - cy);
                    let u = cx + du * cr - dv * sr + ox;
                    let v = cy + du * sr + dv * cr + oy;
                    let a = st.pts[i.saturating_sub(1)];
                    let b = st.pts[(i + 1).min(n - 1)];
                    let (tx, ty) = (b.0 - a.0, b.1 - a.1);
                    let tl = (tx * tx + ty * ty).sqrt().max(1e-6);
                    let k = (st.cum[i] * tf + tp).sin() * tremor;
                    whole(u - ty / tl * k, v + tx / tl * k)
                })
                .collect()
        })
        .collect();
    ink::ink_from_polylines(polys).unwrap_or_else(|| g.clone())
}

fn natural_write_time(g: &Ink, speed: f64) -> f64 {
    (f64::from(g.total) * 0.28 + (g.strokes.len() as f64 - 1.0) * 0.06) / speed
}

/// One character after another, never before it is sung; returns the starts and when the last one ends.
fn schedule_write(chars: &[Char], nat: &[f64], k: f64, starts: &mut Vec<f64>) -> f64 {
    starts.clear();
    let mut end = f64::NEG_INFINITY;
    for (c, n) in chars.iter().filter(|c| c.glyph.is_some()).zip(nat) {
        let t0 = c.t.max(end);
        end = t0 + n * k;
        starts.push(t0);
    }
    end
}

/// The original `applyAppear`: write the line at one pace that finishes by the next line (at most 0.3x the
/// natural time); a line that cannot finish even then fades in when fading is on.
fn apply_appear(line: &mut Line, chars: &mut [Char], next: f64, appear: &[&str], speed: f64, seed: u32, li: usize) {
    let kinds: Vec<&str> = appear.iter().copied().filter(|kind| *kind != "fade").collect();
    let can_fade = appear.contains(&"fade");
    let pick = Rng(seed.wrapping_add((li as u32).wrapping_mul(613)).wrapping_add(11)).next();
    let kind = (!kinds.is_empty()).then(|| kinds[((pick * kinds.len() as f64) as usize).min(kinds.len() - 1)]);
    let nat: Vec<f64> = chars.iter().filter_map(|c| c.glyph.as_ref()).map(|g| natural_write_time(g, speed)).collect();
    let until = next.max(line.end);
    let mut starts = Vec::with_capacity(nat.len());
    let mut k = 1.0;
    if schedule_write(chars, &nat, 1.0, &mut starts) > until {
        let (mut lo, mut hi) = (0.3, 1.0);
        for _ in 0..14 {
            let mid = (lo + hi) / 2.0;
            if schedule_write(chars, &nat, mid, &mut starts) > until {
                hi = mid;
            } else {
                lo = mid;
            }
        }
        k = lo;
    }
    let end = schedule_write(chars, &nat, k, &mut starts);
    let fade = kind.is_none() || (can_fade && !nat.is_empty() && end > until + 1e-3);
    for ((c, start), n) in chars.iter_mut().filter(|c| c.glyph.is_some()).zip(&starts).zip(&nat) {
        c.wt = *start;
        c.wd = n * k;
    }
    if kind == Some("breath")
        && !fade
        && let Some(first) = starts.first()
    {
        line.appear_breath_at = first - BREATH_SPREAD - 0.3;
        line.appear_breath_until = line.end + BREATH_HOLD;
    }
    for c in chars.iter_mut() {
        c.appear_dur = if fade { (c.d * 3.0).clamp(0.35, 1.2) } else { 0.0 };
    }
}

/// The artist is written after the title: its writing moves to 0.4 s after the title's last character.
fn align_credits(lines: &mut [Line], spans: &[layout::Span]) {
    let mut credits = spans.iter().enumerate().filter(|(_, span)| span.credits).map(|(i, _)| i);
    let (Some(title), Some(artist)) = (credits.next(), credits.next()) else { return };
    let Some(last) = lines[title].chars.iter().rev().find(|c| c.glyph.is_some()) else { return };
    let ready = last.wt + last.wd + 0.4;
    let shift = lines[artist].chars.iter().find(|c| c.glyph.is_some()).map_or(0.0, |first| (ready - first.wt).max(0.0));
    for c in &mut lines[artist].chars {
        c.wt += shift;
    }
}

/// Lines that overlap a later line make room: the line lingers while the glass is empty, fogs early enough
/// for the later line, and is breathed away when even that is too late.
fn yield_places(lines: &mut [Line], spans: &[layout::Span], timing: Timing) {
    for i in 0..lines.len() {
        if spans[i].credits {
            continue;
        }
        let fade = timing.fade_of(spans[i].fingers);
        let linger = timing.linger_at(spans, i);
        let gone = linger + fade;
        let a = lines[i].rect;
        let mut by = None;
        for (j, later) in lines.iter().enumerate().skip(i + 1) {
            if later.t >= gone {
                break;
            }
            let b = later.rect;
            if a.x1.min(b.x1) > a.x0.max(b.x0) && a.y1.min(b.y1) > a.y0.max(b.y0) {
                by = Some(j);
                break;
            }
        }
        let fog = timing.fog_at(spans, i);
        let yield_at = by.map_or(f64::INFINITY, |j| lines[j].t);
        let line = &mut lines[i];
        line.yield_at = yield_at;
        line.fog_at = fog.max(linger.min(yield_at - fade));
        if let Some(j) = by
            && !spans[j].credits
            && yield_at < line.fog_at + fade
        {
            line.breath_at = yield_at - BREATH_SPREAD - 0.3;
            line.breath_until = spans[j].end + BREATH_HOLD;
        }
    }
}

fn rect_of(chars: &[Char]) -> Rect {
    let mut rect = Rect { x0: f32::INFINITY, x1: f32::NEG_INFINITY, y0: f32::INFINITY, y1: f32::NEG_INFINITY, s: 0.0 };
    for c in chars.iter().filter(|c| c.glyph.is_some()) {
        rect.x0 = rect.x0.min(c.x - c.s / 2.0);
        rect.x1 = rect.x1.max(c.x + c.s / 2.0);
        rect.y0 = rect.y0.min(c.y - c.s / 2.0);
        rect.y1 = rect.y1.max(c.y + c.s / 2.0);
        rect.s = rect.s.max(c.s);
    }
    rect
}

/// Three or four fingers side by side wipe the line back and forth (the original `buildWipe`).
fn wipe_of(rect: Rect, li: usize) -> Option<Wipe> {
    if !rect.x0.is_finite() {
        return None;
    }
    let mut rnd = Rng((li as u32).wrapping_mul(4241).wrapping_add(77));
    let fw = (rect.s * 0.26).max(14.0);
    let n = 3 + usize::from(rnd.unit() < 0.5);
    let gap = fw * (1.25 + rnd.unit() * 0.25);
    let hand = (n as f32 - 1.0) * gap + fw;
    let cy = (rect.y0 + rect.y1) / 2.0;
    let passes = (((rect.y1 - rect.y0) / (hand * 0.8)).ceil() as usize + 2).clamp(3, 5);
    let span = (rect.y1 - rect.y0) - hand * 0.6;
    let start_dir = if rnd.unit() < 0.7 { 1.0 } else { -1.0 };
    let pad = rect.s * 0.35;
    let mut streaks = Vec::new();
    for k in 0..passes {
        let dir = if k % 2 == 0 { start_dir } else { -start_dir };
        let yc = cy + (k as f32 / (passes - 1) as f32 - 0.5) * span.max(0.0) + (rnd.unit() - 0.5) * fw * 0.8;
        let shrink = 1.0 - k as f32 * 0.08;
        let mid = (rect.x0 + rect.x1) / 2.0;
        let half = ((rect.x1 - rect.x0) / 2.0 + pad) * shrink;
        let xa = mid - dir * half + (rnd.unit() - 0.5) * rect.s * 0.3;
        let xb = mid + dir * half + (rnd.unit() - 0.5) * rect.s * 0.3;
        let tilt = (rnd.unit() - 0.5) * 0.16;
        let bow = (rnd.unit() - 0.5) * rect.s * 0.35;
        let a = k as f32 / passes as f32 * 0.92;
        let b = ((k + 1) as f32 / passes as f32 * 0.92 + 0.08).min(1.0);
        for f in 0..n {
            let y_off = (f as f32 - (n as f32 - 1.0) / 2.0) * gap + (rnd.unit() - 0.5) * fw * 0.25;
            let len_k = if f == n - 1 { 0.84 + rnd.unit() * 0.1 } else { 0.95 + rnd.unit() * 0.05 };
            let wav = rect.s * 0.04 * rnd.unit();
            let pts = (0..=24)
                .map(|i| {
                    let t = i as f32 / 24.0;
                    let x = xa + (xb - xa) * len_k * t;
                    let y = yc
                        + y_off
                        + bow * (std::f32::consts::PI * t).sin()
                        + tilt * (x - mid)
                        + wav * (t * 5.0 + f as f32).sin();
                    (x, y)
                })
                .collect();
            streaks.push(Streak { pts, w: fw * (0.85 + rnd.unit() * 0.3), a, b });
        }
    }
    let mut beads = Vec::new();
    for st in &streaks[streaks.len() - n..] {
        let (ex, ey) = st.pts[st.pts.len() - 1];
        beads.push((ex, ey, st.w * 0.22 + 1.5 * rnd.unit()));
        if rnd.unit() < 0.6 {
            let back = (st.pts[1].0 - st.pts[0].0) * 0.6;
            beads.push((ex - back, ey + (rnd.unit() - 0.5) * st.w * 0.6, 1.5 + rnd.unit() * 2.5));
        }
    }
    Some(Wipe { streaks, beads, dur: 0.32 * passes as f64 })
}

impl Wipe {
    /// How far streak `st` is drawn at overall progress `ue`.
    pub fn progress(st: &Streak, ue: f32) -> f32 {
        ((ue - st.a) / (st.b - st.a)).clamp(0.0, 1.0)
    }

    /// True where the fingers have already swept the edge beads away.
    pub fn swept(&self, ue: f32, x: f32, y: f32) -> bool {
        self.streaks.iter().any(|st| {
            let lp = Wipe::progress(st, ue);
            if lp <= 0.0 {
                return false;
            }
            let p0 = st.pts[0].0;
            let p1 = st.pts[st.pts.len() - 1].0;
            let front = p0 + (p1 - p0) * lp;
            if (x - p0) * (front - x) < 0.0 {
                return false;
            }
            let k = ((x - p0) / (p1 - p0)).clamp(0.0, 1.0);
            let py = st.pts[(k * (st.pts.len() - 1) as f32).round() as usize].1;
            (y - py).abs() < st.w * 0.7
        })
    }
}

/// Big drops land on six in ten word starts, away from the line being sung (the original `buildBig`
/// without a vocal track). Sizes are for 1x; `rainBig` scales them when drawn.
fn big_hits(song: &Song, lines: &[Line], width: f32, height: f32) -> Vec<Hit> {
    let mut rnd = Rng(song.seed.wrapping_mul(0x9e37_79b9) ^ 911);
    let k = width.min(height) / 720.0;
    let mut out = Vec::new();
    for laid in song.chars.iter().filter(|laid| laid.lead) {
        if rnd.unit() >= 0.6 {
            continue;
        }
        let rect = lines[laid.line].rect;
        let (mut x, mut y) = (0.0, 0.0);
        for _ in 0..8 {
            x = width * (0.03 + 0.94 * rnd.unit());
            y = height * (0.03 + 0.94 * rnd.unit());
            if x < rect.x0 - 20.0 || x > rect.x1 + 20.0 || y < rect.y0 - 20.0 || y > rect.y1 + 20.0 {
                break;
            }
        }
        let r = (7.0 + rnd.unit() * 6.0) * k;
        let count = 4 + (rnd.unit() * 5.0) as usize;
        let sat = (0..count)
            .map(|_| {
                let a = rnd.unit() * std::f32::consts::TAU;
                let d = r * (1.5 + rnd.unit() * 1.4);
                [a.cos() * d, a.sin() * d * 0.85 + r * 0.2, (1.0 + rnd.unit() * 1.6) * k]
            })
            .collect();
        out.push(Hit { t: laid.t, x, y, r, sat, seed: rnd.unit() * 100.0 });
    }
    out.sort_by(|a, b| a.t.total_cmp(&b.t));
    out
}
