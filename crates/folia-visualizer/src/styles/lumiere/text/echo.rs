// Background echo (lyricEcho.ts): each sung word becomes a huge hollow fragment near the main source that drifts
// down the main beam for about seven seconds, coming apart, turning and shearing. A line is shaped only shortly before
// its first fragment and dropped after its last.

use folia_core::Line;
use folia_render::glyphs::{Mark, Sprite};

use super::super::art::Rng;
use super::super::beams::{self, Resolved};
use super::super::program::hash_text;
use super::glyph_line::{self, GlyphLine};
use super::slots::smooth;
use super::{Light, camera_axes, mul, rotate, through};

const LIFE: f64 = 7.0;
const SPEED: f32 = 0.075;
const BIRTH_DISTANCE: f32 = 0.08;
const PREPARE: f64 = 3.0;
const OUTLINE: f32 = 0.012;
const LETTER_SPACING: f32 = 0.04;
/// Background glyphs need little resolution: at most 1.5 physical pixels per logical pixel.
const MAX_SCALE: f32 = 1.5;
const FNV: u32 = 2_166_136_261;

struct Piece {
    grapheme: usize,
    blank: bool,
    /// Center offset in the word, set once the line is shaped.
    offset: f32,
    dx: f32,
    dy: f32,
    speed: f32,
    spin: f32,
}

struct Fragment {
    birth: f64,
    across: f32,
    speed: f32,
    scale: f32,
    rotation: f32,
    stretch: f32,
    shear: f32,
    spin: f32,
    line: usize,
    pieces: Vec<Piece>,
}

struct EchoLine {
    /// Unit-local line index.
    index: usize,
    fragments: std::ops::Range<usize>,
    from: f64,
    to: f64,
    layout: Option<GlyphLine>,
}

pub struct Echo {
    fragments: Vec<Fragment>,
    lines: Vec<EchoLine>,
    font_px: f32,
    opacity: f32,
    aspect: f32,
}

impl Echo {
    /// Draws every random value at once, in the original order; nothing is shaped yet.
    pub fn new(lines: &[Line], seed: &str, font_px: f32, opacity: f32, aspect: f32) -> Self {
        let mut rng = Rng(hash_text(FNV, &format!("{seed}:echo")));
        let mut fragments = Vec::new();
        let mut echo_lines = Vec::new();
        for (index, line) in lines.iter().enumerate() {
            let count = folia_lyrics::graphemes(&line.text).len();
            let timings = folia_lyrics::grapheme_timeline(line);
            let first = fragments.len();
            for word in folia_lyrics::word_spans(&line.text) {
                let end = word.end.min(count);
                if word.blank || end <= word.start {
                    continue;
                }
                let mut fragment = Fragment {
                    birth: timings.get(word.start).map_or(line.start, |timing| timing.start),
                    across: (rng.next() - 0.5) * 2.8,
                    speed: 0.7 + rng.next() * 0.7,
                    scale: 0.4 + rng.next() * 0.6,
                    rotation: (rng.next() - 0.5) * 0.9,
                    stretch: 0.75 + rng.next() * 0.7,
                    shear: (rng.next() - 0.5) * 0.12,
                    spin: (rng.next() - 0.5) * 0.08,
                    line: echo_lines.len(),
                    pieces: Vec::with_capacity(end - word.start),
                };
                for grapheme in word.start..end {
                    let angle = rng.next() * std::f32::consts::TAU;
                    fragment.pieces.push(Piece {
                        grapheme,
                        blank: false,
                        offset: 0.0,
                        dx: angle.cos(),
                        dy: angle.sin(),
                        speed: 0.3 + rng.next() * 0.9,
                        spin: (rng.next() - 0.5) * 0.5,
                    });
                }
                fragments.push(fragment);
            }
            let births = fragments[first..].iter().map(|fragment| fragment.birth);
            let Some(from) = births.clone().reduce(f64::min) else { continue };
            let to = births.reduce(f64::max).unwrap_or(from);
            echo_lines.push(EchoLine { index, fragments: first..fragments.len(), from, to, layout: None });
        }
        Self { fragments, lines: echo_lines, font_px, opacity, aspect }
    }

    /// Shapes lines whose fragments are showing (one line a frame ahead of time) and drops lines that are done.
    fn prepare(&mut self, lines: &[Line], time: f64) {
        let mut budget = 1;
        for echo_line in &mut self.lines {
            if time < echo_line.from - PREPARE || time > echo_line.to + LIFE {
                echo_line.layout = None;
                continue;
            }
            if echo_line.layout.is_some() {
                continue;
            }
            if time < echo_line.from {
                if budget == 0 {
                    continue;
                }
                budget -= 1;
            }
            let Some(line) = lines.get(echo_line.index) else { continue };
            let layout = glyph_line::build(&line.text, self.font_px, LETTER_SPACING);
            for fragment in &mut self.fragments[echo_line.fragments.clone()] {
                place(fragment, &layout);
            }
            echo_line.layout = Some(layout);
        }
    }

    /// The fragments of this frame. Shaped lines with no fragment showing still send zero-area sprites, so their
    /// glyphs stay in the atlas until the line is dropped.
    pub fn paint(&mut self, lines: &[Line], time: f64, light: &Light<'_>, intensity: f32, sprites: &mut Vec<Sprite>) {
        self.prepare(lines, time);
        let height = light.height;
        let main = light.beams.first();
        let (ox, oy) = main.map_or((self.aspect / 2.0, 0.0), |beam| (beam.ox, beam.oy));
        let (bx, by) = main.map_or((0.0, 1.0), |beam| (beam.dx, beam.dy));
        let half_at =
            |distance: f32| main.map_or(0.08, |beam: &Resolved| beam.half_width + distance * beam.tan_spread) + 0.04;
        let camera = camera_axes(light.camera);
        for fragment in &self.fragments {
            let Some(layout) = &self.lines[fragment.line].layout else { continue };
            let age = (time - fragment.birth) as f32;
            let presence = smooth(age / 0.9) * (1.0 - smooth((age - (LIFE as f32 - 2.5)) / 2.5));
            let alpha = presence * intensity * self.opacity;
            let showing = (0.0..=LIFE as f32).contains(&age) && alpha > 0.003;
            if !showing {
                warm(fragment, layout, self.font_px, sprites);
                continue;
            }
            let distance = BIRTH_DISTANCE + SPEED * fragment.speed * age;
            let across = fragment.across * half_at(distance);
            let x = (ox + bx * distance - by * across) * height;
            let y = (oy + by * distance + bx * across) * height;
            let turn = fragment.rotation + fragment.spin * age;
            let skew = fragment.shear * age;
            let (sx, sy) = (fragment.scale * fragment.stretch, fragment.scale);
            let holder = [turn.cos() * sx, turn.sin() * sx, -(turn - skew).sin() * sy, (turn - skew).cos() * sy];
            let split = 0.05 * age.max(0.0).powf(1.3);
            let lit = beams::compress(beams::light_at(light.beams, x / height, y / height));
            let color = [light.light[0], light.light[1], light.light[2], alpha * (0.05 + 0.4 * lit)];
            for piece in &fragment.pieces {
                if piece.blank {
                    continue;
                }
                let Some(grapheme) = layout.graphemes.get(piece.grapheme) else { continue };
                let local = [
                    piece.offset + piece.dx * self.font_px * split * piece.speed,
                    piece.dy * self.font_px * split * piece.speed,
                ];
                let at =
                    [x + holder[0] * local[0] + holder[2] * local[1], y + holder[1] * local[0] + holder[3] * local[1]];
                let axes = mul(camera, mul(holder, rotate(piece.spin * split * 4.0)));
                for placed in &layout.glyphs[grapheme.glyphs.clone()] {
                    sprites.push(Sprite {
                        mark: mark(placed.glyph, self.font_px),
                        anchor: through(light.camera, at),
                        pen: placed.pen,
                        axes,
                        color,
                    });
                }
            }
        }
    }
}

fn mark(glyph: folia_render::glyphs::ShapedGlyph, font_px: f32) -> Mark {
    Mark::Glyph { glyph, size: font_px, outline: OUTLINE, max_scale: MAX_SCALE }
}

/// Zero-area sprites that keep a shaped line's glyphs resident.
fn warm(fragment: &Fragment, layout: &GlyphLine, font_px: f32, sprites: &mut Vec<Sprite>) {
    for piece in fragment.pieces.iter().filter(|piece| !piece.blank) {
        let Some(grapheme) = layout.graphemes.get(piece.grapheme) else { continue };
        for placed in &layout.glyphs[grapheme.glyphs.clone()] {
            sprites.push(Sprite {
                mark: mark(placed.glyph, font_px),
                anchor: [0.0, 0.0],
                pen: placed.pen,
                axes: [0.0; 4],
                color: [0.0; 4],
            });
        }
    }
}

/// Offsets of the fragment's characters from the middle of the word.
fn place(fragment: &mut Fragment, layout: &GlyphLine) {
    let (Some(first), Some(last)) = (fragment.pieces.first(), fragment.pieces.last()) else { return };
    let (Some(left), Some(right)) = (layout.graphemes.get(first.grapheme), layout.graphemes.get(last.grapheme)) else {
        return;
    };
    let middle = (left.char_x + right.char_x + right.char_width) / 2.0;
    for piece in &mut fragment.pieces {
        if let Some(grapheme) = layout.graphemes.get(piece.grapheme) {
            piece.blank = grapheme.blank;
            piece.offset = grapheme.char_x + grapheme.char_width / 2.0 - middle;
        }
    }
}
