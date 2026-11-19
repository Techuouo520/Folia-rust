// Wall controls exercise the shell, persisted preferences, live queue, and isolated render surfaces.

use folia_core::Track;
use folia_store::{AppearanceSettings, GeneralSettings, PlaybackSettings, ThemeGenerationSource, WallLook};
use folia_ui::{HomeMessage as Wall, Message, Page, Shell};
use iced::{Color, Event, Point, Size};
use iced_runtime::user_interface::{Cache, UserInterface};
use std::time::{Duration, Instant};

fn press(shell: &mut Shell, action: Wall) {
    let task = shell.update(Message::Home(action));
    super::player_fixture::complete(shell, task);
}

#[test]
fn wall_settings_and_theme_follow_the_player() {
    let temp = tempfile::tempdir().unwrap();
    let init = super::player_fixture::init(temp.path());
    let store = init.store.clone();
    let cache = init.data_dir.join("song_themes.db");
    let runtime = tokio::runtime::Builder::new_multi_thread().enable_all().build().unwrap();
    let _context = runtime.enter();
    let (mut shell, initial) = Shell::new(init);
    super::player_fixture::complete(&mut shell, initial);
    for change in [
        Wall::WallLook(WallLook::Partial),
        Wall::Windows(6),
        Wall::Tint(false),
        Wall::Lights(false),
        Wall::WallLook(WallLook::Clear),
        Wall::Windows(2),
        Wall::ToolsQueueToggle,
        Wall::RandomizeOnEntry,
        Wall::Volume(0.36),
        Wall::VolumeDone,
        Wall::Mute,
    ] {
        press(&mut shell, change);
    }
    let saved = AppearanceSettings::load(&store).unwrap();
    assert_eq!(
        (saved.wall.look, saved.wall.windows_per_block, saved.wall.tint, saved.wall.lights_on),
        (WallLook::Clear, 6, false, false)
    );
    assert_eq!(folia_store::WallAppearance::import(&saved.wall.export().unwrap()).unwrap(), saved.wall);
    assert!(folia_store::WallAppearance::import("volume = 1").is_err());
    assert!(store.load::<GeneralSettings>().unwrap().wall_tools_queue_island_open);
    assert!(store.load::<GeneralSettings>().unwrap().wall_randomize_on_entry);
    let audio = PlaybackSettings::load(&store).unwrap();
    assert_eq!((audio.volume, audio.muted), (0.36, true));
    let track = Track { id: 11, title: "Wall theme".into(), ..Default::default() };
    drop(shell.update(Message::OpenFiles(vec![
        track.clone(),
        Track { id: 12, title: "Another song".into(), ..Default::default() },
    ])));
    drop(shell.update(Message::SongTheme(folia_ui::song_theme::Message::Source(ThemeGenerationSource::Cover))));
    press(&mut shell, Wall::GenerateTheme);
    assert!(folia_ai::ThemeCache::open(&cache).unwrap().get(&folia_ai::song_key(&track)).unwrap().is_some());
    press(&mut shell, Wall::Volume(0.25));
    press(&mut shell, Wall::VolumeDone);
    assert!(!PlaybackSettings::load(&store).unwrap().muted);
}

#[test]
fn fm_keeps_the_queue_while_the_tools_remember_its_disclosure() {
    let mut home = folia_ui::HomeChrome::new(1200.0);
    let mut player = folia_ui::Playback::new(Default::default(), None);
    let tracks = vec![
        Track { id: 1, title: "First".into(), ..Default::default() },
        Track { id: 2, title: "Second".into(), ..Default::default() },
    ];
    let fm = folia_omni::Collection {
        provider: "offline".into(),
        id: "fm".into(),
        kind: folia_omni::CollectionKind::PersonalFm,
        name: "FM".into(),
        cover_url: None,
        track_count: None,
        creator: None,
        description: None,
        owned: false,
    };
    drop(player.play_fm(tracks, fm, folia_core::Origin { name: "FM".into() }));
    home.restore_queue_island(true);
    home.update(&Wall::Tools);
    home.sync_queue(player.queue.items(), Some(0), true, Instant::now());
    home.update(&Wall::ToolsQueueToggle);
    assert!(home.queue_island_open());
    assert!(!home.island_shown(Instant::now() + Duration::from_secs(1)));
    assert!(!player.shuffle(11) && !player.clear_queue());
    assert_eq!(player.queue.items().len(), 2);
    home.sync_queue(player.queue.items(), Some(0), false, Instant::now());
    assert!(home.island_shown(Instant::now() + Duration::from_secs(1)));
}

#[test]
#[cfg_attr(feature = "ci", ignore = "needs a hardware GPU")]
fn popup_actions_render_and_release_the_wall() {
    if super::player_fixture::run_gpu_child("wall_tools::popup_actions_render_and_release_the_wall") {
        return;
    }
    folia_ui::install_fonts();
    let temp = tempfile::tempdir().unwrap();
    let mut init = super::player_fixture::init(temp.path());
    init.general.home_tab = folia_store::HomeTab::Local;
    init.appearance.set_background_mode(Some("common"));
    let store = init.store.clone();
    let mut library = folia_library::Library::open(&init.library_db).unwrap();
    for index in 0..12 {
        std::fs::write(temp.path().join(format!("Song{index:02}.wav")), b"RIFF").unwrap();
    }
    library.add_folder(temp.path()).unwrap();
    library.scan().unwrap();
    let tracks = library.tracks().unwrap();
    let (mut shell, task) = Shell::new(init);
    super::player_fixture::complete(&mut shell, task);
    std::thread::sleep(Duration::from_millis(400));
    drop(shell.update(Message::SplashFrame));
    let (_registration, _device, mut renderer) = renderer();
    let mut cache = Cache::default();
    let size = Size::new(1200.0, 800.0);
    let viewport = iced::advanced::graphics::Viewport::with_physical_size(Size::new(1200, 800), 1.0);
    let mut frame = |shell: &mut Shell, click: Option<Point>| {
        let cursor = click.map_or(iced::mouse::Cursor::Unavailable, iced::mouse::Cursor::Available);
        let mut events = vec![Event::Window(iced::window::Event::RedrawRequested(Instant::now()))];
        if click.is_some() {
            events.extend([
                Event::Mouse(iced::mouse::Event::ButtonPressed(iced::mouse::Button::Left)),
                Event::Mouse(iced::mouse::Event::ButtonReleased(iced::mouse::Button::Left)),
            ]);
        }
        let mut messages = Vec::new();
        let mut ui = UserInterface::build(shell.view(), size, std::mem::take(&mut cache), &mut renderer);
        ui.update(&events, cursor, &mut renderer, &mut iced::advanced::clipboard::Null, &mut messages);
        ui.draw(&mut renderer, &shell.theme(), &iced::advanced::renderer::Style { text_color: Color::BLACK }, cursor);
        cache = ui.into_cache();
        let pixels = renderer.screenshot(&viewport, Color::BLACK);
        let mut actions = Vec::new();
        for message in messages {
            if let Message::Home(action) = &message {
                actions.push(format!("{action:?}"));
            }
            drop(shell.update(message));
        }
        (pixels, actions)
    };
    frame(&mut shell, None);
    let (_, dock) = frame(&mut shell, Some(Point::new(1152.0, 752.0)));
    assert!(dock.iter().any(|action| action == "DockRelease"), "dock tap did not reach release: {dock:?}");
    std::thread::sleep(Duration::from_millis(320));
    let click = |n: usize| Some(Point::new(911.0 + n as f32 * 46.7, 448.0));
    for cell in [0, 1, 4] {
        let (_, actions) = frame(&mut shell, click(cell));
        assert!(
            !actions.iter().any(|action| matches!(action.as_str(), "Shuffle" | "GenerateTheme" | "Player")),
            "empty queue: {actions:?}"
        );
    }
    let pinned = tracks[3].id;
    drop(shell.update(Message::OpenFiles(tracks)));
    drop(shell.update(Message::Playback(folia_ui::PlaybackMessage::Jump(3))));
    drop(shell.update(Message::SongTheme(folia_ui::song_theme::Message::Source(ThemeGenerationSource::Cover))));
    for (cell, expected) in [(0, "Shuffle"), (1, "GenerateTheme"), (2, "ToolsQueueToggle"), (3, "Destination")] {
        let (_, actions) = frame(&mut shell, click(cell));
        assert!(actions.iter().any(|a| a == expected), "{expected}: {actions:?}");
        if cell == 2 {
            press(&mut shell, Wall::SaveQueue);
            press(&mut shell, Wall::QueueName("Wall test".into()));
            press(&mut shell, Wall::CommitQueue);
            let saved = library.playlists().unwrap().into_iter().find(|list| list.name == "Wall test").unwrap();
            assert_eq!(saved.song_ids.len(), 12);
            assert_eq!(saved.song_ids.first(), Some(&pinned));
        }
    }
    std::thread::sleep(Duration::from_millis(450));
    frame(&mut shell, None);
    let (lit, _) = frame(&mut shell, None);
    press(&mut shell, Wall::Tint(false));
    std::thread::sleep(Duration::from_millis(310));
    let (untinted, _) = frame(&mut shell, None);
    let changed = lit.chunks_exact(4).zip(untinted.chunks_exact(4)).filter(|(a, b)| a[0].abs_diff(b[0]) > 10).count();
    assert!(changed > 5000, "poster tint must reach the canvas");
    press(&mut shell, Wall::Lights(false));
    std::thread::sleep(Duration::from_millis(310));
    let (dark, _) = frame(&mut shell, None);
    let difference =
        |a: &[u8], b: &[u8]| a.chunks_exact(4).zip(b.chunks_exact(4)).filter(|(p, q)| p[0].abs_diff(q[0]) > 12).count();
    assert!(difference(&lit, &dark) > 5000, "lights must change poster pixels");
    press(&mut shell, Wall::Destination);
    press(&mut shell, Wall::Tools);
    press(&mut shell, Wall::BackdropBlur(true));
    press(&mut shell, Wall::WallLook(WallLook::Partial));
    std::thread::sleep(Duration::from_millis(900));
    let mut pixels = Vec::new();
    for _ in 0..7 {
        pixels = frame(&mut shell, None).0;
        let deadline = Instant::now() + Duration::from_secs(30);
        while folia_render::gpu_builds().0 > 0 {
            assert!(Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(4));
        }
    }
    assert!(folia_render::quality::target_sizes().len() >= 2, "blur and glass must have isolated targets");
    press(&mut shell, Wall::WallLook(WallLook::Clear));
    std::thread::sleep(Duration::from_millis(900));
    let (clear, _) = frame(&mut shell, None);
    assert!(difference(&pixels, &clear) > 5000, "clear mode must expose the player through content tiles");
    assert_eq!(AppearanceSettings::load(&store).unwrap().wall.look, WallLook::Clear);
    drop(shell.update(Message::Navigate(Page::Lattice)));
    frame(&mut shell, None);
    assert!(folia_render::quality::target_sizes().is_empty(), "leaving the library retained an offscreen target");
}

#[test]
#[cfg_attr(feature = "ci", ignore = "needs a hardware GPU")]
fn hovering_a_loaded_cover_preserves_the_rest_of_the_wall() {
    if super::player_fixture::run_gpu_child("wall_tools::hovering_a_loaded_cover_preserves_the_rest_of_the_wall") {
        return;
    }
    folia_ui::install_fonts();
    let temp = tempfile::tempdir().unwrap();
    let mut init = super::player_fixture::init(temp.path());
    init.general.home_tab = folia_store::HomeTab::Local;
    init.appearance.wall.tint = false;
    init.appearance.wall.lights_on = true;
    init.appearance.set_background_mode(Some("common"));
    let mut library = folia_library::Library::open(&init.library_db).unwrap();
    image::RgbaImage::from_pixel(1024, 1024, image::Rgba([30, 70, 140, 255]))
        .save(temp.path().join("cover.png"))
        .unwrap();
    for index in 0..12 {
        std::fs::write(temp.path().join(format!("Song{index:02}.wav")), b"RIFF").unwrap();
    }
    library.add_folder(temp.path()).unwrap();
    library.scan().unwrap();
    let runtime = tokio::runtime::Builder::new_multi_thread().enable_all().build().unwrap();
    let _context = runtime.enter();
    assert!(folia_library::cover_for(&library.tracks().unwrap()[0].path, 256, 256).is_some());
    let (_registration, _device, mut renderer) = renderer();
    let (mut shell, initial) = Shell::new(init);
    finish_wall_tasks(&mut shell, &renderer, initial);
    let window = iced::window::Id::unique();
    let task = shell.update(Message::WindowReady(Some(window)));
    finish_wall_tasks(&mut shell, &renderer, task);
    std::thread::sleep(Duration::from_millis(900));
    let task = shell.update(Message::SplashFrame);
    finish_wall_tasks(&mut shell, &renderer, task);
    let mut cache = Cache::default();
    for _ in 0..3 {
        wall_frame(&mut shell, &mut renderer, &mut cache);
        std::thread::sleep(Duration::from_millis(300));
    }
    let reference = wall_frame(&mut shell, &mut renderer, &mut cache);
    let sample = (100 * 1200 + 100) * 4;
    assert!(reference[sample + 2] > reference[sample] + 60, "fixture must render the loaded blue cover");
    let mut largest_change = 0;
    let mut hovered_change = 0;
    for pointer in [Some(Point::new(1000.0, 450.0)), None, Some(Point::new(900.0, 550.0)), None] {
        drop(shell.update(Message::Pointer(window, pointer)));
        for _ in 0..18 {
            let pixels = wall_frame(&mut shell, &mut renderer, &mut cache);
            let changed = (40..300)
                .flat_map(|y| (40..300).map(move |x| (y * 1200 + x) * 4))
                .filter(|&i| (0..3).any(|c| reference[i + c].abs_diff(pixels[i + c]) > 5))
                .count();
            largest_change = largest_change.max(changed);
            if pointer.is_some() {
                let sample = (500 * 1200 + 1100) * 4;
                hovered_change = hovered_change.max(reference[sample + 2].abs_diff(pixels[sample + 2]));
            }
            std::thread::sleep(Duration::from_millis(40));
        }
    }
    assert!(largest_change < 100, "hover flashed {largest_change} pixels far from the hovered poster");
    assert!(hovered_change > 5, "the hovered cover must still receive the original brightness filter");
    assert_tools_do_not_flash(&mut shell, &mut renderer, &mut cache, &reference);
    drop(shell.update(Message::OpenFiles(library.tracks().unwrap())));
    press(&mut shell, Wall::Destination);
    for _ in 0..4 {
        wall_frame(&mut shell, &mut renderer, &mut cache);
        std::thread::sleep(Duration::from_millis(300));
    }
    let reference = wall_frame(&mut shell, &mut renderer, &mut cache);
    assert_tools_do_not_flash(&mut shell, &mut renderer, &mut cache, &reference);
}

fn assert_tools_do_not_flash(shell: &mut Shell, renderer: &mut iced::Renderer, cache: &mut Cache, reference: &[u8]) {
    for attempt in 0..4 {
        press(shell, Wall::Tools);
        for frame in 0..24 {
            let pixels = wall_frame(shell, renderer, cache);
            let changed = (40..300)
                .flat_map(|y| (40..300).map(move |x| (y * 1200 + x) * 4))
                .filter(|&i| (0..3).any(|c| reference[i + c].abs_diff(pixels[i + c]) > 5))
                .count();
            assert!(changed < 100, "tools toggle {attempt} frame {frame} flashed {changed} distant cover pixels");
            std::thread::sleep(Duration::from_millis(16));
        }
    }
}

fn finish_wall_tasks(shell: &mut Shell, renderer: &iced::Renderer, initial: iced::Task<Message>) {
    use iced::advanced::image::Renderer as _;
    use iced::futures::StreamExt;
    let mut tasks = vec![initial];
    while let Some(task) = tasks.pop() {
        let messages = pollster::block_on(async {
            let mut messages = Vec::new();
            if let Some(mut stream) = iced_runtime::task::into_stream(task) {
                while let Some(action) = stream.next().await {
                    match action {
                        iced_runtime::Action::Output(message) => messages.push(message),
                        iced_runtime::Action::Image(iced_runtime::image::Action::Allocate(handle, sender)) => {
                            sender.send(renderer.load_image(&handle)).unwrap();
                        }
                        _ => {}
                    }
                }
            }
            messages
        });
        tasks.extend(messages.into_iter().map(|message| shell.update(message)));
    }
}

fn wall_frame(shell: &mut Shell, renderer: &mut iced::Renderer, cache: &mut Cache) -> Vec<u8> {
    let mut messages = Vec::new();
    let mut ui = UserInterface::build(shell.view(), Size::new(1200.0, 800.0), std::mem::take(cache), renderer);
    let cursor = iced::mouse::Cursor::Unavailable;
    ui.update(
        &[Event::Window(iced::window::Event::RedrawRequested(Instant::now()))],
        cursor,
        renderer,
        &mut iced::advanced::clipboard::Null,
        &mut messages,
    );
    ui.draw(renderer, &shell.theme(), &iced::advanced::renderer::Style { text_color: Color::BLACK }, cursor);
    *cache = ui.into_cache();
    let viewport = iced::advanced::graphics::Viewport::with_physical_size(Size::new(1200, 800), 1.0);
    let pixels = renderer.screenshot(&viewport, Color::BLACK);
    for message in messages {
        let task = shell.update(message);
        finish_wall_tasks(shell, renderer, task);
    }
    pixels
}

fn renderer() -> (std::sync::Arc<folia_render::quality::DeviceRegistration>, wgpu::Device, iced::Renderer) {
    let (adapter, device, queue) = pollster::block_on(folia_render::backend::device()).unwrap();
    let registration =
        folia_render::quality::register_device(&adapter, &device, iced_wgpu::graphics::Shell::headless());
    let renderer = iced_wgpu::Renderer::new(
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
    (registration, device, renderer)
}
