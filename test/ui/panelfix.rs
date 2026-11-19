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
        background: folia_visualizer::Background::Latent,
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
        catalog: folia_omni::Capabilities::default(),
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
        scene: None,
        has_lyrics: false,
        font_scale: 1.0,
        drawn_style: Style::Classic,
        origin: None,
        art: None::<&Handle>,
        pill_hover: false,
        platform: None,
        account_controls: true,
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
        transparent_background: false,
        transparent_surface: false,
        has_custom_theme: false,
        can_generate_theme: false,
        generating_theme: false,
        random_pool: None,
        all_styles: &Style::ALL,
        background: folia_visualizer::Background::Latent,
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

// Native service wording and concise workbench hints, in en/zh-CN/in/vi order.
const NATIVE_LOCALE_COPY: &[(&str, [&str; 4])] = &[
    (
        "options.configureAiApiKey",
        ["Open AI service settings", "前往 AI 服务设置", "Buka pengaturan layanan AI", "Mở cài đặt dịch vụ AI"],
    ),
    (
        "options.enablePlayerPageNativeBlurDesc",
        [
            "Applies while the player background is opaque. Moving the window may be less smooth.",
            "在播放页的非透明模式下生效，移动窗口时可能卡顿。",
            "Berlaku saat latar pemutar tidak transparan. Pergerakan jendela mungkin kurang mulus.",
            "Áp dụng khi nền trang phát không trong suốt. Di chuyển cửa sổ có thể kém mượt hơn.",
        ],
    ),
    (
        "options.gpuPreferenceCommandLineNotice",
        [
            "WGPU_POWER_PREF overrides the saved GPU preference for this session",
            "本次启动设置了 WGPU_POWER_PREF，以环境变量指定的显卡偏好为准",
            "WGPU_POWER_PREF menimpa preferensi GPU tersimpan untuk sesi ini",
            "WGPU_POWER_PREF ghi đè lựa chọn GPU đã lưu trong phiên này",
        ],
    ),
    (
        "options.nativeBlurConfirmAction",
        ["Enable system blur", "开启系统毛玻璃", "Aktifkan blur sistem", "Bật nền mờ hệ thống"],
    ),
    (
        "options.nativeBlurConfirmDesc",
        [
            "The player will use a translucent system blur in place of the shared Folia background. This effect can increase rendering cost.",
            "开启后，播放页将使用系统毛玻璃半透明背景，替换 Folia 的共用背景。该效果可能增加渲染开销。",
            "Pemutar akan menggunakan blur sistem tembus pandang sebagai pengganti latar bersama Folia. Efek ini dapat meningkatkan biaya render.",
            "Trang phát sẽ dùng nền mờ hệ thống bán trong suốt thay cho nền chung của Folia. Hiệu ứng này có thể tăng chi phí dựng hình.",
        ],
    ),
    (
        "options.smartBeatDesc_cached",
        [
            "Use saved beat and stem analysis. Tracks without saved results keep their original visuals.",
            "使用已保存的节拍与音轨分析结果，未分析曲目保持原来的画面。",
            "Gunakan analisis ketukan dan stem tersimpan. Lagu tanpa hasil tersimpan mempertahankan tampilan aslinya.",
            "Dùng kết quả phân tích nhịp và stem đã lưu. Bài chưa có kết quả giữ hình ảnh gốc.",
        ],
    ),
    (
        "options.smartBeatDesc_off",
        [
            "Keep the original spectrum response in every visualizer.",
            "所有画面保持原来的频谱响应。",
            "Pertahankan respons spektrum asli pada semua visualizer.",
            "Giữ phản ứng phổ âm gốc cho mọi kiểu hiển thị.",
        ],
    ),
    (
        "options.smartBeatDesc_on",
        [
            "Drive each enabled motion effect with musical analysis. Beat This! finds beats, and HTDemucs separates drums and vocals. Models run in the background when a visible effect needs them, and completed results are saved.",
            "各歌词动画中的动感效果可独立开关和调节强度。Beat This! 识别节拍，HTDemucs 分离鼓声与人声。画面上有效果需要时才在后台分析，完整结果会保存供以后复用。",
            "Gerakkan setiap efek aktif dengan analisis musik. Beat This! mencari ketukan, dan HTDemucs memisahkan drum serta vokal. Model berjalan di latar saat efek yang terlihat membutuhkannya. Hasil lengkap disimpan.",
            "Điều khiển từng hiệu ứng đang bật bằng phân tích âm nhạc. Beat This! nhận diện nhịp, HTDemucs tách trống và giọng hát. Mô hình chạy nền khi hiệu ứng đang hiển thị cần dùng, và lưu kết quả hoàn chỉnh.",
        ],
    ),
    (
        "options.beatEffect_nebbiaDrizzleRideDesc",
        [
            "Raindrops follow vocals and the bus moves with the beat. Adjust their intensity separately.",
            "雨滴随人声变化，车身随鼓点起伏，可分别调节强度。",
            "Tetes hujan mengikuti vokal dan bus bergerak mengikuti irama. Atur intensitasnya secara terpisah.",
            "Giọt mưa theo giọng hát, xe chuyển động theo nhịp trống. Có thể chỉnh cường độ riêng.",
        ],
    ),
];

#[test]
fn every_shared_locale_value_matches_the_original() {
    let locales = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..").join("..").join("crates/folia-i18n/locales");
    let fixtures = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../i18n/fixtures/original");
    let mut compared = 0;
    for (locale_index, (locale, rust_file)) in
        [("en", "en.toml"), ("zh-CN", "zh-CN.toml"), ("in", "in.toml"), ("vi", "vi.toml")].into_iter().enumerate()
    {
        let text = std::fs::read_to_string(fixtures.join(format!("{locale}.ts"))).unwrap();
        let mut expected = flatten_json(parse_original(&text));
        let rust: toml::Table = toml::from_str(&std::fs::read_to_string(locales.join(rust_file)).unwrap()).unwrap();
        let rust = flatten_toml(rust);
        for &(key, translations) in NATIVE_LOCALE_COPY {
            assert!(expected.contains_key(key), "{locale} native copy has no original key: {key}");
            assert!(rust.contains_key(key), "{locale} native copy is missing: {key}");
            expected.insert(key.to_owned(), translations[locale_index].to_owned());
        }
        let mut diffs = Vec::new();
        for (key, value) in &rust {
            if let Some(want) = expected.get(key)
                && want != value
            {
                diffs.push(format!("{key}: {want:?} != {value:?}"));
            }
            if expected.contains_key(key) {
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
            assert_eq!(rust.get(key), expected.get(key), "{locale} {key}");
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
