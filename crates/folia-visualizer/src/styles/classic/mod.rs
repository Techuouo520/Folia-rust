// Classic (流光): one line at a time; each word springs in, glows while sung and drifts once passed.

mod layout;
mod tuning;

pub use layout::{ClassicTuning, Justify, font_px, justify, random, word_configs, wrap};
pub use tuning::manifest;

use std::rc::Rc;

use folia_core::{Line, Word};
use folia_lyrics::RenderHints;
use iced::widget::canvas::Frame;
use iced::{Point, Size};

use crate::styles::glow::{self, WordConfig};
use crate::{LayoutCache, Scene, measure};

const LINE_HEIGHT: f32 = 1.22;
const MAX_LINE_WIDTH: f32 = 1152.0;
const PADDING: f32 = 32.0;

pub struct LineLayout {
    /// Display words: CJK words grouped and punctuation attached, as the original's semantic layout.
    words: Vec<Word>,
    configs: Vec<WordConfig>,
    centers: Vec<Point>,
    font_px: f32,
}

pub fn draw(scene: &Scene<'_>, cache: &mut LayoutCache<LineLayout>, frame: &mut Frame, size: Size) {
    let manifest = manifest();
    let tuning = ClassicTuning {
        word_rotation: scene.tuning.flag(manifest, "enableWordRotation"),
        breathing: f64::from(scene.tuning.number(manifest, "breathingFloatMultiplier")),
        word_spacing: f64::from(scene.tuning.number(manifest, "wordSpacing")),
        legacy: scene.tuning.choice(manifest, "useLegacyLayout") == "true",
        avoid_overlap: scene.tuning.flag(manifest, "avoidOverlap"),
    };
    let salt = scene.layout_key();
    let theme = scene.theme;
    let center = Point::new(size.width / 2.0, size.height / 2.0);
    let (breath_y, breath_scale) = glow::breathing(scene.time, theme.intensity, tuning.breathing);

    glow::visible_lines(&scene.lyrics.lines, scene.time, |line, opacity, scale, _active| {
        let layout = cache.get_or_insert(line, size, scene.font_scale, salt, || build(line, scene, size, tuning));
        let hints = RenderHints::of(line);
        frame.with_save(|frame| {
            glow::scale_around(frame, center, scale * breath_scale, breath_y);
            for ((word, config), at) in layout.words.iter().zip(&layout.configs).zip(&layout.centers) {
                let pose = glow::word_pose(
                    config,
                    word.start,
                    word.end,
                    hints,
                    theme.intensity,
                    tuning.word_rotation,
                    scene.time,
                );
                glow::draw_word(frame, scene, &word.text, *at, &pose, opacity, layout.font_px);
            }
        });
    });
}

fn build(line: &Line, scene: &Scene<'_>, size: Size, tuning: ClassicTuning) -> Rc<LineLayout> {
    let original_font = font_px(size.width, scene.font_scale);
    let mut font_px = original_font;
    let words = if tuning.legacy {
        line.words.clone()
    } else {
        folia_lyrics::display_words(line, &folia_lyrics::layout_units(line, true))
    };
    loop {
        let (configs, height, centers) = arrange(line, scene, size, tuning, &words, font_px);
        let fit = ((size.height * 0.7 - PADDING * 2.0).max(1.0) / height).min(1.0);
        let next = (font_px * fit * 0.98).max(original_font * 0.5);
        if !tuning.avoid_overlap || tuning.legacy || fit >= 1.0 || next >= font_px - original_font * 0.005 {
            return Rc::new(LineLayout { words, configs, centers, font_px });
        }
        font_px = next;
    }
}

fn arrange(
    line: &Line,
    scene: &Scene<'_>,
    size: Size,
    tuning: ClassicTuning,
    words: &[Word],
    font_px: f32,
) -> (Vec<WordConfig>, f32, Vec<Point>) {
    let widths: Vec<f32> = words
        .iter()
        .map(|word| measure(&word.text, font_px, scene.typography.font(crate::BOLD), f32::INFINITY).width)
        .collect();
    let seed = line.start;
    let interlude = line.is_interlude();
    let configs = word_configs(seed, scene.theme.intensity, interlude, &widths, font_px, tuning);
    let margins: Vec<f32> = configs.iter().map(|config| config.margin_right as f32).collect();
    let max_width = (size.width - PADDING * 2.0).clamp(1.0, MAX_LINE_WIDTH);
    let left = (size.width - max_width) / 2.0;
    let justify = justify(seed, scene.theme.intensity, interlude);
    let (mut top, mut bottom) = (0.0_f64, 0.0_f64);
    if tuning.avoid_overlap && !tuning.legacy && !interlude {
        for (width, config) in widths.iter().zip(&configs) {
            let reach = glow::word_reach(f64::from(*width), f64::from(font_px), config);
            let over = reach[1] - f64::from(font_px * LINE_HEIGHT) / 2.0;
            top = top.max(over - config.y);
            bottom = bottom.max(over + config.y);
        }
    }
    let gap = (top + bottom).ceil() as f32;
    let (top, bottom) = (top.ceil() as f32, bottom.ceil() as f32);
    let placed = wrap(&widths, &margins, font_px * LINE_HEIGHT + gap, max_width, left, size.height / 2.0, justify);
    let height = placed.last().zip(placed.first()).map_or(0.0, |(last, first)| last.y - first.y)
        + font_px * LINE_HEIGHT
        + top
        + bottom;
    let centers = placed.iter().map(|p| Point::new(p.x, p.y + (top - bottom) / 2.0)).collect();
    (configs, height, centers)
}
