// Search duration, provider paging rules and Monet audio response.

use std::time::Instant;

use folia_omni::{
    BodianCursor, bodian_search_page, kugou_search_page, netease_search_page, qishui_search_continues,
    qq_anonymous_search_page, qq_session_search_page, spotify_search_done, spotify_search_page, ytm_search_page,
    ytm_walk_done,
};
use folia_ui::{PlayerBar, PlayerPanel, format_duration};
use folia_visualizer::styles::monet::{
    active_bar_height, energy, envelope, pulse, sample_raw_spectrum_profile, sample_spectrum_profile,
};

#[test]
fn duration_is_minutes_and_padded_seconds() {
    assert_eq!(format_duration(224.0), "3:44");
    assert_eq!(format_duration(5.0), "0:05");
    assert_eq!(format_duration(3660.0), "61:00");
    assert_eq!(format_duration(-3.0), "0:00");
}

#[test]
fn each_platform_keeps_its_own_has_more_rule() {
    let (more, next, total) = netease_search_page(0, 30, Some(80));
    assert!(more && next == 30 && total == 80);
    let (more, next, total) = netease_search_page(30, 0, Some(0));
    assert!(!more && next == 30 && total == 0);

    assert_eq!(qq_session_search_page(true, 2, 20), (true, 40));
    assert_eq!(qq_anonymous_search_page(20, 1, 20), (true, 20));
    assert_eq!(qq_anonymous_search_page(7, 1, 20), (false, 20));

    assert_eq!(kugou_search_page(0, 30, 30, Some(100)), (true, 30));
    assert_eq!(kugou_search_page(0, 12, 30, None), (false, 12));
    assert_eq!(kugou_search_page(10, 18, 30, Some(40)), (true, 28));

    let cursor = BodianCursor { next_offset: 40, has_more: true };
    let (more, next, total) = bodian_search_page(20, Some(100.0), 0, 20, Some(cursor));
    assert!(more && next == 40 && total == Some(100));
    let (more, next, _) = bodian_search_page(20, Some(20.0), 0, 20, None);
    assert!(!more && next == 20);
    let (more, _, _) = bodian_search_page(20, None, 0, 20, None);
    assert!(more);

    assert!(!qishui_search_continues(true, true));
    assert!(qishui_search_continues(false, true));
    assert!(!qishui_search_continues(false, false));

    assert!(spotify_search_done(0, 10));
    assert!(spotify_search_done(4, 300));
    assert!(!spotify_search_done(4, 50));
    assert_eq!(spotify_search_page(false, 0, 40, 30), (true, 30));
    assert_eq!(spotify_search_page(true, 30, 40, 10), (false, 40));

    assert!(ytm_walk_done(false, false));
    assert!(ytm_walk_done(true, true));
    assert!(!ytm_walk_done(true, false));
    let (more, next, total) = ytm_search_page(false, 0, 20, 20);
    assert!(more && next == 20 && total.is_none());
    let (more, next, total) = ytm_search_page(true, 0, 12, 12);
    assert!(!more && next == 12 && total == Some(12));
}

#[test]
fn silent_monet_bars_follow_the_floor_and_the_profiles_match() {
    assert!((energy(0.0) - 0.08).abs() < 1e-12);
    let height = 40.0;
    for index in 0..72 {
        let band = sample_spectrum_profile([0.0; 5], index as f64 / 71.0);
        assert!(band.abs() < 1e-12);
        let got = active_bar_height(height, 0.08, 0.0, pulse(index, 0.0), envelope(index));
        let want = height * (0.02 + (0.08 * 0.04 + pulse(index, 0.0) * 0.02) * envelope(index));
        assert!((got - want).abs() < 1e-9);
    }
    let bands = [0.2, 0.4, 0.6, 0.8, 1.0];
    let expect =
        [(0.0, 0.18918384284453205), (0.25, 0.6346626265385367), (0.5, 0.7278791515923921), (1.0, 0.7068331631513326)];
    for (index, want) in expect {
        assert!((sample_spectrum_profile(bands, index) - want).abs() < 1e-5);
    }
    let raw = [10, 20, 30, 40, 50, 60, 80, 120, 200, 255, 40, 30, 20, 10, 5, 0, 15, 90];
    let raw_expect = [
        (0.0, 0.08443868119139333),
        (0.25, 0.14601651495096038),
        (0.5, 0.28936607180168306),
        (1.0, 0.013075148342806436),
    ];
    for (index, want) in raw_expect {
        assert!((sample_raw_spectrum_profile(&raw, index) - want).abs() < 1e-5);
    }
    assert_eq!(sample_raw_spectrum_profile(&[1, 2, 3], 0.5), 0.0);
}

#[test]
fn a_modal_leaves_the_capsule_and_the_handle_alone() {
    let now = Instant::now();
    let mut bar = PlayerBar::new(now);
    bar.set_modal(true);
    bar.on_hover(true, now);
    assert!(!bar.hover().inside);
    assert!(!bar.hover().expanded);
    bar.set_modal(false);
    bar.on_hover(true, now);
    assert!(bar.hover().inside);
    assert!(!bar.hover().expanded);

    let mut panel = PlayerPanel::new(now);
    assert_eq!(panel.handle_hover_goal(), 0.0);
    panel.set_modal(true);
    panel.on_handle_hover(true, now);
    assert_eq!(panel.handle_hover_goal(), 0.0);
    panel.set_modal(false);
    panel.on_handle_hover(true, now);
    assert_eq!(panel.handle_hover_goal(), 1.0);
}
