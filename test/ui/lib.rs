// Chrome deadline, capsule hover and subtitle rows.

#![cfg(test)]

mod chrome_motion;
mod developer;
mod font_surfaces;
mod fonts;
mod glass_tone;
mod icon_dots;
mod loading;
#[cfg(test)]
mod monet;
mod panelfix;
mod player_bar;
mod polish;
mod reorder;
mod scroll_timeline;
mod seam_title;
#[cfg(test)]
mod text_atlas;
#[cfg(test)]
mod text_raster;
mod workbench;

use folia_core::Line;
use folia_ui::{
    CAPSULE_COLLAPSE_MS, CAPSULE_EXPAND_MS, CAPSULE_LAYOUT_SPRING, CAPSULE_SETTLE_MS, CHROME_HOLD_MS, CHROME_LEAVE_MS,
    CapsuleHover, ChromeClock, ChromeSpring, GLOW_BLUR_PX, capsule_open, capsule_row, capsule_width, glow_coverage,
    glow_ellipse, row_gap, subtitle_at, subtitle_line_width, translation_px, upcoming_look, upcoming_px,
};

fn line(start: f64, text: &str, translation: Option<&str>) -> Line {
    Line {
        start,
        end: start + 4.0,
        text: text.to_owned(),
        translation: translation.map(str::to_owned),
        ..Line::default()
    }
}

#[test]
fn a_still_pointer_does_not_keep_the_chrome_up() {
    let mut clock = ChromeClock::shown(0);
    clock = clock.pointer(0, 10.0, 20.0);
    assert!(clock.visible(2_999, false));
    clock = clock.pointer(2_000, 10.0, 20.0);
    assert_eq!(clock.until_ms, Some(CHROME_HOLD_MS));
    clock = clock.tick(CHROME_HOLD_MS, false);
    assert!(!clock.visible(CHROME_HOLD_MS, false));
    clock = clock.pointer(CHROME_HOLD_MS + 10, 10.0, 20.0);
    assert!(clock.until_ms.is_none());
}

#[test]
fn movement_extends_the_deadline_and_leaving_uses_the_short_delay() {
    let clock = ChromeClock::shown(0).pointer(100, 4.0, 5.0).pointer(200, 8.0, 5.0);
    assert_eq!(clock.until_ms, Some(200 + CHROME_HOLD_MS));
    let left = clock.leave(1_000, false);
    assert!(left.pointer.is_none());
    assert_eq!(left.until_ms, Some(1_000 + CHROME_LEAVE_MS));
    assert!(left.visible(1_000 + CHROME_LEAVE_MS - 1, false));
    assert!(!left.tick(1_000 + CHROME_LEAVE_MS, false).visible(1_000 + CHROME_LEAVE_MS, false));
}

#[test]
fn a_held_pointer_stays_visible_after_the_deadline() {
    let clock = ChromeClock::shown(0).tick(CHROME_HOLD_MS, true);
    assert!(clock.visible(CHROME_HOLD_MS + 5_000, true));
    let left = clock.leave(9_000, true);
    assert!(left.visible(9_500, true));
}

#[test]
fn the_capsule_waits_before_expanding_and_collapsing() {
    let hover = CapsuleHover::collapsed(0).set_inside(0, true);
    assert!(!hover.step(CAPSULE_EXPAND_MS - 1).expanded);
    let open = hover.step(CAPSULE_EXPAND_MS);
    assert!(open.expanded);
    let leaving = open.set_inside(100, false);
    assert!(leaving.step(100 + CAPSULE_COLLAPSE_MS - 1).expanded);
    assert!(!leaving.step(100 + CAPSULE_COLLAPSE_MS).expanded);
}

#[test]
fn capsule_width_grows_from_the_collapsed_fraction() {
    // `max-w-[32rem] px-4` on the playing page: the row is 512 less 32 of padding.
    let row = capsule_row(1200.0, false);
    assert!((row - 480.0).abs() < 0.1);
    assert!((capsule_width(row, 1200.0, 0.0) - 480.0 * 0.6).abs() < 0.1);
    assert!((capsule_width(row, 1200.0, 1.0) - 480.0).abs() < 0.1);
    assert!((capsule_width(capsule_row(500.0, false), 500.0, 0.0) - 468.0 * 0.8).abs() < 0.1);
    // Home: `min(32rem, 100vw - 120px)` with no padding.
    assert!((capsule_row(1200.0, true) - 512.0).abs() < 0.1);
    assert!((capsule_row(600.0, true) - 480.0).abs() < 0.1);
}

#[test]
fn subtitles_keep_the_translation_and_the_next_readable_line() {
    let lines = vec![
        line(0.0, "风吹过旧街口", Some("风穿过旧街")),
        line(8.0, "......", None),
        line(12.0, "still here", Some("还在这里")),
    ];
    let at = subtitle_at(&lines, 1.0);
    assert_eq!(at.translation, Some("风穿过旧街"));
    assert_eq!(at.upcoming, Some("still here"));
    let gap = subtitle_at(&lines, 9.0);
    assert_eq!(gap.translation, None);
    assert_eq!(gap.upcoming, Some("still here"));
    assert!((translation_px(1000.0, 1.0) - 20.0).abs() < 0.01);
    assert!((upcoming_px(1000.0, 1.0) - 16.0).abs() < 0.01);
    assert!((upcoming_px(400.0, 1.0) - 14.0).abs() < 0.01);
    let (under_px, under_alpha) = upcoming_look(1000.0, 1.0, true);
    assert!((under_px - 20.0 * 0.88).abs() < 0.01);
    assert!((under_alpha - 0.6 * 0.82).abs() < 0.001);
    let (alone_px, alone_alpha) = upcoming_look(1000.0, 1.0, false);
    assert!((alone_px - 16.0).abs() < 0.01);
    assert!((alone_alpha - 0.6).abs() < 0.001);
    assert!((row_gap(20.0) - 5.0).abs() < 0.01);
}

#[test]
fn the_capsule_stays_open_while_a_track_is_paused() {
    assert!(!capsule_open(false, true, true));
    assert!(capsule_open(true, true, true));
    assert!(capsule_open(false, false, true));
    assert!(!capsule_open(false, false, false));
}

#[test]
fn the_capsule_size_springs_and_settles() {
    let spring = ChromeSpring::settled(0.0, 0, CAPSULE_LAYOUT_SPRING).retarget(10, 1.0);
    assert!(spring.value(10).abs() < 0.001);
    assert!(spring.running(10 + 100));
    // Damping ratio 0.72 overshoots before it settles.
    let peak = (0..60).map(|step| spring.value(10 + step * 10)).fold(0.0_f32, f32::max);
    assert!(peak > 1.0 && peak < 1.1);
    assert!((spring.value(10 + CAPSULE_SETTLE_MS) - 1.0).abs() < 0.001);
    assert!(!spring.running(10 + CAPSULE_SETTLE_MS));
    let back = spring.retarget(10 + 100, 0.0);
    assert!(back.value(10 + 100) > 0.3);
    assert_eq!(back.retarget(10 + 200, 0.0), back);
}

#[test]
fn the_subtitle_glow_uses_the_original_ellipse() {
    let glow = glow_ellipse(1200.0, 800.0, 112.0, 20.0, 17.6, Some("风穿过旧街"), Some("still here"));
    assert!((glow.center[0] - 0.5).abs() < 0.001);
    let wider = subtitle_line_width("风穿过旧街", 20.0).max(subtitle_line_width("still here", 17.6));
    assert!((glow.radius[0] * 1200.0 - ellipse_rx(wider)).abs() < 0.05);
    assert!((glow.radius[1] * 800.0 - 61.49).abs() < 0.2);
    assert!((glow.center[1] * 800.0 - 664.7).abs() < 0.2);
    assert!((glow_coverage(0.0, 0.65) - 0.96).abs() < 0.001);
    assert!((glow_coverage(0.62, 0.65) - 0.78).abs() < 0.001);
    assert!(glow_coverage(1.0, GLOW_BLUR_PX / 61.49) > 0.2);
    assert!(glow_coverage(3.0, 0.65) < 0.001);
}

#[test]
fn the_ellipse_widens_when_the_measured_line_does() {
    let narrow = glow_ellipse(1200.0, 800.0, 112.0, 20.0, 16.0, Some("Hi"), None);
    let wide = glow_ellipse(1200.0, 800.0, 112.0, 20.0, 16.0, Some("Hi there, still waiting"), None);
    let delta =
        (subtitle_line_width("Hi there, still waiting", 20.0) - subtitle_line_width("Hi", 20.0)) * 1.15 * 0.5 / 1200.0;
    assert!(delta > 0.02);
    assert!((wide.radius[0] - narrow.radius[0] - delta).abs() < 0.0001);
    assert!((wide.radius[1] - narrow.radius[1]).abs() < 0.0001);
}

fn ellipse_rx(text_width: f32) -> f32 {
    (text_width.min(896.0) + (6.0 + 40.0) * 2.0) * 1.15 * 0.5
}

#[test]
fn the_seam_follows_the_wall_and_narrows_at_the_edge() {
    use folia_ui::{
        SEAM_ANCHOR, content_opacity, default_level_is_spine, plan_opening, seam_geometry, seam_safe_bottom, wider_span,
    };
    // At 1200px the wall is drawn at 0.76; a camera on the anchor puts the seam on the centre line.
    let middle = seam_geometry(SEAM_ANCHOR, 120.0, 1200.0);
    assert!((middle.center - 600.0).abs() < 0.01);
    assert_eq!(middle.width, 120.0);
    assert!(!middle.collapsed);
    // Panning the wall 100 world px to the right moves the seam 76px with it.
    let moved = seam_geometry(SEAM_ANCHOR - 100.0, 120.0, 1200.0);
    assert!((moved.center - 676.0).abs() < 0.01);
    // Near the right edge only what fits between the seam's centre and the edge, less 14px, stays open.
    let edge = seam_geometry(SEAM_ANCHOR - 740.0, 120.0, 1200.0);
    assert!((edge.width - (2.0 * (1200.0 - edge.center - 14.0))).abs() < 0.01);
    // The home wall is infinite: panning the seam off screen shuts it and raises the edge tab.
    let gone = seam_geometry(SEAM_ANCHOR - 2000.0, 120.0, 1200.0);
    assert_eq!(gone.width, 0.0);
    assert!(gone.collapsed);
    // A camera at the origin opens on the gap between block columns -1 and 0, and does not yield.
    let (anchor, camera) = plan_opening(0.0, 120.0, 1200.0, 600.0, 0.76, 12.0 * 136.0);
    assert!((anchor - SEAM_ANCHOR).abs() < 0.01);
    assert!(camera.abs() < 0.01);
    assert!((content_opacity(120.0, 120.0) - 1.0).abs() < 0.001);
    assert!(content_opacity(42.0, 120.0) < 0.01);
    assert!(default_level_is_spine(800.0));
    assert!(!default_level_is_spine(1200.0));
    // Default bar offset 32, song present: 32 + 80 + 8.
    assert!((seam_safe_bottom(32.0, 1.0) - 120.0).abs() < 0.01);
    assert!((seam_safe_bottom(32.0, 0.0) - 20.0).abs() < 0.01);
    // The hint takes the wider side. A seam just left of centre leaves the right side wider.
    let (left, width) = wider_span(590.0, 120.0, 1200.0);
    assert!((left - 650.0).abs() < 0.01);
    assert!((width - 550.0).abs() < 0.01);
}

#[test]
fn a_wave_lifts_the_far_corner_first_and_lands_the_near_corner_first() {
    use folia_ui::{ShiftKind, ShiftPose, WallShift};
    use iced::{Rectangle, Size};
    use std::time::{Duration, Instant};
    let now = Instant::now();
    let near = Rectangle::new(iced::Point::new(0.0, 0.0), Size::new(97.0, 97.0));
    let far = Rectangle::new(iced::Point::new(1000.0, 700.0), Size::new(97.0, 97.0));
    let wave = WallShift::new(ShiftKind::Wave, 0.76, now, [near, far].into_iter());
    let at = |ms: u64| now + Duration::from_millis(ms);
    // The far corner (stagger capped at 340ms) starts leaving at once; the near corner waits 340ms.
    assert!(wave.leaving(far, at(100)).is_some_and(|pose| pose.alpha < 1.0));
    assert_eq!(wave.leaving(near, at(100)), Some(ShiftPose::REST));
    assert!(wave.leaving(far, at(300)).is_none());
    // Landing starts at 620ms for the near corner and 960ms for the far one, each over 420ms.
    assert!(wave.arriving(near, at(700)).is_some());
    assert!(wave.arriving(far, at(700)).is_none());
    assert_eq!(wave.arriving(far, at(1400)), Some(ShiftPose::REST));
    assert!(wave.running(at(1300)));
    assert!(!wave.running(at(1400)));
}

#[test]
fn a_flip_turns_tiles_out_from_the_origin_and_back_in() {
    use folia_ui::{ShiftKind, ShiftPose, WallShift};
    use iced::{Point, Rectangle, Size};
    use std::time::{Duration, Instant};
    let now = Instant::now();
    let at = |ms: u64| now + Duration::from_millis(ms);
    let pressed = Rectangle::new(Point::new(100.0, 100.0), Size::new(97.0, 97.0));
    let flip = WallShift::new(ShiftKind::Flip(pressed.center()), 0.76, now, [pressed].into_iter());
    // The pressed tile turns at once: halfway through the 150ms out it is squeezed, then gone until the 210ms in.
    assert!(flip.leaving(pressed, at(75)).is_some_and(|pose| pose.squeeze < 1.0 && pose.squeeze > 0.0));
    assert!(flip.leaving(pressed, at(160)).is_none());
    assert!(flip.arriving(pressed, at(140)).is_none());
    assert!(flip.arriving(pressed, at(250)).is_some_and(|pose| pose.squeeze > 0.0 && pose.squeeze < 1.0));
    assert_eq!(flip.arriving(pressed, at(400)), Some(ShiftPose::REST));
}

#[test]
fn release_inertia_runs_three_tenths_of_the_speed_and_skips_slow_or_stale_releases() {
    use folia_ui::{WallInertia, WallRelease};
    use iced::Vector;
    use std::time::{Duration, Instant};
    let now = Instant::now();
    let fling = WallRelease { velocity: Vector::new(-1000.0, 0.0), stale: false };
    let inertia = WallInertia::start(fling, Vector::ZERO, 0.76, now).unwrap();
    // A pointer moving left 1000px/s carries the camera right by 300 screen px, 300/0.76 world units.
    let (end, moving) = inertia.at(now + Duration::from_secs(5));
    assert!(!moving);
    assert!((end.x - 300.0 / 0.76).abs() < 0.01);
    let (early, _) = inertia.at(now + Duration::from_millis(280));
    assert!((early.x - 300.0 / 0.76 * (1.0 - (-1.0f32).exp())).abs() < 0.5);
    // Speeds are capped at 4000px/s.
    let fast = WallRelease { velocity: Vector::new(0.0, 9000.0), stale: false };
    let (capped, _) = WallInertia::start(fast, Vector::ZERO, 1.0, now).unwrap().at(now + Duration::from_secs(10));
    assert!((capped.y + 1200.0).abs() < 0.01);
    assert!(
        WallInertia::start(WallRelease { velocity: Vector::new(30.0, 0.0), stale: false }, Vector::ZERO, 1.0, now)
            .is_none()
    );
    assert!(WallInertia::start(WallRelease { stale: true, ..fling }, Vector::ZERO, 1.0, now).is_none());
}

#[test]
fn a_wheel_glide_closes_in_with_an_80ms_time_constant() {
    use folia_ui::WallGlide;
    use iced::Vector;
    use std::time::{Duration, Instant};
    let now = Instant::now();
    let mut glide = WallGlide::default();
    glide.add(Vector::new(0.0, 100.0));
    // The first frame counts as 1/60s.
    let first = glide.step(now).unwrap();
    assert!((first.y - 100.0 * (1.0 - (-(1000.0f32 / 60.0) / 80.0).exp())).abs() < 0.01);
    let mut total = first.y;
    let mut t = now;
    while let Some(step) = {
        t += Duration::from_millis(16);
        glide.step(t)
    } {
        total += step.y;
    }
    assert!((total - 100.0).abs() < 0.001);
    assert_eq!(glide.pending(), Vector::ZERO);
}

#[test]
fn provider_rows_share_account_priority_and_registration_ties() {
    use folia_ui::{ProviderRowStatus as Status, order_provider_rows};
    let temp = tempfile::tempdir().unwrap();
    let omni = folia_omni::Omni::new(temp.path().to_path_buf(), "Folia-rust-order-test".into()).unwrap();
    let rows = omni.platforms().into_iter().map(|platform| {
        let status = match platform.id {
            "kugou" | "spotify" => Status::SignedIn,
            "qq" => Status::RanOut,
            _ => Status::SignedOut,
        };
        (platform, status)
    });
    let ids: Vec<_> = order_provider_rows(rows, Some("qishui")).into_iter().map(|row| row.id).collect();
    assert_eq!(&ids[..4], &["qishui", "kugou", "spotify", "qq"]);
    assert_eq!(ids[4], "netease");
    assert_eq!(ids[5], "bodian");
}

#[cfg(test)]
mod provider_highlight;

#[cfg(test)]
mod playback_navigation;
