// Restarts the real shell with isolated storage and preserves the selected queue item.
use folia_core::{QueueSnapshot, Track};
use folia_store::{PlaybackSession, PlaybackSettings, Store};
use folia_ui::{HomeMessage, Message, Playback, PlaybackMessage, Shell};
use iced::Task;
use iced::futures::StreamExt;

fn complete(shell: &mut Shell, initial: Task<Message>) -> usize {
    let mut tasks = vec![initial];
    let mut exits = 0;
    while let Some(task) = tasks.pop() {
        let (messages, finished) = pollster::block_on(async {
            let mut messages = Vec::new();
            let mut finished = 0;
            if let Some(mut stream) = iced_runtime::task::into_stream(task) {
                while let Some(action) = stream.next().await {
                    match action {
                        iced_runtime::Action::Output(message) => messages.push(message),
                        iced_runtime::Action::Exit => finished += 1,
                        _ => {}
                    }
                }
            }
            (messages, finished)
        });
        exits += finished;
        tasks.extend(messages.into_iter().map(|message| shell.update(message)));
    }
    exits
}

#[test]
fn reopening_restores_the_selected_song_instead_of_an_empty_player() {
    let temp = tempfile::tempdir().unwrap();
    let (mut shell, task) = Shell::new(super::player_fixture::init(temp.path()));
    complete(&mut shell, task);
    let tracks = (1..=4)
        .map(|id| Track {
            id,
            title: format!("Saved {id}"),
            duration: 120.0,
            path: temp.path().join(format!("missing-{id}.wav")),
            ..Default::default()
        })
        .collect();
    let task = shell.update(Message::OpenFiles(tracks));
    complete(&mut shell, task);
    let task = shell.update(Message::Playback(PlaybackMessage::Jump(1)));
    complete(&mut shell, task);
    let expected = shell.title();
    assert!(expected.contains("Saved 2"));
    let task = shell.update(Message::WindowClosing);
    complete(&mut shell, task);
    drop(shell);
    let (mut restarted, task) = Shell::new(super::player_fixture::init(temp.path()));
    complete(&mut restarted, task);
    assert_eq!(restarted.title(), expected, "restart discarded the last selected song and queue");
}

#[tokio::test(flavor = "multi_thread")]
async fn cache_settings_save_through_the_shell_without_resetting_playback() {
    use folia_store::StorageSettings;
    use folia_ui::StorageMessage;
    let temp = tempfile::tempdir().unwrap();
    let init = super::player_fixture::init(temp.path());
    let store = init.store.clone();
    store.save(&saved_session()).unwrap();
    let (mut shell, task) = Shell::new(init);
    complete(&mut shell, task);
    let title = shell.title();
    let target = temp.path().join("other-cache");
    let task = shell.update(Message::Storage(StorageMessage::Picked(Some(target.clone()))));
    complete(&mut shell, task);
    assert_eq!(StorageSettings::load(&store).unwrap().cache_directory, Some(std::fs::canonicalize(&target).unwrap()));
    assert_eq!(shell.title(), title);
    let task = shell.update(Message::Storage(StorageMessage::Reset));
    complete(&mut shell, task);
    assert_eq!(StorageSettings::load(&store).unwrap(), StorageSettings::default());
    assert_eq!(shell.title(), title);
}

#[tokio::test(flavor = "multi_thread")]
async fn closing_during_cache_migration_waits_for_files_and_preserves_the_bookmark() {
    let temp = tempfile::tempdir().unwrap();
    let init = super::player_fixture::init(temp.path());
    let store = init.store.clone();
    store.save(&saved_session()).unwrap();
    let source = init.cache_dir.clone();
    std::fs::create_dir_all(source.join("audio")).unwrap();
    std::fs::write(source.join("audio/saved.mp3"), b"cached music").unwrap();
    let target = temp.path().join("new-cache");
    let (mut shell, task) = Shell::new(init);
    complete(&mut shell, task);
    let moving = shell.update(Message::Storage(folia_ui::StorageMessage::Picked(Some(target.clone()))));
    let close = shell.update(Message::WindowClosing);
    assert_eq!(complete(&mut shell, close), 0, "window exited while the migration was still pending");
    assert_eq!(complete(&mut shell, moving), 1);
    assert_eq!(std::fs::read(target.join("audio/saved.mp3")).unwrap(), b"cached music");
    assert!(!source.join("audio/saved.mp3").exists());
    assert_eq!(store.load::<PlaybackSession>().unwrap(), saved_session());
}

#[tokio::test(flavor = "multi_thread")]
#[cfg_attr(feature = "ci", ignore = "requires an audio output device")]
async fn cache_migration_restores_playing_and_paused_positions() {
    for playing in [false, true] {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("playing.flac");
        std::fs::write(&path, include_bytes!("../player/fixtures/seek.flac")).unwrap();
        let duration = folia_player::DecodedSource::open(&path).unwrap().duration;
        let mut init = super::player_fixture::init(temp.path());
        init.playback.volume = 0.0;
        let store = init.store.clone();
        let mut saved = saved_session();
        saved.queue.items =
            vec![Track { id: 1, title: "Migration playback".into(), path, duration, ..Default::default() }];
        saved.queue.current = Some(0);
        saved.position = 0.75;
        store.save(&saved).unwrap();
        let (mut shell, task) = Shell::new(init);
        complete(&mut shell, task);
        if playing {
            let task = shell.update(Message::Playback(PlaybackMessage::Toggle));
            complete(&mut shell, task);
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            let task = shell.update(Message::Playback(PlaybackMessage::Tick));
            complete(&mut shell, task);
        }
        let task =
            shell.update(Message::Storage(folia_ui::StorageMessage::Picked(Some(temp.path().join("new-cache")))));
        complete(&mut shell, task);
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
        let tick = shell.update(Message::Playback(PlaybackMessage::Tick));
        complete(&mut shell, tick);
        let close = shell.update(Message::WindowClosing);
        assert_eq!(complete(&mut shell, close), 1);
        let restored: PlaybackSession = store.load().unwrap();
        assert_eq!(restored.queue, saved.queue);
        if playing {
            assert!(
                restored.position > 0.85 && restored.position < 2.0,
                "playback did not continue: {}",
                restored.position
            );
        } else {
            assert!((restored.position - 0.75).abs() < 0.001);
        }
    }
}

fn saved_session() -> PlaybackSession {
    PlaybackSession {
        queue: QueueSnapshot {
            items: (1..=3)
                .map(|id| Track { id, title: format!("Saved {id}"), duration: 120.0, ..Default::default() })
                .collect(),
            current: Some(1),
            ..Default::default()
        },
        position: 42.75,
        fm: None,
    }
}

#[test]
fn media_controls_are_idempotent_and_keep_paused_queue_navigation() {
    use folia_media::Command;
    use folia_player::PlayerState;
    let temp = tempfile::tempdir().unwrap();
    let store = Store::at(temp.path());
    let mut player = Playback::new(PlaybackSettings::default(), None);
    drop(player.restore(saved_session()));
    let pending = player.request_id();
    player.loading.begin(pending);
    for _ in 0..2 {
        drop(player.update(PlaybackMessage::Media(Command::Pause), &store));
    }
    assert!(!player.loading.active());
    assert_ne!(player.request_id(), pending);
    drop(
        player.update(
            PlaybackMessage::AudioReady(pending, 2, Err(folia_ui::AudioFailure::CustomSourceUnavailable)),
            &store,
        ),
    );
    assert_eq!(player.status.state, PlayerState::Paused);
    assert_eq!(player.position(), 42.75);
    drop(player.update(PlaybackMessage::Media(Command::Next), &store));
    assert_eq!(player.current().unwrap().id, 3);
    assert_eq!(player.status.state, PlayerState::Paused);
    assert_eq!(player.position(), 0.0);
    assert!(player.analysis_input().is_none());
    drop(player.update(PlaybackMessage::Media(Command::Previous), &store));
    assert_eq!(player.current().unwrap().id, 2);
    for _ in 0..2 {
        drop(player.update(PlaybackMessage::Media(Command::Stop), &store));
    }
    assert_eq!(player.status.state, PlayerState::Idle);
    assert_eq!(player.position(), 0.0);
    assert_eq!(player.queue.items().len(), 3);
    assert_eq!(player.current().unwrap().id, 2);
    drop(player.update(PlaybackMessage::Media(Command::Next), &store));
    assert_eq!(player.status.state, PlayerState::Idle);
    assert_eq!(player.current().unwrap().id, 3);
    drop(player.update(PlaybackMessage::Media(Command::Volume(2.0)), &store));
    assert_eq!(player.settings.volume, 1.0);
    drop(player.update(PlaybackMessage::Media(Command::Volume(f64::NAN)), &store));
    assert_eq!(player.settings.volume, 1.0);
    player.status.state = PlayerState::Ended;
    drop(player.update(PlaybackMessage::Media(Command::Previous), &store));
    drop(player.update(PlaybackMessage::Tick, &store));
    assert_eq!(player.current().unwrap().id, 2);
    assert_eq!(player.status.state, PlayerState::Idle);
}

#[test]
fn restored_playback_stays_paused_and_can_seek_before_loading_audio() {
    let temp = tempfile::tempdir().unwrap();
    let store = Store::at(temp.path());
    let mut player = Playback::new(PlaybackSettings::default(), None);
    drop(player.restore(saved_session()));
    for _ in 0..3 {
        drop(player.update(PlaybackMessage::Tick, &store));
    }
    assert!(!player.is_playing());
    assert_eq!(player.position(), 42.75);
    assert_eq!(player.queue.current_index(), Some(1));
    assert!(player.analysis_input().is_none());
    assert_eq!(player.refresh_prefetch().units(), 0);
    let i18n = folia_i18n::I18n::new(folia_core::Language::ZhCn).unwrap();
    let mut radio = folia_ui::Lattice::default();
    drop(radio.sync(&player, folia_store::WallAppearance { radio: true, ..Default::default() }, &i18n, None));
    let (_, resume) = radio.update(folia_ui::LatticeMessage::Key(folia_ui::CardKey::Enter), None);
    assert!(
        matches!(resume, Some(Message::Playback(PlaybackMessage::Toggle))),
        "the current radio card must resume the saved song instead of restarting it"
    );
    drop(player.update(PlaybackMessage::Seeking(73.5), &store));
    drop(player.update(PlaybackMessage::SeekDone, &store));
    assert_eq!(player.position(), 73.5);
    drop(player.source_mode_changed());
    assert_eq!(player.snapshot().position, 73.5);
    let mut online = saved_session();
    let song = folia_core::OnlineId { provider: "netease".into(), id: "2".into() };
    online.queue.items[1].id = song.track_id();
    online.queue.items[1].online = Some(song);
    online.fm = Some(folia_store::FmSession { provider: "netease".into(), id: "fm".into(), name: "FM".into() });
    drop(player.restore(online));
    drop(player.source_mode_changed());
    drop(player.update(PlaybackMessage::Tick, &store));
    assert!(player.is_fm());
    assert!(!player.is_playing());
    assert_eq!(player.position(), 42.75, "source initialization discarded the restored online position");
}

#[test]
fn close_flushes_latest_queue_and_seek_after_an_older_write() {
    let temp = tempfile::tempdir().unwrap();
    let init = super::player_fixture::init(temp.path());
    let store = init.store.clone();
    store.save(&saved_session()).unwrap();
    let (mut shell, task) = Shell::new(init);
    complete(&mut shell, task);
    let old_write = shell.update(Message::Playback(PlaybackMessage::Reorder(0, 2)));
    drop(shell.update(Message::Playback(PlaybackMessage::Seeking(73.5))));
    drop(shell.update(Message::Playback(PlaybackMessage::SeekDone)));
    let close = shell.update(Message::WindowClosing);
    assert_eq!(complete(&mut shell, Task::batch([old_write, close])), 1);
    let saved: PlaybackSession = store.load().unwrap();
    assert_eq!(saved.queue.items.iter().map(|track| track.id).collect::<Vec<_>>(), [2, 3, 1]);
    assert_eq!(saved.queue.current, Some(0));
    assert_eq!(saved.position, 73.5);
    let (mut restarted, task) = Shell::new(super::player_fixture::init(temp.path()));
    complete(&mut restarted, task);
    assert!(restarted.title().contains("Saved 2"));
    drop(restarted.update(Message::Home(HomeMessage::ToolsQueueClear)));
    std::thread::sleep(std::time::Duration::from_millis(450));
    let clear = restarted.update(Message::Home(HomeMessage::ToolsQueueClear));
    let close = restarted.update(Message::WindowClosing);
    assert_eq!(complete(&mut restarted, Task::batch([clear, close])), 1);
    let saved: PlaybackSession = store.load().unwrap();
    assert!(saved.queue.items.is_empty());
    assert_eq!(saved.queue.detached.as_ref().unwrap().id, 2);
    assert_eq!(saved.position, 73.5);
    let (mut restarted, task) = Shell::new(super::player_fixture::init(temp.path()));
    complete(&mut restarted, task);
    assert!(restarted.title().contains("Saved 2"));
}

#[test]
fn a_new_selection_wins_over_late_startup_restoration() {
    let temp = tempfile::tempdir().unwrap();
    let init = super::player_fixture::init(temp.path());
    let store = init.store.clone();
    store.save(&saved_session()).unwrap();
    let (mut shell, loading) = Shell::new(init);
    let selected =
        shell.update(Message::OpenFiles(vec![Track { id: 88, title: "New choice".into(), ..Default::default() }]));
    complete(&mut shell, Task::batch([loading, selected]));
    assert!(shell.title().contains("New choice"));
    let close = shell.update(Message::WindowClosing);
    assert_eq!(complete(&mut shell, close), 1);
    assert_eq!(store.load::<PlaybackSession>().unwrap().queue.items[0].id, 88);
    let (mut reopening, loading) = Shell::new(super::player_fixture::init(temp.path()));
    let close = reopening.update(Message::WindowClosing);
    assert_eq!(complete(&mut reopening, Task::batch([loading, close])), 1);
    assert_eq!(
        store.load::<PlaybackSession>().unwrap().queue.items[0].id,
        88,
        "closing before startup restoration must not replace the saved queue with an empty one"
    );
}

#[test]
fn online_resume_keeps_the_bookmark_until_audio_is_ready() {
    let temp = tempfile::tempdir().unwrap();
    let store = Store::at(temp.path().join("config"));
    let omni =
        folia_omni::Omni::new(temp.path().join("cache"), format!("folia-session-{}", temp.path().display())).unwrap();
    let mut player = Playback::new(PlaybackSettings { volume: 0.0, ..Default::default() }, Some(omni));
    let mut session = saved_session();
    let song = folia_core::OnlineId { provider: "netease".into(), id: "2".into() };
    let id = song.track_id();
    session.queue.items[1].id = id;
    session.queue.items[1].online = Some(song);
    drop(player.restore(session));
    // Dropping these futures leaves URL and lyric requests unpolled; no network is contacted.
    drop(player.update(PlaybackMessage::Toggle, &store));
    assert!(player.loading.active());
    assert_eq!(player.snapshot().position, 42.75, "a pending address request erased the bookmark");
    let cancelled = player.request_id();
    drop(player.source_mode_changed());
    assert_ne!(player.request_id(), cancelled);
    assert!(!player.loading.active());
    drop(
        player.update(
            PlaybackMessage::AudioReady(cancelled, id, Err(folia_ui::AudioFailure::SubscriptionRequired)),
            &store,
        ),
    );
    assert!(player.subscription_notice(std::time::Instant::now()).is_none());
    drop(player.update(PlaybackMessage::Toggle, &store));
    let request = player.request_id();
    drop(
        player.update(
            PlaybackMessage::AudioReady(request, id, Err(folia_ui::AudioFailure::SubscriptionRequired)),
            &store,
        ),
    );
    assert_eq!(player.snapshot().position, 42.75, "a stopped resume attempt erased the bookmark");
    drop(player.source_mode_changed());
    assert_eq!(player.position(), 42.75);
}

#[test]
fn corrupt_sessions_are_ignored_and_failed_saves_do_not_block_exit() {
    let temp = tempfile::tempdir().unwrap();
    let init = super::player_fixture::init(temp.path());
    std::fs::create_dir_all(init.store.dir()).unwrap();
    let path = init.store.dir().join("playback_session.toml");
    std::fs::write(&path, "schema_version = 1\nqueue = [[").unwrap();
    let (mut shell, task) = Shell::new(init);
    complete(&mut shell, task);
    assert_eq!(shell.title(), "Folia");
    let close = shell.update(Message::WindowClosing);
    assert_eq!(complete(&mut shell, close), 1);
    assert!(std::fs::read_to_string(path).unwrap().contains("queue = [["));

    let blocked = tempfile::tempdir().unwrap();
    let init = super::player_fixture::init(blocked.path());
    std::fs::write(init.store.dir(), "not a directory").unwrap();
    let (mut shell, task) = Shell::new(init);
    complete(&mut shell, task);
    let task = shell.update(Message::OpenFiles(vec![Track { id: 9, title: "Unsaved".into(), ..Default::default() }]));
    complete(&mut shell, task);
    let close = shell.update(Message::WindowClosing);
    assert_eq!(complete(&mut shell, close), 1);
}

#[test]
fn explicit_launch_files_take_precedence_over_the_saved_queue() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("replacement.flac");
    std::fs::write(&path, include_bytes!("../player/fixtures/seek.flac")).unwrap();
    let mut init = super::player_fixture::init(temp.path());
    init.playback.volume = 0.0;
    init.open_files = vec![path];
    init.store.save(&saved_session()).unwrap();
    let store = init.store.clone();
    let (mut shell, task) = Shell::new(init);
    complete(&mut shell, task);
    assert!(!shell.title().contains("Saved 2"));
    let close = shell.update(Message::WindowClosing);
    assert_eq!(complete(&mut shell, close), 1);
    let saved: PlaybackSession = store.load().unwrap();
    assert_eq!(saved.queue.items.len(), 1);
    assert!(saved.queue.items[0].path.ends_with("replacement.flac"));
}

#[test]
#[cfg_attr(feature = "ci", ignore = "requires an audio output device")]
fn resume_decodes_from_the_saved_time_and_normal_seek_still_works() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("resume.wav");
    let bytes = 48_000_u32 * 3 * 2;
    let mut wav = Vec::new();
    wav.extend_from_slice(b"RIFF");
    wav.extend_from_slice(&(bytes + 36).to_le_bytes());
    wav.extend_from_slice(b"WAVEfmt ");
    wav.extend_from_slice(&16_u32.to_le_bytes());
    wav.extend_from_slice(&1_u16.to_le_bytes());
    wav.extend_from_slice(&1_u16.to_le_bytes());
    wav.extend_from_slice(&48_000_u32.to_le_bytes());
    wav.extend_from_slice(&96_000_u32.to_le_bytes());
    wav.extend_from_slice(&2_u16.to_le_bytes());
    wav.extend_from_slice(&16_u16.to_le_bytes());
    wav.extend_from_slice(b"data");
    wav.extend_from_slice(&bytes.to_le_bytes());
    wav.resize(bytes as usize + 44, 0);
    std::fs::write(&path, wav).unwrap();
    let duration = folia_player::DecodedSource::open(&path).unwrap().duration;
    assert!(duration > 2.0);
    let mut session = saved_session();
    session.queue.items = vec![Track { id: 1, path, duration, ..Default::default() }];
    session.queue.current = Some(0);
    session.position = 0.75;
    let store = Store::at(temp.path().join("config"));
    let mut player = Playback::new(PlaybackSettings { volume: 0.0, ..Default::default() }, None);
    drop(player.restore(session));
    assert!(!player.is_playing());
    assert_eq!(player.position(), 0.75);
    drop(player.update(PlaybackMessage::Toggle, &store));
    assert!(player.position() >= 0.75, "resume briefly reset the transport to zero");
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
    loop {
        drop(player.update(PlaybackMessage::Tick, &store));
        if !player.is_buffering() && player.position() >= 0.75 {
            break;
        }
        assert!(std::time::Instant::now() < deadline, "resume did not reach the saved audio position");
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    assert!(player.is_playing());
    assert!(player.position() < 1.5);
    drop(player.update(PlaybackMessage::Seeking(1.75), &store));
    drop(player.update(PlaybackMessage::SeekDone, &store));
    assert!(player.position() >= 1.75, "ordinary seek discarded its target after restore");
    player.clear();
}
