// Partita (组曲): the line split into staggered rows that slide in one by one, words glowing like classic, with corner guide lines.

use std::rc::Rc;

use folia_core::{AnimationIntensity, Line, Theme, Word};
use folia_lyrics::{LayoutUnit, RenderHints, display_words, layout_units};
use iced::widget::canvas::{Frame, Path, Stroke};
use iced::{Color, Point, Radians, Size, Vector};

use crate::anim::{Ease, spring, tween};
use crate::styles::classic::random;
use crate::styles::glow::{self, WordConfig};
use crate::{LayoutCache, Scene, color, measure};

const ROW_BASE_HEIGHT: f32 = 100.0;
const WORD_GAP: f32 = 12.8;
const PAD_X: f32 = 8.0;
const PAD_Y: f32 = 3.2;

/// One staggered row: its words and where it rests.
struct Row {
    words: Vec<Word>,
    configs: Vec<WordConfig>,
    /// Word centres relative to the row centre.
    offsets: Vec<f32>,
    center: Point,
    width: f32,
    height: f32,
    x: f64,
    y: f64,
    rotate: f64,
    guide_left: bool,
    start: f64,
    last_word: (f64, f64),
    guides: bool,
}

pub struct PartitaLayout {
    rows: Vec<Row>,
    font_px: f32,
}

pub fn draw(scene: &Scene<'_>, cache: &mut LayoutCache<PartitaLayout>, frame: &mut Frame, size: Size) {
    let theme = scene.theme;
    let manifest = manifest();
    let guides = scene.tuning.flag(manifest, "showGuideLines");
    let semantic = scene.tuning.flag(manifest, "useSemanticLayout");
    let stagger_min = f64::from(scene.tuning.number(manifest, "staggerMin"));
    let stagger_max = f64::from(scene.tuning.number(manifest, "staggerMax"));
    let (stagger_min, stagger_max) = (stagger_min.min(stagger_max), stagger_min.max(stagger_max));
    let center = Point::new(size.width / 2.0, size.height / 2.0);
    let (breath_y, breath_scale) = glow::breathing(scene.time, theme.intensity, 1.0);
    let salt = scene.tuning.fingerprint();

    glow::visible_lines(&scene.lyrics.lines, scene.time, |line, opacity, scale, _active| {
        let layout = cache.get_or_insert(line, size, scene.font_scale, salt, || {
            build(line, scene, size, semantic, stagger_min, stagger_max, guides)
        });
        let hints = RenderHints::of(line);
        frame.with_save(|frame| {
            glow::scale_around(frame, center, scale * breath_scale, breath_y);
            for row in &layout.rows {
                draw_row(frame, theme, row, hints, scene.time, opacity, layout.font_px);
            }
        });
    });
}

fn draw_row(
    frame: &mut Frame,
    theme: &Theme,
    row: &Row,
    hints: RenderHints,
    time: f64,
    line_opacity: f64,
    font_px: f32,
) {
    let active_at = row.start - hints.word_lookahead();
    let passed_at = hints.word_active_end(row.last_word.0, row.last_word.1);
    if time < active_at {
        return;
    }
    let from_x = row.x + if row.guide_left { -40.0 } else { 40.0 };
    let elapsed = time - active_at;
    let (mut opacity, mut scale, mut x) = (
        tween(0.0, 1.0, elapsed, 0.1, Ease::OUT),
        spring(0.85, 1.0, elapsed, 200.0, 20.0),
        spring(from_x, row.x, elapsed, 200.0, 20.0),
    );
    if time > passed_at {
        let since = time - passed_at;
        opacity = tween(opacity, 1.0, since, 0.4, Ease::OUT);
        scale = tween(scale, 1.0, since, 0.4, Ease::OUT);
        x = tween(x, row.x, since, 0.4, Ease::OUT);
    }
    let guide = tween(0.0, 1.0, elapsed, 0.4, Ease::OUT);
    let guide_color = if time <= passed_at { color(theme.accent, 1.0) } else { Color { a: 0.22, ..Color::WHITE } };

    frame.with_save(|frame| {
        frame.translate(Vector::new(row.center.x + x as f32, row.center.y + row.y as f32));
        frame.rotate(Radians(row.rotate.to_radians() as f32));
        frame.scale(scale as f32);
        if row.guides {
            draw_guides(frame, row, guide, Color { a: guide_color.a * (opacity * line_opacity) as f32, ..guide_color });
        }
        for ((word, config), offset) in row.words.iter().zip(&row.configs).zip(&row.offsets) {
            let pose = glow::word_pose(config, word.start, word.end, hints, theme.intensity, true, time);
            glow::draw_word(frame, theme, &word.text, Point::new(*offset, 0.0), &pose, opacity * line_opacity, font_px);
        }
    });
}

/// The L-shaped corner under the row, growing from its corner as the row arrives.
fn draw_guides(frame: &mut Frame, row: &Row, progress: f64, color: Color) {
    let (half_w, half_h) = (row.width / 2.0, row.height / 2.0);
    let progress = progress as f32;
    let stroke = Stroke::default().with_width(1.0).with_color(color);
    let side = if row.guide_left { -1.0 } else { 1.0 };
    let edge = side * (half_w + 8.0);
    let vertical = Path::line(Point::new(edge, half_h + 16.0), Point::new(edge, half_h + 16.0 - 32.0 * progress));
    let start_x = side * (half_w + 16.0);
    let horizontal = Path::line(
        Point::new(start_x, half_h + 8.0),
        Point::new(start_x - side * (row.width + 36.0) * progress, half_h + 8.0),
    );
    frame.stroke(&vertical, stroke);
    frame.stroke(&horizontal, stroke);
}

fn build(
    line: &Line,
    scene: &Scene<'_>,
    size: Size,
    semantic: bool,
    stagger_min: f64,
    stagger_max: f64,
    guides: bool,
) -> Rc<PartitaLayout> {
    let intensity = scene.theme.intensity;
    let units = layout_units(line, semantic);
    let graphemes = line.text.chars().filter(|c| !c.is_whitespace()).count().max(line.words.len()).max(1);
    let density = if graphemes > 40 { 0.8 } else { 1.0 };
    let scale = density * scene.font_scale;
    let font_px = (5.5 * scale * size.width / 100.0).clamp(2.5 * scale * 16.0, 4.5 * scale * 16.0);

    let target_rows = ((size.height * 0.65 / ROW_BASE_HEIGHT).floor() as usize).max(1);
    let row_count = units.len().min(target_rows);
    let mut seed = line.start;
    let mut next_random = || {
        let value = random(seed, 0.0);
        seed += 1.0;
        value
    };
    let chunks = split_rows(&units, row_count, &mut next_random);

    let margin_bottom = if intensity == AnimationIntensity::Chaotic { 6.4 } else { 9.6 };
    let mut rows: Vec<Row> = chunks
        .iter()
        .enumerate()
        .filter_map(|(index, chunk)| {
            let words = display_words(line, chunk);
            let first = chunk.first()?;
            let last_word = &line.words[chunk.last()?.words.end - 1];
            let guide_left = index % 2 == 0;
            let stagger = stagger_min + next_random() * (stagger_max - stagger_min);
            let chunk_scale = if intensity == AnimationIntensity::Calm { 1.0 } else { 0.8 + next_random() * 0.9 };
            let row_bias = index as f64 - (row_count as f64 - 1.0) / 2.0;
            let configs = word_configs(&words, intensity, chunk_scale);
            let widths: Vec<f32> =
                words.iter().map(|w| measure(&w.text, font_px, crate::BOLD, f32::INFINITY).width).collect();
            let width = widths.iter().map(|w| w + WORD_GAP).sum::<f32>() + PAD_X * 2.0;
            let mut cursor = -width / 2.0 + PAD_X;
            let offsets = widths
                .iter()
                .map(|w| {
                    let center = cursor + w / 2.0;
                    cursor += w + WORD_GAP;
                    center
                })
                .collect();
            Some(Row {
                words,
                configs,
                offsets,
                center: Point::ORIGIN,
                width,
                height: font_px + PAD_Y * 2.0,
                x: if guide_left { -stagger } else { stagger },
                y: if intensity == AnimationIntensity::Calm { 0.0 } else { row_bias * 2.5 },
                rotate: match intensity {
                    AnimationIntensity::Chaotic if guide_left => -3.0,
                    AnimationIntensity::Chaotic => 3.0,
                    _ => 0.0,
                },
                guide_left,
                start: first.start,
                last_word: (last_word.start, last_word.end),
                guides,
            })
        })
        .collect();

    let total: f32 = rows.iter().map(|row| row.height + margin_bottom).sum();
    let mut y = size.height / 2.0 - total / 2.0;
    for row in &mut rows {
        row.center = Point::new(size.width / 2.0, y + row.height / 2.0);
        y += row.height + margin_bottom;
    }
    Rc::new(PartitaLayout { rows, font_px })
}

/// Splits units into `count` rows of roughly even, seeded-random lengths, as the original does.
fn split_rows(units: &[LayoutUnit], count: usize, next_random: &mut impl FnMut() -> f64) -> Vec<Vec<LayoutUnit>> {
    let mut rows = Vec::with_capacity(count);
    let (mut remaining, mut index) = (units.len(), 0);
    for row in 0..count {
        let rows_left = count - row;
        let average = remaining as f64 / rows_left as f64;
        let mut length = if row == count - 1 {
            remaining
        } else {
            let max = (average * 1.5).ceil();
            (average + (next_random() - 0.5) * average).round().clamp(1.0, max.max(1.0)) as usize
        };
        length = length.clamp(1, remaining - (rows_left - 1));
        rows.push(units[index..index + length].to_vec());
        index += length;
        remaining -= length;
    }
    rows
}

fn word_configs(words: &[Word], intensity: AnimationIntensity, scale: f64) -> Vec<WordConfig> {
    let (spread, rotate) = match intensity {
        AnimationIntensity::Chaotic => (15.0, 8.0),
        AnimationIntensity::Calm => (0.0, 0.0),
        AnimationIntensity::Normal => (6.0, 3.0),
    };
    let base = words.first().map_or(0.0, |word| word.start);
    (0..words.len())
        .map(|i| {
            let seed = base + i as f64 * 7.13;
            WordConfig {
                x: (random(seed, 1.0) - 0.5) * spread * 2.0,
                y: (random(seed, 2.0) - 0.5) * spread * 2.0,
                rotate: (random(seed, 3.0) - 0.5) * rotate * 2.0,
                scale,
                passed_rotate: (random(seed, 8.0) - 0.5) * 20.0,
                margin_right: f64::from(WORD_GAP),
            }
        })
        .collect()
}

const PARTITA: &[crate::tuning::Item] = &[
    crate::tuning::Item::Switch {
        key: "showGuideLines",
        label_key: "options.partitaGuideLines",
        hint_key: None,
        default: true,
    },
    crate::tuning::Item::Switch {
        key: "useSemanticLayout",
        label_key: "options.partitaSemanticLayout",
        hint_key: None,
        default: true,
    },
    crate::tuning::Item::Slider {
        key: "staggerMin",
        label_key: "options.partitaStaggerMin",
        hint_key: None,
        min: 0.0,
        max: 180.0,
        step: 5.0,
        default: 20.0,
    },
    crate::tuning::Item::Slider {
        key: "staggerMax",
        label_key: "options.partitaStaggerMax",
        hint_key: None,
        min: 0.0,
        max: 180.0,
        step: 5.0,
        default: 100.0,
    },
];

const PARTITA_GROUPS: &[crate::tuning::Group] =
    &[crate::tuning::Group { id: "vis-partita", title_key: "options.partitaSettings", foot_key: None, items: PARTITA }];

pub fn manifest() -> crate::tuning::Manifest {
    crate::tuning::Manifest { storage_key: "partita_tuning", older_keys: &[], groups: PARTITA_GROUPS }
}

/// The smaller stagger is stored first, as the original `resolve` does.
pub fn normalize(values: &mut crate::tuning::Values) {
    let manifest = manifest();
    let (min, max) = (values.number(manifest, "staggerMin"), values.number(manifest, "staggerMax"));
    if min > max {
        values.set_number(manifest, "staggerMin", max);
        values.set_number(manifest, "staggerMax", min);
    }
}
