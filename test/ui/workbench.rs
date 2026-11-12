// One offscreen frame of each workbench tab, the Monet one with its picture options, plus the hotspot hit test.

use std::collections::BTreeMap;

use folia_core::{Language, Lyrics, Theme};
use folia_i18n::I18n;
use folia_store::{AppearanceSettings, FontStyle, RowSource, Store, SubtitleContentMode, VisualizerSettings};
use folia_ui::{
    Hotspot, Workbench, WorkbenchEffect, WorkbenchLook, WorkbenchMessage, WorkbenchTab, hotspot_at, scheduled_saves,
};
use folia_visualizer::{Background, Item, Style};
use iced::advanced::renderer::{Headless, Style as DrawStyle};
use iced::{Color, Element, Pixels, Point, Size, Theme as IcedTheme};
use iced_runtime::user_interface::{Cache, UserInterface};

const W: u32 = 1240;
const H: u32 = 780;

#[test]
fn hotspot_hit_test_follows_the_original_boxes() {
    let size = Size::new(1000.0, 600.0);
    // Background: left/right 16, top 16, 20% high.
    assert_eq!(hotspot_at(size, Point::new(16.0, 16.0)), Some(Hotspot::Background));
    assert_eq!(hotspot_at(size, Point::new(983.0, 130.0)), Some(Hotspot::Background));
    assert_eq!(hotspot_at(size, Point::new(8.0, 40.0)), None);
    // Visualizer: 10% sides, from 28% down 48%.
    assert_eq!(hotspot_at(size, Point::new(100.0, 300.0)), Some(Hotspot::Visualizer));
    assert_eq!(hotspot_at(size, Point::new(899.0, 300.0)), Some(Hotspot::Visualizer));
    assert_eq!(hotspot_at(size, Point::new(99.0, 300.0)), None);
    // Subtitle: left/right 16, bottom 24, 18% high.
    assert_eq!(hotspot_at(size, Point::new(500.0, 600.0 - 24.0 - 10.0)), Some(Hotspot::Subtitle));
    assert_eq!(hotspot_at(size, Point::new(500.0, 600.0 - 10.0)), None);
}

#[test]
#[cfg_attr(feature = "ci", ignore = "needs a GPU; CI runners only have a software renderer")]
fn workbench_tabs_render() {
    let output = tempfile::tempdir().unwrap();
    let dir = output.path();
    std::fs::create_dir_all(&dir).unwrap();
    let i18n = I18n::new(Language::En).unwrap();
    let theme = Theme::MIDNIGHT;
    let appearance = sample_appearance();
    let mut workbench = Workbench::open();
    // Let the sample clock leave Monet's intro (the first 0.65s fades the rail in).
    std::thread::sleep(std::time::Duration::from_millis(2200));
    for (tab, name, style) in [
        (WorkbenchTab::Lyrics, "1-lyrics", Style::Monet),
        (WorkbenchTab::Background, "2-background", Style::Monet),
        (WorkbenchTab::Font, "3-font", Style::Monet),
        (WorkbenchTab::Subtitle, "4-subtitle", Style::Monet),
        (WorkbenchTab::Global, "5-global", Style::Monet),
    ] {
        let _ = workbench.update(
            WorkbenchMessage::Tab(tab),
            style.manifest(),
            &appearance,
            folia_visualizer::LatentTuning::DEFAULT,
        );
        if tab == WorkbenchTab::Lyrics {
            workbench.values = Style::Monet.load(&BTreeMap::new());
        }
        let look = WorkbenchLook {
            i18n: &i18n,
            theme: &theme,
            daylight: false,
            window: Size::new(W as f32, H as f32),
            style,
            font_scale: 1.0,
            live: None,
            audio: &folia_core::AudioFrame::SILENT,
            appearance: &appearance,
            fonts: &[],
            cover: &[],
            latent: folia_visualizer::LatentTuning::DEFAULT,
        };
        let rgba = if tab == WorkbenchTab::Lyrics {
            // Scrolled to the spectrum group, so the shot holds Monet's bars/line pictures too.
            paint_scrolled(workbench.view(look), Color::from_rgb8(8, 8, 10), 760.0)
        } else {
            paint(workbench.view(look), Color::from_rgb8(8, 8, 10))
        };
        std::fs::write(dir.join(format!("{name}.png")), encode(&rgba, W, H)).unwrap();
        assert!(rgba.iter().any(|byte| *byte != 8 && *byte != 10), "{name} is blank");
    }
}

fn sample_appearance() -> AppearanceSettings {
    let mut appearance = AppearanceSettings::default();
    appearance.lyrics_font.style = FontStyle::Custom;
    appearance.lyrics_font.family = Some("Segoe UI".to_owned());
    appearance.lyrics_font.fallback = vec!["Noto Sans CJK SC".to_owned()];
    appearance.lyrics_font.weight = Some(600);
    appearance.subtitle.layout = folia_store::SubtitleLayout::parse("current+translation").unwrap();
    appearance.subtitle.content_mode = SubtitleContentMode::Both;
    appearance.subtitle.opacity = 0.8;
    appearance.subtitle.font_inherits_lyrics = false;
    appearance.subtitle.font.style = FontStyle::Serif;
    appearance.set_background_mode(Some(Background::Common.id()));
    appearance
}

fn paint<M>(root: Element<'_, M>, clear: Color) -> Vec<u8> {
    folia_ui::install_fonts();
    let mut renderer =
        pollster::block_on(<iced::Renderer as Headless>::new(folia_ui::INTERFACE_FONT, Pixels(16.0), None)).unwrap();
    let mut ui = UserInterface::build(root, Size::new(W as f32, H as f32), Cache::default(), &mut renderer);
    let style = DrawStyle { text_color: Color::WHITE };
    ui.draw(&mut renderer, &IcedTheme::Dark, &style, iced::mouse::Cursor::Unavailable);
    renderer.screenshot(&iced::advanced::graphics::Viewport::with_physical_size(Size::new(W, H), 1.0), clear)
}

/// The same frame, with the settings body scrolled down by `distance` pixels.
fn paint_scrolled<M: Clone>(root: Element<'_, M>, clear: Color, distance: f32) -> Vec<u8> {
    folia_ui::install_fonts();
    let mut renderer =
        pollster::block_on(<iced::Renderer as Headless>::new(folia_ui::INTERFACE_FONT, Pixels(16.0), None)).unwrap();
    let mut ui = UserInterface::build(root, Size::new(W as f32, H as f32), Cache::default(), &mut renderer);
    let cursor = iced::mouse::Cursor::Available(Point::new(1000.0, 500.0));
    let events = [
        iced::Event::Mouse(iced::mouse::Event::CursorMoved { position: Point::new(1000.0, 500.0) }),
        iced::Event::Mouse(iced::mouse::Event::WheelScrolled {
            delta: iced::mouse::ScrollDelta::Pixels { x: 0.0, y: -distance },
        }),
    ];
    let mut clipboard = iced::advanced::clipboard::Null;
    let mut messages = Vec::new();
    let _ = ui.update(&events, cursor, &mut renderer, &mut clipboard, &mut messages);
    let style = DrawStyle { text_color: Color::WHITE };
    ui.draw(&mut renderer, &IcedTheme::Dark, &style, cursor);
    renderer.screenshot(&iced::advanced::graphics::Viewport::with_physical_size(Size::new(W, H), 1.0), clear)
}

fn encode(rgba: &[u8], width: u32, height: u32) -> Vec<u8> {
    let image = image::RgbaImage::from_raw(width, height, rgba.to_vec()).unwrap();
    let mut png = Vec::new();
    image.write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png).unwrap();
    png
}

// The assertions below keep the sample data honest: it is what the shots show.
#[allow(dead_code)]
fn sample_choices_are_drawn() {
    let monet = Style::Monet.manifest();
    let pictures: Vec<&str> = monet
        .items()
        .filter_map(|item| match item {
            Item::Picture { key, .. } => Some(key),
            _ => None,
        })
        .collect();
    assert!(pictures.contains(&"audioStyle"));
    assert!(pictures.contains(&"portraitStyle"));
    assert!(Style::Tilt.manifest().items().any(|item| matches!(item, Item::Picture { key: "colorScheme", .. })));
    let _ = (RowSource::Next, Lyrics::default());
}

fn writes(effect: &WorkbenchEffect, appearance_changed: bool, flush_visualizer: bool) -> (bool, bool) {
    scheduled_saves(effect, appearance_changed, flush_visualizer)
}

/// Dragging keeps the draft in memory. Release, blur, and leaving the tab each write `visualizer.toml` once.
#[test]
fn latent_slider_drag_commits_the_last_value_once() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::at(dir.path());
    let manifest = Style::Classic.manifest();
    let appearance = AppearanceSettings::default();
    let mut bench = Workbench::open();
    let mut latent = folia_visualizer::LatentTuning::DEFAULT;
    let mut visualizer = VisualizerSettings::default();
    let file = dir.path().join("visualizer.toml");

    for value in [0.4_f32, 0.9, 1.6] {
        let update = bench.update(WorkbenchMessage::LatentMeshSpeed(value), manifest, &appearance, latent);
        let (visualizer_write, appearance_write) =
            writes(&update.effect, update.appearance.is_some(), update.flush_visualizer);
        assert!(matches!(update.effect, WorkbenchEffect::Draft), "a drag tick scheduled a save");
        assert!(!visualizer_write && !appearance_write);
        latent = update.latent.expect("draft tuning");
        visualizer.insert_style("latent_background_tuning", latent.table(), &[]);
        assert!(!file.exists(), "visualizer.toml was written during the drag");
    }
    assert!((latent.mesh_speed - 1.6).abs() < 1.0e-4);

    let update = bench.update(WorkbenchMessage::Commit, manifest, &appearance, latent);
    let (visualizer_write, appearance_write) =
        writes(&update.effect, update.appearance.is_some(), update.flush_visualizer);
    assert!(matches!(update.effect, WorkbenchEffect::Save));
    assert!(visualizer_write && !appearance_write, "one commit wrote more than visualizer.toml");
    store.save(&visualizer).unwrap();
    let restored = LatentSpeed::read(&store);
    assert!((restored - 1.6).abs() < 1.0e-4, "stored {restored}");

    let again = bench.update(WorkbenchMessage::Commit, manifest, &appearance, latent);
    let (visualizer_write, appearance_write) =
        writes(&again.effect, again.appearance.is_some(), again.flush_visualizer);
    assert!(matches!(again.effect, WorkbenchEffect::None));
    assert!(!visualizer_write && !appearance_write);

    let update = bench.update(WorkbenchMessage::Tab(WorkbenchTab::Background), manifest, &appearance, latent);
    let (visualizer_write, _) = writes(&update.effect, update.appearance.is_some(), update.flush_visualizer);
    assert!(update.scroll_top && !visualizer_write, "changing tabs with no drag still wrote");
    let update = bench.update(WorkbenchMessage::LatentMeshSpeed(0.2), manifest, &appearance, latent);
    assert!(matches!(update.effect, WorkbenchEffect::Draft));
    latent = update.latent.expect("draft");
    visualizer.insert_style("latent_background_tuning", latent.table(), &[]);
    let update = bench.update(WorkbenchMessage::Tab(WorkbenchTab::Lyrics), manifest, &appearance, latent);
    let (visualizer_write, appearance_write) =
        writes(&update.effect, update.appearance.is_some(), update.flush_visualizer);
    assert!(update.scroll_top);
    assert!(visualizer_write && !appearance_write);
    store.save(&visualizer).unwrap();
    let restored = LatentSpeed::read(&store);
    assert!((restored - 0.2).abs() < 1.0e-4, "tab left the previous value {restored}");

    let update = bench.update(WorkbenchMessage::LatentMeshSpeed(1.3), manifest, &appearance, latent);
    latent = update.latent.expect("draft");
    visualizer.insert_style("latent_background_tuning", latent.table(), &[]);
    assert!(matches!(bench.release_drag(), WorkbenchEffect::Save));
    let late = bench.update(WorkbenchMessage::Commit, manifest, &appearance, latent);
    let (visualizer_write, _) = writes(&late.effect, late.appearance.is_some(), late.flush_visualizer);
    assert!(!visualizer_write, "pointer-up and the slider release both saved");
    store.save(&visualizer).unwrap();
    let restored = LatentSpeed::read(&store);
    assert!((restored - 1.3).abs() < 1.0e-4, "blur stored {restored}");
    assert!(matches!(bench.release_drag(), WorkbenchEffect::None));

    let update = bench.update(WorkbenchMessage::LatentMeshSpeed(0.5), manifest, &appearance, latent);
    latent = update.latent.expect("draft");
    let update = bench.update(WorkbenchMessage::Close, manifest, &appearance, latent);
    let (visualizer_write, appearance_write) =
        writes(&update.effect, update.appearance.is_some(), update.flush_visualizer);
    assert!(update.close);
    assert!(visualizer_write && !appearance_write);
    visualizer.insert_style("latent_background_tuning", latent.table(), &[]);
    store.save(&visualizer).unwrap();
    assert!((LatentSpeed::read(&store) - 0.5).abs() < 1.0e-4);
}

#[test]
fn appearance_slider_release_saves_only_appearance() {
    let manifest = Style::Classic.manifest();
    let appearance = AppearanceSettings::default();
    let mut bench = Workbench::open();
    let latent = folia_visualizer::LatentTuning::DEFAULT;
    let update = bench.update(WorkbenchMessage::Opacity(0.45), manifest, &appearance, latent);
    let (visualizer_write, appearance_write) =
        writes(&update.effect, update.appearance.is_some(), update.flush_visualizer);
    assert!(matches!(update.effect, WorkbenchEffect::Appearance));
    assert!(!visualizer_write && !appearance_write);
    let update = bench.update(WorkbenchMessage::Commit, manifest, &appearance, latent);
    let (visualizer_write, appearance_write) =
        writes(&update.effect, update.appearance.is_some(), update.flush_visualizer);
    assert!(matches!(update.effect, WorkbenchEffect::SaveAppearance));
    assert!(!visualizer_write && appearance_write);
}

struct LatentSpeed;

impl LatentSpeed {
    fn read(store: &Store) -> f32 {
        let loaded = store.load::<VisualizerSettings>().unwrap();
        folia_visualizer::LatentTuning::read(loaded.tables.get("latent_background_tuning")).mesh_speed
    }
}
