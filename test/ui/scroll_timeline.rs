// Smooth scroll timing, the timeline's line tracking, and one offscreen frame of the timeline in both themes.

use std::time::{Duration, Instant};

use folia_core::{Language, Line, Lyrics, Theme};
use folia_i18n::I18n;
use folia_ui::{Follow, Scroll, Timeline, USER_SCROLL_MS, WHEEL_MS, active_line, seek_time};
use iced::advanced::renderer::{Headless, Style as DrawStyle};
use iced::mouse::ScrollDelta;
use iced::{Color, Font, Pixels, Size, Theme as IcedTheme};
use iced_runtime::user_interface::{Cache, UserInterface};

fn lines() -> Vec<Line> {
    vec![
        Line { start: 0.0, end: 2.0, text: "first line".into(), ..Line::default() },
        Line {
            start: 2.0, end: 4.0, text: "second line".into(), translation: Some("译文".into()), ..Line::default()
        },
        Line { start: 6.0, end: 8.0, text: "third line".into(), ..Line::default() },
    ]
}

#[test]
fn a_wheel_notch_lands_within_150ms_and_never_goes_back() {
    let now = Instant::now();
    let mut scroll = Scroll::default();
    scroll.set_max(1000.0);
    scroll.wheel(ScrollDelta::Lines { x: 0.0, y: -1.0 }, now);
    let mut last = 0.0;
    for ms in 0..=WHEEL_MS as u64 {
        scroll.frame(now + Duration::from_millis(ms));
        let value = scroll.offset();
        assert!(value >= last, "offset went back at {ms}ms: {last} -> {value}");
        last = value;
        if ms < 149 {
            assert!(value < 60.0, "landed early at {ms}ms: {value}");
        }
    }
    assert!((scroll.offset() - 60.0).abs() < 1e-3);
    assert!(!scroll.animating(now + Duration::from_millis(200)));
}

#[test]
fn three_notches_stack_their_targets() {
    let now = Instant::now();
    let mut scroll = Scroll::default();
    scroll.set_max(1000.0);
    let mut last = 0.0;
    for (n, ms) in [0u64, 50, 100].into_iter().enumerate() {
        scroll.wheel(ScrollDelta::Lines { x: 0.0, y: -1.0 }, now + Duration::from_millis(ms));
        for step in 0..=10 {
            scroll.frame(now + Duration::from_millis(ms + step * 5));
            let value = scroll.offset();
            assert!(value >= last - 1e-3, "notch {n} went back: {last} -> {value}");
            last = value;
        }
    }
    scroll.frame(now + Duration::from_millis(600));
    assert!((scroll.offset() - 180.0).abs() < 1e-3);
}

#[test]
fn touchpad_pixels_follow_at_once() {
    let now = Instant::now();
    let mut scroll = Scroll::default();
    scroll.set_max(1000.0);
    scroll.wheel(ScrollDelta::Pixels { x: 0.0, y: -24.0 }, now);
    assert!((scroll.offset() - 24.0).abs() < 1e-6);
    assert!(!scroll.animating(now));
}

#[test]
fn a_scrollbar_drag_is_not_smoothed() {
    let now = Instant::now();
    let mut scroll = Scroll::default();
    let moved = scroll.scrolled_at(120.0, 1000.0, 200.0, now);
    assert!(moved);
    assert!((scroll.offset() - 120.0).abs() < 1e-6);
    assert!(!scroll.animating(now));
}

#[test]
fn a_program_scroll_takes_300ms() {
    let now = Instant::now();
    let mut scroll = Scroll::default();
    scroll.set_max(1000.0);
    scroll.scroll_to(200.0, now);
    scroll.frame(now + Duration::from_millis(299));
    assert!(scroll.offset() < 200.0);
    scroll.frame(now + Duration::from_millis(300));
    assert!((scroll.offset() - 200.0).abs() < 1e-3);
}

#[test]
fn the_active_line_needs_the_whole_span() {
    let lines = lines();
    assert_eq!(active_line(&lines, -1.0), None);
    assert_eq!(active_line(&lines, 0.0), Some(0));
    // Spans include both ends, so a line that ends where the next starts wins first, as `findIndex` does.
    assert_eq!(active_line(&lines, 2.0), Some(0));
    assert_eq!(active_line(&lines, 2.5), Some(1));
    assert_eq!(active_line(&lines, 5.0), None);
    assert_eq!(active_line(&lines, 6.0), Some(2));
    assert_eq!(active_line(&lines, 8.0), Some(2));
    assert_eq!(active_line(&lines, 9.0), None);
}

#[test]
fn auto_scroll_resumes_three_seconds_after_a_user_scroll() {
    let now = Instant::now();
    let mut follow = Follow::default();
    assert!(follow.auto_allowed(now));
    follow.user_scrolled(now);
    assert!(!follow.auto_allowed(now + Duration::from_millis(USER_SCROLL_MS - 1)));
    assert!(follow.auto_allowed(now + Duration::from_millis(USER_SCROLL_MS)));
    follow.user_scrolled(now + Duration::from_millis(1_000));
    assert!(!follow.auto_allowed(now + Duration::from_millis(3_500)));
    assert!(follow.auto_allowed(now + Duration::from_millis(4_000)));
}

#[test]
fn a_clicked_line_seeks_to_its_time_on_the_media_clock() {
    assert_eq!(seek_time(10.0, 0.0, 120.0), Some(10.0));
    assert_eq!(seek_time(10.0, 0.5, 120.0), Some(10.5));
    assert_eq!(seek_time(10.0, -0.5, 120.0), Some(9.5));
    assert_eq!(seek_time(0.0, -5.0, 120.0), Some(0.0));
    assert_eq!(seek_time(120.0, 0.0, 120.0), None);
    assert_eq!(seek_time(10.0, 0.0, 0.0), Some(10.0));
}

#[test]
#[cfg_attr(feature = "ci", ignore = "needs a GPU; CI runners only have a software renderer")]
fn the_timeline_draws_in_daylight_and_midnight() {
    folia_ui::install_fonts();
    let dir = std::env::temp_dir().join("folia-scroll");
    std::fs::create_dir_all(&dir).unwrap();
    let i18n = I18n::new(Language::En).unwrap();
    let lyrics = Lyrics { lines: lines(), ..Lyrics::default() };
    let now = Instant::now();
    let window = Size::new(1200.0, 800.0);
    for (name, theme, daylight) in
        [("timeline-day.png", Theme::DAYLIGHT, true), ("timeline-night.png", Theme::MIDNIGHT, false)]
    {
        let mut timeline = Timeline::default();
        timeline.open(Some(1), Some(&lyrics), 200.0, 2.5, window, now);
        let view = timeline.view(&i18n, &theme, daylight, window, Some(&lyrics), now + Duration::from_millis(400));
        let view = view.expect("the open timeline draws");
        let rgba = paint(view, daylight);
        assert_eq!(rgba.len(), 1200 * 800 * 4);
        assert!(rgba.iter().any(|byte| *byte != 0));
        std::fs::write(dir.join(name), encode(&rgba)).unwrap();
    }
}

fn paint(root: iced::Element<'_, folia_ui::TimelineMessage>, daylight: bool) -> Vec<u8> {
    let mut renderer =
        pollster::block_on(<iced::Renderer as Headless>::new(Font::DEFAULT, Pixels(16.0), None)).unwrap();
    let mut ui = UserInterface::build(root, Size::new(1200.0, 800.0), Cache::default(), &mut renderer);
    let iced_theme = if daylight { IcedTheme::Light } else { IcedTheme::Dark };
    let style = DrawStyle { text_color: Color::WHITE };
    ui.draw(&mut renderer, &iced_theme, &style, iced::mouse::Cursor::Unavailable);
    let background = if daylight { Color::from_rgb8(0xf5, 0xf5, 0xf4) } else { Color::from_rgb8(0x09, 0x09, 0x0b) };
    renderer.screenshot(&iced::advanced::graphics::Viewport::with_physical_size(Size::new(1200, 800), 1.0), background)
}

fn encode(rgba: &[u8]) -> Vec<u8> {
    let image = image::RgbaImage::from_raw(1200, 800, rgba.to_vec()).unwrap();
    let mut png = Vec::new();
    image.write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png).unwrap();
    png
}
