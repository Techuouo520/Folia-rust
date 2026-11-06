// Lumiere (绘光): a dark stage, volumetric shafts and a small window of lyrics lit by those shafts.

mod art;
mod beams;
pub mod bloom;
mod catalog;
mod decor;
mod field;
mod gpu;
mod more;
mod palette;
mod profiles;
mod program;
mod tuning;

use std::cell::RefCell;
use std::marker::PhantomData;
use std::rc::Rc;

use folia_core::{Line, Lyrics};
use folia_lyrics::{RenderHints, active_line};
use iced::font::Weight;
use iced::widget::canvas::{Frame, Path, Stroke, Text};
use iced::widget::shader;
use iced::{Color, Font, Point, Radians, Rectangle, Size, Vector, mouse};

use crate::{Scene, measure};

pub use tuning::{Quality, Tuning, manifest};

use field::View;

const HERO: Font = Font { weight: Weight::Semibold, ..Font::DEFAULT };

#[derive(Default)]
pub struct Cache {
    seed: u32,
    font_key: u64,
    program: program::Program,
    widths: Vec<Measured>,
    decor: decor::Store,
}

struct Measured {
    start: u64,
    key: u64,
    words: Rc<Vec<f32>>,
}

#[derive(Clone, Copy)]
struct Stamp {
    pub x: f32,
    pub y: f32,
    pub size: f32,
    pub width: f32,
    pub vertical: bool,
    pub color: [f32; 4],
}

/// The light field for one frame. Lyrics are drawn by the canvas stacked above this shader.
pub struct Field<'a, Message> {
    scene: Scene<'a>,
    _message: PhantomData<Message>,
}

pub fn field_view<'a, Message>(scene: Scene<'a>) -> Field<'a, Message> {
    Field { scene, _message: PhantomData }
}

impl<Message> shader::Program<Message> for Field<'_, Message> {
    type State = RefCell<Cache>;
    type Primitive = gpu::Pass;

    fn draw(&self, state: &Self::State, _cursor: mouse::Cursor, bounds: Rectangle) -> Self::Primitive {
        let mut cache = state.borrow_mut();
        let size = bounds.size();
        let tuning = Tuning::read(&self.scene.tuning);
        cache.sync(self.scene.lyrics, size, self.scene.font_scale, self.scene.seed, tuning.seamless);
        let view = field::prepare(&cache.program, self.scene.time, size, self.scene.theme, tuning);
        stage_pass(&mut cache, &self.scene, &view, tuning)
    }
}

/// Foreground bokeh, drawn above the lyric canvas.
pub struct Front<'a, Message> {
    scene: Scene<'a>,
    _message: PhantomData<Message>,
}

pub fn front_view<'a, Message>(scene: Scene<'a>) -> Front<'a, Message> {
    Front { scene, _message: PhantomData }
}

impl<Message> shader::Program<Message> for Front<'_, Message> {
    type State = RefCell<Cache>;
    type Primitive = gpu::FrontPass;

    fn draw(&self, state: &Self::State, _cursor: mouse::Cursor, bounds: Rectangle) -> Self::Primitive {
        let mut cache = state.borrow_mut();
        let size = bounds.size();
        let tuning = Tuning::read(&self.scene.tuning);
        cache.sync(self.scene.lyrics, size, self.scene.font_scale, self.scene.seed, tuning.seamless);
        let view = field::prepare(&cache.program, self.scene.time, size, self.scene.theme, tuning);
        let particles = {
            let Cache { decor, program, .. } = &mut *cache;
            decor.front(program, &view, self.scene.time, tuning)
        };
        gpu::FrontPass {
            bytes: field::pack(&view),
            decor: decor::uniform(&cache.program, self.scene.time, tuning),
            particles,
        }
    }
}

fn stage_pass(cache: &mut Cache, scene: &Scene<'_>, view: &View, tuning: Tuning) -> gpu::Pass {
    let Cache { decor, program, widths, seed, .. } = cache;
    decor.ensure(*seed, scene.lyrics, program);
    let bursts = decor.active(scene.time);
    let mut placed = [[0.0_f32; 4]; 14];
    for (slot, burst) in bursts.iter().enumerate() {
        let Some(burst) = burst else { continue };
        let Some(at) = burst_anchor(widths, program, scene, view, burst, tuning) else {
            continue;
        };
        placed[slot] = [at[0], at[1], at[2], 1.0];
    }
    let (lines, particles) = decor.rebuild(program, view, scene.time, tuning);
    let mut decor_bytes = decor::uniform(program, scene.time, tuning);
    for (slot, burst) in bursts.iter().enumerate() {
        let Some(burst) = burst else { continue };
        if placed[slot][3] < 0.5 {
            continue;
        }
        let age = scene.time as f32 - burst.time;
        let at = [placed[slot][0], placed[slot][1], placed[slot][2]];
        decor::stamp(&mut decor_bytes, slot, at, age, burst.tint, view.palette.light);
    }
    gpu::Pass {
        bytes: field::pack(view),
        decor: decor_bytes,
        lines,
        particles,
        kind: profiles::PROFILES[view.profile].kind,
        bloom: tuning.bloom,
    }
}

/// Draws the lyrics, motes and frame. The light field is the shader behind this canvas.
pub fn draw(scene: &Scene<'_>, cache: &mut Cache, frame: &mut Frame, size: Size) {
    let tuning = Tuning::read(&scene.tuning);
    cache.sync(scene.lyrics, size, scene.font_scale, scene.seed, tuning.seamless);
    let view = field::prepare(&cache.program, scene.time, size, scene.theme, tuning);
    frame.with_save(|frame| {
        apply_camera(frame, &view.camera);
        if !tuning.text_only {
            paint_echo(frame, scene, &view, tuning);
        }
        paint_words(scene, cache, &view, tuning, frame);
    });
    if tuning.overlay_frame && !tuning.text_only {
        paint_overlay(frame, size, view.palette.light);
    }
}

impl Cache {
    fn sync(&mut self, lyrics: &Lyrics, size: Size, font_scale: f32, seed: &str, seamless: bool) {
        let lyric = program::song_seed(lyrics);
        let lyric_changed = self.seed != lyric;
        if lyric_changed {
            self.widths.clear();
            self.seed = lyric;
        }
        let seamless_changed = self.program.seamless != (seamless && self.program.spans.len() >= 2);
        if lyric_changed || seamless_changed || self.program.shots.is_empty() || self.program.seed != seed {
            self.program = program::Program::compile(lyrics, seed, seamless);
        }
        let font_key =
            ((size.height.to_bits() as u64) << 32) | size.width.to_bits() as u64 ^ font_scale.to_bits() as u64;
        if self.font_key != font_key {
            self.widths.clear();
            self.font_key = font_key;
        }
    }
}

fn paint_words(scene: &Scene<'_>, cache: &mut Cache, view: &View, tuning: Tuning, frame: &mut Frame) {
    let Some(current) = active_line(&scene.lyrics.lines, scene.time) else { return };
    let profile = profiles::PROFILES[view.profile];
    for index in 0..scene.lyrics.lines.len() {
        let relative = index as i32 - current as i32;
        if !visible(relative, tuning.window_neighbors) {
            continue;
        }
        let line = &scene.lyrics.lines[index];
        let font_px = profile.hero * view.height * scene.font_scale * if relative == 0 { 1.0 } else { 0.62 };
        let vertical = line_vertical(&cache.program, index, &profile);
        let widths = cache.widths_of(line, font_px, view.width * 0.72);
        let draw = LineDraw { line, widths: &widths, view, font_px, relative, vertical, time: scene.time, tuning };
        place_line(&draw, |_index, text, stamp| {
            if stamp.color[3] < 0.02 {
                return;
            }
            frame.fill_text(Text {
                content: text.to_owned(),
                position: Point::new(stamp.x, stamp.y),
                color: Color::from_rgba(stamp.color[0], stamp.color[1], stamp.color[2], stamp.color[3]),
                size: stamp.size.into(),
                font: HERO,
                align_x: iced::widget::text::Alignment::Center,
                align_y: iced::alignment::Vertical::Center,
                ..Text::default()
            });
        });
    }
}

struct LineDraw<'a> {
    line: &'a Line,
    widths: &'a [f32],
    view: &'a View,
    font_px: f32,
    relative: i32,
    vertical: bool,
    time: f64,
    tuning: Tuning,
}

fn place_line(draw: &LineDraw<'_>, mut visit: impl FnMut(usize, &str, Stamp)) {
    let profile = &profiles::PROFILES[draw.view.profile];
    let hints = RenderHints::of(draw.line);
    let gap = draw.font_px * 0.12;
    let total: f32 = draw.widths.iter().sum::<f32>() + gap * draw.widths.len().saturating_sub(1) as f32;
    let origin_x = draw.view.width * 0.5;
    let origin_y = profile.region_cy * draw.view.height + draw.relative as f32 * draw.font_px * 1.5;
    let mut cursor = if draw.vertical { origin_y - total * 0.5 } else { origin_x - total * 0.5 };
    for index in 0..part_count(draw.line) {
        let Some((text, start, _)) = part_at(draw.line, index) else { continue };
        let width = draw.widths.get(index).copied().unwrap_or(draw.font_px);
        let center = cursor + width * 0.5;
        let (mut x, mut y) = if draw.vertical { (origin_x, center) } else { (center, origin_y) };
        cursor += width + gap;
        let reveal = reveal_of(start, hints, draw.time);
        let lit = beams::compress(beams::light_at(
            &draw.view.beams[..draw.view.count as usize],
            x / draw.view.height,
            y / draw.view.height,
        ));
        let mix = lit * reveal;
        let rgb = palette::mix3(draw.view.palette.unlit, draw.view.palette.lit, mix);
        let mut alpha = (draw.tuning.unlit_opacity + (1.0 - draw.tuning.unlit_opacity) * mix)
            * if draw.relative == 0 { 1.0 } else { 0.72 };
        alpha *= draw.view.enter;
        let age = draw.time - start - f64::from(profile.decay_delay);
        if age > 0.0 && draw.tuning.decay > 0.0 {
            let amount = profile.decay * draw.tuning.decay * 0.08 * (age as f32).powf(1.5);
            let angle = beams::hash12(index as f32, 0.37) * std::f32::consts::TAU;
            alpha *= (1.0 - beams::smooth(1.0, 3.0, amount)).clamp(0.0, 1.0);
            x += amount * draw.font_px * angle.cos();
            y += amount * draw.font_px * angle.sin() * 0.35;
        }
        visit(
            index,
            text,
            Stamp { x, y, size: draw.font_px, width, vertical: draw.vertical, color: [rgb[0], rgb[1], rgb[2], alpha] },
        );
    }
}

fn burst_anchor(
    widths: &mut Vec<Measured>,
    program: &program::Program,
    scene: &Scene<'_>,
    view: &View,
    burst: &decor::BurstRef,
    tuning: Tuning,
) -> Option<[f32; 3]> {
    let line = scene.lyrics.lines.get(burst.line)?;
    let current = active_line(&scene.lyrics.lines, scene.time).unwrap_or(burst.line);
    let relative = burst.line as i32 - current as i32;
    let profile = profiles::PROFILES[view.profile];
    let font_px = profile.hero * view.height * scene.font_scale * if relative == 0 { 1.0 } else { 0.62 };
    let vertical = line_vertical(program, burst.line, &profile);
    let widths = measured(widths, line, font_px, view.width * 0.72);
    let draw = LineDraw { line, widths: &widths, view, font_px, relative, vertical, time: scene.time, tuning };
    let mut found = None;
    place_line(&draw, |index, _, stamp| {
        if index == burst.word {
            found = Some(stamp);
        }
    });
    let stamp = found?;
    let count = f32::from(burst.chars.max(1));
    let along = ((f32::from(burst.char_index) + 0.5) / count - 0.5) * stamp.width;
    let (x, y) = if stamp.vertical { (stamp.x, stamp.y + along) } else { (stamp.x + along, stamp.y) };
    Some([x, y + burst.dy * stamp.size, stamp.size * burst.size])
}

fn line_vertical(program: &program::Program, index: usize, fallback: &profiles::Profile) -> bool {
    let typography = program
        .shots
        .iter()
        .find(|shot| {
            let count = usize::from(shot.line_count).min(shot.lines.len());
            shot.lines[..count].contains(&index)
        })
        .map(|shot| profiles::PROFILES[shot.profile].typography)
        .unwrap_or(fallback.typography);
    match typography {
        profiles::Type::Vertical => true,
        profiles::Type::Crossed => index % 2 == 1,
        profiles::Type::Horizontal => false,
    }
}

fn reveal_of(start: f64, hints: RenderHints, time: f64) -> f32 {
    let began = start - hints.word_lookahead();
    beams::smooth(0.0, 1.0, ((time - began) / 0.3) as f32)
}

fn part_count(line: &Line) -> usize {
    if line.words.is_empty() { 1 } else { line.words.len() }
}

fn part_at(line: &Line, index: usize) -> Option<(&str, f64, f64)> {
    if line.words.is_empty() {
        return (index == 0).then_some((line.text.as_str(), line.start, line.end));
    }
    line.words.get(index).map(|word| (word.text.as_str(), word.start, word.end))
}

fn visible(relative: i32, neighbors: u8) -> bool {
    relative == 0 || relative == -1 || (neighbors >= 2 && relative == 1)
}

fn paint_echo(frame: &mut Frame, scene: &Scene<'_>, view: &View, tuning: Tuning) {
    if tuning.echo <= 0.0 {
        return;
    }
    let Some(index) = active_line(&scene.lyrics.lines, scene.time) else { return };
    let line = &scene.lyrics.lines[index];
    if line.text.is_empty() {
        return;
    }
    let size = profiles::PROFILES[view.profile].hero * view.height * 1.8 * tuning.echo.min(2.0);
    frame.fill_text(Text {
        content: line.text.clone(),
        position: Point::new(view.width * 0.5, view.height * profiles::PROFILES[view.profile].region_cy),
        color: Color::from_rgba(
            view.palette.light[0],
            view.palette.light[1],
            view.palette.light[2],
            0.08 * tuning.echo,
        ),
        size: size.into(),
        font: HERO,
        align_x: iced::widget::text::Alignment::Center,
        align_y: iced::alignment::Vertical::Center,
        ..Text::default()
    });
}

fn paint_overlay(frame: &mut Frame, size: Size, light: [f32; 3]) {
    let color = Color::from_rgba(light[0], light[1], light[2], 0.5);
    let unit = size.width.min(size.height);
    let pad_x = (size.width * 0.065).max(30.0);
    let pad_y = (size.height * 0.085).max(30.0);
    let arm = unit * 0.045;
    let stroke = Stroke::default().with_width((unit / 600.0).max(1.2)).with_color(color);
    let corners = [
        (pad_x, pad_y, 1.0, 1.0),
        (size.width - pad_x, pad_y, -1.0, 1.0),
        (size.width - pad_x, size.height - pad_y, -1.0, -1.0),
        (pad_x, size.height - pad_y, 1.0, -1.0),
    ];
    for (x, y, sx, sy) in corners {
        let path = Path::new(|builder| {
            builder.move_to(Point::new(x, y + sy * arm));
            builder.line_to(Point::new(x, y));
            builder.line_to(Point::new(x + sx * arm, y));
        });
        frame.stroke(&path, stroke);
    }
}

fn apply_camera(frame: &mut Frame, camera: &field::Camera) {
    frame.translate(Vector::new(camera.x, camera.y));
    frame.rotate(Radians(camera.rotation));
    frame.scale(camera.scale);
    frame.translate(Vector::new(-camera.pivot_x, -camera.pivot_y));
}

impl Cache {
    fn widths_of(&mut self, line: &Line, font_px: f32, max_width: f32) -> Rc<Vec<f32>> {
        measured(&mut self.widths, line, font_px, max_width)
    }
}

fn measured(widths: &mut Vec<Measured>, line: &Line, font_px: f32, max_width: f32) -> Rc<Vec<f32>> {
    let key = (font_px.to_bits() as u64) << 32 | max_width.to_bits() as u64;
    let start = line.start.to_bits();
    if let Some(found) = widths.iter().find(|item| item.start == start && item.key == key) {
        return found.words.clone();
    }
    let mut words = Vec::with_capacity(part_count(line));
    for index in 0..part_count(line) {
        if let Some((text, _, _)) = part_at(line, index) {
            words.push(measure(text, font_px, HERO, max_width).width);
        }
    }
    let words = Rc::new(words);
    if widths.len() == 8 {
        widths.remove(0);
    }
    widths.push(Measured { start, key, words: words.clone() });
    words
}
