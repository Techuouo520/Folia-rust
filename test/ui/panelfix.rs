// Liked-heart state, closing-panel compositing and original locale values.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use folia_core::{AUDIO_QUALITY_LEVELS, AudioFrame, AudioQuality, Language, Repeat, Theme};
use folia_i18n::I18n;
use folia_store::{ReplayGainMode, ThemeSource};
use folia_ui::{PanelMessage, PanelRoom, PanelView, PlayerPanel, like_look};
use folia_visualizer::Style;
use iced::advanced::renderer::Headless;
use iced::widget::image::Handle;
use iced::{Color, Element, Pixels, Size, Theme as IcedTheme};
use iced_runtime::user_interface::{Cache, UserInterface};

#[test]
fn the_heart_is_hollow_or_filled_and_goes_transparent_with_the_panel() {
    let day = Color::from_rgb8(0x18, 0x18, 0x1b);
    let liked = like_look(true, true, false, 1.0, day);
    assert!(liked.filled);
    for (got, want) in [(liked.ink.r, 0xe0), (liked.ink.g, 0x35), (liked.ink.b, 0x4f)] {
        assert!((got * 255.0 - want as f32).abs() < 0.5);
    }
    assert_eq!(liked.ink.a, 1.0);

    let night = like_look(false, true, false, 1.0, day);
    for (got, want) in [(night.ink.r, 0xf2), (night.ink.g, 0x60), (night.ink.b, 0x7a)] {
        assert!((got * 255.0 - want as f32).abs() < 0.5);
    }

    let empty = like_look(false, false, false, 1.0, day);
    assert!(!empty.filled);
    assert!((empty.ink.a - 0.62).abs() < 1e-6);
    for open in [0.0, 0.25, 1.0] {
        for liked in [false, true] {
            for disabled in [false, true] {
                let look = like_look(false, liked, disabled, open, day);
                let want = open * if liked { 1.0 } else { 0.62 } * if disabled { 0.4 } else { 1.0 };
                assert!((look.ink.a - want).abs() < 1e-6, "{look:?} at {open}");
            }
        }
    }
    assert_eq!(like_look(false, true, false, 0.0, day).ink.a, 0.0);
    assert_eq!(like_look(true, false, false, 0.0, day).ink.a, 0.0);
}

#[test]
#[cfg_attr(feature = "ci", ignore = "needs a GPU; CI runners only have a software renderer")]
fn closing_the_panel_takes_every_pixel_back_to_the_page() {
    let now = Instant::now();
    let i18n = I18n::new(Language::ZhCn).unwrap();
    let theme = Theme::MIDNIGHT;
    let empty: Vec<folia_core::Track> = Vec::new();
    let room = || PanelRoom {
        width: 1200.0,
        height: 800.0,
        bottom: 32.0,
        queue: &empty,
        index: None,
        quality_levels: &AUDIO_QUALITY_LEVELS,
        quality: AudioQuality::High,
    };
    let mut panel = PlayerPanel::new(now);
    panel.set_chrome(false, now);
    let _ = panel.update(&PanelMessage::Toggle, "classic", room());
    std::thread::sleep(Duration::from_millis(360));
    assert!(panel.glass(Instant::now()) > 0.99);

    let _ = panel.update(&PanelMessage::AddToPlaylist, "classic", room());
    let _ = panel.update(&PanelMessage::Toggle, "classic", room());
    let mut frames = 0;
    while panel.animating(Instant::now()) && frames < 80 {
        std::thread::sleep(Duration::from_millis(16));
        let _ = panel.update(&PanelMessage::Frame, "classic", room());
        frames += 1;
    }
    let now = Instant::now();
    assert!(!panel.animating(now), "the close animation stopped after {frames} frames");
    assert_eq!(panel.glass(now), 0.0);

    let view = panel_view(&i18n, &theme, &empty);
    let shot = paint(panel.view(view, now), Size::new(1200.0, 800.0), Color::from_rgb8(64, 140, 204));
    let background = shot.chunks_exact(4).all(|pixel| pixel == [64, 140, 204, 255]);
    assert!(background, "the closed panel left paint behind");
}

pub(super) fn panel_view<'a>(i18n: &'a I18n, theme: &'a Theme, queue: &'a [folia_core::Track]) -> PanelView<'a> {
    PanelView {
        i18n,
        theme,
        daylight: false,
        width: 1200.0,
        height: 800.0,
        track: None,
        queue,
        index: None,
        volume: 1.0,
        muted: false,
        lyric_offset: 0.0,
        bottom: 32.0,
        repeat: Repeat::Off,
        style_id: "classic",
        has_lyrics: false,
        font_scale: 1.0,
        drawn_style: Style::Classic,
        origin: None,
        art: None::<&Handle>,
        pill_hover: false,
        platform: None,
        account: None,
        account_avatar: None,
        syncing: false,
        quality_levels: &AUDIO_QUALITY_LEVELS,
        quality: AudioQuality::High,
        modal: false,
        audio: &AudioFrame::SILENT,
        liked: true,
        like_disabled: None,
        can_add: true,
        add_disabled: None,
        quick: &[],
        theme_source: ThemeSource::Default,
        has_custom_theme: false,
        random_pool: None,
        all_styles: &Style::ALL,
        background: folia_visualizer::Background::Latent,
        sora: folia_visualizer::SoraTuning::DEFAULT,
        monet: folia_visualizer::monet_background::Tuning::DEFAULT,
        picture: None,
        nomand: folia_visualizer::nomand::Tuning::default(),
        background_shapes: true,
        background_vignette: true,
        equalizer_on: false,
        replay_gain: ReplayGainMode::default(),
        gain: None,
        can_match: false,
        can_import_lyrics: false,
        can_export_lyrics: false,
        lyric_provider: "",
        can_shuffle: false,
        can_save_queue: false,
    }
}

fn paint<M>(root: Element<'_, M>, size: Size, clear: Color) -> Vec<u8> {
    folia_ui::install_fonts();
    let mut renderer =
        pollster::block_on(<iced::Renderer as Headless>::new(folia_ui::INTERFACE_FONT, Pixels(16.0), None)).unwrap();
    let mut ui = UserInterface::build(root, size, Cache::default(), &mut renderer);
    let style = iced::advanced::renderer::Style { text_color: Color::WHITE };
    ui.draw(&mut renderer, &IcedTheme::Dark, &style, iced::mouse::Cursor::Unavailable);
    renderer.screenshot(
        &iced::advanced::graphics::Viewport::with_physical_size(Size::new(size.width as u32, size.height as u32), 1.0),
        clear,
    )
}

#[test]
fn every_shared_locale_value_matches_the_original() {
    let locales = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..").join("..").join("crates/folia-i18n/locales");
    let fixtures = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../i18n/fixtures/original");
    let mut compared = 0;
    for (locale, rust_file) in [("en", "en.toml"), ("zh-CN", "zh-CN.toml"), ("in", "in.toml"), ("vi", "vi.toml")] {
        let text = std::fs::read_to_string(fixtures.join(format!("{locale}.ts"))).unwrap();
        let original = flatten_json(parse_original(&text));
        let rust: toml::Table = toml::from_str(&std::fs::read_to_string(locales.join(rust_file)).unwrap()).unwrap();
        let rust = flatten_toml(rust);
        let mut diffs = Vec::new();
        for (key, value) in &rust {
            if let Some(want) = original.get(key)
                && want != value
            {
                diffs.push(format!("{key}: {want:?} != {value:?}"));
            }
            if original.contains_key(key) {
                compared += 1;
            }
        }
        assert!(diffs.is_empty(), "{locale}:\n{}", diffs.join("\n"));
        for key in [
            "ui.equalizerEffectsHint",
            "ui.equalizerEffectNoiseTag",
            "ui.equalizerEffectNoiseTagHint",
            "localMusic.cancel",
        ] {
            assert_eq!(rust.get(key), original.get(key), "{locale} {key}");
        }
    }
    assert!(compared > 500, "only {compared} shared keys were compared");
}

fn parse_original(text: &str) -> serde_json::Value {
    let at = text.find("export default").unwrap() + "export default".len();
    let text = text[at..].trim_start();
    let text = text.split("as const;").next().unwrap_or(text);
    let cleaned = strip_trailing_commas(text);
    serde_json::from_str(&cleaned).unwrap()
}

fn strip_trailing_commas(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    let mut in_string = false;
    let mut escaped = false;
    while let Some(ch) = chars.next() {
        if in_string {
            out.push(ch);
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == '"' {
                in_string = false;
            }
            continue;
        }
        if ch == '"' {
            in_string = true;
            out.push(ch);
        } else if ch == ',' {
            let mut look = chars.clone();
            let trailing = loop {
                match look.next() {
                    Some(next) if next.is_whitespace() => continue,
                    Some('}') | Some(']') => break true,
                    _ => break false,
                }
            };
            if !trailing {
                out.push(ch);
            }
        } else {
            out.push(ch);
        }
    }
    out
}

fn flatten_json(value: serde_json::Value) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    walk_json(&value, "", &mut out);
    out
}

fn walk_json(value: &serde_json::Value, prefix: &str, out: &mut BTreeMap<String, String>) {
    match value {
        serde_json::Value::Object(map) => {
            for (key, child) in map {
                let path = if prefix.is_empty() { key.clone() } else { format!("{prefix}.{key}") };
                walk_json(child, &path, out);
            }
        }
        serde_json::Value::String(text) => {
            out.insert(prefix.to_owned(), text.clone());
        }
        other => {
            out.insert(prefix.to_owned(), other.to_string());
        }
    }
}

fn flatten_toml(table: toml::Table) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    walk_toml(table, "", &mut out);
    out
}

fn walk_toml(table: toml::Table, prefix: &str, out: &mut BTreeMap<String, String>) {
    for (key, value) in table {
        let path = if prefix.is_empty() { key } else { format!("{prefix}.{key}") };
        match value {
            toml::Value::Table(child) => walk_toml(child, &path, out),
            toml::Value::String(text) => {
                out.insert(path, text);
            }
            other => {
                out.insert(path, other.to_string());
            }
        }
    }
}
