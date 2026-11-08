// Monet's lyric rail (`MonetLyricsRail`, `MonetRailLine`, `MonetRailTokens`, `monetRailLayout`, `monetRailMasks`):
// a window of lines on fixed tracks, the sweep and glow on sung words, the translation under the active line, and
// wheel paging through the lyrics.

use std::hash::{DefaultHasher, Hash, Hasher};
use std::rc::Rc;
use std::time::{Duration, Instant};

use folia_core::Line;
use folia_lyrics::{RenderHints, active_line};
use iced::widget::canvas::Frame;
use iced::{Color, Point, Rectangle};

use super::ink::{self, Mask};
use super::layout::{Layout, SEMIBOLD};
use super::motion::{self, SWAP};
use super::text::{self, Shaped};
use crate::anim::{Ease, tween};
use crate::{Place, Scene, color};

const MEDIUM: iced::Font = iced::Font { weight: iced::font::Weight::Medium, ..iced::Font::DEFAULT };
const IDLE_RESET: Duration = Duration::from_millis(1800);
const SETTLE: Duration = Duration::from_millis(1400);
const SCROLL_STEP: f32 = 72.0;
/// Pixels Chromium reports for one wheel notch on Windows.
const WHEEL_NOTCH: f32 = 100.0;
const ACTIVE_ROWS: usize = 14;
const INACTIVE_ROWS: usize = 2;
const TRACK_ROWS: usize = 2;
/// How much of a word's em box its glyphs cover: a text-shadow halo is about this dense next to the word, which the
/// soft box under it stands in for.
const GLYPH_INK: f32 = 0.4;
const CHORUS: [(f32, f32); 7] =
    [(0.14, 0.0), (0.12, 0.15), (0.085, 0.35), (0.05, 0.55), (0.022, 0.72), (0.007, 0.87), (0.0, 1.0)];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    Waiting,
    Active,
    Passed,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct Entry {
    index: usize,
    offset: i32,
    status: Status,
}

#[derive(Debug, Clone, Copy)]
struct Visual {
    index: usize,
    offset: i32,
    status: Status,
    y: f32,
    scale: f32,
    opacity: f32,
    blur: f32,
    base: f32,
    z: i32,
}

struct Token {
    start: usize,
    end: usize,
    timing: Option<(f64, f64)>,
}

#[derive(Clone, Copy)]
struct Span {
    start: usize,
    end: usize,
}

struct Track {
    text: String,
    rows: Vec<Span>,
    px: f32,
    line_height: f32,
    pad_top: f32,
    height: f32,
    clipped: bool,
}

/// A line's reserved box, measured once per layout (`measureMonetLineLayout`).
struct Measured {
    shaped: Shaped,
    /// Each glyph's ink, `(top, bottom)` from the baseline, for the glow halo.
    ink: Vec<(f32, f32)>,
    tokens: Vec<Token>,
    rows: Vec<Span>,
    visible: usize,
    clipped: bool,
    line_height: f32,
    pad_top: f32,
    text_height: f32,
    track: Option<Track>,
    height: f32,
}

/// Wheel paging: the line the rail is held on, and where it moved from.
#[derive(Debug, Default)]
struct Scroll {
    anchor: Option<usize>,
    /// The anchor before the last step; `None` is the playing position.
    from: Option<usize>,
    moved: Option<Instant>,
    input: Option<Instant>,
    /// Released anchor and when, so the rail springs back to the playing position.
    released: Option<(usize, Instant)>,
    wheel: f32,
    direction: f32,
}

/// The rail's measurements and wheel state, kept across frames by the canvas.
#[derive(Default)]
pub struct Rail {
    measured: Vec<(u64, Rc<Measured>)>,
    events: (usize, usize, Vec<f64>),
    scroll: Scroll,
    /// The line boxes drawn last frame, in stage coordinates, with the line each one shows.
    hits: Vec<(usize, Rectangle)>,
}

struct Ctx<'a> {
    lines: &'a [Line],
    lyric_px: f32,
    translation_px: f32,
    inactive_scale: f32,
    width: f32,
    height: f32,
}

impl<'a> Ctx<'a> {
    fn new(scene: &'a Scene<'_>, layout: &Layout) -> Self {
        Ctx {
            lines: &scene.lyrics.lines,
            lyric_px: layout.lyric_px,
            translation_px: layout.translation_px,
            inactive_scale: (layout.inactive_px / layout.lyric_px.max(1.0)).clamp(0.72, 0.92),
            width: (layout.rail.width.round() - layout.glow * 2.0 - 8.0).max(0.0),
            height: layout.rail.height.round(),
        }
    }
}

fn line_status(line: &Line, index: usize, active: Option<usize>, time: f64) -> Status {
    if Some(index) == active || (active.is_none() && time >= line.start && time <= line.end) {
        Status::Active
    } else if time > line.end || active.is_some_and(|active| index < active) {
        Status::Passed
    } else {
        Status::Waiting
    }
}

/// `buildMonetVisibleLineEntries` with two lines either side of the active, upcoming or last finished line.
fn auto_entries(lines: &[Line], time: f64) -> Vec<Entry> {
    let active = active_line(lines, time);
    let upcoming = || lines.iter().position(|line| line.start > time);
    let recent = || lines.iter().rposition(|line| RenderHints::of(line).render_end < time);
    let Some(anchor) = active.or_else(upcoming).or_else(recent) else { return Vec::new() };
    let last = lines.len().saturating_sub(1);
    (anchor.saturating_sub(2)..=(anchor + 2).min(last))
        .map(|index| Entry {
            index,
            offset: index as i32 - anchor as i32,
            status: line_status(&lines[index], index, active, time),
        })
        .collect()
}

/// `buildScrollableRailEntries`: four lines either side of the held anchor.
fn manual_entries(lines: &[Line], anchor: usize, active: Option<usize>) -> Vec<Entry> {
    let last = lines.len().saturating_sub(1);
    let anchor = anchor.min(last);
    (anchor.saturating_sub(4)..=(anchor + 4).min(last))
        .map(|index| Entry {
            index,
            offset: index as i32 - anchor as i32,
            status: if Some(index) == active {
                Status::Active
            } else if active.is_some_and(|active| index < active) {
                Status::Passed
            } else {
                Status::Waiting
            },
        })
        .collect()
}

/// `resolveLineTone`: opacity, scale, blur, base alpha and stacking of a line by status and distance.
fn tone(entry: Entry, inactive_scale: f32) -> (f32, f32, f32, f32, i32) {
    if entry.status == Status::Active {
        return (1.0, 1.0, 0.0, 0.34, 4);
    }
    let distance = entry.offset.unsigned_abs().max(1) as f32;
    let waiting = entry.status == Status::Waiting;
    let scale = (inactive_scale * 0.9_f32.powf(distance - 1.0)).clamp(0.68, 0.92);
    let opacity = if waiting {
        (0.72 - (distance - 1.0) * 0.18).clamp(0.36, 0.72)
    } else {
        (0.52 - (distance - 1.0) * 0.12).clamp(0.28, 0.52)
    };
    let blur = match (waiting, distance as i32) {
        (true, 1) => 0.7,
        (true, _) => 1.8 + (distance - 2.0) * 0.8,
        (false, _) => 1.1 + (distance - 1.0) * 0.7,
    };
    let z = if waiting { 3 - distance as i32 } else { 2 - distance as i32 };
    (opacity, scale, blur, if waiting { 0.46 } else { 0.36 }, z)
}

/// `buildMonetDisplayTokens`: timed words with the untimed text between them; an unsplit line is one timed token.
fn tokens(line: &Line) -> Vec<Token> {
    let whole =
        || vec![Token { start: 0, end: line.text.len(), timing: Some((line.start, RenderHints::of(line).render_end)) }];
    if line.words.is_empty() {
        return whole();
    }
    let mut tokens = Vec::new();
    let mut cursor = 0;
    for word in &line.words {
        let Some(found) = line.text.get(cursor..).and_then(|rest| rest.find(&word.text)) else { continue };
        let at = cursor + found;
        if at > cursor {
            tokens.push(Token { start: cursor, end: at, timing: None });
        }
        cursor = at + word.text.len();
        tokens.push(Token { start: at, end: cursor, timing: Some((word.start, word.end)) });
    }
    if cursor < line.text.len() {
        tokens.push(Token { start: cursor, end: line.text.len(), timing: None });
    }
    if tokens.is_empty() { whole() } else { tokens }
}

/// Rows the way the DOM sets the tokens: each is an `inline-block`, so a row only breaks between them, and a token
/// wider than the column wraps inside itself.
fn wrap_tokens(text: &str, shaped: &Shaped, tokens: &[Token], px: f32, width: f32) -> Vec<Span> {
    let mut rows = Vec::new();
    let mut open: Option<(usize, f32)> = None;
    for token in tokens {
        let advance = shaped.span(token.start, token.end);
        if advance > width {
            if let Some((start, _)) = open.take() {
                rows.push(Span { start, end: token.start });
            }
            let piece = text.get(token.start..token.end).unwrap_or_default();
            rows.extend(
                text::wrap(piece, px, SEMIBOLD, width)
                    .into_iter()
                    .map(|row| Span { start: token.start + row.start, end: token.start + row.end }),
            );
            continue;
        }
        open = match open {
            Some((start, used)) if used + advance > width + 0.5 => {
                rows.push(Span { start, end: token.start });
                Some((token.start, advance))
            }
            Some((start, used)) => Some((start, used + advance)),
            None => Some((token.start, advance)),
        };
    }
    if let Some((start, _)) = open {
        rows.push(Span { start, end: text.len() });
    }
    if rows.is_empty() {
        rows.push(Span { start: 0, end: text.len() });
    }
    rows
}

/// A subtitle row is shown only if it holds a letter or digit (`isReadableSubtitleText`).
fn readable(text: &str) -> bool {
    text.chars().any(char::is_alphanumeric)
}

fn measure(line: &Line, active: bool, ctx: &Ctx<'_>) -> Measured {
    let px = ctx.lyric_px;
    let shaped = text::shape(&line.text, px, SEMIBOLD);
    if shaped.missing > 0 {
        tracing::warn!(missing = shaped.missing, "monet lyric has glyphs no font covers");
    }
    let tokens = tokens(line);
    let rows = wrap_tokens(&line.text, &shaped, &tokens, px, ctx.width);
    let limit = if active { ACTIVE_ROWS } else { INACTIVE_ROWS };
    let visible = rows.len().min(limit);
    let line_height = px * 1.18;
    let pad_top = (px * 0.16).max(8.0);
    let pad_bottom = (px * 0.34).max(14.0);
    let text_height = visible as f32 * line_height + pad_top + pad_bottom;
    // `subtitleContentMode` resolves to 'translation': Folia-rust has no romanization switch yet.
    let track = line.translation.as_deref().filter(|text| active && readable(text)).map(|translation| {
        let px = ctx.translation_px;
        let rows: Vec<Span> = text::wrap(translation, px, MEDIUM, ctx.width)
            .into_iter()
            .map(|row| Span { start: row.start, end: row.end })
            .collect();
        let shown = rows.len().clamp(1, TRACK_ROWS);
        let line_height = px * 1.28;
        let pad_top = (px * 0.45).max(7.0);
        let height = shown as f32 * line_height + pad_top + (px * 0.18).max(5.0);
        Track {
            text: translation.to_owned(),
            clipped: rows.len() > shown,
            rows: rows.into_iter().take(shown).collect(),
            px,
            line_height,
            pad_top,
            height,
        }
    });
    let height = text_height + track.as_ref().map_or(0.0, |track| track.height);
    let ink = text::ink(&shaped);
    Measured {
        shaped,
        ink,
        tokens,
        clipped: rows.len() > visible,
        rows,
        visible,
        line_height,
        pad_top,
        text_height,
        track,
        height,
    }
}

impl Rail {
    fn measured(&mut self, ctx: &Ctx<'_>, index: usize, active: bool) -> Rc<Measured> {
        let line = &ctx.lines[index];
        let mut hasher = DefaultHasher::new();
        (index, line.start.to_bits(), line.end.to_bits(), &line.text, &line.translation, active).hash(&mut hasher);
        (ctx.lyric_px.to_bits(), ctx.translation_px.to_bits(), ctx.width.to_bits()).hash(&mut hasher);
        let key = hasher.finish();
        if let Some((_, measured)) = self.measured.iter().find(|(held, _)| *held == key) {
            return measured.clone();
        }
        let measured = Rc::new(measure(line, active, ctx));
        if self.measured.len() >= 24 {
            self.measured.remove(0);
        }
        self.measured.push((key, measured.clone()));
        measured
    }

    /// `buildPositionedEntries`: the anchor centred at 46% of the rail, neighbours stacked with status gaps.
    fn targets(&mut self, ctx: &Ctx<'_>, entries: &[Entry]) -> Vec<Visual> {
        let mut visuals: Vec<(Visual, f32)> = entries
            .iter()
            .map(|entry| {
                let (opacity, scale, blur, base, z) = tone(*entry, ctx.inactive_scale);
                let height = self.measured(ctx, entry.index, entry.status == Status::Active).height * scale;
                let visual = Visual {
                    index: entry.index,
                    offset: entry.offset,
                    status: entry.status,
                    y: 0.0,
                    scale,
                    opacity,
                    blur,
                    base,
                    z,
                };
                (visual, height)
            })
            .collect();
        if visuals.is_empty() {
            return Vec::new();
        }
        let gap = |a: Status, b: Status| {
            if a == Status::Active || b == Status::Active {
                (ctx.lyric_px * 0.49).max(18.0)
            } else {
                (ctx.lyric_px * 0.38).max(14.0)
            }
        };
        let anchor = visuals.iter().position(|(visual, _)| visual.offset == 0).unwrap_or(0);
        visuals[anchor].0.y = ctx.height * 0.46 - visuals[anchor].1 / 2.0;
        for index in anchor + 1..visuals.len() {
            let (previous, height) = visuals[index - 1];
            visuals[index].0.y = previous.y + height + gap(previous.status, visuals[index].0.status);
        }
        for index in (0..anchor).rev() {
            let next = visuals[index + 1].0;
            let (current, height) = visuals[index];
            visuals[index].0.y = next.y - height - gap(current.status, next.status);
        }
        visuals.into_iter().map(|(visual, _)| visual).collect()
    }

    /// The latest time before `time` at which the visible window or a line's status changed.
    fn last_change(&mut self, lines: &[Line], time: f64) -> Option<f64> {
        let key = (lines.as_ptr() as usize, lines.len());
        if (self.events.0, self.events.1) != key {
            let mut events: Vec<f64> =
                lines.iter().flat_map(|line| [line.start, line.end, RenderHints::of(line).render_end]).collect();
            events.sort_by(f64::total_cmp);
            events.dedup();
            self.events = (key.0, key.1, events);
        }
        let below = self.events.2.partition_point(|event| *event < time);
        self.events.2[..below]
            .iter()
            .rev()
            .take(24)
            .copied()
            .find(|event| auto_entries(lines, event - 1.0e-4) != auto_entries(lines, event + 1.0e-4))
    }

    /// Lines at `time` following the playing position, each mid-way through its spring from the previous window.
    fn visuals(&mut self, ctx: &Ctx<'_>, time: f64, depth: u32) -> Vec<Visual> {
        let entries = auto_entries(ctx.lines, time);
        let target = self.targets(ctx, &entries);
        if depth == 0 {
            return target;
        }
        let Some(change) = self.last_change(ctx.lines, time) else { return target };
        let before = self.visuals(ctx, change - 1.0e-4, depth - 1);
        blend(&before, &target, time - change)
    }
}

/// `MONET_SCROLL_TRANSITION` from the previous window into `target`, plus the enter and exit motions.
fn blend(before: &[Visual], target: &[Visual], elapsed: f64) -> Vec<Visual> {
    let mut out: Vec<Visual> = target
        .iter()
        .map(|to| {
            let from = before.iter().find(|from| from.index == to.index).copied().unwrap_or(Visual {
                y: to.y + if to.offset >= 0 { 34.0 } else { -34.0 },
                scale: to.scale * 0.98,
                opacity: 0.0,
                blur: 5.0,
                ..*to
            });
            Visual {
                y: motion::spring(
                    from.y.into(),
                    to.y.into(),
                    elapsed,
                    motion::SCROLL_STIFFNESS,
                    motion::SCROLL_DAMPING,
                    motion::SCROLL_MASS,
                ) as f32,
                scale: motion::spring(
                    from.scale.into(),
                    to.scale.into(),
                    elapsed,
                    motion::SCALE_STIFFNESS,
                    motion::SCALE_DAMPING,
                    motion::SCALE_MASS,
                ) as f32,
                opacity: tween(from.opacity.into(), to.opacity.into(), elapsed, 0.28, SWAP) as f32,
                blur: tween(from.blur.into(), to.blur.into(), elapsed, 0.32, SWAP) as f32,
                ..*to
            }
        })
        .collect();
    if elapsed < 0.2 {
        for gone in before.iter().filter(|from| target.iter().all(|to| to.index != from.index)) {
            let exit = if gone.status == Status::Passed || gone.offset < 0 { -38.0 } else { 38.0 };
            let k = SWAP.at(elapsed / 0.2) as f32;
            out.push(Visual {
                y: gone.y + exit * k,
                scale: gone.scale * (1.0 - 0.02 * k),
                opacity: gone.opacity * (1.0 - k),
                blur: gone.blur + (6.0 - gone.blur) * k,
                ..*gone
            });
        }
    }
    out
}

/// One wheel event over the rail (`handleRailWheel`): 72px of travel per line, one line per event at most.
pub fn wheel(rail: &mut Rail, scene: &Scene<'_>, lines_down: f32, pixels_down: f32, now: Instant) -> bool {
    let lines = &scene.lyrics.lines;
    if lines.is_empty() {
        return false;
    }
    release(rail, now);
    let scroll = &mut rail.scroll;
    let delta = lines_down * WHEEL_NOTCH + pixels_down;
    let direction = if delta == 0.0 { 0.0 } else { delta.signum() };
    if direction != 0.0 && scroll.direction != 0.0 && direction != scroll.direction {
        scroll.wheel = 0.0;
    }
    if direction != 0.0 {
        scroll.direction = direction;
    }
    scroll.wheel += delta;
    scroll.input = Some(now);
    let steps = (scroll.wheel / SCROLL_STEP).trunc().clamp(-1.0, 1.0) as i64;
    if steps != 0 {
        scroll.wheel = 0.0;
        let fallback = || {
            active_line(lines, scene.time)
                .or_else(|| {
                    auto_entries(lines, scene.time).iter().find(|entry| entry.offset == 0).map(|entry| entry.index)
                })
                .unwrap_or(0)
        };
        let base = scroll.anchor.unwrap_or_else(fallback);
        let next = (base as i64 + steps).clamp(0, lines.len() as i64 - 1) as usize;
        scroll.from = scroll.anchor;
        scroll.anchor = Some(next);
        scroll.moved = Some(now);
        scroll.released = None;
    }
    true
}

/// When a held anchor lets go, if one is held.
pub fn release_due(rail: &Rail) -> Option<Instant> {
    rail.scroll.anchor.and(rail.scroll.input).map(|input| input + IDLE_RESET)
}

/// Lets the held anchor go once the wheel has been idle for 1.8s (`MONET_SCROLL_IDLE_RESET_MS`).
fn release(rail: &mut Rail, now: Instant) {
    let scroll = &mut rail.scroll;
    if let (Some(anchor), Some(input)) = (scroll.anchor, scroll.input)
        && now >= input + IDLE_RESET
    {
        scroll.released = Some((anchor, input + IDLE_RESET));
        scroll.anchor = None;
        scroll.from = None;
        scroll.wheel = 0.0;
        scroll.direction = 0.0;
    }
}

/// Whether the rail still moves on the wall clock and needs another frame.
pub fn settling(rail: &Rail, now: Instant) -> bool {
    let scroll = &rail.scroll;
    scroll.anchor.is_some() && scroll.moved.is_some_and(|at| now < at + SETTLE)
        || scroll.released.is_some_and(|(_, at)| now < at + SETTLE)
}

/// The rail at `scene.time`, faded by the intro (`alpha`, `dy`).
pub fn draw(frame: &mut Frame, place: Place, scene: &Scene<'_>, layout: &Layout, rail: &mut Rail, alpha: f32, dy: f32) {
    let lines = &scene.lyrics.lines;
    if lines.is_empty() || alpha <= 0.0 {
        rail.hits.clear();
        return;
    }
    let ctx = Ctx::new(scene, layout);
    let held = rail.scroll.anchor.is_some() || rail.scroll.released.is_some();
    let now = held.then(Instant::now);
    if let Some(now) = now {
        release(rail, now);
    }
    let active = active_line(lines, scene.time);
    let manual = rail.scroll.anchor;
    let mut visuals = match manual {
        Some(anchor) => {
            let target = rail.targets(&ctx, &manual_entries(lines, anchor, active));
            let from = match rail.scroll.from {
                Some(from) => rail.targets(&ctx, &manual_entries(lines, from, active)),
                None => rail.visuals(&ctx, scene.time, 3),
            };
            let elapsed =
                rail.scroll.moved.zip(now).map_or(10.0, |(at, now)| now.saturating_duration_since(at).as_secs_f64());
            follow(&from, target, elapsed)
        }
        None => {
            let target = rail.visuals(&ctx, scene.time, 3);
            match rail.scroll.released.zip(now) {
                Some(((anchor, at), now)) => {
                    let from = rail.targets(&ctx, &manual_entries(lines, anchor, active));
                    follow(&from, target, now.saturating_duration_since(at).as_secs_f64())
                }
                None => target,
            }
        }
    };
    visuals.sort_by_key(|visual| visual.z);
    let box_ = Rectangle { y: layout.rail.y + dy, ..layout.rail };
    let fade = move |y: f32| {
        let at = (y - box_.y) / box_.height.max(1.0);
        if at <= 0.0 || at >= 1.0 {
            0.0
        } else if at < 0.11 {
            at / 0.11
        } else if at <= 0.88 {
            1.0
        } else {
            (1.0 - at) / 0.12
        }
    };
    let paint = Paint { place, scene, layout, ctx: &ctx, rail: box_, fade: &fade, alpha, manual: manual.is_some() };
    rail.hits.clear();
    for visual in &visuals {
        let measured = rail.measured(&ctx, visual.index, visual.status == Status::Active);
        let hit = clip_to(
            Rectangle { x: box_.x, y: box_.y + visual.y, width: box_.width, height: measured.height * visual.scale },
            box_,
        );
        if hit.height > 0.0 {
            rail.hits.push((visual.index, hit));
        }
        paint.line(frame, visual, &measured);
    }
}

/// Springs the lines shared with `from` from their old place; new lines appear in place (`disableEntryMotion`).
fn follow(from: &[Visual], target: Vec<Visual>, elapsed: f64) -> Vec<Visual> {
    target
        .into_iter()
        .map(|to| match from.iter().find(|from| from.index == to.index) {
            Some(from) => Visual {
                y: motion::spring(
                    from.y.into(),
                    to.y.into(),
                    elapsed,
                    motion::SCROLL_STIFFNESS,
                    motion::SCROLL_DAMPING,
                    motion::SCROLL_MASS,
                ) as f32,
                ..to
            },
            None => to,
        })
        .collect()
}

/// One row of a line as drawn: its pen, where its bytes sit, the clip band and the colours.
#[derive(Clone, Copy)]
struct RowInk<'a> {
    pen: ink::Line<'a>,
    measured: &'a Measured,
    span: Span,
    to_x: &'a dyn Fn(usize) -> f32,
    band: Rectangle,
    base: Color,
    word: Color,
    opacity: f32,
}

/// A line placed on the rail this frame.
#[derive(Clone, Copy)]
struct Placed<'a> {
    line: &'a Line,
    visual: &'a Visual,
    origin: Point,
    scale: f32,
    opacity: f32,
}

struct Paint<'a> {
    place: Place,
    scene: &'a Scene<'a>,
    layout: &'a Layout,
    ctx: &'a Ctx<'a>,
    rail: Rectangle,
    fade: Mask<'a>,
    alpha: f32,
    manual: bool,
}

impl Paint<'_> {
    fn line(&self, frame: &mut Frame, visual: &Visual, measured: &Measured) {
        let Some(line) = self.ctx.lines.get(visual.index) else { return };
        let scale = snap_scale(self.ctx.lyric_px, visual.scale);
        let opacity = visual.opacity * self.alpha;
        if opacity <= 0.002 {
            return;
        }
        let glow = self.layout.glow;
        let origin = Point::new(self.rail.x + glow, self.rail.y + visual.y);
        let width = (self.rail.width - glow * 2.0) * scale;
        let theme = self.scene.theme;
        let active = visual.status == Status::Active;
        let placed = Placed { line, visual, origin, scale, opacity };
        if line.is_chorus {
            self.chorus(frame, &placed, width, measured.height * scale);
        }
        let mut word = color(theme.primary, 0.98);
        if line.is_chorus {
            word = ink::mix(word, color(theme.accent, 1.0), 0.48);
        }
        let base = color(theme.primary, visual.base);
        let line_height = measured.line_height * scale;
        let px = self.ctx.lyric_px * scale;
        let content_bottom = origin.y + (measured.pad_top + measured.visible as f32 * measured.line_height) * scale;
        let fade_px = (self.ctx.lyric_px * 0.55).max(12.0) * scale;
        let clipped = !active && measured.clipped;
        let rail_fade = self.fade;
        let mask = move |y: f32| {
            let cut = if clipped { ((content_bottom - y) / fade_px).clamp(0.0, 1.0) } else { 1.0 };
            rail_fade(y) * cut
        };
        let bleed = glow * scale;
        let block = Rectangle {
            x: origin.x - bleed,
            y: origin.y - bleed,
            width: width + bleed * 2.0,
            height: if active { f32::INFINITY } else { measured.text_height * scale + bleed * 2.0 },
        };
        let block = clip_to(block, self.rail);
        let render_end = RenderHints::of(line).render_end;
        let still = self.manual && !active;
        for (row_index, row) in measured.rows.iter().take(measured.visible).enumerate() {
            let content = line.text.get(row.start..row.end).unwrap_or_default();
            let at =
                Point::new(origin.x, origin.y + (measured.pad_top + row_index as f32 * measured.line_height) * scale);
            let pen = ink::Line { content, at, px, line_height, font: SEMIBOLD };
            let band = clip_to(
                Rectangle { x: block.x, y: at.y - line_height * 0.3, width: block.width, height: line_height * 1.6 },
                block,
            );
            let row_x = measured.shaped.x_at(row.start);
            let to_x = |byte: usize| origin.x + (measured.shaped.x_at(byte) - row_x) * scale;
            let ink_row = RowInk { pen, measured, span: *row, to_x: &to_x, band, base, word, opacity };
            if active {
                let shadow = ink::blur_offsets(17.0 * scale, (0.0, 14.0 * scale));
                let tint = color(theme.background, ink::split_alpha(0.22 * opacity, shadow.len()));
                ink::masked_copies(frame, self.place, pen, tint, band, &mask, &shadow);
            }
            if !still && visual.status != Status::Waiting {
                self.glow(frame, &ink_row, line.is_chorus, render_end);
            }
            let segments = self.segments(&ink_row, visual.status, still);
            let offsets = if visual.blur * scale >= 0.6 {
                ink::blur_offsets(visual.blur * scale, (0.0, 0.0))
            } else {
                vec![(0.0, 0.0)]
            };
            for (x0, x1, ink_color) in segments {
                let tint = Color { a: ink::split_alpha(ink_color.a * opacity, offsets.len()), ..ink_color };
                let clip = Rectangle { x: x0, width: (x1 - x0).max(0.0), ..band };
                ink::masked_copies(frame, self.place, pen, tint, clip, &mask, &offsets);
            }
            if active && !self.manual {
                self.sweep(frame, &ink_row, &mask);
            }
        }
        if let Some(track) = &measured.track {
            self.track(frame, &placed, track, measured.text_height * scale);
        }
    }

    /// Colour runs of one row: passed words in the word colour, the rest in the base colour (`MonetTimedTokenSpan`).
    fn segments(&self, ink_row: &RowInk<'_>, status: Status, still: bool) -> Vec<(f32, f32, Color)> {
        let RowInk { measured, span: row, to_x, band, base, word, .. } = *ink_row;
        let time = self.scene.time;
        let mut runs: Vec<(f32, f32, Color)> = Vec::new();
        for token in measured.tokens.iter().filter(|token| token.end > row.start && token.start < row.end) {
            let tint = match (token.timing, status) {
                (Some(_), _) if still => word,
                (Some(_), Status::Passed) => word,
                (Some((_, end)), Status::Active) if time > end => word,
                _ => base,
            };
            let x0 = to_x(token.start.max(row.start));
            let x1 = to_x(token.end.min(row.end));
            match runs.last_mut() {
                Some(last) if last.2 == tint => last.1 = x1,
                _ => runs.push((x0, x1, tint)),
            }
        }
        if let Some(first) = runs.first_mut() {
            first.0 = band.x;
        }
        if let Some(last) = runs.last_mut() {
            last.1 = band.x + band.width;
        }
        runs
    }

    /// The glow halo under sung words (`resolveMonetGlow`, two text-shadows of 0.28/0.65 em, 0.45/0.9 on a chorus).
    fn glow(&self, frame: &mut Frame, ink_row: &RowInk<'_>, chorus: bool, render_end: f64) {
        let RowInk { pen, measured, span: row, to_x, word, opacity, .. } = *ink_row;
        let time = self.scene.time;
        let scale = pen.px / self.ctx.lyric_px.max(1.0);
        let (near, far) = if chorus { (0.45, 0.9) } else { (0.28, 0.65) };
        let tint = Color { a: if chorus { 1.0 } else { 0.88 }, ..word };
        for token in measured.tokens.iter().filter(|token| token.end > row.start && token.start < row.end) {
            let Some((start, end)) = token.timing else { continue };
            let lit = motion::glow(time, start, end, render_end) * opacity;
            if lit <= 0.01 {
                continue;
            }
            let (from, to) = (token.start.max(row.start), token.end.min(row.end));
            let (top, bottom) = measured
                .shaped
                .glyphs
                .iter()
                .zip(&measured.ink)
                .filter(|(glyph, _)| glyph.start >= from && glyph.start < to)
                .fold((f32::INFINITY, f32::NEG_INFINITY), |(top, bottom), (_, ink)| {
                    (top.min(ink.0), bottom.max(ink.1))
                });
            if top >= bottom {
                continue;
            }
            let baseline = pen.at.y + pen.line_height / 2.0 + measured.shaped.baseline * scale;
            let (x0, x1) = (to_x(from), to_x(to));
            let ink_box =
                Rectangle { x: x0, y: baseline + top * scale, width: x1 - x0, height: (bottom - top) * scale };
            let fade = (self.fade)(ink_box.center_y());
            for reach in [near, far] {
                let radius = (self.ctx.lyric_px * reach).round() * scale;
                let halo = Color { a: tint.a * lit * GLYPH_INK * fade, ..tint };
                // Meshes cannot go through `Place::clip`: a draft's meshes land under the root frame's.
                ink::box_shadow(frame, ink_box, (pen.px * 0.12).min(ink_box.height / 2.0), 0.0, radius, halo);
            }
        }
    }

    /// The fill sweeping across the word being sung, with its soft front edge (`MonetWordSweep`).
    fn sweep(&self, frame: &mut Frame, ink_row: &RowInk<'_>, mask: Mask<'_>) {
        let RowInk { pen, measured, span: row, to_x, band, base, word, opacity } = *ink_row;
        let time = self.scene.time;
        let scale = pen.px / self.ctx.lyric_px.max(1.0);
        for token in measured.tokens.iter().filter(|token| token.end > row.start && token.start < row.end) {
            let Some((start, end)) = token.timing else { continue };
            if time <= start || time >= end {
                continue;
            }
            let progress = ((time - start) / (end - start).max(0.001)) as f32;
            let token_x = measured.shaped.x_at(token.start);
            let marks: Vec<f32> = token_marks(measured, token).into_iter().map(|x| x - token_x).collect();
            let full = marks.last().copied().unwrap_or(0.0);
            if full <= 0.0 {
                continue;
            }
            let at = progress * (marks.len() - 1) as f32;
            let whole = at.floor() as usize;
            let low = marks.get(whole).copied().unwrap_or(full);
            let high = marks.get(whole + 1).copied().unwrap_or(full);
            let fill = low + (high - low) * (at - whole as f32);
            let soft = (self.ctx.lyric_px * 0.45).clamp(6.0, 16.0);
            let front = fill + soft * (fill / full).clamp(0.0, 1.0);
            let solid = (front - soft).max(0.0);
            let feather = (front - soft * 0.55).max(0.0);
            let tint = ink::mix(base, word, progress);
            let gradient = |x: f32| {
                let at = x / full;
                if at <= 0.68 { 1.0 - 0.08 * at / 0.68 } else { 0.92 - 0.2 * (at - 0.68) / 0.32 }
            };
            let token_left =
                to_x(token.start.max(row.start)) - (measured.shaped.x_at(token.start.max(row.start)) - token_x) * scale;
            let pieces = [
                (0.0, solid * 0.5, 1.0),
                (solid * 0.5, solid, 1.0),
                (solid, feather, 0.96),
                (feather, (feather + front) / 2.0, 0.6),
                ((feather + front) / 2.0, front, 0.22),
            ];
            for (from, to, strength) in pieces {
                if to <= from {
                    continue;
                }
                let x0 = token_left + from * scale;
                let x1 = token_left + to * scale;
                let clip = Rectangle { x: x0, width: x1 - x0, ..band };
                let clip = clip_to(
                    clip,
                    Rectangle {
                        x: to_x(token.start.max(row.start)),
                        width: to_x(token.end.min(row.end)) - to_x(token.start.max(row.start)),
                        ..band
                    },
                );
                let alpha = tint.a * opacity * strength * gradient((from + to) / 2.0);
                ink::masked(frame, self.place, pen, Color { a: alpha, ..tint }, clip, mask);
            }
        }
    }

    /// The translation row(s) under the active line, fading up 8px over 0.28s.
    fn track(&self, frame: &mut Frame, placed: &Placed<'_>, track: &Track, top: f32) {
        let Placed { line, origin, scale, opacity, .. } = *placed;
        let since = self.scene.time - line.start;
        let enter = tween(0.0, 1.0, since, 0.28, SWAP) as f32;
        let rise = 8.0 * (1.0 - enter) * scale;
        let theme = self.scene.theme;
        let tint = color(theme.primary, 0.68 * opacity * enter);
        let glow = self.layout.glow * scale;
        let box_top = origin.y + top + rise;
        let bottom = box_top + track.height * scale;
        let fade_px = (track.px * 0.65).max(10.0) * scale;
        let clipped = track.clipped;
        let rail_fade = self.fade;
        let mask = move |y: f32| {
            let cut = if clipped { ((bottom - y) / fade_px).clamp(0.0, 1.0) } else { 1.0 };
            rail_fade(y) * cut
        };
        let region = clip_to(
            Rectangle {
                x: origin.x - glow,
                y: box_top,
                width: (self.rail.width - self.layout.glow * 2.0) * scale + glow * 2.0,
                height: track.height * scale,
            },
            self.rail,
        );
        for (index, row) in track.rows.iter().enumerate() {
            let content = track.text.get(row.start..row.end).unwrap_or_default();
            let at = Point::new(origin.x, box_top + (track.pad_top + index as f32 * track.line_height) * scale);
            let pen =
                ink::Line { content, at, px: track.px * scale, line_height: track.line_height * scale, font: MEDIUM };
            ink::masked(frame, self.place, pen, tint, region, &mask);
        }
    }

    /// The accent ellipse behind an active chorus line, past the lines above and below.
    fn chorus(&self, frame: &mut Frame, placed: &Placed<'_>, width: f32, height: f32) {
        let Placed { line, visual, origin, opacity, .. } = *placed;
        let time = self.scene.time;
        let active = visual.status == Status::Active;
        let shown = if active {
            tween(0.0, 1.0, time - line.start, 0.45, Ease::OUT) as f32
        } else {
            tween(1.0, 0.0, time - RenderHints::of(line).render_end, 0.45, Ease::OUT) as f32
        };
        if shown <= 0.0 {
            return;
        }
        let grow = if active { 1.02 } else { 0.96 };
        let reach = 56.0 * visual.scale;
        let center = Point::new(origin.x + width / 2.0, origin.y + height / 2.0);
        let (w, h) = (width * grow, (height + reach * 2.0) * grow);
        let rect = Rectangle { x: center.x - w / 2.0, y: center.y - h / 2.0, width: w, height: h };
        let tint = color(self.scene.theme.accent, shown * opacity * (self.fade)(center.y));
        ink::radial(frame, rect, tint, &CHORUS);
    }
}

/// Pen positions of the token's character boundaries, its start through its end.
fn token_marks(measured: &Measured, token: &Token) -> Vec<f32> {
    let mut marks: Vec<f32> = measured
        .shaped
        .glyphs
        .iter()
        .filter(|glyph| glyph.start >= token.start && glyph.start < token.end)
        .map(|glyph| glyph.x)
        .collect();
    marks.dedup_by(|a, b| (*a - *b).abs() < 0.01);
    marks.push(measured.shaped.x_at(token.end));
    marks
}

fn clip_to(rect: Rectangle, bounds: Rectangle) -> Rectangle {
    let x0 = rect.x.max(bounds.x);
    let y0 = rect.y.max(bounds.y);
    let x1 = (rect.x + rect.width).min(bounds.x + bounds.width);
    let y1 = (rect.y + rect.height).min(bounds.y + bounds.height);
    Rectangle { x: x0, y: y0, width: (x1 - x0).max(0.0), height: (y1 - y0).max(0.0) }
}

/// Whether `at` is over the rail, where the wheel pages the lyrics.
pub fn over(layout: &Layout, at: Point) -> bool {
    layout.rail.contains(at)
}

/// The line drawn under `at` in the last frame, when a press there should play from it.
pub fn hit(rail: &Rail, at: Point) -> Option<usize> {
    rail.hits.iter().rev().find(|(_, rect)| rect.contains(at)).map(|(index, _)| *index)
}

/// The line boxes the rail lays out at `scene.time`, for tests and for callers that need the geometry without a draw.
pub fn boxes(scene: &Scene<'_>, layout: &Layout) -> Vec<(usize, Rectangle)> {
    let ctx = Ctx::new(scene, layout);
    let mut rail = Rail::default();
    let visuals = rail.visuals(&ctx, scene.time, 3);
    visuals
        .iter()
        .filter_map(|visual| {
            let measured = rail.measured(&ctx, visual.index, visual.status == Status::Active);
            let hit = clip_to(
                Rectangle {
                    x: layout.rail.x,
                    y: layout.rail.y + visual.y,
                    width: layout.rail.width,
                    height: measured.height * visual.scale,
                },
                layout.rail,
            );
            (hit.height > 0.0).then_some((visual.index, hit))
        })
        .collect()
}

/// Drops a held manual anchor, so the rail returns to the playing position (`setManualScrollAnchorIndex(null)`).
pub fn end_manual(rail: &mut Rail) {
    rail.scroll.anchor = None;
    rail.scroll.from = None;
    rail.scroll.released = None;
    rail.scroll.wheel = 0.0;
    rail.scroll.direction = 0.0;
}

/// Scale with the font size snapped to a quarter pixel, so a spring reuses glyph sizes.
fn snap_scale(px: f32, scale: f32) -> f32 {
    if px <= 0.0 {
        return scale;
    }
    (px * scale * 4.0).round() / 4.0 / px
}
