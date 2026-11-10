// Platform-column geometry, interruption and rest behaviour through the public animation primitive.

use folia_ui::ProviderHighlight;
use iced::Rectangle;
use std::time::{Duration, Instant};

fn at(start: Instant, ms: u64) -> Instant {
    start + Duration::from_millis(ms)
}
fn row(y: f32) -> Rectangle {
    Rectangle { x: 0.0, y, width: 196.0, height: 68.0 }
}

#[test]
fn first_hover_fades_at_the_row_instead_of_flying_from_the_origin() {
    let now = Instant::now();
    let mut highlight = ProviderHighlight::new(now);
    highlight.set(Some(row(272.0)), now);
    assert_eq!(highlight.bounds(now), row(272.0));
    assert!(highlight.opacity(now).abs() < 0.00001);
    assert!(highlight.opacity(at(now, 80)) > 0.0);
    assert_eq!(highlight.opacity(at(now, 220)), 1.0);
    assert!(!highlight.animating(at(now, 220)));
}

#[test]
fn leaving_without_a_selection_fades_in_place() {
    let now = Instant::now();
    let mut highlight = ProviderHighlight::new(now);
    highlight.set(Some(row(136.0)), now);
    highlight.set(None, at(now, 250));
    assert_eq!(highlight.bounds(at(now, 300)), row(136.0));
    let alpha = highlight.opacity(at(now, 300));
    assert!(alpha > 0.0 && alpha < 1.0);
    assert_eq!(highlight.opacity(at(now, 470)), 0.0);
    assert!(!highlight.animating(at(now, 470)));
}

#[test]
fn rapid_cross_row_retargets_continue_from_the_drawn_position() {
    let now = Instant::now();
    let mut highlight = ProviderHighlight::new(now);
    highlight.set(Some(row(0.0)), now);
    highlight.set(Some(row(340.0)), at(now, 300));
    let before = highlight.bounds(at(now, 350));
    highlight.set(Some(row(68.0)), at(now, 350));
    let after = highlight.bounds(at(now, 350));
    assert!((after.y - before.y).abs() < 0.01);
    assert_eq!(highlight.bounds(at(now, 570)), row(68.0));
    assert!(!highlight.animating(at(now, 570)));
}

#[test]
fn narrow_rows_slide_and_resize_the_same_highlight() {
    let now = Instant::now();
    let mut highlight = ProviderHighlight::new(now);
    let first = Rectangle { x: 0.0, y: 0.0, width: 80.0, height: 36.0 };
    let second = Rectangle { x: 84.0, y: 0.0, width: 144.0, height: 36.0 };
    highlight.set(Some(first), now);
    highlight.set(Some(second), at(now, 300));
    let moving = highlight.bounds(at(now, 350));
    assert!(moving.x > first.x && moving.x < second.x);
    assert!(moving.width > first.width && moving.width < second.width);
    assert_eq!(highlight.bounds(at(now, 520)), second);
}

#[test]
fn repeated_layout_does_not_restart_a_transition() {
    let now = Instant::now();
    let mut highlight = ProviderHighlight::new(now);
    highlight.set(Some(row(0.0)), now);
    highlight.set(Some(row(204.0)), at(now, 300));
    for ms in [320, 350, 400, 450, 500] {
        highlight.set(Some(row(204.0)), at(now, ms));
    }
    assert_eq!(highlight.bounds(at(now, 520)), row(204.0));
    assert!(!highlight.animating(at(now, 520)));
}
