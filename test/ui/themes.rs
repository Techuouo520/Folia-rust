// Song themes preserve user preference rules and ignore delayed cache or generation results after navigation.

use folia_core::{OnlineId, Track};
use folia_store::{AiSettings, DualTheme, SongThemeSettings, Store, ThemeGenerationSource};
use folia_ui::song_theme::{Message, SongThemes};
use folia_ui::{Message as AppMessage, Playback};
use iced::futures::StreamExt;

fn messages(task: iced::Task<AppMessage>) -> Vec<Message> {
    tokio_runtime(async {
        let mut result = Vec::new();
        if let Some(mut stream) = iced_runtime::task::into_stream(task) {
            while let Some(action) = stream.next().await {
                if let iced_runtime::Action::Output(AppMessage::SongTheme(message)) = action {
                    result.push(message);
                }
            }
        }
        result
    })
}

fn tokio_runtime<T>(future: impl std::future::Future<Output = T>) -> T {
    tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap().block_on(future)
}

#[test]
fn song_theme_switches_persist_and_late_replies_keep_the_current_song() {
    let temp = tempfile::tempdir().unwrap();
    let store = Store::at(temp.path().join("settings"));
    let omni = folia_omni::Omni::with_providers(temp.path().join("audio"), "unused".into(), vec![]).unwrap();
    let mut playback = Playback::new(Default::default(), Some(omni));
    let song = |id| Track {
        id,
        title: "fixture".into(),
        online: Some(OnlineId { provider: "fixture".into(), id: id.to_string() }),
        ..Default::default()
    };
    drop(playback.play(vec![song(1)], 0, None));
    let (mut themes, init) = SongThemes::new(store.clone(), temp.path().to_owned(), "unused".into());
    drop(init);
    let mut preferences = SongThemeSettings { generation_source: ThemeGenerationSource::Cover, ..Default::default() };
    preferences.set_auto_generate(true);
    drop(themes.update(Message::Loaded(AiSettings::default(), preferences, Ok(false), None), &store, &playback, &[]));
    let old_cache = messages(themes.observe(&playback, &[], true, false));
    drop(playback.play(vec![song(2)], 0, None));
    let new_cache = messages(themes.observe(&playback, &[], true, false));
    for message in old_cache {
        drop(themes.update(message, &store, &playback, &[]));
    }
    assert!(!themes.generating);
    assert!(themes.current.is_none());
    for message in new_cache {
        drop(themes.update(message, &store, &playback, &[]));
    }
    let due = messages(themes.observe(&playback, &[], true, false));
    for message in due {
        drop(themes.update(message, &store, &playback, &[]));
    }
    let generated = messages(themes.observe(&playback, &[], true, false));
    assert_eq!(generated.len(), 1);
    drop(playback.play(vec![song(3)], 0, None));
    drop(themes.observe(&playback, &[], true, false));
    for message in generated {
        drop(themes.update(message, &store, &playback, &[]));
    }
    assert!(themes.current.is_none());
    assert!(themes.applied.is_none());
    let cache = folia_ai::ThemeCache::open(&temp.path().join("song_themes.db")).unwrap();
    assert!(cache.get(&folia_ai::song_key(&song(2))).unwrap().is_some());
    assert!(cache.get(&folia_ai::song_key(&song(3))).unwrap().is_none());
    let previous = folia_ai::cover_theme(&[folia_core::Rgb::hex(0x3388bb)], 9);
    drop(themes.update(Message::Cached(playback.request_id(), Ok(Some(previous.clone()))), &store, &playback, &[]));
    drop(playback.play(vec![song(4)], 0, None));
    let miss = messages(themes.observe(&playback, &[], true, false));
    for message in miss {
        drop(themes.update(message, &store, &playback, &[]));
    }
    assert_eq!(themes.applied, Some(previous), "a song without a theme keeps the previously applied colors");

    for message in [
        Message::PreferCustom(true),
        Message::AutoSwitch(true),
        Message::AutoGenerate(true),
        Message::AutoSwitch(false),
    ] {
        let replies = messages(themes.update(message, &store, &playback, &[]));
        assert!(replies.is_empty());
        assert_eq!(store.load::<SongThemeSettings>().unwrap(), themes.preferences);
        assert!(!themes.preferences.prefer_custom || !themes.preferences.auto_switch);
        assert!(!themes.preferences.auto_generate || themes.preferences.auto_switch);
    }
    let text = format!("{:?}", Message::Key("private-secret".into()));
    assert!(!text.contains("private-secret"));
}

#[test]
fn startup_restores_the_last_song_theme_only_when_selected() {
    let temp = tempfile::tempdir().unwrap();
    let theme = folia_ai::cover_theme(&[folia_core::Rgb::hex(0xe34c87)], 7);
    let mut init = super::player_fixture::init(temp.path());
    init.theme.source = folia_store::ThemeSource::Ai;
    folia_ai::ThemeCache::open(&init.data_dir.join("song_themes.db"))
        .unwrap()
        .remember(&theme, folia_ai::ThemeCache::reserve_revision())
        .unwrap();
    let (mut shell, task) = folia_ui::Shell::new(init.clone());
    super::player_fixture::complete(&mut shell, task);
    let color = theme.light.background;
    assert_eq!(shell.theme().palette().background, iced::Color::from_rgb8(color.r, color.g, color.b));
    init.theme.source = folia_store::ThemeSource::Default;
    let (mut shell, task) = folia_ui::Shell::new(init);
    super::player_fixture::complete(&mut shell, task);
    let color = folia_core::Theme::DAYLIGHT.background;
    assert_eq!(shell.theme().palette().background, iced::Color::from_rgb8(color.r, color.g, color.b));
}

#[test]
fn ai_without_a_key_waits_for_lyrics_then_uses_cover_colors() {
    let temp = tempfile::tempdir().unwrap();
    let store = Store::at(temp.path().join("settings"));
    let omni = folia_omni::Omni::with_providers(temp.path().join("audio"), "unused".into(), vec![]).unwrap();
    let mut playback = Playback::new(Default::default(), Some(omni));
    let (mut themes, init) = SongThemes::new(store.clone(), temp.path().to_owned(), "unused".into());
    drop(init);
    for auto in [false, true] {
        let mut preferences = SongThemeSettings { generation_source: ThemeGenerationSource::Ai, ..Default::default() };
        preferences.set_auto_generate(auto);
        drop(themes.update(
            Message::Loaded(AiSettings::default(), preferences, Ok(false), None),
            &store,
            &playback,
            &[],
        ));
        let track = Track {
            id: i64::from(auto) + 1,
            title: "fixture".into(),
            online: Some(OnlineId { provider: "fixture".into(), id: auto.to_string() }),
            ..Default::default()
        };
        drop(playback.play(vec![track], 0, None));
        let cache = messages(themes.observe(&playback, &[], true, false));
        for message in cache {
            drop(themes.update(message, &store, &playback, &[]));
        }
        assert!(!themes.can_generate(&playback));
        assert!(messages(themes.update(Message::Generate, &store, &playback, &[])).is_empty());
        playback.set_lyrics(Some(folia_visualizer::preview_lyrics()));
        assert!(themes.can_generate(&playback));
        let task = if auto {
            let due = messages(themes.observe(&playback, &[], true, true));
            for message in due {
                drop(themes.update(message, &store, &playback, &[]));
            }
            themes.observe(&playback, &[], true, true)
        } else {
            themes.update(Message::Generate, &store, &playback, &[])
        };
        for message in messages(task) {
            drop(themes.update(message, &store, &playback, &[]));
        }
        assert!(themes.current.is_some());
        assert_eq!(themes.preferences.generation_source, ThemeGenerationSource::Ai);
    }
    let prompt = folia_ai::theme_prompt("only a title", "");
    assert!(prompt.contains("Music type: unknown"));
    assert!(prompt.contains("Source: song title, lyrics unavailable"));
}

#[test]
fn theme_park_keeps_both_targets_and_saves_the_song_opened_in_the_editor() {
    use folia_ui::theme_park::{Event, Message as Edit, ThemePark};
    let theme = DualTheme::default();
    let mut editor =
        ThemePark::open(Some(&theme), None, false, true, Some("2:qq:original-song".into()), "prompt".into());
    editor.update(Edit::Name(true, "Song light".into()));
    editor.update(Edit::Name(false, "Song dark".into()));
    editor.update(Edit::AddWord);
    editor.update(Edit::Word(0, "summer".into()));
    editor.update(Edit::WordColor(0, "#120034".into()));
    editor.update(Edit::IconDraft("Sun".into()));
    editor.update(Edit::AddIcon);
    editor.update(Edit::Target(true));
    editor.update(Edit::Name(true, "My light".into()));
    editor.update(Edit::Swatch(folia_core::Rgb::hex(0x112244)));
    let Event::Save { custom, theme: custom_theme, .. } = editor.update(Edit::Save) else {
        panic!("custom save missing")
    };
    assert!(custom);
    assert_eq!(custom_theme.light.name, "My light");
    assert_eq!(custom_theme.light.accent, folia_core::Rgb::hex(0x112244));
    editor.update(Edit::Reset);
    let Event::Save { theme: reset, .. } = editor.update(Edit::Save) else { panic!("custom reset missing") };
    assert_eq!(reset.light.name, "My light");
    assert_eq!(reset.light.accent, DualTheme::default().light.accent);
    assert_eq!(reset.dark.accent, DualTheme::default().dark.accent);
    editor.update(Edit::Target(false));
    let Event::Save { custom, theme, song } = editor.update(Edit::Save) else { panic!("song save missing") };
    assert!(!custom);
    assert_eq!(song.as_deref(), Some("2:qq:original-song"));
    assert_eq!(theme.light.name, "Song light");
    assert_eq!(theme.dark.name, "Song dark");
    assert_eq!(theme.light.word_colors, theme.dark.word_colors);
    assert_eq!(theme.dark.lyrics_icons, ["Sun"]);
    editor.update(Edit::Name(false, String::new()));
    assert!(matches!(editor.update(Edit::Save), Event::None));
    editor.update(Edit::Reset);
    assert!(matches!(editor.update(Edit::Save), Event::Save { .. }));
    let Event::Copy(json) = editor.update(Edit::CopyJson) else { panic!("copy missing") };
    assert_eq!(folia_ai::parse_theme(&json).unwrap(), DualTheme::default());
}
