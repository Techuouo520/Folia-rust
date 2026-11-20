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
fn details_finish_their_last_frame_without_resuming_the_preview() {
    let mut bench = Workbench::open();
    let appearance = AppearanceSettings::default();
    let manifest = Style::Nebbia.manifest();
    let latent = folia_visualizer::LatentTuning::DEFAULT;
    bench.update(WorkbenchMessage::Pause, manifest, &appearance, latent);
    assert!(!bench.ticking());
    for message in
        [WorkbenchMessage::ToggleDetails("options.nebbiaBehindHint"), WorkbenchMessage::Tab(WorkbenchTab::Global)]
    {
        let update = bench.update(message, manifest, &appearance, latent);
        assert!(matches!(update.effect, WorkbenchEffect::None));
        assert!(bench.controls_animating());
        assert!(!bench.ticking());
        std::thread::sleep(std::time::Duration::from_millis(350));
        assert!(bench.controls_animating(), "the final control frame must remain pending");
        bench.update(WorkbenchMessage::ControlsFrame, manifest, &appearance, latent);
        assert!(!bench.controls_animating());
        assert!(!bench.ticking());
    }
}

#[test]
#[cfg_attr(feature = "ci", ignore = "needs a GPU for native scene-window layout")]
fn scene_window_seats_and_controls_follow_wide_and_narrow_layouts() {
    use folia_core::{SceneId, SceneQuality};
    use folia_ui::{SceneSettingsLook, SceneSettingsMessage as Change, SceneSettingsTab, SceneSettingsWindow};
    folia_ui::install_fonts();
    let mut renderer =
        pollster::block_on(<iced::Renderer as Headless>::new(folia_ui::INTERFACE_FONT, Pixels(16.0), None)).unwrap();
    let i18n = I18n::new(Language::ZhCn).unwrap();
    let theme = Theme::DAYLIGHT;
    let mut window = SceneSettingsWindow::open();
    let mut settings = folia_store::SceneSettings::default();
    let look = |size| SceneSettingsLook {
        i18n: &i18n,
        theme: &theme,
        daylight: true,
        window: size,
        sound_available: true,
        sound_loading: false,
        sound_error: None,
    };
    let size = Size::new(1200.0, 800.0);
    let mut cache = Cache::default();
    let mut ui = UserInterface::build(window.view(&settings, look(size)), size, cache, &mut renderer);
    let mut bounds = SceneWindowBounds::default();
    ui.operate(&renderer, &mut bounds);
    let point = bounds.text("影厅").center();
    let messages = scene_press(&mut ui, &mut renderer, point);
    assert!(matches!(messages.as_slice(), [Change::Select(Some(SceneId::Cinema))]));
    cache = ui.into_cache();
    settings.scene_id = Some(SceneId::Cinema);
    window.update(&messages[0]);
    let mut ui = UserInterface::build(window.view(&settings, look(size)), size, cache, &mut renderer);
    let mut bounds = SceneWindowBounds::default();
    ui.operate(&renderer, &mut bounds);
    assert!(matches!(
        scene_press(&mut ui, &mut renderer, bounds.text("高").center()).as_slice(),
        [Change::Quality(SceneQuality::High)]
    ));
    let toggles: Vec<_> = bounds
        .boxes
        .iter()
        .filter(|bounds| (bounds.width - 40.0).abs() < 0.01 && (bounds.height - 24.0).abs() < 0.01)
        .copied()
        .collect();
    assert!(matches!(scene_press(&mut ui, &mut renderer, toggles[0].center()).as_slice(), [Change::HouseLights(true)]));
    let screen = bounds.text("荧幕");
    let seats = folia_visualizer::cinema::seats();
    let min_x = seats.iter().map(|seat| seat.eye[0]).fold(f32::INFINITY, f32::min);
    let max_x = seats.iter().map(|seat| seat.eye[0]).fold(f32::NEG_INFINITY, f32::max);
    let min_z = seats.iter().map(|seat| seat.eye[2]).fold(f32::INFINITY, f32::min);
    let scale = (352.0 - 16.0) / (max_x - min_x);
    for occupied in [false, true] {
        let seat = seats.iter().find(|seat| seat.row == 1 && seat.occupied == occupied).unwrap();
        let at = Point::new(
            screen.center_x() - 352.0 / 2.0 + 8.0 + (seat.eye[0] - min_x) * scale,
            screen.y + screen.height + 16.0 + (seat.eye[2] - min_z) * scale,
        );
        let messages = scene_press(&mut ui, &mut renderer, at);
        if occupied {
            assert!(messages.is_empty(), "an occupied seat was selectable");
        } else {
            assert!(matches!(&messages[0], Change::Seat(Some(id)) if id == &seat.id));
        }
    }
    cache = ui.into_cache();
    settings.scene_options.cinema.quality = SceneQuality::High;
    let mut ui = UserInterface::build(window.view(&settings, look(size)), size, cache, &mut renderer);
    let mut bounds = SceneWindowBounds::default();
    ui.operate(&renderer, &mut bounds);
    assert!(matches!(scene_press(&mut ui, &mut renderer, bounds.text("默认").center()).as_slice(), [Change::Reset]));
    cache = ui.into_cache();
    window.update(&Change::Tab(SceneSettingsTab::Sound));
    let unavailable =
        SceneSettingsLook { sound_available: false, sound_error: Some("missing test library"), ..look(size) };
    let mut ui = UserInterface::build(window.view(&settings, unavailable), size, cache, &mut renderer);
    let mut bounds = SceneWindowBounds::default();
    ui.operate(&renderer, &mut bounds);
    assert!(bounds.labels.iter().any(|(label, _)| label == "原生声学运行库不可用"));
    assert!(scene_press(&mut ui, &mut renderer, bounds.text("默认").center()).is_empty());
    window.update(&Change::Tab(SceneSettingsTab::Scene));
    let narrow = Size::new(800.0, 650.0);
    let mut ui = UserInterface::build(window.view(&settings, look(narrow)), narrow, Cache::default(), &mut renderer);
    let mut bounds = SceneWindowBounds::default();
    ui.operate(&renderer, &mut bounds);
    assert!(bounds.text("场景设置").y > 400.0, "narrow controls stayed beside the preview");
    ui.draw(
        &mut renderer,
        &IcedTheme::Light,
        &DrawStyle { text_color: Color::BLACK },
        iced::mouse::Cursor::Unavailable,
    );
    let pixels = renderer
        .screenshot(&iced::advanced::graphics::Viewport::with_physical_size(Size::new(800, 650), 1.0), Color::WHITE);
    assert!(pixels.chunks_exact(4).any(|pixel| pixel[0] < 100 && pixel[1] < 100));
}

#[derive(Default)]
struct SceneWindowBounds {
    labels: Vec<(String, iced::Rectangle)>,
    boxes: Vec<iced::Rectangle>,
}
impl SceneWindowBounds {
    fn text(&self, label: &str) -> iced::Rectangle {
        self.labels
            .iter()
            .find(|(text, _)| text == label)
            .unwrap_or_else(|| panic!("missing {label}: {:?}", self.labels))
            .1
    }
}
impl iced::advanced::widget::Operation for SceneWindowBounds {
    fn traverse(&mut self, operate: &mut dyn FnMut(&mut dyn iced::advanced::widget::Operation)) {
        operate(self);
    }
    fn text(&mut self, _: Option<&iced::widget::Id>, bounds: iced::Rectangle, text: &str) {
        self.labels.push((text.into(), bounds));
    }
    fn container(&mut self, _: Option<&iced::widget::Id>, bounds: iced::Rectangle) {
        if !self.boxes.contains(&bounds) {
            self.boxes.push(bounds);
        }
    }
}
fn scene_press(
    ui: &mut UserInterface<'_, folia_ui::SceneSettingsMessage, iced::Theme, iced::Renderer>,
    renderer: &mut iced::Renderer,
    point: Point,
) -> Vec<folia_ui::SceneSettingsMessage> {
    let mut messages = Vec::new();
    ui.update(
        &[
            iced::Event::Mouse(iced::mouse::Event::ButtonPressed(iced::mouse::Button::Left)),
            iced::Event::Mouse(iced::mouse::Event::ButtonReleased(iced::mouse::Button::Left)),
        ],
        iced::mouse::Cursor::Available(point),
        renderer,
        &mut iced::advanced::clipboard::Null,
        &mut messages,
    );
    messages
}

#[test]
fn workbench_frames_follow_the_selected_limit_and_pause() {
    use std::time::Duration;

    use folia_store::FrameRate;
    use folia_ui::{Message, Shell};
    let temporary = tempfile::tempdir().unwrap();
    let (mut shell, initial) = Shell::new(super::player_fixture::init(temporary.path()));
    drop(initial);
    let window = iced::window::Id::unique();
    drop(shell.update(Message::WindowReady(Some(window))));
    drop(shell.update(Message::OpenWorkbench));
    for rate in FrameRate::ALL {
        drop(shell.update(Message::FrameRate(rate)));
        let expected = if rate.fps().is_some() { 10 } else { 150 };
        let frames = visual_frames(
            shell.subscription(),
            window,
            if rate.fps().is_some() { expected } else { 300 },
            if rate.fps().is_some() { Duration::from_secs(5) } else { Duration::from_millis(250) },
        );
        assert_eq!(frames.len(), expected, "{rate:?} did not keep producing visual frames");
        if let Some(fps) = rate.fps() {
            let elapsed = frames.last().unwrap().duration_since(frames[0]);
            // The cap allows a slow runner and one buffered tick, while still rejecting an uncapped stream.
            let maximum = (elapsed.as_secs_f64() * f64::from(fps)).ceil() as usize + 2;
            assert!(frames.len() <= maximum, "{rate:?} produced {} frames in {elapsed:?}", frames.len());
        }
    }
    drop(shell.update(Message::Workbench(WorkbenchMessage::Pause)));
    assert!(
        visual_frames(shell.subscription(), window, 1, Duration::from_millis(250)).is_empty(),
        "paused Builtin preview kept its frame loop"
    );
}

fn visual_frames(
    subscription: iced::Subscription<folia_ui::Message>,
    window: iced::window::Id,
    wanted: usize,
    timeout: std::time::Duration,
) -> Vec<std::time::Instant> {
    use iced::advanced::subscription::{Event, into_recipes};
    use iced::futures::{StreamExt, stream};
    let runtime = iced::executor::Default::new().unwrap();
    runtime.block_on(async {
        let other = iced::window::Id::unique();
        let redraws: Vec<_> = (0..150)
            .flat_map(|_| {
                [window, other].map(|window| Event::Interaction {
                    window,
                    event: iced::Event::Window(iced::window::Event::RedrawRequested(std::time::Instant::now())),
                    status: iced::event::Status::Ignored,
                })
            })
            .collect();
        let streams = into_recipes(subscription)
            .into_iter()
            .map(|recipe| recipe.stream(Box::pin(stream::iter(redraws.clone()).chain(stream::pending()))));
        let mut frames = stream::select_all(streams);
        let mut sampled = Vec::with_capacity(wanted);
        let _ = tokio::time::timeout(timeout, async {
            while let Some(message) = frames.next().await {
                if matches!(message, folia_ui::Message::VisualFrame) {
                    sampled.push(std::time::Instant::now());
                    if sampled.len() == wanted {
                        break;
                    }
                }
            }
        })
        .await;
        sampled
    })
}

#[test]
fn graphics_messages_persist_and_motion_surfaces_are_independent() {
    use folia_store::{GpuPreference, GraphicsSettings, MotionSurface};
    use folia_ui::{Message, Shell, graphics::Message as Graphics};
    let temporary = tempfile::tempdir().unwrap();
    let init = super::player_fixture::init(temporary.path());
    let store = init.store.clone();
    let (mut shell, initial) = Shell::new(init);
    drop(initial);
    for message in [
        Graphics::Static(true),
        Graphics::BackgroundThrottling(true),
        Graphics::Gpu(GpuPreference::LowPower),
        Graphics::RenderScale(0.75),
        Graphics::Antialias(folia_core::LyricAntialias::Off),
        Graphics::Motion(MotionSurface::CollectionMorph, true),
        Graphics::Motion(MotionSurface::SettingsScroll, true),
    ] {
        let task = shell.update(Message::Graphics(message));
        super::player_fixture::complete(&mut shell, task);
    }
    let saved: GraphicsSettings = store.load().unwrap();
    assert!(saved.static_mode && saved.background_throttling);
    assert_eq!(saved.gpu_preference, GpuPreference::LowPower);
    assert_eq!(saved.lyric_render_quality.scale, 0.75);
    assert_eq!(saved.lyric_render_quality.antialias, folia_core::LyricAntialias::Off);
    let now = std::time::Instant::now();
    let shift = folia_ui::WallShift::new(
        folia_ui::ShiftKind::Wave,
        1.0,
        now,
        [iced::Rectangle::new(iced::Point::ORIGIN, Size::new(100.0, 100.0))].into_iter(),
    );
    assert!(!shift.running(now));
    let mut scroll = folia_ui::Scroll::default();
    scroll.scroll_to(100.0, now);
    assert!(scroll.animating(now), "settings-only scrolling must leave other lists animated");
    let task = shell.update(Message::Graphics(Graphics::Motion(MotionSurface::CollectionMorph, false)));
    super::player_fixture::complete(&mut shell, task);
    assert!(shift.running(now));
    let task = shell.update(Message::Graphics(Graphics::Motion(MotionSurface::SettingsScroll, false)));
    super::player_fixture::complete(&mut shell, task);
    let task = shell.update(Message::OpenWorkbench);
    super::player_fixture::complete(&mut shell, task);
    for style in Style::ALL {
        for message in [
            WorkbenchMessage::Style(style),
            WorkbenchMessage::RenderScale(Some(1.5)),
            WorkbenchMessage::RenderAntialias(Some(folia_core::LyricAntialias::On)),
        ] {
            let task = shell.update(Message::Workbench(message));
            super::player_fixture::complete(&mut shell, task);
        }
        let settings: VisualizerSettings = store.load().unwrap();
        let values = style.load(&settings.tables);
        assert_eq!(
            values.render_quality(saved.lyric_render_quality),
            folia_core::LyricRenderQuality { scale: 1.5, antialias: folia_core::LyricAntialias::On }
        );
        let task = shell.update(Message::Workbench(WorkbenchMessage::RenderScale(None)));
        super::player_fixture::complete(&mut shell, task);
        let settings: VisualizerSettings = store.load().unwrap();
        let quality = style.load(&settings.tables).render_quality(saved.lyric_render_quality);
        assert_eq!(quality.scale, 0.75);
        assert_eq!(quality.antialias, folia_core::LyricAntialias::On);
        let task = shell.update(Message::Workbench(WorkbenchMessage::Reset));
        super::player_fixture::complete(&mut shell, task);
        let settings: VisualizerSettings = store.load().unwrap();
        assert_eq!(style.load(&settings.tables).render_quality(saved.lyric_render_quality), saved.lyric_render_quality);
        assert!(!settings.tables[style.manifest().storage_key].as_table().unwrap().contains_key("render"));
    }
}

#[test]
#[cfg_attr(feature = "ci", ignore = "needs a GPU")]
fn harmony_and_ring_translation_reach_the_preview_pixels() {
    folia_ui::install_fonts();
    let mut appearance = AppearanceSettings::default();
    let graphics = folia_store::GraphicsSettings::default();
    appearance.set_background_mode(Some("common"));
    appearance.subtitle.hide_translation = true;
    let mut bench = Workbench::open();
    bench.update(
        WorkbenchMessage::Source(folia_ui::WorkbenchSource::Live),
        Style::Still.manifest(),
        &appearance,
        folia_visualizer::LatentTuning::DEFAULT,
    );
    bench.update(
        WorkbenchMessage::Pause,
        Style::Still.manifest(),
        &appearance,
        folia_visualizer::LatentTuning::DEFAULT,
    );
    let lyrics = folia_visualizer::preview_lyrics();
    let live = folia_ui::WorkbenchLive {
        smart_beat: Default::default(),
        lyrics: &lyrics,
        time: 2.2,
        track: None,
        seed: "fixture",
        audio: &folia_core::AudioFrame::SILENT,
        portrait: None,
        colors: &[],
        source: None,
    };
    let capture = |bench: &Workbench, appearance: &AppearanceSettings, style, live| {
        let i18n = I18n::new(Language::ZhCn).unwrap();
        let theme = Theme::DAYLIGHT;
        let root = bench.view(WorkbenchLook {
            portrait: None,
            smart_beat: Default::default(),
            i18n: &i18n,
            theme: &theme,
            daylight: true,
            window: Size::new(W as f32, H as f32),
            style,
            font_scale: 1.0,
            live: Some(live),
            audio: &folia_core::AudioFrame::SILENT,
            appearance,
            graphics: &graphics,
            fonts: &[],
            dpi: 1.0,
            cover: &[],
            latent: folia_visualizer::LatentTuning::DEFAULT,
            sora: folia_visualizer::SoraTuning::DEFAULT,
            monet: folia_visualizer::monet_background::Tuning::DEFAULT,
            nomand: folia_visualizer::nomand::Tuning::default(),
            picture: None,
            has_background_upload: false,
        });
        let mut renderer =
            pollster::block_on(<iced::Renderer as Headless>::new(folia_ui::INTERFACE_FONT, Pixels(16.0), None))
                .unwrap();
        let mut ui = UserInterface::build(root, Size::new(W as f32, H as f32), Cache::default(), &mut renderer);
        ui.draw(
            &mut renderer,
            &IcedTheme::Light,
            &DrawStyle { text_color: Color::BLACK },
            iced::mouse::Cursor::Unavailable,
        );
        renderer.screenshot(&iced::advanced::graphics::Viewport::with_physical_size(Size::new(W, H), 1.0), Color::WHITE)
    };
    let harmony = capture(&bench, &appearance, Style::Still, live);
    appearance.subtitle.show_harmony = false;
    let hidden = capture(&bench, &appearance, Style::Still, live);
    let preview_changes = |a: &[u8], b: &[u8]| {
        a.chunks_exact(4)
            .zip(b.chunks_exact(4))
            .enumerate()
            .filter(|(index, (a, b))| index % (W as usize) < 800 && index / (W as usize) < 500 && a != b)
            .count()
    };
    assert!(preview_changes(&harmony, &hidden) > 200);
    let ring = capture(&bench, &appearance, Style::Claddagh, live);
    bench.values.set_flag(Style::Claddagh.manifest(), "showTranslation", true);
    let translated = capture(&bench, &appearance, Style::Claddagh, live);
    assert!(
        preview_changes(&translated, &ring) > 100,
        "ring translation stayed hidden with shared translations disabled"
    );
    bench.values.set_number(Style::Claddagh.manifest(), "translationScale", 1.5);
    bench.values.set_number(Style::Claddagh.manifest(), "translationLetterSpacingOffset", 3.0);
    assert!(preview_changes(&capture(&bench, &appearance, Style::Claddagh, live), &translated) > 100);
    let original_track = folia_core::Track {
        id: 1,
        title: "Original live song".into(),
        artist: Some("First artist".into()),
        ..Default::default()
    };
    let other_track = folia_core::Track {
        id: 2,
        title: "Next song after pause".into(),
        artist: Some("Second artist".into()),
        ..Default::default()
    };
    let original_cover = iced::widget::image::Handle::from_rgba(1, 1, vec![200, 40, 20, 255]);
    let other_cover = iced::widget::image::Handle::from_rgba(1, 1, vec![20, 40, 200, 255]);
    let first = folia_ui::WorkbenchLive {
        track: Some(&original_track),
        portrait: Some(&original_cover),
        seed: "first song",
        ..live
    };
    let second = folia_ui::WorkbenchLive {
        track: Some(&other_track),
        portrait: Some(&other_cover),
        seed: "second song",
        time: 9.0,
        ..live
    };
    bench.capture_live(Some(first), true);
    let held = capture(&bench, &appearance, Style::Monet, first);
    assert_eq!(
        preview_changes(&held, &capture(&bench, &appearance, Style::Monet, second)),
        0,
        "paused live content followed the next track"
    );
    use iced::futures::StreamExt;
    let task = bench.load_art();
    let replies = pollster::block_on(async {
        let mut replies = Vec::new();
        if let Some(mut stream) = iced_runtime::task::into_stream(task) {
            while let Some(action) = stream.next().await {
                if let iced_runtime::Action::Output(message) = action {
                    replies.push(message);
                }
            }
        }
        replies
    });
    assert_eq!(replies.len(), 2);
    for message in replies {
        bench.update(message, Style::Monet.manifest(), &appearance, folia_visualizer::LatentTuning::DEFAULT);
    }
    bench.update(
        WorkbenchMessage::Source(folia_ui::WorkbenchSource::Builtin),
        Style::Monet.manifest(),
        &appearance,
        folia_visualizer::LatentTuning::DEFAULT,
    );
    let builtin = capture(&bench, &appearance, Style::Monet, second);
    assert!(preview_changes(&builtin, &held) > 1000);
    bench.update(
        WorkbenchMessage::Sample(folia_ui::WorkbenchSample::Reserved),
        Style::Monet.manifest(),
        &appearance,
        folia_visualizer::LatentTuning::DEFAULT,
    );
    assert!(preview_changes(&builtin, &capture(&bench, &appearance, Style::Monet, second)) > 1000);
}

#[test]
fn smart_beat_controls_save_on_release_and_global_reset() {
    use folia_store::{NEBBIA_DRIZZLE_RIDE, SMART_BEAT_KEY, SmartBeatMode, SmartBeatSettings};
    use folia_ui::{Message, Shell, smart_beat::Change};
    let temporary = tempfile::tempdir().unwrap();
    let init = super::player_fixture::init(temporary.path());
    let store = init.store.clone();
    let (mut shell, initial) = Shell::new(init);
    drop(initial);
    drop(shell.update(Message::OpenWorkbench));
    let task = shell.update(Message::Workbench(WorkbenchMessage::SmartBeat(Change::Mode(SmartBeatMode::On))));
    super::player_fixture::complete(&mut shell, task);
    let saved: VisualizerSettings = store.load().unwrap();
    assert_eq!(SmartBeatSettings::read(saved.tables.get(SMART_BEAT_KEY)).mode, SmartBeatMode::On);
    for value in [0.4, 0.8, 1.6] {
        drop(shell.update(Message::Workbench(WorkbenchMessage::SmartBeat(Change::Strength(
            NEBBIA_DRIZZLE_RIDE,
            "rain",
            value,
        )))));
    }
    let saved: VisualizerSettings = store.load().unwrap();
    assert_eq!(SmartBeatSettings::read(saved.tables.get(SMART_BEAT_KEY)).effect(NEBBIA_DRIZZLE_RIDE).part("rain"), 1.0);
    let task = shell.update(Message::Workbench(WorkbenchMessage::Commit));
    super::player_fixture::complete(&mut shell, task);
    let saved: VisualizerSettings = store.load().unwrap();
    assert_eq!(SmartBeatSettings::read(saved.tables.get(SMART_BEAT_KEY)).effect(NEBBIA_DRIZZLE_RIDE).part("rain"), 1.6);
    drop(shell.update(Message::Workbench(WorkbenchMessage::Tab(WorkbenchTab::Global))));
    let task = shell.update(Message::Workbench(WorkbenchMessage::Reset));
    super::player_fixture::complete(&mut shell, task);
    let saved: VisualizerSettings = store.load().unwrap();
    assert_eq!(SmartBeatSettings::read(saved.tables.get(SMART_BEAT_KEY)), SmartBeatSettings::default());
}

#[test]
fn smart_beat_discards_late_updates_and_freezes_preview_timelines() {
    use folia_store::{NEBBIA_AQUA_BEAT, SmartBeatMode};
    use folia_ui::{
        Message,
        smart_beat::{Change, Snapshot, State, Status},
    };
    use iced::futures::StreamExt;
    let temporary = tempfile::tempdir().unwrap();
    let path = temporary.path().join("audio.flac");
    std::fs::write(&path, include_bytes!("../player/fixtures/seek.flac")).unwrap();
    let mut playback = folia_ui::Playback::new(Default::default(), None);
    playback.queue.replace(vec![folia_core::Track { path, ..Default::default() }], 0);
    playback.status.state = folia_player::PlayerState::Playing;
    let mut visualizer = VisualizerSettings::default();
    let mut state =
        State::new(&visualizer, temporary.path().join("missing-models"), temporary.path().join("analysis.db"));
    state.change(Change::Mode(SmartBeatMode::Cached), &mut visualizer);
    let task = state.sync(&playback, folia_core::AnalysisSources { beats: true, stems: false });
    let (message, stream) = pollster::block_on(async move {
        let mut stream = iced_runtime::task::into_stream(task).unwrap();
        loop {
            let action = stream.next().await.unwrap();
            if let iced_runtime::Action::Output(Message::SmartBeat(message)) = action {
                break (message, stream);
            }
        }
    });
    state.update(message.clone());
    assert!(matches!(state.view(0.0).status, Status::CachedMiss));
    state.change(Change::Mode(SmartBeatMode::Off), &mut visualizer);
    state.cancel();
    state.update(message);
    assert!(matches!(state.view(0.0).status, Status::Idle));
    drop(stream);
    let mut beats = vec![folia_core::BeatEvent { time: 1.0, ..Default::default() }];
    let snapshot = Snapshot::capture(folia_core::SmartBeat { beats: Some(&beats), heard: 3.0, ..Default::default() });
    beats[0].time = 9.0;
    let mut settings = folia_store::SmartBeatSettings { mode: SmartBeatMode::On, ..Default::default() };
    settings.set_part(NEBBIA_AQUA_BEAT, "jelly", 0.45);
    let held = snapshot.frame(&settings);
    assert_eq!(held.beats.unwrap()[0].time, 1.0);
    assert_eq!(held.heard, 3.0);
    assert_eq!(held.aqua_strength[0], 0.45);
}

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
    let graphics = folia_store::GraphicsSettings::default();
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
                    portrait: None,
                    smart_beat: Default::default(),
                    i18n: &i18n,
                    theme: &theme,
                    daylight: true,
                    window: Size::new(W as f32, H as f32),
                    style: Style::Classic,
                    font_scale: 1.0,
                    live: None,
                    audio: &folia_core::AudioFrame::SILENT,
                    appearance: &appearance,
                    graphics: &graphics,
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

#[test]
fn monet_text_and_common_background_commit_and_reset_independently() {
    let manifest = Style::Monet.manifest();
    let mut appearance = AppearanceSettings::default();
    appearance.set_background_mode(Some("common"));
    let mut bench = Workbench::open();
    let latent = folia_visualizer::LatentTuning::DEFAULT;
    let update = bench.update(
        WorkbenchMessage::Text("customDescription", "Custom lyric note".into()),
        manifest,
        &appearance,
        latent,
    );
    assert!(matches!(update.effect, WorkbenchEffect::Draft));
    assert_eq!(bench.values.text("customDescription"), "Custom lyric note");
    let update = bench.update(WorkbenchMessage::Tab(WorkbenchTab::Background), manifest, &appearance, latent);
    assert!(matches!(update.effect, WorkbenchEffect::Save));
    let update = bench.update(WorkbenchMessage::CoverColor(true), manifest, &appearance, latent);
    appearance = update.appearance.unwrap();
    let update = bench.update(WorkbenchMessage::CoverOpacity(0.3), manifest, &appearance, latent);
    appearance = update.appearance.unwrap();
    assert!(appearance.use_cover_color_bg);
    assert_eq!(appearance.cover_background_opacity, 0.3);
    let update = bench.update(WorkbenchMessage::Reset, manifest, &appearance, latent);
    let reset = update.appearance.unwrap();
    assert!(!reset.use_cover_color_bg);
    assert_eq!(reset.cover_background_opacity, 0.75);
    assert!(update.monet.is_none() && update.latent.is_none() && update.nomand.is_none() && update.sora.is_none());
    assert_eq!(bench.values.text("customDescription"), "Custom lyric note");
    assert!(matches!(bench.release_drag(), WorkbenchEffect::None));
    appearance.set_background_mode(Some("sora"));
    let update = bench.update(WorkbenchMessage::Reset, manifest, &appearance, latent);
    assert!(update.appearance.is_none());
    assert!(update.sora.is_some() && update.monet.is_none() && update.latent.is_none() && update.nomand.is_none());
}

#[test]
fn system_theme_changes_stop_after_a_manual_choice() {
    let directory = tempfile::tempdir().unwrap();
    let mut setup = super::player_fixture::init(directory.path());
    setup.appearance.follow_system_theme = true;
    let store = setup.store.clone();
    let (mut shell, initial) = folia_ui::Shell::new(setup);
    drop(initial);
    let task = shell.update(folia_ui::Message::SystemTheme(iced::theme::Mode::Dark));
    super::player_fixture::complete(&mut shell, task);
    let settings = AppearanceSettings::load(&store).unwrap();
    assert!(settings.follow_system_theme);
    assert_eq!(settings.theme, folia_store::ThemeMode::Dark);
    let task = shell.update(folia_ui::Message::Theme(folia_store::ThemeMode::Light));
    super::player_fixture::complete(&mut shell, task);
    let task = shell.update(folia_ui::Message::SystemTheme(iced::theme::Mode::Dark));
    super::player_fixture::complete(&mut shell, task);
    let settings = AppearanceSettings::load(&store).unwrap();
    assert!(!settings.follow_system_theme);
    assert_eq!(settings.theme, folia_store::ThemeMode::Light);
}

#[test]
fn live_pause_keeps_its_song_time_separate_from_the_builtin_clock() {
    use folia_ui::WorkbenchSource;
    use std::time::{Duration, Instant};
    let manifest = Style::Classic.manifest();
    let appearance = AppearanceSettings::default();
    let latent = folia_visualizer::LatentTuning::DEFAULT;
    let mut bench = Workbench::open();
    let instant = Instant::now() + Duration::from_secs(7);
    let builtin = bench.position(instant, None);
    bench.update(WorkbenchMessage::Source(WorkbenchSource::Live), manifest, &appearance, latent);
    bench.update(WorkbenchMessage::Source(WorkbenchSource::Builtin), manifest, &appearance, latent);
    assert_eq!(bench.position(instant, None), builtin, "source changes must preserve the builtin origin");
    bench.update(WorkbenchMessage::Source(WorkbenchSource::Live), manifest, &appearance, latent);
    let lyrics = folia_core::Lyrics {
        lines: vec![folia_core::Line { start: 60.0, end: 70.0, text: "live".into(), ..Default::default() }],
        ..Default::default()
    };
    bench.capture_live(
        Some(folia_ui::WorkbenchLive {
            smart_beat: Default::default(),
            lyrics: &lyrics,
            time: 64.25,
            track: None,
            seed: "fixture",
            audio: &folia_core::AudioFrame::SILENT,
            portrait: None,
            colors: &[],
            source: None,
        }),
        false,
    );
    bench.update(WorkbenchMessage::Pause, manifest, &appearance, latent);
    assert_eq!(bench.position(instant + Duration::from_secs(30), Some(85.0)), 64.25);
    bench.update(WorkbenchMessage::Source(WorkbenchSource::Builtin), manifest, &appearance, latent);
    let held = bench.position(instant, None);
    assert!(held < 64.25);
    assert_eq!(bench.position(instant + Duration::from_secs(30), None), held);
    bench.update(WorkbenchMessage::Pause, manifest, &appearance, latent);
    bench.update(WorkbenchMessage::Source(WorkbenchSource::Live), manifest, &appearance, latent);
    assert_eq!(bench.position(instant, Some(85.0)), 85.0);
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
