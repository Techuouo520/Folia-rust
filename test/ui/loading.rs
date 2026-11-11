// Loading belongs to the current request and continues until decoding reaches the output buffer.
use folia_ui::{LoadPhase, Loading};

#[test]
fn fullscreen_uses_the_reported_logical_size_even_below_window_minimum() {
    use iced::Size;
    let window = Size::new(1200.0, 800.0);
    let small_monitor = Size::new(683.0, 384.0);
    assert_eq!(folia_ui::resize_viewport(window, small_monitor, true), small_monitor);
    assert_eq!(folia_ui::resize_viewport(window, Size::new(160.0, 28.0), false), window);
    assert_eq!(folia_ui::resize_viewport(window, Size::ZERO, true), window);
    assert_eq!(folia_ui::resize_viewport(window, Size::new(f32::NAN, 100.0), true), window);
    assert_eq!(folia_ui::resize_viewport(small_monitor, window, false), window);
}

#[test]
fn controller_rejects_stale_audio_and_cancels_loading_at_queue_end() {
    use folia_core::{OnlineId, Track};
    use folia_ui::{AudioFailure, Playback, PlaybackMessage};
    let temp = tempfile::tempdir().unwrap();
    let store = folia_store::Store::at(temp.path().join("config"));
    let omni = folia_omni::Omni::new(temp.path().join("cache"), "unused".into()).unwrap();
    let mut playback = Playback::new(Default::default(), Some(omni));
    let track = Track {
        id: -1,
        duration: 30.0,
        online: Some(OnlineId { provider: "qq".into(), id: "fixture".into() }),
        ..Default::default()
    };
    let _ = playback.play(vec![track.clone()], 0, None);
    let old = playback.request_id();
    let _ = playback.play(vec![track.clone()], 0, None);
    let current = playback.request_id();
    let _ = playback.update(PlaybackMessage::AudioReady(old, -1, Err(AudioFailure::Other("stale".into()))), &store);
    assert!(playback.loading.active());
    let _ = playback.update(PlaybackMessage::Toggle, &store);
    assert_eq!(playback.request_id(), current);
    let _ = playback.update(PlaybackMessage::Seeking(20.0), &store);
    assert_eq!(playback.position(), 0.0);
    let _ = playback.update(PlaybackMessage::Next, &store);
    assert!(!playback.loading.active());
    let _ = playback
        .update(PlaybackMessage::AudioReady(current, -1, Ok(temp.path().join("must-not-open.mp3").into())), &store);
    assert!(!playback.is_playing());
    let _ = playback.play(vec![track.clone()], 0, None);
    let _ = playback.update(PlaybackMessage::Remove(0), &store);
    assert!(playback.current().is_none());
    assert!(!playback.loading.active());
    let _ = playback.play(vec![track], 0, None);
    let _ = playback.update(
        PlaybackMessage::AudioReady(playback.request_id(), -1, Err(AudioFailure::SubscriptionRequired)),
        &store,
    );
    assert!(!playback.loading.active());
    assert_eq!(playback.queue.items().len(), 1);
    assert!(playback.subscription_notice(std::time::Instant::now()).is_some());
    assert!(playback.loading_notice(std::time::Instant::now()).is_none());
}

#[test]
fn loading_survives_old_completions_and_ends_on_success_or_failure() {
    let mut loading = Loading::default();
    assert!(!loading.active());
    loading.begin(1);
    assert_eq!(loading.phase, LoadPhase::Resolving);
    loading.begin(2);
    loading.decoding(1);
    assert_eq!(loading.phase, LoadPhase::Resolving);
    assert!(!loading.finish(1));
    assert!(loading.active());
    loading.decoding(2);
    assert_eq!(loading.phase, LoadPhase::Decoding);
    assert!(loading.finish(2));
    assert!(!loading.active());
    loading.begin(3);
    assert!(loading.finish(3));
    assert_eq!(loading.phase, LoadPhase::Idle);
}

#[test]
fn seek_buffering_keeps_transport_and_a_second_seek_available() {
    let Ok(output) = folia_player::Player::new() else {
        eprintln!("no audio output device; skipping transport check");
        return;
    };
    drop(output);
    use folia_core::{AudioInput, Download, OnlineId, Track};
    use folia_ui::{Playback, PlaybackMessage};
    use std::time::{Duration, Instant};
    let temp = tempfile::tempdir().unwrap();
    let data = include_bytes!("../player/fixtures/seek.flac");
    let path = temp.path().join("partial.flac");
    std::fs::write(&path, &data[..65536]).unwrap();
    let download = Download::new(std::fs::File::open(&path).unwrap(), Some(data.len() as u64));
    download.publish(65536);
    let omni = folia_omni::Omni::new(temp.path().join("cache"), "unused".into()).unwrap();
    let store = folia_store::Store::at(temp.path().join("config"));
    let mut playback = Playback::new(folia_store::PlaybackSettings { volume: 0.0, ..Default::default() }, Some(omni));
    let track = Track {
        id: -1,
        duration: 2.0,
        online: Some(OnlineId { provider: "qq".into(), id: "fixture".into() }),
        ..Default::default()
    };
    let _ = playback.play(vec![track], 0, None);
    let _ = playback.update(
        PlaybackMessage::AudioReady(playback.request_id(), -1, Ok(AudioInput { path, download: Some(download) })),
        &store,
    );
    let until = Instant::now() + Duration::from_secs(2);
    while playback.loading.active() && Instant::now() < until {
        std::thread::sleep(Duration::from_millis(5));
        let _ = playback.update(PlaybackMessage::Tick, &store);
    }
    assert!(!playback.loading.active());
    let _ = playback.update(PlaybackMessage::Seeking(1.8), &store);
    let _ = playback.update(PlaybackMessage::SeekDone, &store);
    assert!((playback.position() - 1.8).abs() < 0.05);
    assert!(playback.is_buffering());
    assert!(!playback.loading.active(), "buffering must not disable all further seeks");
    let _ = playback.update(PlaybackMessage::Toggle, &store);
    assert!(!playback.is_playing());
    let _ = playback.update(PlaybackMessage::Seeking(0.05), &store);
    let _ = playback.update(PlaybackMessage::SeekDone, &store);
    let until = Instant::now() + Duration::from_secs(2);
    while playback.is_buffering() && Instant::now() < until {
        std::thread::sleep(Duration::from_millis(5));
    }
    assert!(!playback.is_buffering());
    assert!(!playback.is_playing(), "a completed seek must preserve pause intent");
    playback.clear();
}
