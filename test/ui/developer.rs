// Offscreen shots of the developer page, the session log overlay and the memory monitor, in both themes.

use std::time::{Duration, SystemTime};

use folia_core::{Language, Theme};
use folia_i18n::I18n;
use folia_store::{DeveloperSettings, LogMode, MemorySample, RuntimeLog};
use folia_ui::developer::{self, Developer};
use folia_ui::{Tokens, install_fonts};
use iced::advanced::renderer::Headless;
use iced::widget::container;
use iced::{Alignment, Color, Element, Pixels, Size, Theme as IcedTheme};
use iced_runtime::user_interface::{Cache, UserInterface};
use tracing::Level;

#[test]
#[cfg_attr(feature = "ci", ignore = "needs a GPU; CI runners only have a software renderer")]
fn developer_page_and_overlays_are_painted_in_both_themes() {
    let i18n = I18n::new(Language::ZhCn).unwrap();
    let output = tempfile::tempdir().unwrap();
    let dir = output.path();
    std::fs::create_dir_all(&dir).unwrap();
    let log_path = dir.join("runtime.log");
    let memory_path = dir.join("memory.log");

    let log = RuntimeLog::new(log_path);
    log.configure(true, false, LogMode::Append);
    for index in 0..24 {
        let (level, target) = match index % 5 {
            0 => (Level::WARN, "folia_ui::library"),
            1 => (Level::ERROR, "folia_player::output"),
            2 => (Level::DEBUG, "folia_library::scan"),
            _ => (Level::INFO, "folia_ui::playback"),
        };
        log.push(level, target, &format!("sample line {index}: track loaded, queue ready"));
    }
    let mut developer = Developer::default();
    developer.refresh_log(&log);
    developer.log_open = true;
    developer.memory_open = true;
    developer.samples = memory_samples(40);
    developer.latest = developer.samples.last().cloned();

    let settings = DeveloperSettings {
        runtime_log_enabled: true,
        runtime_log_mode: LogMode::Append,
        console_capture_enabled: true,
        memory_monitor_enabled: true,
        memory_log_enabled: true,
        memory_log_mode: LogMode::Overwrite,
        memory_interval_ms: 2000,
    };

    for (name, theme, daylight) in [("day", Theme::DAYLIGHT, true), ("night", Theme::MIDNIGHT, false)] {
        let tokens = Tokens::new(&theme, daylight);
        let page = developer::page(&i18n, tokens, &developer, &settings, &log, &memory_path);
        let page = container(page).width(768).padding(32).style(move |_| iced::widget::container::Style {
            background: Some(
                Color::from_rgb8(
                    if daylight { 0xf3 } else { 0x1a },
                    if daylight { 0xf3 } else { 0x1a },
                    if daylight { 0xf4 } else { 0x1d },
                )
                .into(),
            ),
            ..iced::widget::container::Style::default()
        });
        let shot = paint(
            page.into(),
            Size::new(900.0, 800.0),
            Color::from_rgb8(
                if daylight { 0xe9 } else { 0x14 },
                if daylight { 0xe9 } else { 0x14 },
                if daylight { 0xeb } else { 0x16 },
            ),
        );
        std::fs::write(dir.join(format!("developer-page-{name}.png")), encode(&shot, 900, 800)).unwrap();

        let overlay = developer::log_overlay(&i18n, tokens, &developer, &settings, Size::new(1200.0, 800.0)).unwrap();
        let overlay = container(overlay)
            .width(iced::Length::Fill)
            .height(iced::Length::Fill)
            .align_x(Alignment::End)
            .padding(iced::Padding { top: 16.0, right: 16.0, bottom: 0.0, left: 0.0 });
        let shot = paint(overlay.into(), Size::new(1200.0, 800.0), Color::from_rgb8(64, 140, 204));
        std::fs::write(dir.join(format!("developer-log-{name}.png")), encode(&shot, 1200, 800)).unwrap();

        let overlay =
            developer::memory_overlay(&i18n, tokens, &developer, &settings, &memory_path, Size::new(1200.0, 800.0))
                .unwrap();
        let overlay = container(overlay)
            .width(iced::Length::Fill)
            .height(iced::Length::Fill)
            .align_x(Alignment::End)
            .padding(iced::Padding { top: 72.0, right: 48.0, bottom: 0.0, left: 0.0 });
        let shot = paint(overlay.into(), Size::new(1200.0, 800.0), Color::from_rgb8(64, 140, 204));
        std::fs::write(dir.join(format!("developer-memory-{name}.png")), encode(&shot, 1200, 800)).unwrap();
    }
}

fn memory_samples(count: usize) -> Vec<MemorySample> {
    let now = SystemTime::now();
    (0..count)
        .map(|index| {
            let working = 118 + (index as u64 / 4);
            MemorySample {
                at: now - Duration::from_secs((count - index) as u64),
                working_set_mb: working,
                private_mb: Some(92 + index as u64 / 6),
                cpu_percent: 1.2 + (index % 5) as f32 * 0.2,
                peak_mb: working,
                floor_mb: 118,
                avg_mb: 120,
                samples: index as u64 + 1,
                uptime_sec: (index * 2) as u64,
                pid: 4242,
                name: "folia".to_owned(),
            }
        })
        .collect()
}

fn paint<M>(root: Element<'_, M>, size: Size, clear: Color) -> Vec<u8> {
    install_fonts();
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

fn encode(rgba: &[u8], width: u32, height: u32) -> Vec<u8> {
    let image = image::RgbaImage::from_raw(width, height, rgba.to_vec()).unwrap();
    let mut png = Vec::new();
    image.write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png).unwrap();
    png
}
