// Checks that every locale parses, carries the same keys as zh-CN, falls back correctly and fills placeholders.

use std::collections::BTreeSet;

use folia_core::Language;
use folia_i18n::{I18n, load};

#[test]
fn all_locales_share_the_zh_cn_keys() {
    let reference: BTreeSet<String> = load(Language::ZhCn).unwrap().into_keys().collect();
    for language in Language::ALL {
        let keys: BTreeSet<String> = load(language).unwrap().into_keys().collect();
        assert_eq!(keys, reference, "{}", language.code());
    }
}

#[test]
fn traditional_chinese_is_generated() {
    let i18n = I18n::new(Language::ZhTw).unwrap();
    assert_eq!(i18n.t("options.appLanguage"), "界面語言");
    assert_eq!(i18n.t("options.appLanguageZhCN"), "簡體中文");
}

#[test]
fn switches_language() {
    let mut i18n = I18n::new(Language::En).unwrap();
    assert_eq!(i18n.t("options.appLanguage"), "Interface language");
    i18n.set_language(Language::Vi).unwrap();
    assert_eq!(i18n.language(), Language::Vi);
    assert_eq!(i18n.t("options.appLanguage"), "Ngôn ngữ giao diện");
}

#[test]
fn unknown_keys_return_the_key() {
    let i18n = I18n::new(Language::En).unwrap();
    assert_eq!(i18n.t("no.such.key"), "no.such.key");
}

#[test]
fn fills_placeholders() {
    let i18n = I18n::new(Language::En).unwrap();
    assert_eq!(i18n.format("options.settingsSearchCount", &[("n", "3")]), "3 found");
    assert_eq!(i18n.format("options.appLanguageSystemHint", &[("language", "English")]), "Current: English");
    assert_eq!(i18n.format("options.settingsSearchEmpty", &[("n", "3")]), "No matching settings");
}

#[test]
fn provider_switch_copy_formats_all_languages() {
    for language in Language::ALL {
        let i18n = I18n::new(language).unwrap();
        assert_ne!(i18n.t("home.switchOnlineProvider"), "home.switchOnlineProvider");
        let copy = i18n.format("home.confirmOnlineProviderSwitch", &[("provider", "KuGou")]);
        assert!(copy.contains("KuGou"));
        assert!(!copy.contains("{{"));
    }
}

#[test]
fn subscription_failure_names_the_provider_in_every_language() {
    for language in Language::ALL {
        let i18n = I18n::new(language).unwrap();
        let message = i18n.format("status.songSubscriptionRequired", &[("provider", "Spotify")]);
        assert!(message.contains("Spotify"));
        assert!(!message.contains("{{"));
    }
}
