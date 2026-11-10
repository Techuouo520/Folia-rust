// Checks settings files: defaults, round trips, file layout and schema version checks.

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
fn save_then_load() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::at(dir.path().join("nested"));
    let settings = GeneralSettings {
        language: LanguagePreference::Fixed(Language::Vi),
        home_tab: HomeTab::Radio,
        online_provider: None,
        wall_tools_queue_island_open: false,
    };

    store.save(&settings).unwrap();
    assert_eq!(store.load::<GeneralSettings>().unwrap(), settings);

    let text = fs::read_to_string(dir.path().join("nested/general.toml")).unwrap();
    assert_eq!(
        text,
        "schema_version = 1\nlanguage = \"vi\"\nhome_tab = \"radio\"\nwall_tools_queue_island_open = false\n"
    );
    assert!(!dir.path().join("nested/general.toml.tmp").exists());
}

#[test]
fn rejects_other_schema_versions() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("general.toml"), "schema_version = 9\nlanguage = \"en\"\n").unwrap();
    let err = Store::at(dir.path()).load::<GeneralSettings>().unwrap_err();
    assert!(matches!(err, StoreError::SchemaVersion { found: 9, expected: 1, .. }), "{err}");
}

#[test]
fn rejects_malformed_files() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("general.toml"), "schema_version = 1\nlanguage = 42\n").unwrap();
    let err = Store::at(dir.path()).load::<GeneralSettings>().unwrap_err();
    assert!(matches!(err, StoreError::Parse { .. }), "{err}");
}

#[test]
fn playback_keeps_volume_when_quality_is_missing() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("playback.toml"), "schema_version = 1\nvolume = 0.35\nrepeat = \"all\"\n").unwrap();
    let settings: PlaybackSettings = Store::at(dir.path()).load().unwrap();
    assert_eq!(settings.volume, 0.35);
    assert_eq!(settings.repeat, folia_core::Repeat::All);
    assert_eq!(settings.audio_quality, AudioQuality::High);
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
    let appearance: AppearanceSettings = Store::at(dir.path()).load().unwrap();
    assert_eq!(appearance.lyrics_font_scale, 1.1);
    assert_eq!(appearance.visualizer, "monet");
    assert_eq!(appearance.stored_background_mode(), None);
    assert_eq!(appearance.subtitle, folia_store::SubtitleSettings::default());
    assert_eq!(appearance.lyrics_font, folia_store::FontSettings::default());
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
