// Checks the theme domain: the random pool, the custom pair and the light/dark choice, and the appearance domain's
// animation intensity.

use folia_core::AnimationIntensity;
use folia_store::{AppearanceSettings, Domain, DualTheme, Store, ThemeSettings, ThemeSource};

#[test]
fn theme_settings_round_trip_pool_and_custom_pair() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::at(dir.path());
    let mut settings = ThemeSettings::default();
    settings.random_pool = Some(vec!["nebbia".to_owned(), "lumiere".to_owned()]);
    settings.transparent_player_background = true;
    store.save(&settings).unwrap();

    let read: ThemeSettings = store.load().unwrap();
    assert_eq!(read.random_pool.as_deref(), Some(&["nebbia".to_owned(), "lumiere".to_owned()][..]));
    assert!(read.transparent_player_background);
    assert!(dir.path().join(ThemeSettings::FILE).is_file());

    let mut theme = DualTheme::default();
    theme.dark.accent = folia_core::Rgb::hex(0x12_34_56);
    theme.light.name = "Mine".to_owned();
    theme.dark.name = "Mine".to_owned();
    settings.custom_dual_theme = Some(theme);
    settings.source = ThemeSource::Custom;
    store.save(&settings).unwrap();

    let read: ThemeSettings = store.load().unwrap();
    assert_eq!(read.source, ThemeSource::Custom);
    assert_eq!(read.custom_dual_theme.as_ref().unwrap().dark.accent, folia_core::Rgb::hex(0x12_34_56));
    assert_eq!(read.custom_dual_theme.unwrap().light.name, "Mine");
}

#[test]
fn animation_intensity_round_trips_in_appearance() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::at(dir.path());
    let mut appearance = AppearanceSettings::default();
    appearance.animation_intensity = AnimationIntensity::Chaotic;
    store.save(&appearance).unwrap();

    let read: AppearanceSettings = store.load().unwrap();
    assert_eq!(read.animation_intensity, AnimationIntensity::Chaotic);
    assert!(dir.path().join(AppearanceSettings::FILE).is_file());
}
