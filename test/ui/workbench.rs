// Workbench interaction, persistence, subtitle selection and upload-control alignment.

use folia_core::{Language, Theme};
use folia_i18n::I18n;
use folia_store::{AppearanceSettings, Store, VisualizerSettings};
use folia_ui::{
    Hotspot, Workbench, WorkbenchEffect, WorkbenchLook, WorkbenchMessage, WorkbenchTab, hotspot_at, scheduled_saves,
};
use folia_visualizer::{Background, Style};
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
#[cfg_attr(feature = "ci", ignore = "needs a GPU")]
fn background_upload_controls_render() {
    let i18n = I18n::new(Language::ZhCn).unwrap();
    let theme = Theme::DAYLIGHT;
    for mode in [Background::Monet, Background::Nomand] {
        let mut appearance = AppearanceSettings::default();
        appearance.set_background_mode(Some(mode.id()));
        let mut workbench = Workbench::open();
        let _ = workbench.update(
            WorkbenchMessage::Tab(WorkbenchTab::Background),
            Style::Classic.manifest(),
            &appearance,
            folia_visualizer::LatentTuning::DEFAULT,
        );
        let capture = |has_background_upload| {
            paint_upload(
                workbench.view(WorkbenchLook {
                    i18n: &i18n,
                    theme: &theme,
                    daylight: true,
                    window: Size::new(W as f32, H as f32),
                    style: Style::Classic,
                    font_scale: 1.0,
                    live: None,
                    audio: &folia_core::AudioFrame::SILENT,
                    appearance: &appearance,
                    fonts: &[],
                    dpi: 1.0,
                    cover: &[],
                    latent: folia_visualizer::LatentTuning::DEFAULT,
                    sora: folia_visualizer::SoraTuning::DEFAULT,
                    monet: folia_visualizer::monet_background::Tuning::DEFAULT,
                    nomand: folia_visualizer::nomand::Tuning::default(),
                    picture: None,
                    has_background_upload,
                }),
                Color::WHITE,
            )
        };
        let disabled = capture(false);
        let enabled = capture(true);
        let changed = disabled
            .chunks_exact(4)
            .zip(enabled.chunks_exact(4))
            .enumerate()
            .filter(|(index, (a, b))| index % W as usize > 820 && a != b)
            .count();
        assert!(changed > 20, "clear control did not show its disabled state");
        if let Some(path) = std::env::var_os("FOLIA_PANEL_CAPTURE") {
            std::fs::create_dir_all(&path).unwrap();
            image::RgbaImage::from_raw(W, H, disabled)
                .unwrap()
                .save(std::path::PathBuf::from(path).join(format!("{}-upload.png", mode.id())))
                .unwrap();
        }
    }
}

fn paint_upload<M>(root: Element<'_, M>, clear: Color) -> Vec<u8> {
    folia_ui::install_fonts();
    let mut renderer =
        pollster::block_on(<iced::Renderer as Headless>::new(folia_ui::INTERFACE_FONT, Pixels(16.0), None)).unwrap();
    let mut ui = UserInterface::build(root, Size::new(W as f32, H as f32), Cache::default(), &mut renderer);
    let mut alignment = UploadAlignment::default();
    ui.operate(&renderer, &mut alignment);
    assert_eq!(alignment.0.len(), 2, "upload row label and button text must both be measurable");
    assert!(
        (alignment.0[0].center_y() - alignment.0[1].center_y()).abs() <= 1.0,
        "upload button text must align with its row label: {:?}",
        alignment.0
    );
    let style = DrawStyle { text_color: Color::WHITE };
    ui.draw(&mut renderer, &IcedTheme::Dark, &style, iced::mouse::Cursor::Unavailable);
    renderer.screenshot(&iced::advanced::graphics::Viewport::with_physical_size(Size::new(W, H), 1.0), clear)
}

#[derive(Default)]
struct UploadAlignment(Vec<iced::Rectangle>);

impl iced::advanced::widget::Operation for UploadAlignment {
    fn traverse(&mut self, operate: &mut dyn FnMut(&mut dyn iced::advanced::widget::Operation)) {
        operate(self);
    }
    fn text(&mut self, _id: Option<&iced::widget::Id>, bounds: iced::Rectangle, text: &str) {
        if text == "上传背景图" && bounds.x > 820.0 {
            self.0.push(bounds);
        }
    }
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

#[test]
fn subtitle_rows_follow_the_selected_layout_and_skip_duplicates() {
    use folia_store::{RowSource, SubtitleLayout, SubtitleSettings};
    let lines = vec![
        folia_core::Line {
            text: "原文".into(),
            start: 0.0,
            end: 5.0,
            translation: Some("Translation".into()),
            romanization: Some("Romaji".into()),
            ..Default::default()
        },
        folia_core::Line { text: "......".into(), start: 5.0, end: 6.0, ..Default::default() },
        folia_core::Line { text: "下一句".into(), start: 6.0, end: 9.0, ..Default::default() },
        folia_core::Line { text: "再下一句".into(), start: 9.0, end: 12.0, ..Default::default() },
    ];
    let mut settings = SubtitleSettings {
        layout: SubtitleLayout::parse("current+translation+romanization").unwrap(),
        ..Default::default()
    };
    let rows = folia_ui::subtitle_rows_at(&lines, 2.0, &settings);
    assert_eq!(rows.rows.map(|row| row.map(|row| row.text)), [Some("原文"), Some("Translation"), Some("Romaji")]);
    settings.layout = SubtitleLayout::parse("current+next+next").unwrap();
    let rows = folia_ui::subtitle_rows_at(&lines, 2.0, &settings);
    assert_eq!(rows.rows.map(|row| row.map(|row| row.text)), [Some("原文"), Some("下一句"), Some("再下一句")]);
    let mut duplicate = lines.clone();
    duplicate[0].translation = Some(" 原文 ".into());
    settings.layout = SubtitleLayout::parse("current+translation").unwrap();
    let rows = folia_ui::subtitle_rows_at(&duplicate, 2.0, &settings);
    assert_eq!(rows.rows.iter().flatten().count(), 1);
    settings.layout = SubtitleLayout::parse("romanization").unwrap();
    duplicate[0].romanization = None;
    let rows = folia_ui::subtitle_rows_at(&duplicate, 2.0, &settings);
    assert!(rows.fallback);
    assert_eq!(rows.rows[0].unwrap().source, RowSource::Next);
    settings.hide_translation = true;
    assert!(folia_ui::subtitle_rows_at(&lines, 2.0, &settings).rows.iter().all(Option::is_none));
}

impl LatentSpeed {
    fn read(store: &Store) -> f32 {
        let loaded = store.load::<VisualizerSettings>().unwrap();
        folia_visualizer::LatentTuning::read(loaded.tables.get("latent_background_tuning")).mesh_speed
    }
}
