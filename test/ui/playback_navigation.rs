// Auto-navigation waits for the matching successful source and respects cancellation.

use folia_ui::{Page, PlaybackNavigation};

#[test]
fn membership_failure_does_not_open_the_player() {
    let mut navigation = PlaybackNavigation::default();
    navigation.begin(1, Page::Library);
    navigation.failed(1);
    assert!(!navigation.ready(1, Page::Library));
}

#[test]
fn a_successful_source_opens_once_from_its_origin() {
    let mut navigation = PlaybackNavigation::default();
    navigation.begin(2, Page::Search);
    assert!(navigation.ready(2, Page::Search));
    assert!(!navigation.ready(2, Page::Search));
}

#[test]
fn a_late_reply_does_not_consume_a_newer_play() {
    let mut navigation = PlaybackNavigation::default();
    navigation.begin(1, Page::Library);
    navigation.begin(2, Page::Library);
    navigation.failed(1);
    assert!(!navigation.ready(1, Page::Library));
    assert!(navigation.ready(2, Page::Library));
}

#[test]
fn leaving_the_origin_does_not_pull_the_listener_back() {
    let mut navigation = PlaybackNavigation::default();
    navigation.begin(3, Page::Library);
    assert!(!navigation.ready(3, Page::Search));
    assert!(!navigation.ready(3, Page::Library));
}

#[test]
fn cancelling_search_or_switching_provider_discards_the_pending_navigation() {
    let mut navigation = PlaybackNavigation::default();
    navigation.begin(4, Page::Search);
    navigation.cancel();
    assert!(!navigation.ready(4, Page::Search));
}
