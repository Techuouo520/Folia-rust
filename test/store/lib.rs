// Checks settings files: defaults, round trips, file layout and schema version checks.

#![cfg(test)]

use std::fs;

use folia_core::{AudioQuality, Language, LanguagePreference};
use folia_store::{
    AppearanceSettings, FontStyle, GeneralSettings, HomeTab, PlaybackSettings, RowSource, Store, StoreError,
    SubtitleContentMode, SubtitleLayout, VisualizerSettings,
};

mod developer;
mod log_file;
mod theme_settings;

#[test]
fn missing_file_gives_defaults() {
    let dir = tempfile::tempdir().unwrap();
    let settings: GeneralSettings = Store::at(dir.path()).load().unwrap();
    assert_eq!(settings, GeneralSettings::default());
    assert_eq!(settings.language, LanguagePreference::System);
}

#[test]
fn playback_sessions_round_trip_and_normalize_invalid_times() {
    use folia_core::{OnlineId, QueueSnapshot, Track};
    use folia_store::{Domain, FmSession, PlaybackSession};
    let dir = tempfile::tempdir().unwrap();
    let store = Store::at(dir.path());
    let local =
        Track {
            id: 1, title: "本地歌曲".into(), path: "music/song.wav".into(), duration: 120.0, ..Default::default()
        };
    let online = Track {
        id: -2,
        title: "Online song".into(),
        duration: 180.0,
        online: Some(OnlineId { provider: "netease".into(), id: "2".into() }),
        ..Default::default()
    };
    let mut session = PlaybackSession {
        queue: QueueSnapshot { items: vec![local, online.clone(), online], current: Some(2), ..Default::default() },
        position: 42.75,
        fm: Some(FmSession { provider: "netease".into(), id: "fm".into(), name: "FM".into() }),
    };
    store.save(&session).unwrap();
    assert_eq!(store.load::<PlaybackSession>().unwrap(), session);
    session.queue.items[1].path = "cache/unfinished.part".into();
    assert!(session.clone().normalized().queue.items[1].path.as_os_str().is_empty());
    assert!(fs::read_to_string(dir.path().join(PlaybackSession::FILE)).unwrap().starts_with("schema_version = 1\n"));
    session.queue.current = Some(99);
    session.queue.detached = Some(session.queue.items[0].clone());
    session.queue.detached.as_mut().unwrap().duration = f64::INFINITY;
    session.position = f64::NAN;
    let normalized = session.normalized();
    assert_eq!(normalized.position, 0.0);
    assert_eq!(normalized.queue.current, None);
    assert_eq!(normalized.queue.detached.unwrap().duration, 0.0);
    assert!(normalized.fm.is_none());
}

#[test]
fn graphics_preferences_round_trip_and_system_motion_is_opt_in() {
    use folia_store::{GpuPreference, GraphicsSettings, MotionSurface};
    let dir = tempfile::tempdir().unwrap();
    let store = Store::at(dir.path());
    let defaults = store.load::<GraphicsSettings>().unwrap();
    assert_eq!(defaults.gpu_preference, GpuPreference::HighPerformance);
    assert_eq!(defaults.lyric_render_quality, folia_core::LyricRenderQuality::default());
    for surface in MotionSurface::ALL {
        assert!(!defaults.motion.reduced(surface, true));
    }
    let mut settings = GraphicsSettings {
        static_mode: true,
        background_throttling: true,
        gpu_preference: GpuPreference::LowPower,
        ..defaults.clone()
    };
    settings.motion.set(MotionSurface::SettingsScroll, true);
    settings.lyric_render_quality =
        folia_core::LyricRenderQuality { scale: 0.75, antialias: folia_core::LyricAntialias::Off };
    store.save(&settings).unwrap();
    assert_eq!(store.load::<GraphicsSettings>().unwrap(), settings);
    assert!(settings.motion.reduced(MotionSurface::SettingsScroll, false));
    assert!(!settings.motion.reduced(MotionSurface::UiMicroMotion, true));
    settings.motion.follow_system = true;
    for surface in MotionSurface::ALL {
        assert!(settings.motion.reduced(surface, true));
    }
    assert!(!settings.motion.reduced(MotionSurface::UiMicroMotion, false));
    store.save(&defaults).unwrap();
    assert_eq!(store.load::<GraphicsSettings>().unwrap(), defaults);
    fs::write(dir.path().join("graphics.toml"), "schema_version = 1\nstatic_mode = true\n").unwrap();
    let older = store.load::<GraphicsSettings>().unwrap();
    assert!(older.static_mode);
    assert_eq!(older.motion, defaults.motion);
    assert_eq!(older.lyric_render_quality, defaults.lyric_render_quality);
}

#[test]
fn save_then_load() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::at(dir.path().join("nested"));
    let settings = GeneralSettings {
        language: LanguagePreference::Fixed(Language::Vi),
        home_tab: HomeTab::Radio,
        online_provider: None,
        wall_tools_queue_island_open: false,
        wall_randomize_on_entry: true,
    };

    store.save(&settings).unwrap();
    assert_eq!(store.load::<GeneralSettings>().unwrap(), settings);

    let text = fs::read_to_string(dir.path().join("nested/general.toml")).unwrap();
    assert_eq!(
        text,
        "schema_version = 2\nlanguage = \"vi\"\nhome_tab = \"radio\"\nwall_tools_queue_island_open = false\nwall_randomize_on_entry = true\n"
    );
    assert!(!dir.path().join("nested/general.toml.tmp").exists());
}

#[test]
fn rejects_other_schema_versions() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("general.toml"), "schema_version = 9\nlanguage = \"en\"\n").unwrap();
    let err = Store::at(dir.path()).load::<GeneralSettings>().unwrap_err();
    assert!(matches!(err, StoreError::SchemaVersion { found: 9, expected: 2, .. }), "{err}");
}

#[test]
fn rejects_malformed_files() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("general.toml"), "schema_version = 1\nlanguage = 42\n").unwrap();
    let err = GeneralSettings::load(&Store::at(dir.path())).unwrap_err();
    assert!(matches!(err, StoreError::Parse { .. }), "{err}");
}

#[test]
fn playback_keeps_volume_when_quality_is_missing() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("playback.toml"), "schema_version = 1\nvolume = 0.35\nrepeat = \"all\"\n").unwrap();
    let settings = PlaybackSettings::load(&Store::at(dir.path())).unwrap();
    assert_eq!(settings.volume, 0.35);
    assert_eq!(settings.repeat, folia_core::Repeat::All);
    assert_eq!(settings.audio_quality, AudioQuality::High);
    assert!(!settings.muted);
    assert!(!settings.netease_scrobble_enabled);
}

#[test]
fn cache_directory_migration_moves_owned_files_and_can_reset() {
    use folia_store::StorageSettings;
    let dir = tempfile::tempdir().unwrap();
    let store = Store::at(dir.path().join("config"));
    let default = dir.path().join("cache");
    let legacy = StorageSettings { cache_directory: Some(default.clone()), ..Default::default() };
    store.save(&legacy).unwrap();
    let config = store.dir().join("storage.toml");
    let old = fs::read_to_string(&config).unwrap().replace("schema_version = 2", "schema_version = 1");
    fs::write(&config, old).unwrap();
    assert_eq!(StorageSettings::load(&store).unwrap(), legacy);
    assert!(fs::read_to_string(config).unwrap().starts_with("schema_version = 2"));
    fs::create_dir_all(default.join("audio")).unwrap();
    fs::write(default.join("audio/keep.bin"), b"old audio").unwrap();
    fs::write(default.join("unrelated.txt"), b"leave here").unwrap();
    let target = dir.path().join("音乐 缓存");
    fs::create_dir_all(target.join("covers")).unwrap();
    fs::write(target.join("unrelated.txt"), b"keep target").unwrap();
    let moved = StorageSettings::migrate(&store, &default, Some(target.clone()), &default, |_, _| {}).unwrap();
    assert!(!moved.cleanup_pending);
    let settings = moved.settings;
    assert_eq!(StorageSettings::load(&store).unwrap(), settings);
    assert_eq!(settings.cache_path(&default), fs::canonicalize(&target).unwrap());
    assert_eq!(fs::read(target.join("audio/keep.bin")).unwrap(), b"old audio");
    assert!(!default.join("audio/keep.bin").exists());
    assert_eq!(fs::read(default.join("unrelated.txt")).unwrap(), b"leave here");
    assert_eq!(fs::read(target.join("unrelated.txt")).unwrap(), b"keep target");
    assert!(
        !fs::read_dir(&target).unwrap().any(|entry| entry
            .unwrap()
            .file_name()
            .to_string_lossy()
            .starts_with(".folia-cache-migration-"))
    );
    let reset = StorageSettings::migrate(&store, &target, None, &default, |_, _| {}).unwrap();
    assert_eq!(reset.settings.cache_path(&default), default);
    assert_eq!(fs::read(default.join("audio/keep.bin")).unwrap(), b"old audio");
    assert!(!target.join("audio/keep.bin").exists());
    assert_eq!(StorageSettings::load(&store).unwrap(), StorageSettings::default());
    assert!(target.is_dir());
}

#[test]
fn invalid_cache_directory_never_replaces_the_last_good_setting() {
    use folia_store::StorageSettings;
    let dir = tempfile::tempdir().unwrap();
    let store = Store::at(dir.path().join("config"));
    let default = dir.path().join("cache");
    let target = dir.path().join("selected");
    let before =
        StorageSettings::migrate(&store, &default, Some(target.clone()), &default, |_, _| {}).unwrap().settings;
    let file = dir.path().join("file");
    fs::write(&file, b"unrelated").unwrap();
    for invalid in [file.clone(), file.join("nested"), std::path::PathBuf::from("relative-cache")] {
        assert!(StorageSettings::migrate(&store, &target, Some(invalid), &default, |_, _| {}).is_err());
        assert_eq!(StorageSettings::load(&store).unwrap(), before);
    }
    assert_eq!(fs::read(file).unwrap(), b"unrelated");
}

#[test]
fn cache_migration_recovers_interrupted_copy_and_cleanup() {
    use folia_store::{CacheMigration, StorageSettings};
    let temp = tempfile::tempdir().unwrap();
    let store = Store::at(temp.path().join("config"));
    let source = temp.path().join("source");
    let target = temp.path().join("target");
    fs::create_dir_all(source.join("audio")).unwrap();
    let bytes = vec![37; 200_000];
    fs::write(source.join("audio/song.flac"), &bytes).unwrap();
    let interrupted = std::panic::catch_unwind(|| {
        let _ = StorageSettings::migrate(&store, &source, Some(target.clone()), &source, |copied, _| {
            if copied > 0 {
                panic!("simulated process interruption")
            }
        });
    });
    assert!(interrupted.is_err());
    let pending = StorageSettings::load(&store).unwrap();
    assert!(pending.cache_directory.is_none());
    assert!(!pending.migration.as_ref().unwrap().cleanup);
    assert_eq!(fs::read(source.join("audio/song.flac")).unwrap(), bytes);
    let recovered = StorageSettings::load_and_resume(&store).unwrap();
    assert!(recovered.migration.is_none());
    assert_eq!(fs::read(target.join("audio/song.flac")).unwrap(), bytes);
    assert!(!source.join("audio/song.flac").exists());
    fs::write(source.join("audio/song.flac"), &bytes).unwrap();
    let mut interrupted_cleanup = recovered;
    interrupted_cleanup.migration = Some(CacheMigration {
        source: fs::canonicalize(&source).unwrap(),
        destination: fs::canonicalize(&target).unwrap(),
        restore_default: false,
        staging: "dead-beef".into(),
        cleanup: true,
    });
    store.save(&interrupted_cleanup).unwrap();
    assert!(StorageSettings::load_and_resume(&store).unwrap().migration.is_none());
    assert!(!source.join("audio/song.flac").exists());
    assert_eq!(fs::read(target.join("audio/song.flac")).unwrap(), bytes);
}

#[test]
fn cache_migration_rejects_conflicts_overlaps_and_active_consumers() {
    use folia_store::{CacheLease, StorageSettings};
    let temp = tempfile::tempdir().unwrap();
    let store = Store::at(temp.path().join("config"));
    let source = temp.path().join("source");
    let target = temp.path().join("target");
    for root in [&source, &target] {
        fs::create_dir_all(root.join("audio")).unwrap();
    }
    fs::write(source.join("audio/song.bin"), b"source").unwrap();
    fs::write(target.join("audio/song.bin"), b"target").unwrap();
    assert!(StorageSettings::migrate(&store, &source, Some(target.clone()), &source, |_, _| {}).is_err());
    assert!(StorageSettings::load(&store).unwrap().cache_directory.is_none());
    assert_eq!(fs::read(target.join("audio/song.bin")).unwrap(), b"target");
    assert_eq!(fs::read(source.join("audio/song.bin")).unwrap(), b"source");
    let cancelled = StorageSettings::migrate(&store, &source, None, &source, |_, _| {}).unwrap();
    assert!(cancelled.settings.migration.is_none());
    assert!(StorageSettings::load_and_resume(&store).unwrap().cache_directory.is_none());
    assert!(StorageSettings::migrate(&store, &source, Some(source.join("nested")), &source, |_, _| {}).is_err());
    let next = temp.path().join("new-target");
    let lease = CacheLease::shared(&source).unwrap();
    assert!(StorageSettings::migrate(&store, &source, Some(next.clone()), &source, |_, _| {}).is_err());
    drop(lease);
    let moved = StorageSettings::migrate(&store, &source, Some(next.clone()), &source, |_, _| {}).unwrap();
    assert!(!moved.cleanup_pending);
    assert_eq!(fs::read(next.join("audio/song.bin")).unwrap(), b"source");
    assert_eq!(fs::read(target.join("audio/song.bin")).unwrap(), b"target");
}

#[cfg(unix)]
#[test]
fn cache_migration_rejects_linked_cache_entries() {
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("source");
    let target = temp.path().join("target");
    let external = temp.path().join("external");
    fs::create_dir_all(&source).unwrap();
    fs::create_dir_all(&external).unwrap();
    fs::write(external.join("keep.bin"), b"not a cache file").unwrap();
    std::os::unix::fs::symlink(&external, source.join("audio")).unwrap();
    let store = Store::at(temp.path().join("config"));
    assert!(folia_store::StorageSettings::migrate(&store, &source, Some(target), &source, |_, _| {}).is_err());
    assert_eq!(fs::read(external.join("keep.bin")).unwrap(), b"not a cache file");
}

#[cfg(unix)]
#[test]
#[ignore = "set FOLIA_CACHE_TEST_DEST to a second temporary filesystem"]
fn cache_move_across_filesystems_preserves_bytes() {
    use std::os::unix::fs::MetadataExt;
    let source = tempfile::tempdir().unwrap();
    let target = tempfile::tempdir_in(std::env::var_os("FOLIA_CACHE_TEST_DEST").unwrap()).unwrap();
    assert_ne!(source.path().metadata().unwrap().dev(), target.path().metadata().unwrap().dev());
    fs::create_dir(source.path().join("audio")).unwrap();
    let bytes = vec![73; 200_000];
    fs::write(source.path().join("audio/cross.flac"), &bytes).unwrap();
    let store = Store::at(source.path().join("config"));
    let result = folia_store::StorageSettings::migrate(
        &store,
        source.path(),
        Some(target.path().into()),
        source.path(),
        |_, _| {},
    )
    .unwrap();
    assert!(!result.cleanup_pending);
    assert!(result.settings.migration.is_none());
    assert_eq!(fs::read(target.path().join("audio/cross.flac")).unwrap(), bytes);
    assert!(!source.path().join("audio/cross.flac").exists());
}

#[test]
fn listening_report_preference_migrates_off_and_can_be_saved() {
    for schema in [1, 2] {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::at(dir.path());
        fs::write(dir.path().join("playback.toml"), format!("schema_version = {schema}\nvolume = 0.4\nmuted = true\nrepeat = \"one\"\naudio_quality = \"lossless\"\n")).unwrap();
        let mut settings = PlaybackSettings::load(&store).unwrap();
        assert!(!settings.netease_scrobble_enabled);
        assert_eq!(settings.volume, 0.4);
        assert!(settings.muted);
        assert_eq!(settings.repeat, folia_core::Repeat::One);
        assert_eq!(settings.audio_quality, AudioQuality::Lossless);
        settings.netease_scrobble_enabled = true;
        store.save(&settings).unwrap();
        assert_eq!(PlaybackSettings::load(&store).unwrap(), settings);
    }
}

#[test]
fn general_migration_preserves_preferences_and_defaults_wall_randomization_off() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::at(dir.path());
    fs::write(dir.path().join("general.toml"), "schema_version = 1\nlanguage = \"vi\"\nhome_tab = \"albums\"\nonline_provider = \"qq\"\nwall_tools_queue_island_open = true\n").unwrap();
    let settings = GeneralSettings::load(&store).unwrap();
    assert_eq!(settings.language, LanguagePreference::Fixed(Language::Vi));
    assert_eq!(settings.home_tab, HomeTab::Albums);
    assert_eq!(settings.online_provider.as_deref(), Some("qq"));
    assert!(settings.wall_tools_queue_island_open);
    assert!(!settings.wall_randomize_on_entry);
    assert_eq!(store.load::<GeneralSettings>().unwrap(), settings);
    assert!(fs::read_to_string(dir.path().join("general.toml")).unwrap().starts_with("schema_version = 2\n"));
}

#[test]
fn bottom_bar_offset_stays_between_the_base_and_half_the_window() {
    use folia_store::{BOTTOM_BAR_BASE, PlayerSettings, bottom_bar_max_offset, clamp_bottom_bar_offset};
    assert_eq!(bottom_bar_max_offset(800.0), 268.0, "half of 800 less the 132px expanded bar");
    assert_eq!(bottom_bar_max_offset(200.0), BOTTOM_BAR_BASE, "a short window collapses the range to the base");
    assert_eq!(clamp_bottom_bar_offset(10.0, 800.0), BOTTOM_BAR_BASE);
    assert_eq!(clamp_bottom_bar_offset(500.0, 800.0), 268.0);
    assert_eq!(clamp_bottom_bar_offset(f32::NAN, 800.0), BOTTOM_BAR_BASE);
    let dir = tempfile::tempdir().unwrap();
    let store = folia_store::Store::at(dir.path().to_path_buf());
    let lifted = PlayerSettings { bottom_bar_offset: 120.0 };
    store.save(&lifted).unwrap();
    assert_eq!(store.load::<PlayerSettings>().unwrap(), lifted);
}

#[test]
fn appearance_keeps_the_background_mode_and_the_new_font_and_subtitle_fields() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::at(dir.path());
    let mut appearance = AppearanceSettings::default();
    assert_eq!(appearance.stored_background_mode(), None);
    appearance.set_background_mode(Some("common"));
    appearance.disable_visualizer_vignette = true;
    appearance.lyrics_font.style = FontStyle::Custom;
    appearance.lyrics_font.family = Some("Songti SC".to_owned());
    appearance.lyrics_font.fallback = vec!["SimSun".to_owned(), "serif".to_owned()];
    appearance.lyrics_font.weight = Some(600);
    appearance.subtitle.layout = SubtitleLayout::parse("current+translation").unwrap();
    appearance.subtitle.content_mode = SubtitleContentMode::Both;
    appearance.subtitle.opacity = 0.8;
    appearance.subtitle.font_inherits_lyrics = false;
    appearance.subtitle.font_scale = 1.2;
    store.save(&appearance).unwrap();
    assert_eq!(store.load::<AppearanceSettings>().unwrap(), appearance);

    let text = fs::read_to_string(dir.path().join("appearance.toml")).unwrap();
    assert!(text.contains("visualizer_background_mode = \"common\""), "{text}");
    assert!(text.contains("layout = \"current+translation\""), "{text}");
    // Clearing the mode stores nothing and the default follows again.
    let mut cleared = appearance.clone();
    cleared.set_background_mode(None);
    assert_eq!(cleared.stored_background_mode(), None);
}

#[test]
fn an_older_appearance_file_reads_with_the_new_defaults() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(
        dir.path().join("appearance.toml"),
        "schema_version = 1\ntheme = \"light\"\nframe_rate = \"45\"\nlyrics_font_scale = 1.1\nvisualizer = \"monet\"\n",
    )
    .unwrap();
    let appearance = AppearanceSettings::load(&Store::at(dir.path())).unwrap();
    assert!(!appearance.follow_system_theme);
    assert!(!appearance.use_cover_color_bg);
    assert_eq!(appearance.cover_background_opacity, 0.75);
    assert!(fs::read_to_string(dir.path().join("appearance.toml")).unwrap().starts_with("schema_version = 4\n"));
    assert_eq!(appearance.lyrics_font_scale, 1.1);
    assert_eq!(appearance.visualizer, "monet");
    assert_eq!(appearance.stored_background_mode(), None);
    assert_eq!(appearance.subtitle, folia_store::SubtitleSettings::default());
    assert_eq!(appearance.lyrics_font, folia_store::FontSettings::default());
    let store = Store::at(dir.path());
    fs::write(dir.path().join("appearance.toml"), "schema_version = 2\ntheme = \"light\"\nlyrics_font_scale = 1.4\n")
        .unwrap();
    let migrated = AppearanceSettings::load(&store).unwrap();
    assert_eq!(migrated.wall, folia_store::WallAppearance::default());
    assert_eq!(migrated.lyrics_font_scale, 1.4);
    fs::write(
        dir.path().join("appearance.toml"),
        "schema_version = 3\ntheme = \"light\"\n[wall]\nradio = true\nintensity = 0.7\nlights_on = false\n",
    )
    .unwrap();
    let migrated = AppearanceSettings::load(&store).unwrap();
    assert!(migrated.wall.radio && migrated.wall.auto_focus && migrated.wall.vignette);
    assert!(!migrated.wall.lights_on && !migrated.wall.badge_current_only);
    assert_eq!(migrated.wall.intensity, 0.7);
}

#[test]
fn subtitle_layouts_follow_the_original_row_rules() {
    let layout = SubtitleLayout::default();
    assert_eq!(layout.as_str(), "translation");
    assert_eq!(layout.count(), 1);
    assert_eq!(layout.rows(), vec![RowSource::Translation]);

    let two = layout.with_count(2);
    assert_eq!(two.as_str(), "current+translation");
    let three = two.with_count(3);
    assert_eq!(three.as_str(), "current+translation+romanization");
    // Shrinking keeps a translation or romanization row; from a current+next layout it falls back to current.
    assert_eq!(three.with_count(1).as_str(), "translation");
    let next = SubtitleLayout::parse("current+next").unwrap();
    assert_eq!(next.with_count(1).as_str(), "current");
    assert_eq!(next.with_row(1, RowSource::Translation).as_str(), "current+translation");
    // Row 0 only changes in a single-row layout.
    assert_eq!(next.with_row(0, RowSource::Translation).as_str(), "current+next");
    assert_eq!(SubtitleLayout::parse("translation+next"), None);
    assert_eq!(SubtitleLayout::parse("current+next+translation+bogus"), None);
}

#[test]
fn visualizer_tables_keep_style_keys() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::at(dir.path());
    let mut settings = VisualizerSettings::default();
    let mut table = toml::map::Map::new();
    table.insert("wordSpacing".to_owned(), toml::Value::Float(1.25));
    settings.insert_style("classic_tuning", table, &["cadenze_tuning"]);
    store.save(&settings).unwrap();
    let text = fs::read_to_string(dir.path().join("visualizer.toml")).unwrap();
    assert!(text.starts_with("schema_version = 1\n"));
    assert!(text.contains("[classic_tuning]"));
    assert!(text.contains("wordSpacing = 1.25"));
    let loaded = store.load::<VisualizerSettings>().unwrap();
    assert_eq!(loaded, settings);
}
