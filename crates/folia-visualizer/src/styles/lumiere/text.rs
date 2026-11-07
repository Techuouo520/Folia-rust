// Lumiere text: the lyric window and the background echo of the current scene unit, as glyph sprites.

mod echo;
mod glyph_line;
mod marks;
mod slots;
mod window;
mod wrap;

use folia_core::Lyrics;
use folia_render::glyphs::{Mark, Sprite};

use super::beams::Resolved;
use super::decor;
use super::field::{Camera, View};
use super::profiles::{PROFILES, Type};
use super::program::Program;
use super::tuning::Tuning;
use echo::Echo;
use slots::Region;
use window::{Decay, Setup, Window};

pub use glyph_line::LYRIC_FONT;
pub use marks::images;
pub use wrap::frame_insets;

/// A sprite that draws nothing: zero area.
pub const ZERO: Sprite =
    Sprite { mark: Mark::Image(0), anchor: [0.0; 2], pen: [0.0; 2], axes: [0.0; 4], color: [0.0; 4] };

/// Light and color shared by the window and the echo for one frame.
pub struct Light<'a> {
    pub beams: &'a [Resolved],
    pub light: [f32; 3],
    pub lit: [f32; 3],
    pub unlit: [f32; 3],
    pub unlit_alpha: f32,
    pub intensity: f32,
    pub hide_trails: bool,
    pub camera: &'a Camera,
    pub height: f32,
}

#[derive(Clone, PartialEq)]
struct Key {
    unit: Option<u16>,
    seed: u32,
    lyric: u32,
    size: (u32, u32),
    decay: u32,
    neighbors: u8,
    trails: bool,
    echo: u32,
}

/// The current scene unit's window and echo, rebuilt when the unit, size or the tuning they read changes.
#[derive(Default)]
pub struct Text {
    key: Option<Key>,
    lines: std::ops::Range<usize>,
    start: f64,
    window: Option<Window>,
    echo: Option<Echo>,
}

impl Text {
    pub fn sync(&mut self, program: &Program, lyrics: &Lyrics, lyric: u32, view: &View, tuning: Tuning, time: f64) {
        let unit = (!program.seamless).then(|| decor::unit_paragraph(program, time));
        let key = Key {
            unit,
            seed: super::program::hash_text(2_166_136_261, &program.seed),
            lyric,
            size: (view.width.to_bits(), view.height.to_bits()),
            decay: tuning.decay.to_bits(),
            neighbors: tuning.window_neighbors,
            trails: tuning.trails,
            echo: if tuning.text_only { 0 } else { tuning.echo.to_bits() },
        };
        if self.key.as_ref() == Some(&key) {
            return;
        }
        let (lines, start) = unit_lines(program, lyrics, unit);
        let seed = match unit {
            Some(paragraph) => format!("{}:lumiere-p{paragraph}", program.seed),
            None => format!("{}:lumiere-seamless", program.seed),
        };
        let lead = &PROFILES[decor::unit_lead(program, time)];
        let (width, height) = (view.width, view.height);
        let aspect = width / height.max(1.0);
        let unit_lines = &lyrics.lines[lines.clone()];
        let setup = Setup {
            width,
            height,
            region: Region {
                cx: lead.region_cx * aspect,
                cy: lead.region_cy,
                w: lead.region_w * aspect,
                h: lead.region_h,
            },
            hero_px: lead.hero * height,
            neighbors: tuning.window_neighbors,
            decay: Decay { strength: lead.decay * tuning.decay, delay: lead.decay_delay },
            always_fly: tuning.trails,
            seed: seed.clone(),
        };
        self.window = Some(Window::new(setup, unit_lines, kinds(program, unit, lines.clone())));
        self.echo = (tuning.echo > 0.0 && !tuning.text_only)
            .then(|| Echo::new(unit_lines, &seed, lead.echo * height, tuning.echo, aspect));
        self.lines = lines;
        self.start = start;
        self.key = Some(key);
    }

    /// Echo sprites (graphics group).
    pub fn paint_echo(&mut self, lyrics: &Lyrics, time: f64, light: &Light<'_>, sprites: &mut Vec<Sprite>) {
        let lines = lyrics.lines.get(self.lines.clone()).unwrap_or_default();
        let intensity = slots::smooth(((time - self.start) / 1.2) as f32);
        if let Some(echo) = &mut self.echo {
            echo.paint(lines, time, light, intensity, sprites);
        }
    }

    /// Window sprites (text group).
    pub fn paint_window(&mut self, lyrics: &Lyrics, time: f64, light: &Light<'_>, sprites: &mut Vec<Sprite>) {
        let lines = lyrics.lines.get(self.lines.clone()).unwrap_or_default();
        if let Some(window) = &mut self.window {
            window.paint(lines, time, light, sprites);
        }
    }

    /// The window's glyphs as canvas text, for pictures with no shader under them.
    pub fn each_glyph(
        &mut self,
        lyrics: &Lyrics,
        time: f64,
        light: &Light<'_>,
        visit: impl FnMut(&str, [f32; 2], f32, f32, [f32; 4]),
    ) {
        let lines = lyrics.lines.get(self.lines.clone()).unwrap_or_default();
        if let Some(window) = &mut self.window {
            window.each_glyph(lines, time, light, visit);
        }
    }

    /// Stage position and font size of grapheme `glyph` of song line `line`, if it is in this unit.
    pub fn glyph_anchor(&mut self, lyrics: &Lyrics, line: usize, glyph: usize, time: f64) -> Option<[f32; 3]> {
        let local = line.checked_sub(self.lines.start).filter(|_| self.lines.contains(&line))?;
        let lines = lyrics.lines.get(self.lines.clone())?;
        self.window.as_mut()?.glyph_anchor(lines, local, glyph, time)
    }

    /// Light intensity of the window: the unit fades in over 0.6 s.
    pub fn window_intensity(&self, time: f64) -> f32 {
        slots::smooth(((time - self.start) / 0.6) as f32)
    }
}

/// The unit's song lines and start time: one paragraph, or every line when seamless.
fn unit_lines(program: &Program, lyrics: &Lyrics, unit: Option<u16>) -> (std::ops::Range<usize>, f64) {
    let all = 0..lyrics.lines.len();
    let Some(paragraph) = unit else {
        return (all, program.spans.first().map_or(0.0, |span| span.start));
    };
    program.spans.get(usize::from(paragraph)).map_or((all, 0.0), |span| {
        (span.lines[0].min(lyrics.lines.len())..span.lines[1].min(lyrics.lines.len()), span.start)
    })
}

/// `lumiereTypographyOfLine` over the unit's shots, indexed by unit-local line.
fn kinds(program: &Program, unit: Option<u16>, lines: std::ops::Range<usize>) -> Vec<Type> {
    let shots: Vec<_> = program.shots.iter().filter(|shot| unit.is_none_or(|p| shot.paragraph == p)).collect();
    lines
        .map(|line| {
            let mut found = shots.first().map_or(Type::Horizontal, |shot| PROFILES[shot.profile].typography);
            for shot in &shots {
                let covered = &shot.lines[..usize::from(shot.line_count).min(shot.lines.len())];
                if covered.contains(&line) {
                    return PROFILES[shot.profile].typography;
                }
                if covered.first().is_some_and(|first| *first < line) {
                    found = PROFILES[shot.profile].typography;
                }
            }
            found
        })
        .collect()
}

/// A stage point through the camera that moves the whole stage.
pub fn through(camera: &Camera, point: [f32; 2]) -> [f32; 2] {
    let (sin, cos) = camera.rotation.sin_cos();
    let (dx, dy) = ((point[0] - camera.pivot_x) * camera.scale, (point[1] - camera.pivot_y) * camera.scale);
    [camera.x + dx * cos - dy * sin, camera.y + dx * sin + dy * cos]
}

/// The camera's linear part, columns `(a, b)`, `(c, d)`.
pub fn camera_axes(camera: &Camera) -> [f32; 4] {
    mul(rotate(camera.rotation), [camera.scale, 0.0, 0.0, camera.scale])
}

/// `a × b` for 2×2 matrices stored by columns.
pub fn mul(a: [f32; 4], b: [f32; 4]) -> [f32; 4] {
    [a[0] * b[0] + a[2] * b[1], a[1] * b[0] + a[3] * b[1], a[0] * b[2] + a[2] * b[3], a[1] * b[2] + a[3] * b[3]]
}

pub fn rotate(angle: f32) -> [f32; 4] {
    let (sin, cos) = angle.sin_cos();
    [cos, sin, -sin, cos]
}
