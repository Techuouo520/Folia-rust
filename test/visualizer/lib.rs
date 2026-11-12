// Checks animation curves, render hints and the classic style's layout and word poses.

#![cfg(test)]

mod controls;
mod nebbia_gpu;
mod nebbia_legibility;
mod nebbia_png;

use folia_core::{AnimationIntensity, Line, Word};
use folia_lyrics::{RenderHints, Reveal, Transition, active_line};
use folia_visualizer::styles::cadenza::{
    WordMotion, font_px as cadenza_font, glow_envelope, layout as cadenza_layout, motion, timing_of,
};
use folia_visualizer::styles::claddagh::{RingMoment, adjust, glyph, line_marks, placed, ring_angle, visual_length};
use folia_visualizer::styles::classic::{ClassicTuning, Justify, font_px, justify, random, word_configs, wrap};
use folia_visualizer::styles::glow::{exit_at, word_pose};
use folia_visualizer::styles::tilt::{
    MAX_ROWS, TiltTuning, arrange, char_pulse, leaving_row, line_count, revealed_through, seeded, visible_row_count,
};
use folia_visualizer::{Ease, Style, keyframes, spring, tween};

#[test]
fn tuning_round_trip_and_member_lock() {
    use folia_visualizer::{Style, Values};
    let manifest = Style::Classic.manifest();
    let mut values = Values::default();
    values.set_number(manifest, "wordSpacing", 1.4);
    values.set_flag(manifest, "enableWordRotation", false);
    let loaded = Values::from_table(manifest, Some(&values.to_table(manifest)));
    assert!((loaded.number(manifest, "wordSpacing") - 1.4).abs() < 1e-4);
    assert!(!loaded.flag(manifest, "enableWordRotation"));

    let manifest = Style::Nebbia.manifest();
    let mut values = Values::default();
    assert!(values.toggle_member(manifest, "appear", "write", false));
    assert!(!values.member(manifest, "appear", "write"));
    assert!(!values.toggle_member(manifest, "appear", "fade", false));
    assert!(values.member(manifest, "appear", "fade"));
    assert_eq!(Style::Cadenza.manifest().older_keys, ["cadenze_tuning"]);
}

fn nebbia_strokes_path() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/nebbia/nebbia-strokes.bin.gz")
}

#[test]
fn nebbia_animcjk_medians_decode() {
    let map = folia_visualizer::styles::nebbia::load_file(&nebbia_strokes_path()).unwrap();
    assert!(map.len() > 10_000, "decoded {}", map.len());
    let kana = map.get(&'あ').expect("kana median");
    assert!(kana.iter().any(|stroke| stroke.len() > 2));
    let han = map.get(&'我').expect("han median");
    assert!(han.len() > 1);
    assert!(han.iter().flatten().all(|point| point.0 > -0.1 && point.0 < 1.2 && point.1 > -0.1 && point.1 < 1.2));
}

/// Per-character timed Chinese lines, like a NetEase YRC: the layout must find the words itself.
fn nebbia_song() -> folia_core::Lyrics {
    let lines = [
        (0.5, 4.0, "这难熬的寒冬谁陪你度过"),
        (4.5, 7.5, "想抓却抓不住"),
        (8.0, 11.0, "谁为你披上了外套"),
        (11.5, 15.5, "独自去面对这刺骨的寒风"),
    ];
    let lines = lines
        .iter()
        .map(|(start, end, text)| {
            let n = text.chars().count() as f64;
            let step = (end - start) / n;
            let words = text
                .chars()
                .enumerate()
                .map(|(i, ch)| Word {
                    text: ch.to_string(),
                    start: start + step * i as f64,
                    end: start + step * (i as f64 + 1.0),
                })
                .collect();
            Line { words, start: *start, end: *end, text: (*text).to_owned(), ..Line::default() }
        })
        .collect();
    folia_core::Lyrics { lines, title: Some("寒冬".to_owned()), word_timed: true, ..folia_core::Lyrics::default() }
}

#[test]
#[cfg_attr(feature = "ci", ignore = "needs a GPU; CI runners only have a software renderer")]
fn nebbia_vocal_energy_changes_the_frame() {
    if nebbia_gpu::with_strokes_in_child("nebbia_vocal_energy_changes_the_frame") {
        return;
    }
    let _guard = nebbia_gpu::GPU.lock().unwrap();
    let lyrics = nebbia_song();
    let theme = folia_core::Theme::MIDNIGHT;
    let (energy, _, _) = folia_visualizer::styles::nebbia::vocal_energy(0.0, 1.0, 200.0, 1.0 / 60.0);
    let quiet = nebbia_png::rgba(1200, 800, &lyrics, 6.0, &theme);
    let loud = nebbia_png::rgba_energy(1200, 800, &lyrics, 6.0, &theme, energy);
    let temporary = tempfile::tempdir().unwrap();
    let dir = temporary.path();
    let quiet_path = dir.join("nebbia-power0.png");
    let loud_path = dir.join("nebbia-power200.png");
    image::RgbaImage::from_raw(1200, 800, quiet.clone()).unwrap().save(&quiet_path).unwrap();
    image::RgbaImage::from_raw(1200, 800, loud.clone()).unwrap().save(&loud_path).unwrap();
    assert_ne!(quiet, loud, "vocal energy 200 did not change the nebbia frame");
}

fn close(a: f64, b: f64, eps: f64) -> bool {
    (a - b).abs() < eps
}

fn line(start: f64, end: f64, words: &[(&str, f64, f64)]) -> Line {
    Line {
        words: words.iter().map(|&(text, start, end)| Word { text: text.to_owned(), start, end }).collect(),
        start,
        end,
        text: words.iter().map(|w| w.0).collect(),
        ..Line::default()
    }
}

#[test]
fn curves() {
    assert!(close(tween(0.0, 10.0, 0.0, 1.0, Ease::LINEAR), 0.0, 1e-9));
    assert!(close(tween(0.0, 10.0, 0.5, 1.0, Ease::LINEAR), 5.0, 1e-9));
    assert!(close(tween(0.0, 10.0, 2.0, 1.0, Ease::OUT), 10.0, 1e-9));
    assert!(Ease::OUT.at(0.5) > 0.5 && Ease::IN_OUT.at(0.5) > 0.49 && Ease::IN_OUT.at(0.5) < 0.51);
    assert!(close(spring(0.5, 1.4, 0.0, 200.0, 20.0), 0.5, 1e-9));
    assert!(close(spring(0.5, 1.4, 2.0, 200.0, 20.0), 1.4, 1e-6));
    assert!(spring(0.0, 1.0, 0.25, 200.0, 20.0) > 1.0, "this spring overshoots");
    assert!(close(keyframes(&[0.0, 1.0, 0.0], &[0.0, 0.3, 1.0], 0.3, 1.0, Ease::LINEAR), 1.0, 1e-9));
}

#[test]
fn random_matches_the_original() {
    assert!(close(random(12.5, 1.0), 0.8442655162098163, 1e-6));
    assert!(close(random(12.5, 4.0), 0.1465763087699088, 1e-6));
    assert_eq!(justify(12.5, AnimationIntensity::Normal, false), Justify::End);
    assert_eq!(justify(4.0, AnimationIntensity::Normal, false), Justify::Between);
    assert_eq!(justify(4.0, AnimationIntensity::Calm, false), Justify::Center);
}

#[test]
fn font_size_follows_window_width() {
    assert!(close(f64::from(font_px(400.0, 1.0)), 36.0, 1e-4));
    assert!(close(f64::from(font_px(1000.0, 1.0)), 60.0, 1e-4));
    assert!(close(f64::from(font_px(4000.0, 1.0)), 72.0, 1e-4));
}

#[test]
fn layout_is_deterministic_and_wraps() {
    let widths = [120.0, 80.0, 200.0, 60.0];
    let tuning = ClassicTuning::default();
    let a = word_configs(7.25, AnimationIntensity::Normal, false, &widths, 60.0, tuning);
    let b = word_configs(7.25, AnimationIntensity::Normal, false, &widths, 60.0, tuning);
    assert_eq!(a, b);
    assert!(a.iter().all(|c| c.margin_right >= 0.12 * 60.0 * 0.7 - 1e-9));
    assert!(a.iter().all(|c| c.rotate.abs() <= 5.0 && c.x.abs() <= 20.0));

    let margins = [10.0; 4];
    let placed = wrap(&widths, &margins, 70.0, 300.0, 0.0, 200.0, Justify::Start);
    assert_eq!(placed.len(), 4);
    assert!(placed[0].y < placed[2].y, "third word wraps to a new row");
    assert!(close(f64::from(placed[0].x), 60.0, 1e-4));
}

#[test]
fn render_hints() {
    let micro = line(1.0, 1.05, &[("a", 1.0, 1.05)]);
    let hints = RenderHints::of(&micro);
    assert_eq!((hints.transition, hints.reveal), (Transition::None, Reveal::Instant));
    let normal = line(1.0, 3.0, &[("a", 1.0, 2.0), ("b", 2.0, 3.0)]);
    let hints = RenderHints::of(&normal);
    assert_eq!(hints.transition, Transition::Normal);
    assert!(hints.render_end >= 3.0);
    assert!(close(hints.word_lookahead(), 0.15, 1e-9));

    let lines = [normal.clone(), line(5.0, 6.0, &[("c", 5.0, 6.0)])];
    assert_eq!(active_line(&lines, 0.5), None);
    assert_eq!(active_line(&lines, 2.0), Some(0));
    assert_eq!(active_line(&lines, 4.5), None);
    assert_eq!(active_line(&lines, 5.5), Some(1));
}

#[test]
fn word_pose_states() {
    let tuning = ClassicTuning::default();
    let config = word_configs(3.0, AnimationIntensity::Normal, false, &[100.0], 60.0, tuning)[0];
    let hints = RenderHints::of(&line(3.0, 5.0, &[("w", 3.0, 4.0)]));
    let pose = |time| word_pose(&config, 3.0, 4.0, hints, AnimationIntensity::Normal, tuning.word_rotation, time);

    assert_eq!(pose(2.0).opacity, 0.0);
    let active = pose(3.8);
    assert!(close(active.opacity, 1.0, 1e-9));
    assert!(close(active.scale, config.scale * 1.4, 0.05));
    assert!(active.color > 0.7);

    let passed = pose(12.0);
    assert!(close(passed.opacity, 0.82, 1e-9));
    assert!(close(passed.scale, config.scale, 1e-9));
    assert!(close(passed.rotate, config.rotate + config.passed_rotate, 1e-9));
    assert!(close(passed.color, 0.0, 1e-9));
}

#[test]
fn style_registry() {
    assert_eq!(Style::from_id("classic"), Some(Style::Classic));
    assert_eq!(Style::from_id("nope"), None);
    assert_eq!(Style::Classic.label_key(), "ui.visualizerClassic");
    assert_eq!(Style::from_id("still"), Some(Style::Still));
    assert_eq!(Style::from_id("partita"), Some(Style::Partita));
    assert_eq!(Style::Partita.label_key(), "ui.visualizerPartita");
    assert_eq!(Style::Still.max_fps(), Some(10));
    assert_eq!(Style::Classic.max_fps(), None);
    assert_eq!(Style::from_id("tilt"), Some(Style::Tilt));
    assert_eq!(Style::Tilt.label_key(), "ui.visualizerTilt");
    assert_eq!(Style::Tilt.max_fps(), None);
    assert_eq!(Style::from_id("claddagh"), Some(Style::Claddagh));
    assert_eq!(Style::Claddagh.label_key(), "ui.visualizerCladdagh");
    assert_eq!(Style::from_id("cadenza"), Some(Style::Cadenza));
    assert_eq!(Style::Cadenza.label_key(), "ui.visualizerCadenze");
    assert_eq!(Style::Cadenza.max_fps(), None);
    assert!(Style::ALL.iter().all(|style| Style::from_id(style.id()) == Some(*style)));
}

#[test]
fn random_style_is_stable_per_song_and_spread() {
    assert_eq!(Style::random_for(42), Style::random_for(42));
    let drawn: std::collections::HashSet<_> = (1..1000).map(|id| Style::random_for(id).id()).collect();
    assert_eq!(drawn.len(), Style::ALL.len(), "every style gets drawn: {drawn:?}");
    assert!(Style::ALL.contains(&Style::random_for(-7_000_000_000)));
}

#[test]
fn tilt_layout_matches_the_original_rolls() {
    assert!(close(seeded(12.5, 1.0), 0.2296561770626795, 1e-9));
    assert!(close(seeded(3.2, 100.0), 0.006198119477630826, 1e-9));
    assert_eq!(line_count(4, 1.5, 0.75), 1);
    assert_eq!(line_count(18, 2.0, 0.75), 2);
    assert_eq!(line_count(40, 8.0, 0.75), 2);
    assert_eq!(line_count(80, 1.0, 0.75), 3);

    assert_eq!(char_pulse(1.0, 1.2, 1.5), 0.0);
    assert!(close(char_pulse(1.3, 1.2, 1.5), 0.866025403784439, 1e-12));
    assert!(close(char_pulse(2.0, 1.2, 1.5), 0.25, 1e-12));

    let wide =
        arrange("你好，世界。", 2.0, TiltTuning::default(), 100.0, |text, _| text.chars().count() as f32 * 40.0);
    assert!(wide.scale < 1.0, "a row wider than the viewport scales down");
    let fitted = arrange(
        "你好，世界。",
        2.0,
        TiltTuning { split_probability: 0.0, tilt_style_probability: 0.0 },
        400.0,
        |text, _| text.chars().count() as f32 * 10.0,
    );
    assert_eq!(fitted.segments.len(), 1);
    assert!(!fitted.segments[0].tilt);
    assert!(close(f64::from(fitted.scale), 1.0, 1e-6));

    assert_eq!(revealed_through(1.0, &[1.0, 2.0], 1.0, 4.0), Some(0));
    assert_eq!(revealed_through(1.8, &[1.0, 2.0], 1.0, 4.0), Some(1));
    assert_eq!(revealed_through(0.5, &[1.0, 2.0], 1.0, 4.0), None);
    assert_eq!(revealed_through(4.1, &[1.0, 2.0], 1.0, 4.0), None);
}

#[test]
fn leaving_line_keeps_only_the_rows_shown_at_exit() {
    let starts = [1.0, 3.0, 5.0];
    assert_eq!(visible_row_count(&starts, 2.0, 1.0, 8.0), 1, "the later rows have not been revealed yet");
    assert_eq!(visible_row_count(&starts, 4.0, 1.0, 8.0), 2);
    assert_eq!(visible_row_count(&starts, 6.0, 1.0, 8.0), 3);
    assert_eq!(visible_row_count(&starts, 8.1, 1.0, 8.0), 0);

    let lines = [line(1.0, 8.0, &[("a", 1.0, 8.0)]), line(2.0, 9.0, &[("b", 2.0, 9.0)])];
    assert!(close(exit_at(&lines, &lines[0]), 2.0, 1e-9));
    assert_eq!(visible_row_count(&starts, exit_at(&lines, &lines[0]), 1.0, 8.0), 1);
}

#[test]
fn tilt_never_splits_past_the_row_cap() {
    assert_eq!(line_count(10_000, 1.0, 1.0), MAX_ROWS);
    let tuning = TiltTuning { split_probability: 1.0, tilt_style_probability: 1.0 };
    let text = "这是一段没有标点的很长歌词用来把一行拆开再拆直到不能再拆为止然后应该停在上限";
    let layout = arrange(text, 0.5, tuning, 30.0, |sample, _| sample.chars().count() as f32 * 40.0);
    assert!(layout.segments.len() <= MAX_ROWS, "{}", layout.segments.len());
}

#[test]
fn mixed_line_can_italicize_the_chinese_row() {
    let text = "风吹过旧街口 waiting for you";
    let tuning = TiltTuning::default();
    let seed = (0..40).map(|i| i as f64 / 10.0).find(|&seed| {
        arrange(text, seed, tuning, 2000.0, |sample, _| sample.chars().count() as f32 * 8.0)
            .segments
            .iter()
            .any(|segment| segment.tilt && segment.text.chars().any(|c| ('\u{4e00}'..='\u{9fff}').contains(&c)))
    });
    let seed = seed.expect("a timestamp in the first four seconds italicizes the Chinese row");
    let rows: Vec<_> = arrange(text, seed, tuning, 2000.0, |sample, _| sample.chars().count() as f32 * 8.0)
        .segments
        .iter()
        .map(|segment| (segment.text.clone(), segment.tilt))
        .collect();
    assert!(
        rows.iter().any(|(text, tilt)| *tilt && text.chars().any(|c| ('\u{4e00}'..='\u{9fff}').contains(&c))),
        "{seed} {rows:?}"
    );
}

#[test]
fn tilt_row_exit_matches_the_original_offsets() {
    let (opacity, y, scale) = leaving_row(0.0, false);
    assert!(close(opacity, 1.0, 1e-5) && close(y, 0.0, 1e-5) && close(scale, 1.0, 1e-5));
    let (_, y, scale) = leaving_row(1.0, false);
    assert!(close(y, -12.0, 1e-9) && close(scale, 1.0, 1e-9));
    let (_, y, scale) = leaving_row(1.0, true);
    assert!(close(y, -16.0, 1e-9) && close(scale, 0.95, 1e-9));
}

#[test]
fn claddagh_gap_borrows_time_and_the_front_glyph_is_the_large_one() {
    assert!(close(visual_length("回环"), 2.0, 1e-9));
    assert!(close(visual_length("ab"), 1.0, 1e-9));
    let lyric = line(0.0, 2.0, &[("a", 0.0, 1.0), (" ", 1.0, 1.0), ("b", 1.0, 2.0)]);
    let mut marks = line_marks(&lyric);
    adjust(&mut marks, lyric.start, lyric.end);
    let space = marks.iter().find(|mark| mark.text == " ").expect("space");
    assert!(space.end - space.start >= 0.06 - 1e-9);

    let one = Line { text: "回".to_owned(), start: 0.0, end: 1.0, ..Line::default() };
    let items = placed(&one, 72.0, 400.0, 1.0, 0.0);
    let pose = glyph(&items[0], 0, &front_moment());
    assert!(pose.opacity > 0.8);
    assert!(pose.scale > 1.0);

    let lines = vec![line(0.0, 1.0, &[("a", 0.0, 1.0)]), line(1.0, 2.0, &[("b", 1.0, 2.0)])];
    assert!(close(ring_angle(&lines, 0.4), 0.0, 1e-6));
    assert!(close(ring_angle(&lines, 1.0), 0.0, 1e-6));
    assert!(close(ring_angle(&lines, 8.0), std::f64::consts::PI, 1e-3));
}

#[test]
fn claddagh_centers_single_glyph_and_keeps_the_last_gap() {
    let one = line(0.0, 2.0, &[("回", 0.0, 2.0)]);
    assert!(placed(&one, 72.0, 400.0, 1.0, 0.0)[0].nominal.abs() < 1e-6);
    let three = line(0.0, 3.0, &[("回", 0.0, 1.0), ("回", 1.0, 2.0), ("回", 2.0, 3.0)]);
    let p = placed(&three, 72.0, 400.0, 1.0, 0.0);
    let first = p[1].nominal - p[0].nominal;
    let last = p[2].nominal - p[1].nominal;
    assert!(last > first * 0.85, "last glyphs are squeezed together: {first}, {last}");
}

#[test]
fn claddagh_far_lines_do_not_overlay_the_front_line() {
    let one = line(0.0, 2.0, &[("回", 0.0, 2.0)]);
    let p = placed(&one, 72.0, 400.0, 1.0, 0.0);
    let moment = RingMoment { line_index: 2, ..front_moment() };
    assert_eq!(glyph(&p[0], 0, &moment).opacity, 0.0);
}

#[test]
fn claddagh_short_lines_keep_turning_and_a_skipped_index_snaps() {
    let lines: Vec<_> = (0..8)
        .map(|i| {
            let start = i as f64 * 0.2;
            line(start, start + 0.2, &[("a", start, start + 0.2)])
        })
        .collect();
    let boundary = ring_angle(&lines, 0.4);
    assert!(boundary < std::f64::consts::PI - 0.5, "reset to the previous target: {boundary}");
    assert!(close(boundary, ring_angle(&lines, 0.4 - 1e-4), 1e-3));
    let mut previous = ring_angle(&lines, 0.0);
    let mut time = 0.0;
    while time < 1.4 - 1e-9 {
        time += 0.02;
        let angle = ring_angle(&lines, time);
        assert!(angle + 1e-6 >= previous, "rewound at {time}: {previous} -> {angle}");
        previous = angle;
    }
    assert!(previous > std::f64::consts::PI, "never left the first turn: {previous}");

    let jumped = vec![
        line(0.0, 1.0, &[("a", 0.0, 1.0)]),
        line(2.0, 3.0, &[("b", 2.0, 3.0)]),
        line(2.0, 3.0, &[("c", 2.0, 3.0)]),
        line(2.0, 3.0, &[("d", 2.0, 3.0)]),
    ];
    assert!(close(ring_angle(&jumped, 2.0), 3.0 * std::f64::consts::PI, 1e-9));
    assert!(close(ring_angle(&jumped, 2.4), 3.0 * std::f64::consts::PI, 1e-9));
}

#[test]
fn cadenza_keeps_one_word_large_and_the_glow_tracks_the_sung_word() {
    let px = f64::from(cadenza_font(800.0, 6, 3, 1.0));
    assert!(close(px, f64::from((800.0_f32 * 0.086).clamp(34.0, 94.0) * 1.12), 1e-3));
    let lyric = line(0.0, 4.0, &[("风", 0.0, 1.0), ("吹过", 1.0, 2.0), ("旧街", 2.0, 4.0)]);
    let placed = cadenza_layout(&lyric, 800.0, 600.0, 1.0, AnimationIntensity::Normal, 0.72);
    assert_eq!(placed.len(), 3);
    let hero = placed.iter().max_by(|left, right| left.emphasis.total_cmp(&right.emphasis)).expect("hero");
    assert_eq!(hero.text, "吹过");
    assert!(
        hero.scale
            > placed
                .iter()
                .filter(|item| item.word_index != hero.word_index)
                .map(|item| item.scale)
                .fold(0.0, f64::max)
    );
    assert!(hero.x.abs() < hero.width && hero.y.abs() < hero.height, "hero {}, {}", hero.x, hero.y);
    let spread = placed
        .iter()
        .filter(|item| item.word_index != hero.word_index)
        .any(|item| (item.x - hero.x).hypot(item.y - hero.y) > 8.0);
    assert!(
        spread,
        "{:?}",
        placed.iter().map(|item| (item.text.clone(), item.x, item.y, item.scale)).collect::<Vec<_>>()
    );

    let timing = timing_of(&lyric);
    assert!(close(glow_envelope(-0.1, &timing, 1.0, 2.0), 0.0, 1e-9));
    assert!(close(glow_envelope(1.18, &timing, 1.0, 2.0), 1.0, 1e-6));
    assert!(glow_envelope(3.5, &timing, 1.0, 2.0) < 0.05);
    let posed = motion(
        &WordMotion {
            scale: hero.scale,
            passed_rotate: 0.0,
            drift_x: 0.0,
            drift_y: 0.0,
            start: 1.0,
            end: 2.0,
            line_start: 0.0,
            motion_amount: 1.0,
        },
        0.82,
        &timing,
        AnimationIntensity::Normal,
    );
    assert!(posed.scale < hero.scale, "scale jumped to the active size: {}", posed.scale);
}

fn front_moment() -> RingMoment {
    RingMoment {
        line_index: 0,
        center: 0,
        ring: 0.0,
        word_offset: 0.0,
        rx: 400.0,
        focus_scale: 0.65,
        tilt_deg: 45.0,
        power: 0.0,
        chorus: false,
        length_fade: 1.0,
        length_scale: 1.0,
        upcoming: 0.0,
        font_px: 72.0,
    }
}
