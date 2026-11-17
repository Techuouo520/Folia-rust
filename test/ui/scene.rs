// Scene settings reach the live player, and obsolete room replies cannot reactivate a departed scene.

use std::sync::{Arc, Mutex};

use folia_core::{RoomImpulseResponse, RoomOutput, SceneId, SceneQuality};
use folia_ui::{Message, Page, SceneSettingsMessage as Change, SceneSoundMessage as Sound, Shell};

fn prepared() -> Arc<Mutex<Option<Box<folia_player::RoomConvolver>>>> {
    let impulse =
        RoomImpulseResponse { sample_rate: 48_000, channels: [vec![1.0], vec![0.0], vec![0.0], vec![1.0]], gain: 1.0 };
    Arc::new(Mutex::new(Some(Box::new(folia_player::RoomConvolver::new(&impulse).unwrap()))))
}

fn reply(shell: &mut Shell, generation: u64, accepted: bool) {
    let value = prepared();
    drop(shell.update(Message::SceneSound(Sound::Ready(generation, Ok(value.clone())))));
    assert_eq!(value.lock().unwrap().is_none(), accepted, "room reply generation {generation}");
}

#[test]
#[cfg_attr(feature = "ci", ignore = "needs the native audio output for the room slot")]
fn scene_settings_persist_and_room_replies_follow_the_live_page() {
    let temporary = tempfile::tempdir().unwrap();
    let init = super::player_fixture::init(temporary.path());
    let store = init.store.clone();
    let (mut shell, initial) = Shell::new(init);
    drop(initial);
    drop(shell.update(Message::OpenScene));
    for change in [
        Change::Select(Some(SceneId::Cinema)),
        Change::Quality(SceneQuality::High),
        Change::Seat(Some("R4-13".into())),
        Change::HouseLights(true),
        Change::Curtains(false),
        Change::Output(RoomOutput::Speakers),
        Change::Sound(true),
    ] {
        let task = shell.update(Message::SceneSettings(change));
        super::player_fixture::complete(&mut shell, task);
    }
    let saved: folia_store::SceneSettings = store.load().unwrap();
    assert_eq!(saved.scene_id, Some(SceneId::Cinema));
    assert_eq!(saved.scene_options.cinema.quality, SceneQuality::High);
    assert_eq!(saved.scene_options.cinema.viewpoint.as_deref(), Some("R4-13"));
    assert!(saved.scene_options.cinema.house_lights && !saved.scene_options.cinema.curtains_on_pause);
    assert!(saved.scene_room_sound && saved.scene_room_output == RoomOutput::Speakers);
    reply(&mut shell, 0, false);
    drop(shell.update(Message::Navigate(Page::NowPlaying)));
    reply(&mut shell, 1, true);
    drop(shell.update(Message::Navigate(Page::Library)));
    reply(&mut shell, 1, false);
    drop(shell.update(Message::Navigate(Page::NowPlaying)));
    drop(shell.update(Message::SceneSettings(Change::Sound(false))));
    drop(shell.update(Message::SceneSettings(Change::Sound(true))));
    drop(shell.update(Message::SceneSettings(Change::Sound(false))));
    reply(&mut shell, 3, false);
    reply(&mut shell, 5, false);
    drop(shell.update(Message::SceneSound(Sound::Released(4))));
    drop(shell.update(Message::SceneSettings(Change::Sound(true))));
    reply(&mut shell, 7, true);
    drop(shell.update(Message::SceneFailed(folia_visualizer::cinema::Failure {
        generation: 4,
        error: folia_visualizer::cinema::CinemaError::Asset("test initialization failure"),
    })));
    reply(&mut shell, 7, false);
    drop(shell.update(Message::SceneSettings(Change::Select(Some(SceneId::Cinema)))));
    drop(shell.update(Message::SceneFailed(folia_visualizer::cinema::Failure {
        generation: 4,
        error: folia_visualizer::cinema::CinemaError::Asset("late initialization failure"),
    })));
    reply(&mut shell, 9, true);
    drop(shell.update(Message::WindowClosing));
    reply(&mut shell, 9, false);
    let task = shell.update(Message::SceneSettings(Change::Reset));
    super::player_fixture::complete(&mut shell, task);
    let reset: folia_store::SceneSettings = store.load().unwrap();
    assert_eq!(reset.scene_options.cinema, Default::default());
    assert!(reset.scene_room_sound);
    assert_eq!(reset.scene_room_output, RoomOutput::Speakers);
}

#[test]
#[cfg_attr(feature = "ci", ignore = "needs a GPU")]
fn scene_window_selection_projects_the_real_player_and_closes_cleanly() {
    use iced_runtime::user_interface::{Cache, UserInterface};
    if super::player_fixture::run_gpu_child("scene::scene_window_selection_projects_the_real_player_and_closes_cleanly")
    {
        return;
    }
    folia_ui::install_fonts();
    let temporary = tempfile::tempdir().unwrap();
    let mut init = super::player_fixture::init(temporary.path());
    init.graphics.lyric_render_quality.scale = 2.0;
    let store = init.store.clone();
    let (mut shell, task) = Shell::new(init);
    super::player_fixture::complete(&mut shell, task);
    std::thread::sleep(std::time::Duration::from_millis(350));
    drop(shell.update(Message::SplashFrame));
    let (adapter, device, queue) = pollster::block_on(folia_render::backend::device()).unwrap();
    let _registration =
        folia_render::quality::register_device(&adapter, &device, iced_wgpu::graphics::Shell::headless());
    let mut renderer = iced_wgpu::Renderer::new(
        iced_wgpu::Engine::new(
            &adapter,
            device.clone(),
            queue,
            wgpu::TextureFormat::Rgba8Unorm,
            None,
            iced_wgpu::graphics::Shell::headless(),
        ),
        folia_ui::INTERFACE_FONT,
        iced::Pixels(16.0),
    );
    let size = iced::Size::new(1200.0, 800.0);
    let viewport = iced::advanced::graphics::Viewport::with_physical_size(iced::Size::new(1200, 800), 1.0);
    let mut cache = Cache::default();
    drop(shell.update(Message::OpenSettings));
    std::thread::sleep(std::time::Duration::from_millis(350));
    super::picker::click(&mut shell, &mut renderer, &mut cache, size, "场景");
    super::picker::click(&mut shell, &mut renderer, &mut cache, size, "影厅");
    assert_eq!(store.load::<folia_store::SceneSettings>().unwrap().scene_id, Some(SceneId::Cinema));
    assert!(folia_visualizer::cinema::target_sizes(&device).is_empty(), "settings preview created a theater");
    drop(shell.update(Message::SceneSettings(Change::Close)));
    drop(shell.update(Message::Settings(folia_ui::SettingsMessage::Close)));
    drop(shell.update(Message::Navigate(Page::NowPlaying)));
    std::thread::sleep(std::time::Duration::from_millis(350));
    let mut pixels = Vec::new();
    for _ in 0..6 {
        let mut ui = UserInterface::build(shell.view(), size, cache, &mut renderer);
        ui.draw(
            &mut renderer,
            &shell.theme(),
            &iced::advanced::renderer::Style { text_color: iced::Color::WHITE },
            iced::mouse::Cursor::Unavailable,
        );
        cache = ui.into_cache();
        pixels = renderer.screenshot(&viewport, iced::Color::BLACK);
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
        while folia_render::gpu_builds().0 > 0 {
            assert!(std::time::Instant::now() < deadline);
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
    }
    assert_eq!(folia_render::quality::target_sizes(), [[2400, 1298]]);
    assert_eq!(folia_visualizer::cinema::target_sizes(&device), [[1500, 1000]]);
    assert!(pixels.chunks_exact(4).any(|pixel| pixel[0] > 80 && pixel[1] > 50));
    drop(shell.update(Message::Navigate(Page::Library)));
    for _ in 0..3 {
        let mut ui = UserInterface::build(shell.view(), size, cache, &mut renderer);
        ui.draw(
            &mut renderer,
            &shell.theme(),
            &iced::advanced::renderer::Style { text_color: iced::Color::WHITE },
            iced::mouse::Cursor::Unavailable,
        );
        cache = ui.into_cache();
        let _ = renderer.screenshot(&viewport, iced::Color::BLACK);
    }
    assert!(folia_visualizer::cinema::target_sizes(&device).is_empty());
    assert!(folia_render::quality::target_sizes().is_empty());
}
