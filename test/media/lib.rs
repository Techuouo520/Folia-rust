// Public metadata privacy and media-clock contracts.
#![cfg(test)]

#[cfg(target_os = "linux")]
mod bus;

use folia_core::{ArtistRef, OnlineId, OnlineTrackInfo, Track};
use folia_media::{Metadata, Snapshot, Status, microseconds};
use std::time::{Duration, Instant};

#[test]
fn metadata_has_stable_object_ids_and_only_public_tags() {
    let mut track = Track {
        id: -7,
        title: "Song".into(),
        artist: Some("fallback".into()),
        online: Some(OnlineId { provider: "netease".into(), id: "7".into() }),
        online_info: Some(OnlineTrackInfo {
            artists: vec![
                ArtistRef { id: "1".into(), name: "One".into() },
                ArtistRef { id: "2".into(), name: "Two".into() },
            ],
            ..Default::default()
        }),
        path: "/cache/private-token/audio".into(),
        cover_url: Some("https://user:private-token@example.org/cover?token=private-token".into()),
        ..Default::default()
    };
    let metadata = Metadata::from_track(&track, 123.456);
    assert_eq!(metadata.length, 123_456_000);
    assert_eq!(metadata.artists, ["One", "Two"]);
    assert!(!format!("{metadata:?}").contains("private-token"));
    assert!(metadata.track_id().starts_with("/org/mpris/MediaPlayer2/track/t"));
    track.path = "/new-cache".into();
    assert_eq!(metadata.track_id(), Metadata::from_track(&track, 0.0).track_id());
    track.online.as_mut().unwrap().provider = "qq".into();
    assert_ne!(metadata.track_id(), Metadata::from_track(&track, 0.0).track_id());
}

#[test]
fn clock_freezes_on_pause_and_buffering_and_clamps_to_length() {
    let now = Instant::now();
    let mut state = Snapshot {
        metadata: Some(Metadata::from_track(&Track::default(), 10.0)),
        status: Status::Playing,
        advancing: true,
        position: 8_000_000,
        sampled_at: now,
        ..Default::default()
    };
    assert_eq!(state.position_at(now + Duration::from_secs(1)), 9_000_000);
    assert_eq!(state.position_at(now + Duration::from_secs(5)), 10_000_000);
    state.advancing = false;
    assert_eq!(state.position_at(now + Duration::from_secs(5)), 8_000_000);
    state.advancing = true;
    state.status = Status::Paused;
    assert_eq!(state.position_at(now + Duration::from_secs(5)), 8_000_000);
    state.metadata = None;
    assert_eq!(state.position_at(now), 0);
    for seconds in [f64::NAN, f64::INFINITY, -10.0] {
        assert_eq!(microseconds(seconds), 0);
    }
    assert_eq!(microseconds(f64::MAX), i64::MAX);
}
