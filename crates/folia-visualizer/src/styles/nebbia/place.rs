// One frame on the glass, as the original nebbiaMask.ts, nebbiaInk.ts, nebbiaHits.ts and nebbiaSpot.ts:
// wiped segments for the mask (mask pixels), drop sprites (CSS pixels), breath lobes and the spotlight.
// Everything is a function of playback time; the buffers are refilled each frame, not reallocated.

use std::sync::Mutex;

use super::scene::{Char, Drip, Hit, Line, Scene, Wipe};
use super::{Rng, smooth};

/// The wipe mask is this fraction of the CSS pixel size (the original `MASK_SCALE`).
pub const MASK_SCALE: f32 = 0.5;
/// Beads outlast the wipe they sit on by this many seconds (`NEBBIA_FIXED.beadHold`).
const BEAD_HOLD: f64 = 4.0;
/// Characters smaller than this many pixels get no end bead (`NEBBIA_FIXED.endBeadMin`).
const END_BEAD_MIN: f32 = 48.0;
/// Small raindrops are drawn per 20 ms bucket.
const BUCKET: f64 = 0.02;

/// One wiped segment: x0, y0, x1, y1, width (mask pixels), clarity 0..1.
pub type Segment = [f32; 6];
/// One drop sprite: centre x, y, width, height (CSS pixels), refraction size, alpha.
pub type Drop = [f32; 6];

#[derive(Default)]
pub struct Paint {
    pub segments: Vec<Segment>,
    pub drops: Vec<Drop>,
    path: Vec<(f32, f32)>,
}

fn store() -> &'static Mutex<Paint> {
    static STORE: Mutex<Paint> = Mutex::new(Paint { segments: Vec::new(), drops: Vec::new(), path: Vec::new() });
    &STORE
}

pub fn with_paint(read: impl FnOnce(&Paint)) {
    read(&store().lock().unwrap_or_else(std::sync::PoisonError::into_inner));
}

/// The frame's tuning, read once.
pub struct Knobs {
    pub stroke_width: f32,
    pub write_speed: f32,
    pub refog: f64,
    pub rain_hit: f32,
    pub rain_big: f32,
}

/// What the window shader needs besides the textures.
pub struct Drive {
    pub lobes: [[f32; 4]; 8],
    pub lobe_sd: [[f32; 4]; 4],
    /// Spotlight target x, y and radius in CSS pixels, then its intensity 0..1.
    pub spot: [f32; 4],
}

/// Collects this frame's segments and drops into the shared buffers and returns the rest.
pub fn paint(scene: &Scene, time: f64, width: f32, height: f32, knobs: &Knobs, energy: f32) -> Drive {
    let mut held = store().lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    let Paint { segments, drops, path } = &mut *held;
    segments.clear();
    drops.clear();
    path.clear();
    let mut pen = Pen { out: segments, path, m: [1.0, 0.0, 0.0, 1.0, 0.0, 0.0], k: 1.0 };
    let mut glass = Glass { pen: &mut pen, drops, knobs };
    for line in &scene.lines {
        if time >= line.t - 0.5 {
            glass.line(line, time);
        }
    }
    glass.hits(scene, time, width, height, energy);
    let mut drive = Drive { lobes: [[0.0; 4]; 8], lobe_sd: [[0.0; 4]; 4], spot: spot(scene, time) };
    let mut n = 0;
    for (index, line) in scene.lines.iter().enumerate() {
        n = push_breath(&mut drive, line, index, time, line.breath_at, line.breath_until, n);
        n = push_breath(&mut drive, line, index, time, line.appear_breath_at, line.appear_breath_until, n);
    }
    drive
}

/// The original `NebbiaSegments`: canvas-style paths turned into segments in mask pixels.
struct Pen<'a> {
    out: &'a mut Vec<Segment>,
    path: &'a mut Vec<(f32, f32)>,
    m: [f32; 6],
    k: f32,
}

impl Pen<'_> {
    fn transform(&mut self, m: [f32; 6]) {
        self.m = m;
        self.k = m[0].hypot(m[1]);
    }

    fn move_to(&mut self, x: f32, y: f32) {
        if !self.path.is_empty() {
            self.path.push((f32::NAN, f32::NAN));
        }
        self.line_to(x, y);
    }

    fn line_to(&mut self, x: f32, y: f32) {
        let m = self.m;
        self.path.push((m[0] * x + m[2] * y + m[4], m[1] * x + m[3] * y + m[5]));
    }

    fn stroke(&mut self, width: f32, v: f32) {
        let w = width * self.k;
        for pair in self.path.windows(2) {
            let (a, b) = (pair[0], pair[1]);
            if a.0.is_nan() || b.0.is_nan() {
                continue;
            }
            self.out.push([a.0, a.1, b.0, b.1, w, v]);
        }
        self.path.clear();
    }
}

/// Stroke width and the two fade levels a drip inherits from its character.
#[derive(Clone, Copy)]
struct Wet {
    width: f32,
    clarity: f32,
    bead_v: f32,
}

struct Glass<'a, 'p> {
    pen: &'a mut Pen<'p>,
    drops: &'a mut Vec<Drop>,
    knobs: &'a Knobs,
}

const SCREEN: [f32; 6] = [MASK_SCALE, 0.0, 0.0, MASK_SCALE, 0.0, 0.0];

impl Glass<'_, '_> {
    /// A drop of visible radius `r`; the sprite quad is 8r (the original `put` without bumps).
    fn put(&mut self, x: f32, y: f32, r: f32, alpha: f32, sx: f32, sy: f32) {
        if r < 0.5 || alpha <= 0.01 {
            return;
        }
        let w = r * 8.0 * sx;
        self.drops.push([x, y, w, r * 8.0 * sy, w / 100.0, alpha.min(1.0)]);
    }

    /// One line: it fogs over slowly, or fingers wipe it (the original `drawLine`).
    fn line(&mut self, line: &Line, time: f64) {
        let refog = self.knobs.refog;
        let yf = line.yield_factor(time);
        let Some(wipe) = line.wipe.as_ref().filter(|_| line.fingers) else {
            let v = (1.0 - smooth((time - line.fog_at) / refog)) as f32 * yf;
            let vb = (1.0 - smooth((time - line.fog_at - BEAD_HOLD) / (refog * 3.0))) as f32 * yf;
            if vb > 0.002 {
                for c in &line.chars {
                    self.char(c, time, v, vb, None);
                }
            }
            return;
        };
        let early = if line.breath_at.is_finite() { f64::NEG_INFINITY } else { line.yield_at - wipe.dur - 0.9 };
        let t0 = if line.fog_at > early && early >= line.end { early } else { line.fog_at };
        let ue = ((time - t0) / wipe.dur).clamp(0.0, 1.0) as f32;
        let vw = (1.0 - smooth((time - t0 - wipe.dur - 0.5) / refog)) as f32 * yf;
        let vbw = (1.0 - smooth((time - t0 - wipe.dur - 0.5 - BEAD_HOLD) / (refog * 3.0))) as f32 * yf;
        if vbw <= 0.002 {
            return;
        }
        let swept = (ue > 0.0).then_some((wipe, ue));
        for c in &line.chars {
            if ue <= 0.0 {
                self.char(c, time, yf, yf, None);
            } else {
                self.char(c, time, vw, vbw, swept);
            }
        }
        if ue <= 0.0 {
            return;
        }
        self.pen.transform(SCREEN);
        if vw > 0.002 {
            for st in &wipe.streaks {
                let lp = Wipe::progress(st, ue);
                if lp > 0.0 {
                    partial_streak(self.pen, &st.pts, lp);
                    self.pen.stroke(st.w, vw);
                }
            }
        }
        if ue >= 1.0 {
            for &(x, y, r) in &wipe.beads {
                self.put(x, y, r * (0.55 + 0.45 * vbw), vbw, 1.0, 1.0);
            }
        }
    }

    /// One character at `time`, stroke by stroke (the original `drawChar`). A fading character is drawn whole
    /// and without beads.
    fn char(&mut self, c: &Char, time: f64, clarity_in: f32, bead_in: f32, swept: Option<(&Wipe, f32)>) {
        let Some(g) = c.glyph.as_ref() else { return };
        let fade_in = c.appear_dur > 0.0;
        if time < c.start() {
            return;
        }
        let (mut clarity, mut bead_v) = (clarity_in, bead_in);
        if fade_in {
            let a = smooth((time - c.t) / c.appear_dur) as f32;
            clarity *= a;
            bead_v *= a;
        }
        let gap = (0.06 / f64::from(self.knobs.write_speed)).min(c.wd * 0.08);
        let ink_time = c.wd - gap * (g.strokes.len() as f64 - 1.0);
        let mut local = time - c.start();
        let (sn, cs) = c.rot.sin_cos();
        let k = MASK_SCALE;
        let char_m = [
            cs * c.s * k,
            sn * c.s * k,
            -sn * c.s * k,
            cs * c.s * k,
            (c.x - c.s / 2.0 * (cs - sn)) * k,
            (c.y - c.s / 2.0 * (sn + cs)) * k,
        ];
        self.pen.transform(char_m);
        let thin = (9.0 / g.strokes.len() as f32).sqrt().clamp(0.55, 1.0);
        let width = (self.knobs.stroke_width * thin).max(8.0 * thin / c.s);
        let alive = |x: f32, y: f32| match swept {
            Some((wipe, ue)) if wipe.swept(ue, x, y) => 0.0,
            _ => 1.0,
        };
        for (si, st) in g.strokes.iter().enumerate() {
            if local <= 0.0 {
                break;
            }
            let dur = if fade_in { 0.0 } else { ink_time * f64::from(st.len / g.total) };
            let p = if dur > 0.0 { (local / dur).clamp(0.0, 1.0) } else { 1.0 };
            let drawn = (p * 0.4 + smooth(p) * 0.6) as f32 * st.len;
            if clarity > 0.002 {
                partial_stroke(self.pen, st, drawn);
                self.pen.stroke(width, clarity);
            }
            if !fade_in {
                for b in c.beads[si].iter().take_while(|b| b.s <= drawn) {
                    let off = width / 2.0 + (b.r * 0.7 + b.j * 1.5) / c.s;
                    let (x, y) = to_screen(c, b.x + b.nx * off, b.y + b.ny * off);
                    let bv = bead_v * alive(x, y);
                    self.put(x, y, b.r * (0.55 + 0.45 * bv), bv, 1.0 + b.j * 0.25, 1.0 + (1.0 - b.j) * 0.3);
                }
                if drawn > 0.0 && c.s >= END_BEAD_MIN {
                    let (h, j) = c.ends[si];
                    let [ux, uy, _, _] = st.point_at(drawn);
                    let [vx, vy, _, _] = st.point_at((drawn - 2.0 / c.s).max(0.0));
                    let len = (ux - vx).hypot(uy - vy);
                    let (dx, dy) = if len > 0.0 { ((ux - vx) / len, (uy - vy) / len) } else { (0.0, 0.0) };
                    let r = c.s * (0.031 + 0.018 * h) * (0.5 + 0.5 * smooth(p) as f32);
                    let (x, y) = to_screen(c, ux + dx * r * 0.35 / c.s, uy + (dy * r * 0.35 + r * 0.15) / c.s);
                    let ev = bead_v * alive(x, y);
                    self.put(x, y, r * (0.6 + 0.4 * ev), ev, 1.0, 1.12 + j * 0.12);
                }
                if let Some(d) = &c.drips[si]
                    && p >= 1.0
                    && local - dur > d.delay
                {
                    self.drip(c, st, d, local - dur - d.delay, Wet { width, clarity, bead_v });
                    self.pen.transform(char_m);
                }
            }
            local -= dur + if fade_in { 0.0 } else { gap };
        }
    }

    /// A drop running down from the stroke's lowest point, leaving a wiped trail (the original `drawDrip`).
    fn drip(&mut self, c: &Char, st: &super::ink::Stroke, d: &Drip, age: f64, wet: Wet) {
        let Wet { width, clarity, bead_v } = wet;
        let (mut bx, mut by) = (0.0, f32::NEG_INFINITY);
        for &(u, v) in &st.pts {
            let (x, y) = to_screen(c, u, v);
            if y > by {
                (bx, by) = (x, y);
            }
        }
        by += width * c.s * 0.35;
        let u = (age / d.dur).clamp(0.0, 1.0) as f32;
        let tau = std::f32::consts::PI * 6.0;
        let dist = d.len * (u - (u * tau).sin() / tau);
        let r = d.r * (1.0 - 0.35 * u);
        let seed = d.seed;
        let wob = |y: f32| (y * 0.07 + seed).sin() * 1.6 + (y * 0.023 + seed * 2.0).sin() * 2.5;
        self.pen.transform(SCREEN);
        if dist > 1.0 && clarity > 0.002 {
            trail(self.pen, bx, (by, by + dist), r * 1.5, clarity * 0.95, seed, |y| wob(y - by));
        }
        let mut y = 14.0 + (seed % 7.0) * 2.0;
        while y < dist - r * 2.0 {
            self.put(bx + wob(y), by + y, r * (0.3 + 0.2 * (((y + seed) * 0.37) % 1.0)), bead_v, 1.0, 1.0);
            y += 18.0 + ((y * 7.0 + seed * 13.0) % 11.0);
        }
        let speed = 1.0 - (u * tau).cos();
        let stretch = if u < 1.0 { 0.35 * speed } else { 0.0 };
        self.put(bx + wob(dist), by + dist + r * 0.3, r, bead_v, 1.0, 1.0 + stretch);
    }

    /// Raindrops on the glass (the original `NebbiaHits.draw`): big ones on word starts, small ones at
    /// `(0.15 + 7 e²) × rainHit` per second, where `e` is the vocal-band energy.
    fn hits(&mut self, scene: &Scene, time: f64, width: f32, height: f32, energy: f32) {
        let refog = self.knobs.refog;
        let life = 0.4 + refog * 0.3 + BEAD_HOLD * 0.5 + refog;
        self.pen.transform(SCREEN);
        let size = self.knobs.rain_big;
        let from = scene.big.partition_point(|hit| hit.t < time - life);
        for hit in scene.big[from..].iter().take_while(|hit| hit.t <= time) {
            if size > 0.0 && hit.r * size >= 1.0 {
                self.hit(hit, true, size, time, refog);
            }
        }
        let level = f64::from(energy);
        let rate = (0.15 + 7.0 * level * level) * f64::from(self.knobs.rain_hit) * BUCKET;
        if rate <= 0.0 {
            return;
        }
        let k = width.min(height) / 720.0;
        let last = (time / BUCKET).floor() as i64;
        for b in (last - (life / BUCKET).ceil() as i64).max(0)..=last {
            let mut rnd = Rng((scene.seed ^ 0x5ad1).wrapping_add((b as u32).wrapping_mul(0x9e37_79b1)));
            if rnd.next() >= rate {
                continue;
            }
            let t = (b as f64 + rnd.next()) * BUCKET;
            let x = width * (0.03 + 0.94 * rnd.unit());
            let y = height * (0.03 + 0.94 * rnd.unit());
            let small = Hit { t, x, y, r: (2.0 + rnd.unit() * 3.5) * k, sat: Vec::new(), seed: rnd.unit() * 100.0 };
            if small.t <= time {
                self.hit(&small, false, 1.0, time, refog);
            }
        }
    }

    fn hit(&mut self, hit: &Hit, big: bool, s: f32, time: f64, refog: f64) {
        let r = hit.r * s;
        let age = time - hit.t;
        let v = (1.0 - smooth((age - 0.4) / (refog * 0.3))) as f32;
        let bv = (1.0 - smooth((age - 0.4 - BEAD_HOLD * 0.5) / refog)) as f32;
        if big && v > 0.002 {
            for j in 0..7 {
                let a = hit.seed + j as f32 * 0.9;
                let (x, y) = (hit.x + a.cos() * r * 0.3, hit.y + (a * 1.3).sin() * r * 0.3);
                self.pen.move_to(x, y);
                self.pen.line_to(x + 0.1, y);
            }
            self.pen.stroke(r * 1.3, v);
        }
        if bv <= 0.01 {
            return;
        }
        let sq = (-age / 0.05).exp() as f32;
        self.put(hit.x, hit.y + r * 0.1, r * 0.9 * (0.6 + 0.4 * bv), bv, 1.0 + 0.5 * sq, 1.0 - 0.35 * sq);
        let out = (age / 0.08).clamp(0.0, 1.0) as f32;
        for &[dx, dy, rr] in &hit.sat {
            self.put(hit.x + dx * s * out, hit.y + dy * s * out, rr * (0.55 + 0.45 * bv), bv * out, 1.0, 1.0);
        }
    }
}

/// Em coordinates of a character to CSS pixels.
fn to_screen(c: &Char, u: f32, v: f32) -> (f32, f32) {
    let (sn, cs) = c.rot.sin_cos();
    let lx = (u - 0.5) * c.s;
    let ly = (v - 0.5) * c.s;
    (c.x + cs * lx - sn * ly, c.y + sn * lx + cs * ly)
}

/// The part of a stroke up to arc length `l`, as the current path.
fn partial_stroke(pen: &mut Pen<'_>, st: &super::ink::Stroke, l: f32) {
    pen.move_to(st.pts[0].0, st.pts[0].1);
    for i in 1..st.pts.len() {
        let (a, b) = (st.pts[i - 1], st.pts[i]);
        if st.cum[i] >= l {
            let k = (l - st.cum[i - 1]) / (st.cum[i] - st.cum[i - 1]).max(1e-6);
            pen.line_to(a.0 + (b.0 - a.0) * k, a.1 + (b.1 - a.1) * k);
            return;
        }
        pen.line_to(b.0, b.1);
    }
}

/// A finger streak drawn to fraction `lp` of its points.
fn partial_streak(pen: &mut Pen<'_>, pts: &[(f32, f32)], lp: f32) {
    let n = pts.len() - 1;
    let f = lp * n as f32;
    let i = f as usize;
    pen.move_to(pts[0].0, pts[0].1);
    for p in &pts[1..=i.min(n)] {
        pen.line_to(p.0, p.1);
    }
    if i < n {
        let (a, b, r) = (pts[i], pts[i + 1], f - i as f32);
        pen.line_to(a.0 + (b.0 - a.0) * r, a.1 + (b.1 - a.1) * r);
    }
}

/// The wiped trail of a running drop: about as wide as the drop, wavy, thinner towards the top.
fn trail(pen: &mut Pen<'_>, x: f32, (y0, y1): (f32, f32), width: f32, v: f32, seed: f32, wob: impl Fn(f32) -> f32) {
    let mut y = y0;
    while y < y1 {
        let ye = y1.min(y + 5.0);
        let along = (y - y0) / (y1 - y0).max(1.0);
        let w = width * (0.72 + 0.28 * along) * (0.85 + 0.3 * (0.5 + 0.5 * (y * 0.19 + seed * 3.0).sin()));
        pen.move_to(x + wob(y), y);
        pen.line_to(x + wob(ye), ye);
        pen.stroke(w, v);
        y += 4.0;
    }
}

/// The two spotlights aim at the character being written, sliding to the next one while it is written, and
/// go out 0.8 s after the line is done (the original `NebbiaSpot` with its easing as a function of time).
fn spot(scene: &Scene, time: f64) -> [f32; 4] {
    for line in scene.lines.iter().rev() {
        if time < line.t - 0.05 || time > line.done + 0.8 {
            continue;
        }
        let written = |c: &&Char| c.glyph.is_some() && c.start() <= time;
        let latest = line.chars.iter().enumerate().filter(|(_, c)| written(c));
        let Some((k, c)) = latest.max_by(|a, b| a.1.start().total_cmp(&b.1.start())) else {
            continue;
        };
        let first = line.chars.iter().filter(|c| c.glyph.is_some()).map(Char::start).fold(f64::INFINITY, f64::min);
        let on = (1.0 - (-(time - first) / 0.15).exp()) as f32;
        let Some(next) = line.chars[k + 1..].iter().find(|c| c.glyph.is_some()) else {
            return [c.x, c.y, c.s, on];
        };
        let u = smooth((time - c.start()) / (next.start() - c.start()).max(0.05)) as f32;
        return [c.x + (next.x - c.x) * u, c.y + (next.y - c.y) * u, c.s + (next.s - c.s) * u, on];
    }
    let gone = scene.lines.iter().filter(|line| line.done + 0.8 < time).max_by(|a, b| a.done.total_cmp(&b.done));
    let Some(line) = gone else { return [0.0; 4] };
    let Some(c) = line.chars.iter().rev().find(|c| c.glyph.is_some()) else { return [0.0; 4] };
    [c.x, c.y, c.s, (-(time - line.done - 0.8) / 0.5).exp() as f32]
}

/// A breath of mist over a line, from `at` until it thins out after `until`: one ellipse over the line and
/// a ring of smaller puffs. Up to eight lobes in all.
fn push_breath(drive: &mut Drive, line: &Line, index: usize, time: f64, at: f64, until: f64, mut n: usize) -> usize {
    if n >= 8 || !at.is_finite() || time < at || !line.rect.x0.is_finite() {
        return n;
    }
    let spread = 0.3;
    let shape = 1.0 - smooth((time - until.max(at + spread)) / 8.0);
    if shape <= 0.002 {
        return n;
    }
    let u = ((time - at) / spread).clamp(0.0, 1.0);
    let e = 1.0 - (1.0 - u).powi(3);
    let kx = (0.7 + 0.3 * e) as f32;
    let ky = (0.08 + 0.92 * e) as f32;
    let dense = (u * 4.0).clamp(0.0, 1.0) as f32;
    let rect = line.rect;
    let cx = (rect.x0 + rect.x1) / 2.0;
    let cy = (rect.y0 + rect.y1) / 2.0;
    let rx = ((rect.x1 - rect.x0) / 2.0 + rect.s * 0.3) * kx;
    let ry = ((rect.y1 - rect.y0) / 2.0 + rect.s * 0.3) * ky;
    write_lobe(drive, n, [cx, cy, rx * 0.95, ry * 0.95], shape as f32, dense);
    n += 1;
    let mut rng = Rng((index as u32).wrapping_mul(6151).wrapping_add(29));
    let extra = 5 + i32::from(rng.next() < 0.5);
    let r0 = rx.min(ry);
    for k in 0..extra {
        if n >= 8 {
            break;
        }
        let a = (k as f64 + rng.next() * 0.6) / f64::from(extra) * std::f64::consts::TAU;
        let r = r0 * (0.45 + rng.unit() * 0.25);
        let lobe = [cx + (a.cos() as f32) * rx * 0.6, cy + (a.sin() as f32) * ry * 0.6, r, r];
        write_lobe(drive, n, lobe, shape as f32, dense);
        n += 1;
    }
    n
}

fn write_lobe(drive: &mut Drive, n: usize, lobe: [f32; 4], shape: f32, dense: f32) {
    drive.lobes[n] = lobe;
    let pair = &mut drive.lobe_sd[n / 2];
    let at = if n.is_multiple_of(2) { 0 } else { 2 };
    pair[at] = shape;
    pair[at + 1] = dense;
}

/// One character for the shelf thumbnails, which have no street shader. `x`/`y` are from the stage centre.
pub struct Glyph {
    pub text: String,
    pub x: f32,
    pub y: f32,
    pub size: f32,
    pub rotation: f32,
    pub alpha: f32,
}

pub fn glyphs(scene: &Scene, time: f64, width: f32, height: f32, refog: f64) -> Vec<Glyph> {
    let mut out = Vec::new();
    for line in scene.lines.iter().filter(|line| time >= line.t - 0.5) {
        let clarity = (1.0 - smooth((time - line.fog_at) / refog)) as f32 * line.yield_factor(time);
        if clarity <= 0.01 {
            continue;
        }
        for c in line.chars.iter().filter(|c| c.start() <= time) {
            out.push(Glyph {
                text: c.ch.to_string(),
                x: c.x - width / 2.0,
                y: c.y - height / 2.0,
                size: c.s,
                rotation: c.rot,
                alpha: 0.92 * clarity,
            });
        }
    }
    out
}
