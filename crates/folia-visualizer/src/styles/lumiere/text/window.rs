// The lyric window (lyricWindow.ts, windowGeometry.ts, windowLines.ts, windowTiming.ts): a few lines around the
// current one, each glyph flying between slots, decaying into smoke, lit by the beams. Lines are built on demand.

use std::collections::HashMap;
use std::f32::consts::{PI, TAU};

use folia_core::Line;
use folia_lyrics::GraphemeTiming;
use folia_render::glyphs::{Mark, Sprite};

use super::super::art::Rng;
use super::super::beams;
use super::super::palette::mix3;
use super::super::profiles::Type;
use super::super::program::hash_text;
use super::glyph_line::{self, GlyphLine};
use super::marks::{DOT, STAR, TRACK, TRACK_SPAN};
use super::slots::{
    CROSSED_CLEARANCE, PROTECT_MARGIN, ProtectBox, Region, Slot, away_drift, away_of, fixed_slot, free_placements,
    held_drift, placement_index, shield, smooth, stack_x,
};
use super::wrap::{self, Band, Flow, FlowGlyph};
use super::{Light, ZERO, camera_axes, mul, rotate, through};

pub const MAX_WORD_SCALE: f32 = 1.5;
const LETTER_SPACING: f32 = 0.04;
const LEAD: f64 = 1.2;
const SLIDE: f64 = 1.5;
const MAX_LAG: f64 = 0.6;
const TRACK_TIME: f64 = 0.45;
const TRACK_SAMPLES: usize = 20;
const TRACK_WIDTH: f32 = 1.2;
const DRIFT_SPEED: [f32; 2] = [0.013, 0.022];
const ORBIT: f32 = 0.03;
const LIGHT_UP: f64 = 0.3;
const SPOT_WINDOW: f64 = 0.3;
const SPOT_SAMPLES: usize = 8;
const GATHER_LEAD: f64 = 1.6;
const GATHER: f64 = 1.8;
const DISSOLVE_FROM: f32 = 1.0;
const DISSOLVE_SPAN: f32 = 2.0;
const WINDOW_REACH: i64 = 3;
const PREBUILD: usize = 2;
const KEEP_MARGIN: usize = 2;
const GLYPH_DRAWS: u32 = 19;
const LINE_DRAWS: u32 = 4;
const MAX_MOVES: usize = 8;
const MAX_BOXES: usize = 4;
const WHITE: [f32; 3] = [1.0, 1.0, 1.0];
const FUNCTION_WORDS: &str = "的了在把是我你他她它们和与也就都着过吗呢吧啊呀哦被让给从向到这那之而又很";
const MINOR_LATIN: [&str; 19] = [
    "a", "an", "the", "of", "to", "in", "on", "at", "me", "my", "with", "and", "or", "is", "i", "you", "it", "by",
    "for",
];

#[derive(Clone, Copy)]
pub struct Decay {
    pub strength: f32,
    pub delay: f32,
}

/// What the window is laid out for: one scene unit at one size.
pub struct Setup {
    pub width: f32,
    pub height: f32,
    pub region: Region,
    pub hero_px: f32,
    pub neighbors: u8,
    pub decay: Decay,
    pub always_fly: bool,
    /// The unit seed string (`{song}:lumiere-p{n}` or `{song}:lumiere-seamless`).
    pub seed: String,
}

struct Meta {
    start: f64,
    end: f64,
    timings: Vec<GraphemeTiming>,
    random_offset: u32,
    sing_start: f64,
    sing_end: f64,
}

#[derive(Clone, Copy)]
struct Flight {
    c1: [f32; 2],
    c2: [f32; 2],
    delay: f32,
    duration: f32,
    spin: f32,
    wobble: f32,
}

struct GlyphView {
    timing: GraphemeTiming,
    blank: bool,
    v_rotation: f32,
    flight: Flight,
    scale: f32,
    /// Flash star offset, rotation and size (font sizes).
    star: [f32; 4],
    /// Decay direction, speed and spin.
    drift: [f32; 4],
    scatter: [f32; 2],
    phase: f32,
}

struct LineView {
    start: f64,
    line: GlyphLine,
    glyphs: Vec<GlyphView>,
    flow: Flow,
    placements: [Slot; 4],
    jitter: f32,
    velocity: [f32; 2],
    motion_phase: f32,
}

#[derive(Clone, Copy, Default)]
struct Move {
    to_orient: f32,
    to_wrap: f32,
    phase: f32,
    fly: bool,
}

#[derive(Clone, Copy, Default)]
struct Transform {
    x: f32,
    y: f32,
    scale: f32,
    rotation: f32,
    alpha: f32,
    to_orient: f32,
    wrap: f32,
    orient: f32,
    fly: bool,
    rest_orient: f32,
    rest_wrap: f32,
    moves: [Move; MAX_MOVES],
    move_count: usize,
    /// Start of the slide still running for this line, `None` when none is.
    slide_start: Option<f64>,
}

struct Local {
    x: f32,
    y: f32,
    rotation: f32,
    gather: f32,
    flying: f32,
    scale: f32,
}

/// Size of a line settled into a slot: its wrap, extra fit scale and layout sizes.
struct Settled {
    wrap: usize,
    scale: f32,
    across: f32,
    ink_width: f32,
    ink_height: f32,
}

pub struct Window {
    setup: Setup,
    band: Band,
    column_cap: f32,
    next_alpha: f32,
    font_h: f32,
    kinds: Vec<Type>,
    metas: Vec<Meta>,
    views: Vec<Option<LineView>>,
    slots: HashMap<(usize, i64, Type), Slot>,
    transforms: Vec<Transform>,
    boxes: [ProtectBox; MAX_BOXES],
    box_count: usize,
}

impl Window {
    /// `kinds` is the typography of each unit line (`lumiereTypographyOfLine`).
    pub fn new(setup: Setup, lines: &[Line], kinds: Vec<Type>) -> Self {
        let band = wrap::frame_band(setup.width, setup.height);
        let mut offset = 0_u32;
        let metas = lines
            .iter()
            .map(|line| {
                let timings = folia_lyrics::grapheme_timeline(line);
                let ranges = folia_lyrics::graphemes(&line.text);
                let sung: Vec<GraphemeTiming> = ranges
                    .iter()
                    .zip(&timings)
                    .filter(|(range, _)| !line.text[(*range).clone()].trim().is_empty())
                    .map(|(_, timing)| *timing)
                    .collect();
                let meta = Meta {
                    start: line.start,
                    end: line.end,
                    random_offset: offset,
                    sing_start: sung.iter().map(|timing| timing.start).reduce(f64::min).unwrap_or(line.start),
                    sing_end: sung.iter().map(|timing| timing.end).reduce(f64::max).unwrap_or(line.end),
                    timings,
                };
                offset = offset.wrapping_add(ranges.len() as u32 * GLYPH_DRAWS + LINE_DRAWS);
                meta
            })
            .collect();
        let count = lines.len();
        Self {
            column_cap: (band.bottom - band.top) * setup.height,
            next_alpha: if setup.neighbors >= 2 { 0.95 } else { 0.0 },
            font_h: setup.hero_px / setup.height,
            band,
            setup,
            kinds,
            metas,
            views: (0..count).map(|_| None).collect(),
            slots: HashMap::new(),
            transforms: vec![Transform::default(); count],
            boxes: [ProtectBox::default(); MAX_BOXES],
            box_count: 0,
        }
    }

    /// Spots, tracks, halos, glyphs, then stars (`view.addChild(spots, trackHolder, halos, glyphLayer, stars)`).
    pub fn paint(&mut self, lines: &[Line], time: f64, light: &Light<'_>, sprites: &mut Vec<Sprite>) {
        let Some((low, high, current)) = self.prepare_frame(lines, time) else { return };
        for index in low..=high {
            self.paint_spot(index, time, light, sprites);
        }
        if !light.hide_trails {
            for index in low..=high {
                self.paint_tracks(index, time, current, light, sprites);
            }
        }
        let (mut graphemes, mut glyphs) = (0, 0);
        for index in low..=high {
            if let Some(view) = &self.views[index]
                && self.transforms[index].alpha * light.intensity > 0.003
            {
                for (grapheme, info) in view.line.graphemes.iter().zip(&view.glyphs) {
                    if !info.blank {
                        graphemes += 1;
                        glyphs += grapheme.glyphs.len();
                    }
                }
            }
        }
        let halos = sprites.len();
        let marks = halos + graphemes;
        let stars = marks + glyphs;
        sprites.resize(stars + graphemes, ZERO);
        let (mut at_halo, mut at_mark, mut at_star) = (halos, marks, stars);
        for index in low..=high {
            let Some(view) = &self.views[index] else { continue };
            let transform = self.transforms[index];
            if transform.alpha * light.intensity <= 0.003 {
                continue;
            }
            for (number, grapheme) in view.line.graphemes.iter().enumerate() {
                if view.glyphs[number].blank {
                    continue;
                }
                let look = self.look(index, view, number, &transform, (current, time), light);
                sprites[at_halo] = look.halo;
                sprites[at_star] = look.star;
                at_halo += 1;
                at_star += 1;
                for placed in &view.line.glyphs[grapheme.glyphs.clone()] {
                    sprites[at_mark] = Sprite {
                        mark: Mark::Glyph {
                            glyph: placed.glyph,
                            size: view.line.font_px,
                            outline: 0.0,
                            max_scale: f32::MAX,
                        },
                        anchor: through(light.camera, look.world),
                        pen: placed.pen,
                        axes: mul(camera_axes(light.camera), look.axes),
                        color: look.color,
                    };
                    at_mark += 1;
                }
            }
        }
        self.prebuild(high, lines);
    }

    /// Each visible glyph as text for a canvas: grapheme, stage center, rotation, font size and color.
    pub fn each_glyph(
        &mut self,
        lines: &[Line],
        time: f64,
        light: &Light<'_>,
        mut visit: impl FnMut(&str, [f32; 2], f32, f32, [f32; 4]),
    ) {
        let Some((low, high, current)) = self.prepare_frame(lines, time) else { return };
        for (index, line) in lines.iter().enumerate().take(high + 1).skip(low) {
            let Some(view) = &self.views[index] else { continue };
            let transform = self.transforms[index];
            for (number, grapheme) in view.line.graphemes.iter().enumerate() {
                if view.glyphs[number].blank || transform.alpha * light.intensity <= 0.003 {
                    continue;
                }
                let look = self.look(index, view, number, &transform, (current, time), light);
                let text = &line.text[grapheme.text.clone()];
                visit(text, look.world, look.rotation, look.font_px, look.color);
            }
        }
    }

    /// Where grapheme `glyph` of line `index` is at `time` (stage pixels, decay included), and its font size.
    pub fn glyph_anchor(&mut self, lines: &[Line], index: usize, glyph: usize, time: f64) -> Option<[f32; 3]> {
        self.ensure(index, lines)?;
        let transform = self.line_transform(index, time);
        let view = self.views.get(index)?.as_ref()?;
        view.glyphs.get(glyph)?;
        let local = self.glyph_local(view, glyph, time, &transform);
        let world = to_world(&transform, [local.x, local.y]);
        Some([world[0], world[1], self.setup.hero_px * transform.scale * local.scale])
    }

    fn prepare_frame(&mut self, lines: &[Line], time: f64) -> Option<(usize, usize, i64)> {
        if lines.len() != self.metas.len() || lines.is_empty() {
            return None;
        }
        let (low, high, current) = self.active_range(time);
        self.sync_built(low, high);
        for index in low..=high {
            self.ensure(index, lines);
        }
        for index in low..=high {
            self.transforms[index] = self.line_transform(index, time);
        }
        self.build_boxes(time, current, low);
        Some((low, high, current))
    }

    fn paint_spot(&self, index: usize, time: f64, light: &Light<'_>, sprites: &mut Vec<Sprite>) {
        let Some(view) = &self.views[index] else { return };
        let transform = &self.transforms[index];
        let meta = &self.metas[index];
        let line_alpha = transform.alpha * light.intensity;
        if line_alpha <= 0.003 {
            return;
        }
        let alpha = line_alpha
            * smooth(((time - meta.sing_start + 0.3) / 0.3) as f32)
            * (1.0 - smooth(((time - meta.sing_end) / 0.6) as f32));
        if alpha <= 0.003 {
            return;
        }
        let vertical = usize::from(transform.to_orient >= 0.5);
        let spot = |wrap: usize| {
            let path = &view.flow[vertical][wrap].spots;
            [
                spot_along(&meta.timings, |at| path.get(at).map_or(0.0, |point| point[0]), time),
                spot_along(&meta.timings, |at| path.get(at).map_or(0.0, |point| point[1]), time),
            ]
        };
        let local = if transform.wrap <= 0.0 {
            spot(0)
        } else if transform.wrap >= 1.0 {
            spot(1)
        } else {
            lerp2(spot(0), spot(1), transform.wrap)
        };
        let world = to_world(transform, local);
        let size = self.setup.hero_px * transform.scale * 5.0;
        let (w, h) = if vertical == 1 { (size, size * 1.6) } else { (size * 1.6, size) };
        let axes = mul(camera_axes(light.camera), mul(rotate(transform.rotation), [w, 0.0, 0.0, h]));
        let lit = light.lit;
        sprites.push(Sprite {
            mark: Mark::Image(DOT),
            anchor: through(light.camera, world),
            pen: [0.0, 0.0],
            axes,
            color: [lit[0], lit[1], lit[2], 0.07 * alpha],
        });
    }

    /// The path each glyph really took over the last `TRACK_TIME` of a slide, like a cloud-chamber track.
    fn paint_tracks(&mut self, index: usize, time: f64, current: i64, light: &Light<'_>, sprites: &mut Vec<Sprite>) {
        let transform = self.transforms[index];
        let Some(slide_start) = transform.slide_start else { return };
        let alpha = transform.alpha * light.intensity * self.passed_dim(index, current, time);
        if alpha <= 0.003 || time > slide_start + SLIDE + TRACK_TIME {
            return;
        }
        let from = slide_start.max(time - TRACK_TIME);
        if time - from < 1e-3 {
            return;
        }
        let mut samples = [Transform::default(); TRACK_SAMPLES + 1];
        let mut moments = [0.0_f64; TRACK_SAMPLES + 1];
        let mut any_fly = false;
        for step in 0..=TRACK_SAMPLES {
            moments[step] = from + (time - from) * step as f64 / TRACK_SAMPLES as f64;
            samples[step] = self.line_transform(index, moments[step]);
            any_fly |= samples[step].fly;
        }
        let Some(view) = &self.views[index] else { return };
        if !any_fly {
            return;
        }
        let hero = self.setup.hero_px;
        let axes = camera_axes(light.camera);
        for (number, glyph) in view.glyphs.iter().enumerate() {
            if glyph.blank {
                continue;
            }
            let mut points = [[0.0_f32; 2]; TRACK_SAMPLES + 1];
            let (mut length, mut font_px, mut flying) = (0.0, hero, 0.0);
            for step in 0..=TRACK_SAMPLES {
                let local = self.glyph_local(view, number, moments[step], &samples[step]);
                let world = to_world(&samples[step], [local.x, local.y]);
                font_px = hero * samples[step].scale * local.scale;
                flying = local.flying;
                let wobble = (step as f32 * 0.9 + glyph.flight.wobble + moments[step] as f32 * 6.0).sin() * hero * 0.02;
                points[step] = [world[0] + wobble, world[1] - wobble * 0.6];
                if step > 0 {
                    length += (points[step][0] - points[step - 1][0]).hypot(points[step][1] - points[step - 1][1]);
                }
            }
            let strength = (length / (hero * 3.0)).min(1.0);
            if strength <= 0.05 {
                continue;
            }
            let head = points[TRACK_SAMPLES];
            let guard = shield(&self.boxes[..self.box_count], index, head[0], head[1], font_px / 2.0, flying);
            let lit = light.lit;
            let color = [lit[0], lit[1], lit[2], alpha * 0.55 * strength * guard];
            for pair in points.windows(2) {
                let (dx, dy) = (pair[1][0] - pair[0][0], pair[1][1] - pair[0][1]);
                let span = dx.hypot(dy);
                if span < 1e-4 {
                    continue;
                }
                let thick = TRACK_WIDTH * TRACK_SPAN;
                let normal = [-dy / span * thick, dx / span * thick];
                let middle = [(pair[0][0] + pair[1][0]) / 2.0, (pair[0][1] + pair[1][1]) / 2.0];
                sprites.push(Sprite {
                    mark: Mark::Image(TRACK),
                    anchor: through(light.camera, middle),
                    pen: [0.0, 0.0],
                    axes: mul(axes, [dx, dy, normal[0], normal[1]]),
                    color,
                });
            }
        }
    }

    /// One glyph's pose, color, halo and flash star this frame. `at` is (current line, time).
    fn look(
        &self,
        index: usize,
        view: &LineView,
        number: usize,
        transform: &Transform,
        at: (i64, f64),
        light: &Light<'_>,
    ) -> Look {
        let (current, time) = at;
        let glyph = &view.glyphs[number];
        let line_alpha = transform.alpha * light.intensity;
        let passed_dim = self.passed_dim(index, current, time);
        let local = self.glyph_local(view, number, time, transform);
        let font_px = self.setup.hero_px * transform.scale * local.scale;
        let world = to_world(transform, [local.x, local.y]);
        let timing = glyph.timing;
        let lit = smooth(((time - timing.start) / (timing.end - timing.start).max(LIGHT_UP)) as f32);
        let flash = flash_envelope(timing.start, time, 0.45);
        let illumination =
            beams::compress(beams::light_at(light.beams, world[0] / self.setup.height, world[1] / self.setup.height));
        let heat = (illumination * 1.3 + flash * 0.2).clamp(0.0, 1.0);
        let dissolve = 1.0
            - ((decay_amount(self.setup.decay, timing.start, time) * glyph.drift[2] - DISSOLVE_FROM) / DISSOLVE_SPAN)
                .clamp(0.0, 1.0);
        let guard = shield(&self.boxes[..self.box_count], index, world[0], world[1], font_px / 2.0, local.flying);
        let glyph_alpha = line_alpha * passed_dim * dissolve * (0.35 + 0.65 * local.gather) * guard;
        let revealed = (light.unlit_alpha + illumination * 0.45 + local.flying * 0.4).min(1.0);
        let alpha = glyph_alpha * (revealed * (1.0 - lit) + lit * (0.55 + 0.4 * heat + 0.3 * local.flying).min(1.0));
        let hot = mix3(light.lit, WHITE, (0.08 * illumination + 0.1 * flash + 0.25 * local.flying).clamp(0.0, 1.0));
        let cold = mix3(light.unlit, light.lit, illumination * 0.35);
        let tint = mix3(cold, hot, lit);
        let camera = camera_axes(light.camera);
        let halo_alpha = glyph_alpha * lit * (0.03 + 0.14 * illumination + 0.08 * flash);
        let halo = if halo_alpha > 0.003 {
            let size = font_px * (2.2 + 0.8 * illumination);
            Sprite {
                mark: Mark::Image(DOT),
                anchor: through(light.camera, world),
                pen: [0.0, 0.0],
                axes: mul(camera, [size, 0.0, 0.0, size * 0.9]),
                color: [light.lit[0], light.lit[1], light.lit[2], halo_alpha],
            }
        } else {
            ZERO
        };
        let star_alpha = line_alpha * flash * 0.55 * guard;
        let star = if star_alpha > 0.003 {
            let at = [world[0] + font_px * glyph.star[0], world[1] + font_px * glyph.star[1]];
            let size = font_px * glyph.star[3] * (0.8 + 0.5 * flash);
            let rotation = glyph.star[2] * (1.0 + (1.0 - flash) * 0.6);
            let color = mix3(light.lit, WHITE, 0.5);
            Sprite {
                mark: Mark::Image(STAR),
                anchor: through(light.camera, at),
                pen: [0.0, 0.0],
                axes: mul(camera, mul(rotate(rotation), [size, 0.0, 0.0, size])),
                color: [color[0], color[1], color[2], star_alpha],
            }
        } else {
            ZERO
        };
        let rotation = transform.rotation + local.rotation;
        let scale = transform.scale * local.scale / MAX_WORD_SCALE;
        Look {
            world,
            rotation,
            font_px,
            axes: mul(rotate(rotation), [scale, 0.0, 0.0, scale]),
            color: [tint[0], tint[1], tint[2], alpha],
            halo,
            star,
        }
    }

    fn current_of(&self, time: f64) -> i64 {
        cursor(&self.metas, time)
    }

    /// Sung lines dim to 0.75 over 2.5 s.
    fn passed_dim(&self, index: usize, current: i64, time: f64) -> f32 {
        let end = self.metas[index].end;
        let passed = (index as i64) < current || (index as i64 == current && time > end);
        if passed { lerp(1.0, 0.75, (((time - end) / 2.5) as f32).clamp(0.0, 1.0)) } else { 1.0 }
    }

    fn kind_at(&self, c: i64) -> Type {
        let last = self.kinds.len().saturating_sub(1) as i64;
        self.kinds.get(c.clamp(0, last) as usize).copied().unwrap_or(Type::Horizontal)
    }

    fn max_width_of(&self, kind: Type) -> f32 {
        (self.band.right - self.band.left) * self.setup.height * if kind == Type::Crossed { 0.9 } else { 1.0 }
    }

    fn settle(&self, index: usize, kind: Type, orient: usize, scale: f32) -> Option<Settled> {
        let view = self.views.get(index)?.as_ref()?;
        let flow = &view.flow[orient];
        let budget = if orient == 1 { self.column_cap } else { self.max_width_of(kind) };
        let wrap = usize::from(wrap::should_wrap(flow, budget, scale));
        let variant = &flow[wrap];
        Some(Settled {
            wrap,
            scale: scale * wrap::fit_scale(variant.along, budget, scale),
            across: variant.across,
            ink_width: variant.ink_width,
            ink_height: variant.ink_height,
        })
    }

    fn across_of(&self, index: i64, kind: Type, orient: usize, scale: f32) -> f32 {
        if index < 0 {
            return self.font_h * scale;
        }
        self.settle(index as usize, kind, orient, scale)
            .map_or(self.font_h * scale, |settled| settled.across / self.setup.height * settled.scale)
    }

    fn window_shift(&self, current: i64, kind: Type) -> [f32; 2] {
        if current < 0 {
            return [0.0, 0.0];
        }
        let orient = usize::from(kind == Type::Vertical);
        let Some(settled) = self.settle(current as usize, kind, orient, 1.0) else { return [0.0, 0.0] };
        let height = self.setup.height;
        let (region, band) = (self.setup.region, self.band);
        let w = settled.ink_width / height * settled.scale;
        let h = settled.ink_height / height * settled.scale;
        [
            wrap::clamp_into(region.cx, w, band.left, band.right) - region.cx,
            wrap::clamp_into(region.cy, h, band.top, band.bottom) - region.cy,
        ]
    }

    fn slot_of(&mut self, index: usize, current: i64, kind: Type) -> Slot {
        if let Some(slot) = self.slots.get(&(index, current, kind)) {
            return *slot;
        }
        let slot = self.compute_slot(index, current, kind);
        if self.slots.len() > 4096 {
            self.slots.clear();
        }
        self.slots.insert((index, current, kind), slot);
        slot
    }

    /// `slotOf`: the slot of line `index` when `current` is current, with the shift that keeps the current line in
    /// the frame and the line's wrap.
    fn compute_slot(&self, index: usize, current: i64, kind: Type) -> Slot {
        let clamped = (index as i64 - current).clamp(-2, 2) as i32;
        let shift = self.window_shift(current, kind);
        let height = self.setup.height;
        let count = self.metas.len() as i64;
        let (slot, wrap) = if kind == Type::Crossed && clamped != 0 {
            let Some(view) = self.views.get(index).and_then(Option::as_ref) else { return Slot::default() };
            let base = view.placements[placement_index(clamped)];
            let base_orient = usize::from(base.orient >= 0.5);
            let sign = if base.dy < 0.0 { -1.0 } else { 1.0 };
            let current_half = if current >= 0 && current < count {
                self.settle(current as usize, kind, 0, 1.0)
                    .map_or(self.font_h / 2.0, |settled| settled.ink_height / height * settled.scale / 2.0)
            } else {
                self.font_h / 2.0
            };
            let gap = self.font_h * CROSSED_CLEARANCE;
            let center_y = self.setup.region.cy + shift[1];
            let room = if sign < 0.0 {
                center_y - current_half - gap - self.band.top
            } else {
                self.band.bottom - (center_y + current_half + gap)
            };
            let flow = &view.flow[base_orient];
            let budget = if base_orient == 1 { self.column_cap } else { self.max_width_of(kind) };
            let tilt = base.rotation.abs() + 0.03;
            let height_of = |w: usize, s: f32| {
                (flow[w].ink_height * tilt.cos() + flow[w].ink_width * tilt.sin()) / height
                    * s
                    * wrap::fit_scale(flow[w].along, budget, s)
            };
            let mut scale = base.scale;
            let mut w = self.settle(index, kind, base_orient, scale).map_or(0, |settled| settled.wrap);
            if base_orient == 1 && room > 0.0 {
                if w == 0 && flow[1].lines > 1 && height_of(0, scale) > room {
                    w = 1;
                }
                let tall = height_of(w, scale);
                if tall > room {
                    scale = (base.scale * 0.5).max(scale * (room / tall));
                }
            }
            let clearance = current_half + gap + height_of(w, scale) / 2.0;
            let dy = sign * base.dy.abs().max(clearance);
            let wider = if base_orient == 1 {
                (self.across_of(index as i64, kind, 1, scale) - self.font_h * scale).max(0.0) / 2.0
            } else {
                0.0
            };
            (Slot { scale, dy, dx: base.dx + base.dx.signum() * wider, ..base }, Some(w as f32))
        } else {
            let fixed = if kind == Type::Crossed { Type::Horizontal } else { kind };
            let orient = usize::from(kind == Type::Vertical);
            let slot = fixed_slot(fixed, clamped, self.setup.region, self.font_h, self.next_alpha, |offset, scale| {
                let line = if offset == clamped { index as i64 } else { current + i64::from(offset) };
                self.across_of(line, kind, orient, scale)
            });
            (slot, None)
        };
        let wrap = wrap.unwrap_or_else(|| {
            self.settle(index, kind, usize::from(slot.orient >= 0.5), slot.scale)
                .map_or(0.0, |settled| settled.wrap as f32)
        });
        Slot { dx: slot.dx + shift[0], dy: slot.dy + shift[1], wrap, ..slot }
    }

    fn first_live_change(&self, time: f64, current: i64) -> i64 {
        let mut c = current;
        while c >= 0 && self.metas[c as usize].start - LEAD + MAX_LAG + SLIDE + TRACK_TIME > time {
            c -= 1;
        }
        c + 1
    }

    /// `lineTransform`: the base slot plus every live change's slot difference times its eased progress.
    fn line_transform(&mut self, index: usize, time: f64) -> Transform {
        let current = self.current_of(time);
        let first = self.first_live_change(time, current);
        let base = first - 1;
        let initial_kind = self.kind_at(base);
        let initial = self.slot_of(index, base, initial_kind);
        let (mut dx, mut dy, mut scale, mut rotation) = (initial.dx, initial.dy, initial.scale, initial.rotation);
        let (mut alpha, mut orient, mut wrap) = (initial.alpha, initial.orient, initial.wrap);
        let mut latest: Option<(Slot, Slot, f32, f64, Type)> = None;
        let mut out = Transform { rest_orient: initial.orient, rest_wrap: initial.wrap, ..Transform::default() };
        let mut width_cap = self.max_width_of(initial_kind);
        let mut jitter_on = if initial_kind == Type::Crossed { 0.0 } else { 1.0 };
        let away0 = away_of(index, base, initial_kind);
        let mut away_x = stack_x(initial_kind) * away0;
        let mut away_y = (1.0 - stack_x(initial_kind)) * away0;
        let mut hero = if index as i64 == base { 1.0 } else { 0.0 };
        let last = current.min(self.metas.len() as i64 - 1);
        for c in first..=last {
            let before_kind = self.kind_at(c - 1);
            let after_kind = self.kind_at(c);
            let before = self.slot_of(index, c - 1, before_kind);
            let after = self.slot_of(index, c, after_kind);
            let kind_changed = before_kind != after_kind;
            if before == after && !kind_changed {
                continue;
            }
            let (phase, start) = self.line_phase(c, index as i64 - c, time);
            let k = ease_sine(phase);
            if kind_changed {
                width_cap += (self.max_width_of(after_kind) - self.max_width_of(before_kind)) * k;
                let crossed = |kind: Type| if kind == Type::Crossed { 0.0 } else { 1.0 };
                jitter_on += (crossed(after_kind) - crossed(before_kind)) * k;
            }
            let away_before = away_of(index, c - 1, before_kind);
            let away_after = away_of(index, c, after_kind);
            away_x += (stack_x(after_kind) * away_after - stack_x(before_kind) * away_before) * k;
            away_y += ((1.0 - stack_x(after_kind)) * away_after - (1.0 - stack_x(before_kind)) * away_before) * k;
            let is = |line: i64| if index as i64 == line { 1.0 } else { 0.0 };
            hero += (is(c) - is(c - 1)) * k;
            let fade = if after.alpha < before.alpha {
                ease_cubic(phase / 0.6)
            } else if after.alpha > before.alpha {
                ease_cubic((phase - 0.4) / 0.6)
            } else {
                k
            };
            dx += (after.dx - before.dx) * k;
            dy += (after.dy - before.dy) * k;
            scale += (after.scale - before.scale) * k;
            rotation += (after.rotation - before.rotation) * k;
            alpha += (after.alpha - before.alpha) * fade;
            orient += (after.orient - before.orient) * phase;
            wrap += (after.wrap - before.wrap) * k;
            if phase > 0.0 {
                latest = Some((before, after, phase, start, after_kind));
            }
            if phase >= 1.0 {
                out.rest_orient = after.orient;
                out.rest_wrap = after.wrap;
                out.move_count = 0;
            } else if phase > 0.0 && out.move_count < MAX_MOVES {
                out.moves[out.move_count] = Move {
                    to_orient: after.orient,
                    to_wrap: after.wrap,
                    phase,
                    fly: self.setup.always_fly || after_kind == Type::Crossed || before.orient != after.orient,
                };
                out.move_count += 1;
            }
        }
        let Some(view) = self.views.get(index).and_then(Option::as_ref) else { return out };
        let o = orient.clamp(0.0, 1.0);
        let w = wrap.clamp(0.0, 1.0);
        let [h_single, h_wrapped] = &view.flow[0];
        let [v_single, v_wrapped] = &view.flow[1];
        let column_cap = self.column_cap;
        let fit_h = lerp(
            wrap::fit_scale(h_single.along, width_cap, scale),
            wrap::fit_scale(h_wrapped.along, width_cap, scale),
            w,
        );
        let fit_v = lerp(
            wrap::fit_scale(v_single.along, column_cap, scale),
            wrap::fit_scale(v_wrapped.along, column_cap, scale),
            w,
        );
        let fitted = scale * lerp(fit_h, fit_v, o);
        let (region, band, height) = (self.setup.region, self.band, self.setup.height);
        let jitter = jitter_on * view.jitter * region.w * ((1.0 - scale) / 0.5).clamp(0.0, 1.0);
        let base_x = region.cx + dx + jitter * (1.0 - orient);
        let base_y = region.cy + dy + jitter * 0.5 * orient;
        let along_h = lerp(h_single.ink_width, h_wrapped.ink_width, w) / height * fitted;
        let along_v = lerp(v_single.ink_height, v_wrapped.ink_height, w) / height * fitted;
        let x = base_x + (wrap::clamp_into(base_x, along_h, band.left, band.right) - base_x) * (1.0 - o) * jitter_on;
        let y = base_y + (wrap::clamp_into(base_y, along_v, band.top, band.bottom) - base_y) * o * jitter_on;
        let age = (time - self.metas[index].start) as f32;
        let t = time as f32;
        let m = view.motion_phase;
        let [vx, vy] = view.velocity;
        let orbit_x = (t * 0.52 + m).sin() * ORBIT;
        let orbit_y = (t * 0.41 + m * 1.3).cos() * ORBIT * 0.7;
        let held = hero.clamp(0.0, 1.0);
        let drift_x = lerp(away_drift(vx * age + orbit_x, away_x), held_drift(vx, vy, age, 0) + orbit_x, held);
        let drift_y = lerp(away_drift(vy * age + orbit_y, away_y), held_drift(vx, vy, age, 1) + orbit_y, held);
        let settled_orient = latest.map_or(initial.orient, |(_, after, ..)| after.orient);
        Transform {
            x: (x + drift_x) * height,
            y: (y + drift_y) * height,
            scale: fitted * (1.0 + 0.025 * (t * 0.43 + m).sin()),
            rotation: rotation + 0.03 * (t * 0.23 + m * 0.7).sin(),
            alpha: alpha.clamp(0.0, 1.0),
            to_orient: settled_orient,
            wrap: w,
            orient: o,
            fly: latest.is_some_and(|(before, after, phase, _, kind)| {
                phase < 1.0 && (self.setup.always_fly || kind == Type::Crossed || before.orient != after.orient)
            }),
            slide_start: latest.map(|(_, _, _, start, _)| start),
            ..out
        }
    }

    /// `glyphLocal`: flight between slots, gathering, decay and breathing, in the line's own space.
    fn glyph_local(&self, view: &LineView, number: usize, time: f64, transform: &Transform) -> Local {
        let glyph = &view.glyphs[number];
        let hero = self.setup.hero_px;
        let decay = self.setup.decay;
        let at = |orient: f32, wrap: f32| {
            view.flow[usize::from(orient >= 0.5)][usize::from(wrap >= 0.5)]
                .points
                .get(number)
                .copied()
                .unwrap_or_default()
        };
        let mut point = at(transform.rest_orient, transform.rest_wrap);
        let mut orientation = transform.rest_orient;
        let mut flying: f32 = 0.0;
        for step in &transform.moves[..transform.move_count] {
            let target = at(step.to_orient, step.to_wrap);
            if step.fly {
                let s = flight_progress(&glyph.flight, step.phase);
                point = flight_point(&glyph.flight, point, target, s);
                orientation = lerp(orientation, step.to_orient, s);
                flying = flying.max((PI * s).sin());
            } else {
                let k = ease_sine(step.phase);
                point = lerp2(point, target, k);
                orientation = lerp(orientation, step.to_orient, k);
            }
        }
        let [mut x, mut y] = point;
        let mut rotation = glyph.v_rotation * orientation + glyph.flight.spin * flying;
        let pulse = 1.0 - 0.28 * flying;
        let gather = smooth(((time - (view.start - LEAD - GATHER_LEAD)) / GATHER) as f32);
        let scatter = (1.0 - gather).powi(2) * decay.strength;
        x += glyph.scatter[0] * hero * scatter;
        y += glyph.scatter[1] * hero * scatter;
        let amount = decay_amount(decay, glyph.timing.start, time) * glyph.drift[2];
        x += glyph.drift[0] * hero * amount;
        y += glyph.drift[1] * hero * amount;
        rotation += glyph.drift[3] * amount * 0.18;
        let breath = hero * 0.022 * (decay.strength + 0.3).min(1.0);
        let t = time as f32;
        x += (t * 0.9 + glyph.phase).sin() * breath;
        y += (t * 1.13 + glyph.phase * 1.7).cos() * breath;
        Local { x, y, rotation, gather, flying, scale: glyph.scale * pulse }
    }

    fn line_phase(&self, current: i64, relative: i64, time: f64) -> (f32, f64) {
        if current < 0 {
            return (1.0, f64::NEG_INFINITY);
        }
        let lag = match relative.clamp(-2, 2) {
            -2 => 0.0,
            -1 => 0.15,
            0 => 0.3,
            1 => 0.5,
            _ => 0.6,
        };
        let start = self.metas[current as usize].start - LEAD + lag;
        ((((time - start) / SLIDE) as f32).clamp(0.0, 1.0), start)
    }

    fn active_range(&self, time: f64) -> (usize, usize, i64) {
        let current = self.current_of(time);
        let first = self.first_live_change(time, current);
        let last = self.metas.len() as i64 - 1;
        let low = (current.min(first) - WINDOW_REACH).clamp(0, last);
        let high = (current.max(0) + WINDOW_REACH).min(last);
        (low as usize, high.max(low) as usize, current)
    }

    /// `buildProtectBoxes`: each recent current line's ink box, weighted by how far its change has run.
    fn build_boxes(&mut self, time: f64, current: i64, low: usize) {
        self.box_count = 0;
        let mut later = 0.0;
        let mut j = current;
        while j >= low as i64 && self.box_count < MAX_BOXES {
            let index = j as usize;
            let eased = ease_sine(((time - (self.metas[index].start - LEAD)) / SLIDE) as f32);
            let transform = self.transforms[index];
            let o = transform.orient.clamp(0.0, 1.0);
            let weight = (eased - later) * transform.alpha.clamp(0.0, 1.0) * (1.0 - 4.0 * o * (1.0 - o));
            later = eased;
            if weight > 1e-4
                && let Some(view) = &self.views[index]
            {
                let [h_single, h_wrapped] = &view.flow[0];
                let [v_single, v_wrapped] = &view.flow[1];
                let w = transform.wrap;
                let ink_w = lerp(
                    lerp(h_single.ink_width, h_wrapped.ink_width, w),
                    lerp(v_single.ink_width, v_wrapped.ink_width, w),
                    transform.orient,
                );
                let ink_h = lerp(
                    lerp(h_single.ink_height, h_wrapped.ink_height, w),
                    lerp(v_single.ink_height, v_wrapped.ink_height, w),
                    transform.orient,
                );
                self.boxes[self.box_count] = ProtectBox {
                    line: index,
                    x: transform.x,
                    y: transform.y,
                    cos: transform.rotation.cos(),
                    sin: transform.rotation.sin(),
                    half_w: ink_w / 2.0 * transform.scale,
                    half_h: ink_h / 2.0 * transform.scale,
                    margin: PROTECT_MARGIN * self.setup.hero_px * transform.scale,
                    weight,
                };
                self.box_count += 1;
            }
            if eased >= 1.0 {
                break;
            }
            j -= 1;
        }
    }

    fn sync_built(&mut self, low: usize, high: usize) {
        for (index, view) in self.views.iter_mut().enumerate() {
            if view.is_some() && (index + KEEP_MARGIN < low || index > high + KEEP_MARGIN) {
                *view = None;
            }
        }
    }

    /// Builds one line ahead per frame, so a line is ready before it slides in.
    fn prebuild(&mut self, high: usize, lines: &[Line]) {
        for index in high + 1..=(high + PREBUILD).min(lines.len().saturating_sub(1)) {
            if self.views[index].is_none() {
                self.ensure(index, lines);
                return;
            }
        }
    }

    fn ensure(&mut self, index: usize, lines: &[Line]) -> Option<()> {
        if self.views.get(index)?.is_some() {
            return Some(());
        }
        let view = self.build_line(index, lines.get(index)?);
        self.views[index] = Some(view);
        Some(())
    }

    /// `buildLineView`: glyph line, word scales and jags, the four layouts and every per-glyph random draw.
    fn build_line(&self, index: usize, line: &Line) -> LineView {
        let hero = self.setup.hero_px;
        let meta = &self.metas[index];
        let glyph_line = glyph_line::build(&line.text, hero * MAX_WORD_SCALE, LETTER_SPACING);
        let words = folia_lyrics::word_spans(&line.text);
        let word_seed = format!("{}:{}:{}", self.setup.seed, index, line.text);
        let word_text = |word: &folia_lyrics::WordSpan| {
            let range =
                glyph_line.graphemes.get(word.start)?.text.start..glyph_line.graphemes.get(word.end - 1)?.text.end;
            line.text.get(range)
        };
        let scales = word_scales(&words, &word_seed, |word| word_text(word).unwrap_or_default().to_owned());
        let jags = word_jags(&scales, &word_seed);
        let word_of: Vec<usize> = (0..glyph_line.graphemes.len())
            .map(|at| words.iter().position(|word| at >= word.start && at < word.end).unwrap_or(0))
            .collect();
        let flow_glyphs: Vec<FlowGlyph<'_>> = glyph_line
            .graphemes
            .iter()
            .enumerate()
            .map(|(at, grapheme)| {
                let scale = scales.get(word_of[at]).copied().unwrap_or(1.0);
                FlowGlyph {
                    text: &line.text[grapheme.text.clone()],
                    scale,
                    advance: grapheme.char_width / MAX_WORD_SCALE * scale,
                    upright: grapheme.upright,
                    jag: jags.get(word_of[at]).copied().unwrap_or(0.0) * hero,
                }
            })
            .collect();
        let limits = [self.max_width_of(Type::Horizontal), self.column_cap];
        let flow = wrap::flow_line(&flow_glyphs, &words, hero, LETTER_SPACING, limits);
        let seed = hash_text(FNV, &format!("{}:window", self.setup.seed));
        let mut rng = Rng(seed.wrapping_add(meta.random_offset.wrapping_mul(0x6d2b_79f5)));
        let glyphs = glyph_line
            .graphemes
            .iter()
            .enumerate()
            .map(|(at, grapheme)| {
                let timing =
                    meta.timings.get(at).copied().unwrap_or(GraphemeTiming { start: line.start, end: line.end });
                glyph_view(&mut rng, hero, timing, grapheme.blank, grapheme.upright, flow_glyphs[at].scale)
            })
            .collect();
        let jitter = (rng.next() - 0.5) * 0.16;
        let angle = rng.next() * TAU;
        let speed = DRIFT_SPEED[0] + rng.next() * (DRIFT_SPEED[1] - DRIFT_SPEED[0]);
        let motion_phase = rng.next() * TAU;
        let mut placement_rng = Rng(hash_text(FNV, &format!("{}:placements:{}", self.setup.seed, index)));
        LineView {
            start: line.start,
            line: glyph_line,
            glyphs,
            flow,
            placements: free_placements(&mut placement_rng, self.setup.region, self.next_alpha),
            jitter,
            velocity: [angle.cos() * speed, angle.sin() * speed],
            motion_phase,
        }
    }
}

struct Look {
    world: [f32; 2],
    rotation: f32,
    font_px: f32,
    axes: [f32; 4],
    color: [f32; 4],
    halo: Sprite,
    star: Sprite,
}

const FNV: u32 = 2_166_136_261;

/// The 19 draws of one glyph, in the original order.
fn glyph_view(rng: &mut Rng, hero: f32, timing: GraphemeTiming, blank: bool, upright: bool, scale: f32) -> GlyphView {
    let a1 = rng.next() * TAU;
    let a2 = a1 + PI * if rng.next() < 0.5 { 0.5 } else { 1.5 } + (rng.next() - 0.5) * 0.8;
    let m1 = hero * (1.6 + rng.next() * 3.2);
    let m2 = hero * (1.0 + rng.next() * 2.6);
    let duration = 0.45 + rng.next() * 0.4;
    let flight = Flight {
        c1: [a1.cos() * m1, a1.sin() * m1],
        c2: [a2.cos() * m2, a2.sin() * m2],
        delay: rng.next() * (1.0 - duration),
        duration,
        spin: (rng.next() - 0.5) * 2.4,
        wobble: rng.next() * TAU,
    };
    let angle = -PI / 2.0 + (rng.next() - 0.5) * 2.2;
    let scatter_angle = rng.next() * TAU;
    let scatter_distance = 0.6 + rng.next() * 1.4;
    let star = [
        -0.15 + rng.next() * 0.5,
        -0.62 + rng.next().powf(1.4) * 0.9,
        (rng.next() - 0.5) * 0.5,
        0.65 + rng.next() * 0.7,
    ];
    let drift = [angle.cos(), angle.sin(), 0.6 + rng.next() * 0.9, (rng.next() - 0.5) * 2.0];
    GlyphView {
        timing,
        blank,
        v_rotation: if upright { 0.0 } else { PI / 2.0 },
        flight,
        scale,
        star,
        drift,
        scatter: [scatter_angle.cos() * scatter_distance, scatter_angle.sin() * scatter_distance],
        phase: rng.next() * TAU,
    }
}

/// `wordScales`: function words smaller, the key word (longest content word, the later on ties) larger.
fn word_scales(
    words: &[folia_lyrics::WordSpan],
    seed: &str,
    text: impl Fn(&folia_lyrics::WordSpan) -> String,
) -> Vec<f32> {
    let mut rng = Rng(hash_text(FNV, &format!("{seed}:words")));
    let length = |word: &folia_lyrics::WordSpan| word.end - word.start;
    let minor = |word: &folia_lyrics::WordSpan| {
        let piece = text(word);
        word.blank
            || (length(word) == 1 && FUNCTION_WORDS.contains(piece.as_str()) && !piece.is_empty())
            || MINOR_LATIN.contains(&piece.to_lowercase().as_str())
    };
    let mut key: Option<usize> = None;
    for (index, word) in words.iter().enumerate() {
        if minor(word) {
            continue;
        }
        if key.is_none_or(|key| length(word) >= length(&words[key])) {
            key = Some(index);
        }
    }
    words
        .iter()
        .enumerate()
        .map(|(index, word)| {
            let jitter = rng.next();
            if word.blank && text(word).trim().is_empty() {
                1.0
            } else if Some(index) == key && words.len() > 1 {
                1.45
            } else if minor(word) {
                0.62
            } else {
                0.85 + jitter * 0.35
            }
        })
        .collect()
}

/// `wordJags`: the key word stays, small words jump more.
fn word_jags(scales: &[f32], seed: &str) -> Vec<f32> {
    let mut rng = Rng(hash_text(FNV, &format!("{seed}:jags")));
    let key_scale = scales.iter().copied().fold(f32::MIN, f32::max);
    scales
        .iter()
        .map(|&scale| {
            let offset = (rng.next() - 0.5) * 2.0;
            if scale == key_scale && scales.len() > 1 { 0.0 } else { offset * if scale < 0.7 { 0.32 } else { 0.2 } }
        })
        .collect()
}

/// `resolveWindowCursor`: the last line whose change has begun (−1 before the first).
fn cursor(metas: &[Meta], time: f64) -> i64 {
    let mut current = -1;
    for (index, meta) in metas.iter().enumerate() {
        if meta.start - LEAD <= time {
            current = index as i64;
        } else {
            break;
        }
    }
    current
}

fn to_world(transform: &Transform, point: [f32; 2]) -> [f32; 2] {
    let (sin, cos) = transform.rotation.sin_cos();
    [
        transform.x + (point[0] * cos - point[1] * sin) * transform.scale,
        transform.y + (point[0] * sin + point[1] * cos) * transform.scale,
    ]
}

fn flight_progress(flight: &Flight, phase: f32) -> f32 {
    ease_cubic((phase - flight.delay) / flight.duration)
}

fn flight_point(flight: &Flight, from: [f32; 2], to: [f32; 2], s: f32) -> [f32; 2] {
    let u = 1.0 - s;
    let (a, b, c, d) = (u * u * u, 3.0 * u * u * s, 3.0 * u * s * s, s * s * s);
    [
        a * from[0] + b * (from[0] + flight.c1[0]) + c * (to[0] + flight.c2[0]) + d * to[0],
        a * from[1] + b * (from[1] + flight.c1[1]) + c * (to[1] + flight.c2[1]) + d * to[1],
    ]
}

/// `decayAmount`: drift in font sizes, `0.08 × age^1.5` after `delay`.
fn decay_amount(decay: Decay, lit_at: f64, time: f64) -> f32 {
    let age = (time - lit_at) as f32 - decay.delay;
    if age > 0.0 { decay.strength * 0.08 * age.powf(1.5) } else { 0.0 }
}

/// `flashEnvelope` with a 0.15 s attack.
fn flash_envelope(start: f64, time: f64, decay: f32) -> f32 {
    let age = (time - start) as f32;
    if age <= 0.0 {
        return 0.0;
    }
    if age < 0.15 {
        return smooth(age / 0.15);
    }
    (-(age - 0.15) / decay).exp()
}

/// `resolveSpotX` along one axis: the sung position mapped onto glyph centers, averaged over the last 0.3 s.
fn spot_along(timings: &[GraphemeTiming], center: impl Fn(usize) -> f32, time: f64) -> f32 {
    let count = timings.len();
    if count == 0 {
        return 0.0;
    }
    let mut sum = 0.0;
    for sample in 0..SPOT_SAMPLES {
        let at = time - SPOT_WINDOW * sample as f64 / (SPOT_SAMPLES - 1) as f64;
        let position: f32 = timings
            .iter()
            .map(|timing| (((at - timing.start) / (timing.end - timing.start).max(0.08)) as f32).clamp(0.0, 1.0))
            .sum();
        let index = (position - 0.5).clamp(0.0, (count - 1) as f32);
        let lower = index.floor() as usize;
        let upper = (lower + 1).min(count - 1);
        sum += center(lower) + (center(upper) - center(lower)) * (index - lower as f32);
    }
    sum / SPOT_SAMPLES as f32
}

fn ease_sine(value: f32) -> f32 {
    (1.0 - (PI * value.clamp(0.0, 1.0)).cos()) / 2.0
}

fn ease_cubic(value: f32) -> f32 {
    let t = value.clamp(0.0, 1.0);
    if t < 0.5 { 4.0 * t * t * t } else { 1.0 - (-2.0 * t + 2.0).powi(3) / 2.0 }
}

fn lerp(a: f32, b: f32, k: f32) -> f32 {
    a + (b - a) * k
}

fn lerp2(a: [f32; 2], b: [f32; 2], k: f32) -> [f32; 2] {
    [lerp(a[0], b[0], k), lerp(a[1], b[1], k)]
}
