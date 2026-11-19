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
fn radio_escape_returns_to_current_before_leaving() {
    use folia_ui::{CardDirection, CardKey, Lattice, LatticeMessage, Playback, PlaybackMessage, WallMessage};
    folia_ui::install_fonts();
    let mut player = Playback::new(Default::default(), None);
    player.queue.replace((0..6).map(|id| Track { id, title: format!("Song {id}"), ..Default::default() }).collect(), 2);
    let before = player.queue.clone();
    let appearance = folia_store::WallAppearance { radio: true, ..Default::default() };
    let i18n = folia_i18n::I18n::new(folia_core::Language::ZhCn).unwrap();
    let mut page = Lattice::default();
    drop(page.sync(&player, appearance, &i18n, None));
    drop(page.update(LatticeMessage::Wall(WallMessage::Viewport(Size::new(1200.0, 800.0))), None));
    assert!(page.update(LatticeMessage::Key(CardKey::Move(CardDirection::Right)), None).1.is_none());
    assert!(
        page.update(LatticeMessage::Key(CardKey::Escape), None).1.is_none(),
        "Esc while browsing must return to the playing card"
    );
    assert!(matches!(
        page.update(LatticeMessage::Key(CardKey::Enter), None).1,
        Some(Message::Playback(PlaybackMessage::Toggle))
    ));
    assert!(matches!(page.update(LatticeMessage::Key(CardKey::Escape), None).1, Some(Message::Back)));
    assert_eq!(player.queue.items(), before.items());
    assert_eq!(player.queue.current_index(), before.current_index());
    assert_eq!(player.queue.revision(), before.revision());
}

#[test]
fn radio_geometry_and_input_follow_the_reference_contract() {
    use folia_ui::{
        CardDirection, CardKey, Lattice, LatticeMessage, Playback, PlaybackMessage, WallMessage,
        radio::{Input, Metrics},
    };
    let desktop = Metrics::new(Size::new(1920.0, 1032.0));
    assert_eq!(desktop.now_size, 578.0);
    assert_eq!(desktop.card_size, 335.0);
    assert_eq!(desktop.gap, 35.0);
    let center = desktop.rect(0.0);
    assert!((center.bounds.x - 671.0).abs() < 0.01 && (center.bounds.y - 278.6).abs() < 0.01);
    assert_eq!(desktop.rect(-2.0).rotate, -3.2);
    assert_eq!(desktop.rect(2.0).rotate, 3.2);
    let side = desktop.rect(1.0);
    let angle = side.rotate.to_radians();
    let down = iced::Vector::new(-angle.sin(), angle.cos());
    let endpoint = side.bounds.center() + down * (side.bounds.height / 2.0 + desktop.station_drop(side));
    assert!((endpoint.distance(desktop.wave_center()) - desktop.station_radius()).abs() < 0.01);
    let half = desktop.rect(0.5);
    assert!((half.bounds.x - (center.bounds.x + desktop.rect(1.0).bounds.x) / 2.0).abs() < 0.01);
    let phone = Metrics::new(Size::new(390.0, 844.0));
    assert_eq!((phone.now_size, phone.card_size, phone.gap), (342.0, 137.0, 12.0));
    folia_ui::install_fonts();
    let mut player = Playback::new(Default::default(), None);
    player.queue.replace((0..8).map(|id| Track { id, title: format!("Song {id}"), ..Default::default() }).collect(), 2);
    let before = player.queue.clone();
    let appearance = folia_store::WallAppearance { radio: true, ..Default::default() };
    let i18n = folia_i18n::I18n::new(folia_core::Language::ZhCn).unwrap();
    let mut page = Lattice::default();
    drop(page.sync(&player, appearance, &i18n, None));
    drop(page.update(LatticeMessage::Wall(WallMessage::Viewport(Size::new(1200.0, 800.0))), None));
    let chosen = |page: &mut Lattice| page.update(LatticeMessage::Key(CardKey::Enter), None).1;
    for delta in [30.0, 30.0] {
        drop(page.update(LatticeMessage::Radio(Input::Wheel(delta)), None));
    }
    assert!(matches!(chosen(&mut page), Some(Message::Playback(PlaybackMessage::Toggle))));
    drop(page.update(LatticeMessage::Radio(Input::Wheel(30.0)), None));
    assert!(matches!(chosen(&mut page), Some(Message::Playback(PlaybackMessage::Jump(3)))));
    drop(page.update(LatticeMessage::Radio(Input::Wheel(100.0)), None));
    assert!(matches!(chosen(&mut page), Some(Message::Playback(PlaybackMessage::Jump(3)))));
    drop(page.update(LatticeMessage::Locate, None));
    std::thread::sleep(Duration::from_millis(920));
    drop(page.update(LatticeMessage::Radio(Input::Down(Point::new(600.0, 440.0))), None));
    drop(page.update(LatticeMessage::Radio(Input::Drag(Point::new(540.0, 440.0))), None));
    drop(page.update(LatticeMessage::Radio(Input::Up(Point::new(540.0, 440.0))), None));
    assert!(
        matches!(chosen(&mut page), Some(Message::Playback(PlaybackMessage::Jump(3)))),
        "a short horizontal fling advances one slot"
    );
    drop(page.update(LatticeMessage::Key(CardKey::Move(CardDirection::Up)), None));
    assert!(matches!(chosen(&mut page), Some(Message::Playback(PlaybackMessage::Jump(3)))));
    assert_eq!(player.queue.items(), before.items());
    assert_eq!(player.queue.revision(), before.revision());
}

#[test]
fn radio_wrapping_and_fm_keep_the_playback_identity() {
    use folia_ui::{CardDirection, CardKey, Lattice, LatticeMessage, Playback, PlaybackMessage, WallMessage};
    folia_ui::install_fonts();
    let i18n = folia_i18n::I18n::new(folia_core::Language::ZhCn).unwrap();
    let mut player = Playback::new(Default::default(), None);
    let tracks = (0..3).map(|id| Track { id, title: format!("Track {id}"), ..Default::default() }).collect::<Vec<_>>();
    player.queue.replace(tracks.clone(), 0);
    player.queue.repeat = folia_core::Repeat::All;
    let mut page = Lattice::default();
    let appearance = folia_store::WallAppearance { radio: true, ..Default::default() };
    drop(page.sync(&player, appearance, &i18n, None));
    drop(page.update(LatticeMessage::Wall(WallMessage::Viewport(Size::new(1200.0, 800.0))), None));
    drop(page.update(LatticeMessage::Key(CardKey::Move(CardDirection::Left)), None));
    assert!(matches!(
        page.update(LatticeMessage::Key(CardKey::Enter), None).1,
        Some(Message::Playback(PlaybackMessage::Jump(2)))
    ));
    player.queue.select(2);
    drop(page.sync(&player, appearance, &i18n, None));
    assert!(matches!(
        page.update(LatticeMessage::Key(CardKey::Enter), None).1,
        Some(Message::Playback(PlaybackMessage::Toggle))
    ));
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
    drop(player.play_fm(tracks.clone(), fm, folia_core::Origin { name: "FM".into() }));
    drop(page.sync(&player, Default::default(), &i18n, None));
    drop(page.update(LatticeMessage::Key(CardKey::Move(CardDirection::Left)), None));
    assert!(
        matches!(
            page.update(LatticeMessage::Key(CardKey::Enter), None).1,
            Some(Message::Playback(PlaybackMessage::Toggle))
        ),
        "FM never wraps before its queue head"
    );
    drop(page.update(LatticeMessage::Key(CardKey::Move(CardDirection::Right)), None));
    assert!(matches!(
        page.update(LatticeMessage::Key(CardKey::Enter), None).1,
        Some(Message::Playback(PlaybackMessage::Jump(1)))
    ));
    assert!(player.is_fm());
    assert_eq!(player.queue.items(), tracks);
    assert_eq!(player.queue.current_index(), Some(0));
}

#[test]
fn radio_tilt_keeps_momentum_across_pointer_updates() {
    use folia_ui::radio::Tilt;
    let start = Instant::now();
    let mut samples = Vec::new();
    for rate in [60, 120, 1000] {
        let mut tilt = Tilt::new(start);
        for step in 0..=rate / 2 {
            let t = step as f64 / rate as f64;
            tilt.retarget([0.0, 11.2 * t as f32, 0.98], start + Duration::from_secs_f64(t));
        }
        let value = tilt.at(start + Duration::from_millis(500));
        eprintln!("RADIO_TILT rate={rate} angle={} scale={}", value[1], value[2]);
        samples.push(value);
    }
    let spread = samples.iter().map(|v| v[1]).fold(f32::NEG_INFINITY, f32::max)
        - samples.iter().map(|v| v[1]).fold(f32::INFINITY, f32::min);
    assert!(spread < 0.12, "the same pointer path depends on event rate: {samples:?}");
    assert!(samples.iter().all(|v| v[1] > 4.5 && v[1] < 4.9 && (v[2] - 0.98).abs() < 0.0001));

    let mut tilt = Tilt::new(start);
    tilt.retarget([0.0, 5.6, 0.98], start);
    let turn = start + Duration::from_millis(80);
    let before = tilt.at(turn - Duration::from_micros(100))[1];
    let at = tilt.at(turn)[1];
    tilt.retarget([0.0, -5.6, 0.98], turn);
    assert!((tilt.at(turn)[1] - at).abs() < 0.00001);
    let after = tilt.at(turn + Duration::from_micros(100))[1];
    let speed_before = (at - before) / 0.0001;
    let speed_after = (after - at) / 0.0001;
    assert!((speed_before - speed_after).abs() < 1.0, "retarget reset velocity: {speed_before} -> {speed_after}");
    tilt.retarget([0.0, 0.0, 1.0], turn + Duration::from_millis(40));
    assert_eq!(tilt.at(start + Duration::from_secs(2)), [0.0, 0.0, 1.0]);
}

#[test]
#[cfg_attr(feature = "ci", ignore = "needs a hardware GPU")]
fn radio_draws_the_transmitter_and_animates_focus_without_playing() {
    if super::player_fixture::run_gpu_child(
        "wall_tools::radio_draws_the_transmitter_and_animates_focus_without_playing",
    ) {
        return;
    }
    use folia_ui::{CardKey, Lattice, LatticeMessage, Playback, PlaybackMessage, WallMessage};
    folia_ui::install_fonts();
    let covers = tempfile::tempdir().unwrap();
    image::RgbaImage::from_pixel(1024, 1024, image::Rgba([30, 70, 140, 255]))
        .save(covers.path().join("cover.png"))
        .unwrap();
    let mut player = Playback::new(Default::default(), None);
    player.queue.replace(
        (0..8)
            .map(|id| Track {
                id,
                title: format!("Song {id}"),
                artist: Some("Artist".into()),
                duration: 240.0,
                path: covers.path().join(format!("song{id}.wav")),
                ..Default::default()
            })
            .collect(),
        2,
    );
    let before = player.queue.clone();
    let mut appearance = AppearanceSettings::default();
    appearance.wall.radio = true;
    appearance.wall.vignette = false;
    appearance.wall.tint = false;
    let theme = folia_core::Theme::DAYLIGHT;
    let i18n = folia_i18n::I18n::new(folia_core::Language::ZhCn).unwrap();
    let mut page = Lattice::default();
    drop(page.sync(&player, appearance.wall, &i18n, None));
    let load = page.update(LatticeMessage::Wall(WallMessage::Viewport(Size::new(1200.0, 800.0))), None).0;
    let (_registration, _device, mut renderer) = renderer();
    finish_radio_tasks(&mut page, &renderer, load);
    let viewport = iced::advanced::graphics::Viewport::with_physical_size(Size::new(1200, 800), 1.0);
    let mut cache = Cache::default();
    let mut frame = |page: &mut Lattice, appearance: &AppearanceSettings, click: Option<Point>| {
        let cursor = click.map_or(iced::mouse::Cursor::Unavailable, iced::mouse::Cursor::Available);
        let mut ui = UserInterface::build(
            page.view(&i18n, &theme, appearance, &player, Size::new(1200.0, 800.0)),
            Size::new(1200.0, 800.0),
            std::mem::take(&mut cache),
            &mut renderer,
        );
        let mut events = vec![Event::Window(iced::window::Event::RedrawRequested(Instant::now()))];
        if click.is_some() {
            events.extend([
                Event::Mouse(iced::mouse::Event::ButtonPressed(iced::mouse::Button::Left)),
                Event::Mouse(iced::mouse::Event::ButtonReleased(iced::mouse::Button::Left)),
            ]);
        }
        let mut messages = Vec::new();
        ui.update(&events, cursor, &mut renderer, &mut iced::advanced::clipboard::Null, &mut messages);
        ui.draw(
            &mut renderer,
            &iced::Theme::Light,
            &iced::advanced::renderer::Style { text_color: Color::BLACK },
            cursor,
        );
        cache = ui.into_cache();
        let pixels = renderer.screenshot(&viewport, Color::BLACK);
        let effects = messages.into_iter().filter_map(|m| page.update(m, None).1).collect::<Vec<_>>();
        (pixels, effects)
    };
    frame(&mut page, &appearance, None);
    std::thread::sleep(Duration::from_millis(1400));
    for _ in 0..90 {
        frame(&mut page, &appearance, None);
        if !page.animating(Instant::now() + Duration::from_secs(2)) {
            break;
        }
        std::thread::sleep(Duration::from_millis(16));
    }
    assert!(
        !page.animating(Instant::now() + Duration::from_secs(2)),
        "the radio projection must become ready after its image uploads"
    );
    let (rest, _) = frame(&mut page, &appearance, None);
    let core = (104 * 1200 + 218) * 4;
    assert!(
        rest[core] > 180 && rest[core + 1] < 150 && rest[core + 2] < 100,
        "the transmitter core must use the theme accent: {:?}",
        &rest[core..core + 4]
    );
    let paper = (100 * 1200 + 700) * 4;
    assert!(
        (i32::from(rest[paper]) - 245).abs() < 6 && (i32::from(rest[paper + 2]) - 244).abs() < 6,
        "radio must use paper instead of the poster wash"
    );
    let slope = (-9.0_f32).to_radians().tan();
    for x in [960, 984, 1008] {
        let center = (800.0 * 0.94 + 3.5 + slope * (x as f32 - 1200.0 * 0.75)).round() as usize;
        let ink = (center - 10..=center + 10)
            .filter(|&y| rest[(y * 1200 + x) * 4..(y * 1200 + x) * 4 + 3].iter().all(|&c| c < 160))
            .count();
        assert!((5..=12).contains(&ink), "the 7px diagonal ink strip filled {ink} rows at x={x}");
    }
    let tick_x = 1200 * 3 / 4 + 36;
    let tick_y = (800.0 * 0.94 - 24.0 + 3.5 + slope * 14.0).round() as usize;
    let accent_rows = (tick_y - 10..=tick_y + 10)
        .filter(|&y| {
            let i = (y * 1200 + tick_x) * 4;
            rest[i] > 180 && rest[i + 1] < 150 && rest[i + 2] < 100
        })
        .count();
    assert!((5..=10).contains(&accent_rows), "the independent accent strip collapsed to {accent_rows} rows");
    let side = folia_ui::radio::Metrics::new(Size::new(1200.0, 800.0)).rect(1.0).bounds;
    let corner = ((side.y as usize + 5) * 1200 + side.x as usize + 1) * 4;
    assert!(
        (i32::from(rest[corner]) - 245).abs() < 8,
        "the side card must rotate as a complete face, leaving its old corner clear: {:?}",
        &rest[corner..corner + 4]
    );
    page.pointer(Some(Point::new(side.x + 35.0, side.y + 35.0)), Instant::now());
    std::thread::sleep(Duration::from_millis(200));
    let (tilted, _) = frame(&mut page, &appearance, None);
    let tilt_changes = (side.y as usize..(side.y + side.height) as usize)
        .flat_map(|y| (side.x as usize..(side.x + side.width) as usize).map(move |x| (y * 1200 + x) * 4))
        .filter(|&i| rest[i..i + 3] != tilted[i..i + 3])
        .count();
    assert!(tilt_changes > 100, "pointer tilt must project the composed card");
    page.pointer(None, Instant::now());
    let (_, effects) = frame(&mut page, &appearance, Some(Point::new(980.0, 500.0)));
    assert!(effects.is_empty(), "browsing a side card must not play");
    std::thread::sleep(Duration::from_millis(150));
    let (moving, _) = frame(&mut page, &appearance, None);
    std::thread::sleep(Duration::from_millis(800));
    drop(page.update(LatticeMessage::Frame, None));
    let (landed, _) = frame(&mut page, &appearance, None);
    let changed = |a: &[u8], b: &[u8]| {
        (220..650)
            .flat_map(|y| (350..850).map(move |x| (y * 1200 + x) * 4))
            .filter(|&i| a[i..i + 3] != b[i..i + 3])
            .count()
    };
    assert!(
        changed(&rest, &moving) > 1000 && changed(&moving, &landed) > 1000,
        "focus must have an intermediate moving frame"
    );
    assert!(matches!(
        page.update(LatticeMessage::Key(CardKey::Enter), None).1,
        Some(Message::Playback(PlaybackMessage::Jump(3)))
    ));
    let (_, effects) = frame(&mut page, &appearance, Some(Point::new(770.0, 610.0)));
    assert!(
        effects.iter().any(|e| matches!(e, Message::Playback(PlaybackMessage::Jump(3)))),
        "the central play button must play the browsed identity"
    );
    appearance.wall.hide_upcoming = true;
    drop(page.sync(&player, appearance.wall, &i18n, None));
    std::thread::sleep(Duration::from_millis(600));
    let (_, effects) = frame(&mut page, &appearance, Some(Point::new(770.0, 610.0)));
    assert!(!effects.iter().any(|e| matches!(e, Message::Playback(_))), "a hidden future card has no play hint");
    assert_eq!(player.queue.items(), before.items());
    assert_eq!(player.queue.current_index(), before.current_index());
    assert_eq!(player.queue.revision(), before.revision());
    assert!(
        !folia_render::quality::target_sizes().is_empty(),
        "radio must compose its live faces in the projection target"
    );
    appearance.wall.radio = false;
    drop(page.sync(&player, appearance.wall, &i18n, None));
    let (leaving, _) = frame(&mut page, &appearance, None);
    std::thread::sleep(Duration::from_millis(1100));
    drop(page.update(LatticeMessage::Frame, None));
    frame(&mut page, &appearance, None);
    let (poster, _) = frame(&mut page, &appearance, None);
    assert!(changed(&leaving, &poster) > 1000, "radio departure must survive as the poster wall arrives");
    assert!(folia_render::quality::target_sizes().is_empty(), "leaving the radio must release its projection target");
    appearance.wall.radio = true;
    drop(page.sync(&player, appearance.wall, &i18n, None));
    let (closing, _) = frame(&mut page, &appearance, None);
    std::thread::sleep(Duration::from_millis(1900));
    drop(page.update(LatticeMessage::Frame, None));
    frame(&mut page, &appearance, None);
    let (returned, _) = frame(&mut page, &appearance, None);
    assert!(changed(&closing, &returned) > 1000, "poster handoff must hold the radio deal until the closing wave");
    assert_eq!(player.queue, before);
    let prefs = tempfile::tempdir().unwrap();
    let mut init = super::player_fixture::init(prefs.path());
    init.graphics.motion.collection_morph = true;
    let (_host, initial) = Shell::new(init);
    drop(initial);
    let mut static_page = Lattice::default();
    drop(static_page.sync(&player, appearance.wall, &i18n, None));
    drop(static_page.update(LatticeMessage::Wall(WallMessage::Viewport(Size::new(1200.0, 800.0))), None));
    for _ in 0..90 {
        frame(&mut static_page, &appearance, None);
        if !static_page.animating(Instant::now()) {
            break;
        }
        std::thread::sleep(Duration::from_millis(16));
    }
    assert!(!static_page.animating(Instant::now()), "reduced motion must stop after preparing the faces");
    drop(static_page.update(LatticeMessage::Key(CardKey::Move(folia_ui::CardDirection::Right)), None));
    assert!(matches!(
        static_page.update(LatticeMessage::Key(CardKey::Enter), None).1,
        Some(Message::Playback(PlaybackMessage::Jump(3)))
    ));
    assert!(!static_page.animating(Instant::now()), "reduced motion must not start a rail or disclosure spring");
}

fn finish_radio_tasks(
    page: &mut folia_ui::Lattice,
    renderer: &iced::Renderer,
    initial: iced::Task<folia_ui::LatticeMessage>,
) {
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
        tasks.extend(messages.into_iter().map(|message| page.update(message, None).0));
    }
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

#[test]
fn lattice_keeps_the_selected_song_when_the_queue_changes() {
    use folia_ui::{CardKey, Lattice, LatticeMessage, Playback, PlaybackMessage, WallMessage};
    let i18n = folia_i18n::I18n::new(folia_core::Language::ZhCn).unwrap();
    let mut player = Playback::new(Default::default(), None);
    player
        .queue
        .replace((1..=3).map(|id| Track { id, title: format!("Song {id}"), ..Default::default() }).collect(), 1);
    let mut page = Lattice::default();
    let mut appearance = folia_store::WallAppearance::default();
    drop(page.sync(&player, appearance, &i18n, None));
    drop(page.update(LatticeMessage::Wall(WallMessage::Viewport(Size::new(1200.0, 800.0))), None));
    let enter = |page: &mut Lattice| page.update(LatticeMessage::Key(CardKey::Enter), None).1;
    assert!(
        matches!(enter(&mut page), Some(Message::Playback(PlaybackMessage::Toggle))),
        "entry must open the current song"
    );
    appearance.auto_focus = false;
    player.queue.select(2);
    drop(page.sync(&player, appearance, &i18n, None));
    assert!(
        matches!(enter(&mut page), Some(Message::Playback(PlaybackMessage::Jump(1)))),
        "a song change must respect disabled following"
    );
    assert!(player.queue.move_to(1, 0));
    drop(page.sync(&player, appearance, &i18n, None));
    assert!(
        matches!(enter(&mut page), Some(Message::Playback(PlaybackMessage::Jump(0)))),
        "selection follows identity through a queue edit"
    );
    player.queue.remove(0);
    drop(page.sync(&player, appearance, &i18n, None));
    assert!(enter(&mut page).is_none(), "a removed selection must not act on its replacement");
    appearance.auto_focus = true;
    drop(page.sync(&player, appearance, &i18n, None));
    assert!(enter(&mut page).is_none(), "enabling follow must wait for the next song change");
    player.queue.select(0);
    drop(page.sync(&player, appearance, &i18n, None));
    assert!(matches!(enter(&mut page), Some(Message::Playback(PlaybackMessage::Toggle))));
}

#[test]
#[cfg_attr(feature = "ci", ignore = "requires an audio output device")]
fn lattice_lyric_clock_advances_at_the_visual_frame_rate() {
    use iced::futures::{StreamExt, stream};
    let runtime = tokio::runtime::Builder::new_multi_thread().enable_all().build().unwrap();
    let _context = runtime.enter();
    let temp = tempfile::tempdir().unwrap();
    let mut init = silent_wall_init(temp.path(), 30);
    init.appearance.visualizer = "still".into();
    let (mut shell, initial) = Shell::new(init);
    let settle = |shell: &mut Shell, initial| {
        let mut tasks = vec![initial];
        while let Some(task) = tasks.pop() {
            let messages = pollster::block_on(async {
                let Some(stream) = iced_runtime::task::into_stream(task) else {
                    return Vec::new();
                };
                stream
                    .filter_map(|action| async {
                        if let iced_runtime::Action::Output(message) = action { Some(message) } else { None }
                    })
                    .collect::<Vec<_>>()
                    .await
            });
            tasks.extend(messages.into_iter().map(|message| shell.update(message)));
        }
    };
    settle(&mut shell, initial);
    drop(shell.update(Message::SplashFrame));
    std::thread::sleep(Duration::from_millis(200));
    drop(shell.update(Message::SplashFrame));
    let task = shell.update(Message::Playback(folia_ui::PlaybackMessage::Tick));
    settle(&mut shell, task);
    let task = shell.update(Message::Home(Wall::Destination));
    settle(&mut shell, task);
    let task = shell.update(Message::Lattice(
        0,
        folia_ui::LatticeMessage::Wall(folia_ui::WallMessage::Viewport(Size::new(1200.0, 800.0))),
    ));
    settle(&mut shell, task);
    std::thread::sleep(Duration::from_millis(700));
    let task = shell.update(Message::Playback(folia_ui::PlaybackMessage::Tick));
    settle(&mut shell, task);
    let count = |shell: &Shell| {
        pollster::block_on(async {
            let recipes = iced::advanced::subscription::into_recipes(shell.subscription());
            let mut messages = stream::select_all(recipes.into_iter().map(|recipe| {
                let input = stream::unfold(iced::window::Id::unique(), |window| async move {
                    tokio::time::sleep(Duration::from_millis(16)).await;
                    Some((
                        iced::advanced::subscription::Event::Interaction {
                            window,
                            event: Event::Window(iced::window::Event::RedrawRequested(Instant::now())),
                            status: iced::event::Status::Ignored,
                        },
                        window,
                    ))
                })
                .boxed();
                recipe.stream(input)
            }));
            let deadline = Instant::now() + Duration::from_millis(1100);
            let mut counts = [0, 0, 0];
            while let Ok(Some(message)) =
                tokio::time::timeout(deadline.saturating_duration_since(Instant::now()), messages.next()).await
            {
                match message {
                    Message::VisualFrame => counts[0] += 1,
                    Message::Playback(folia_ui::PlaybackMessage::Tick) => counts[1] += 1,
                    Message::Lattice(_, folia_ui::LatticeMessage::Frame) => counts[2] += 1,
                    _ => {}
                }
                if Instant::now() >= deadline {
                    break;
                }
            }
            counts
        })
    };
    let moving = count(&shell);
    eprintln!("lattice clock: visual={}, playback={}, wall={}", moving[0], moving[1], moving[2]);
    assert!(
        moving[0] + moving[1] >= 30,
        "lyric time is only refreshed {} times despite {} wall redraws",
        moving[0] + moving[1],
        moving[2]
    );
    assert_eq!(moving[2], 0, "wall motion must share the lyric frame clock");
    let task = shell.update(Message::FrameRate(folia_store::FrameRate::Fps30));
    settle(&mut shell, task);
    let limited = count(&shell);
    eprintln!("lattice 30fps: visual={}, playback={}, wall={}", limited[0], limited[1], limited[2]);
    assert!((20..=38).contains(&limited[0]), "the Lattice clock must respect the selected frame limit");
    let task = shell.update(Message::Home(Wall::Radio(true)));
    settle(&mut shell, task);
    let radio = count(&shell);
    eprintln!("radio 30fps: visual={}, playback={}, wall={}", radio[0], radio[1], radio[2]);
    assert!(
        (20..=38).contains(&radio[0]) && radio[1] == 0 && radio[2] == 0,
        "radio must use the same single limited visual clock"
    );
    let task = shell.update(Message::Home(Wall::Radio(false)));
    settle(&mut shell, task);
    std::thread::sleep(Duration::from_millis(700));
    for _ in 0..2 {
        let task = shell.update(Message::VisualFrame);
        settle(&mut shell, task);
    }
    let task = shell.update(Message::Playback(folia_ui::PlaybackMessage::Toggle));
    settle(&mut shell, task);
    let paused = count(&shell);
    assert_eq!(paused[0] + paused[1], 0, "paused lyrics must stop their clock");
}

#[test]
#[cfg_attr(feature = "ci", ignore = "needs a hardware GPU")]
fn lattice_card_draws_lyrics_and_routes_transport_without_changing_queue() {
    if super::player_fixture::run_gpu_child(
        "wall_tools::lattice_card_draws_lyrics_and_routes_transport_without_changing_queue",
    ) {
        return;
    }
    use folia_ui::{Lattice, LatticeMessage, Playback, PlaybackMessage, WallMessage};
    folia_ui::install_fonts();
    let mut player = Playback::new(Default::default(), None);
    player.queue.replace(
        vec![Track {
            id: 1,
            title: "Queue poster".into(),
            artist: Some("Artist".into()),
            duration: 240.0,
            ..Default::default()
        }],
        0,
    );
    player.status.state = folia_player::PlayerState::Playing;
    player.status.duration = 240.0;
    player.status.position = 1.1;
    player.lyrics = Some(folia_core::Lyrics {
        lines: vec![
            folia_core::Line {
                text: "A line that lights up".into(),
                translation: Some("一句逐字亮起的歌词".into()),
                start: 0.0,
                end: 4.0,
                words: vec![folia_core::Word { text: "A line that lights up".into(), start: 0.0, end: 4.0 }],
                ..Default::default()
            },
            folia_core::Line { text: "The next line stays nearby".into(), start: 4.0, end: 8.0, ..Default::default() },
        ],
        ..Default::default()
    });
    let before = player.queue.clone();
    let appearance = AppearanceSettings::default();
    let i18n = folia_i18n::I18n::new(folia_core::Language::ZhCn).unwrap();
    let theme = appearance.theme.theme();
    let mut page = Lattice::default();
    drop(page.sync(&player, appearance.wall, &i18n, None));
    drop(page.update(LatticeMessage::Wall(WallMessage::Viewport(Size::new(1200.0, 800.0))), None));
    let (_registration, _device, mut renderer) = renderer();
    let mut cache = Cache::default();
    let mut frame = |page: &mut Lattice, player: &Playback, click: Option<Point>| {
        let cursor = click.map_or(iced::mouse::Cursor::Unavailable, iced::mouse::Cursor::Available);
        let mut messages = Vec::new();
        let mut ui = UserInterface::build(
            page.view(&i18n, &theme, &appearance, player, Size::new(1200.0, 800.0)),
            Size::new(1200.0, 800.0),
            std::mem::take(&mut cache),
            &mut renderer,
        );
        let mut events = vec![Event::Window(iced::window::Event::RedrawRequested(Instant::now()))];
        if click.is_some() {
            events.extend([
                Event::Mouse(iced::mouse::Event::ButtonPressed(iced::mouse::Button::Left)),
                Event::Mouse(iced::mouse::Event::ButtonReleased(iced::mouse::Button::Left)),
            ]);
        }
        ui.update(&events, cursor, &mut renderer, &mut iced::advanced::clipboard::Null, &mut messages);
        ui.draw(
            &mut renderer,
            &iced::Theme::Dark,
            &iced::advanced::renderer::Style { text_color: Color::WHITE },
            cursor,
        );
        cache = ui.into_cache();
        let pixels = renderer.screenshot(
            &iced::advanced::graphics::Viewport::with_physical_size(Size::new(1200, 800), 1.0),
            Color::BLACK,
        );
        let effects = messages.into_iter().filter_map(|message| page.update(message, None).1).collect::<Vec<_>>();
        (pixels, effects)
    };
    frame(&mut page, &player, None);
    std::thread::sleep(Duration::from_millis(300));
    let first = frame(&mut page, &player, None).0;
    player.status.position = 2.6;
    let second = frame(&mut page, &player, None).0;
    let changed = (220..590)
        .flat_map(|y| (330..820).map(move |x| (y * 1200 + x) * 4))
        .filter(|&i| first[i..i + 3] != second[i..i + 3])
        .count();
    assert!(changed > 100, "the expanded current poster must render timed lyrics");
    drop(page.update(LatticeMessage::Details, None));
    page.pointer(None, Instant::now());
    std::thread::sleep(Duration::from_millis(200));
    let (_, effects) = frame(&mut page, &player, Some(Point::new(410.0, 653.0)));
    assert!(
        effects.iter().any(|effect| matches!(effect, Message::Playback(PlaybackMessage::CycleRepeat))),
        "keyboard disclosure must survive the shell's pointer synchronization"
    );
    for _ in 0..2 {
        let (_, effects) = frame(&mut page, &player, Some(Point::new(600.0, 400.0)));
        assert!(
            !effects.iter().any(|effect| matches!(effect, Message::Playback(_))),
            "double clicking the playing poster must not pause"
        );
    }
    let (_, effects) = frame(&mut page, &player, Some(Point::new(334.0, 653.0)));
    assert!(
        effects.iter().any(|effect| matches!(effect, Message::Playback(PlaybackMessage::Toggle))),
        "the inline play button must toggle playback"
    );
    let (_, effects) = frame(&mut page, &player, Some(Point::new(450.0, 684.0)));
    assert!(
        effects.iter().any(
            |effect| matches!(effect, Message::Playback(PlaybackMessage::Seeking(time)) if *time > 0.0 && *time < 240.0)
        ),
        "the inline progress line must seek"
    );
    assert!(effects.iter().any(|effect| matches!(effect, Message::Playback(PlaybackMessage::SeekDone))));
    assert_eq!(player.queue, before);
}

#[test]
#[cfg(feature = "stress")]
#[ignore = "release Lattice frame and process budget"]
fn lattice_release_frame_budget() {
    measure_lattice(false, false);
}

#[test]
#[cfg(feature = "stress")]
#[ignore = "release radio projection and process budget"]
fn radio_release_frame_budget() {
    measure_lattice(true, false);
}

#[test]
#[cfg(feature = "stress")]
#[ignore = "release radio hover and process budget"]
fn radio_hover_frame_budget() {
    measure_lattice(true, true);
}

#[cfg(feature = "stress")]
fn measure_lattice(radio: bool, hover: bool) {
    assert!(!cfg!(debug_assertions), "measure an optimized build");
    use folia_ui::{Lattice, LatticeMessage, Playback, WallMessage};
    folia_ui::install_fonts();
    let temp = tempfile::tempdir().unwrap();
    let monitor = folia_store::MemoryMonitor::new(temp.path().join("memory.log"));
    let mut player = Playback::new(Default::default(), None);
    player.queue.replace(
        (1..=12)
            .map(|id| Track { id, title: format!("测试歌曲 {id}"), duration: 240.0, ..Default::default() })
            .collect(),
        0,
    );
    player.status.state = folia_player::PlayerState::Playing;
    player.status.duration = 240.0;
    player.lyrics = Some(folia_core::Lyrics {
        lines: (0..8)
            .map(|i| {
                let words = ["一行", "随着", "播放", "逐字", "移动的", "歌词"]
                    .into_iter()
                    .enumerate()
                    .map(|(n, text)| folia_core::Word {
                        text: text.into(),
                        start: i as f64 * 3.0 + n as f64 * 0.5,
                        end: i as f64 * 3.0 + (n + 1) as f64 * 0.5,
                    })
                    .collect::<Vec<_>>();
                folia_core::Line {
                    text: words.iter().map(|word| word.text.as_str()).collect(),
                    words,
                    translation: Some("A line moving with the music".into()),
                    start: i as f64 * 3.0,
                    end: (i + 1) as f64 * 3.0,
                    is_chorus: i % 3 == 1,
                    ..Default::default()
                }
            })
            .collect(),
        word_timed: true,
        ..Default::default()
    });
    let mut appearance = AppearanceSettings { theme: folia_store::ThemeMode::Light, ..Default::default() };
    appearance.wall.radio = radio;
    let mut theme = appearance.theme.theme();
    theme.primary = folia_core::Rgb::hex(0x9b3416);
    theme.accent = folia_core::Rgb::hex(0xf5a04a);
    let i18n = folia_i18n::I18n::new(folia_core::Language::ZhCn).unwrap();
    let mut page = Lattice::default();
    drop(page.sync(&player, appearance.wall, &i18n, None));
    drop(page.update(LatticeMessage::Wall(WallMessage::Viewport(Size::new(1200.0, 800.0))), None));
    if hover {
        drop(page.update(LatticeMessage::Key(folia_ui::CardKey::Move(folia_ui::CardDirection::Right)), None));
    }
    let (_registration, device, mut renderer) = renderer();
    let target = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("lattice-budget"),
        size: wgpu::Extent3d { width: 1200, height: 800, depth_or_array_layers: 1 },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        view_formats: &[],
    });
    let view = target.create_view(&Default::default());
    let viewport = iced::advanced::graphics::Viewport::with_physical_size(Size::new(1200, 800), 1.0);
    let mut cache = Cache::default();
    let mut samples = Vec::new();
    let mut present_samples = Vec::new();
    let started = Instant::now();
    println!("LATTICE_BENCH_PID={}", std::process::id());
    monitor.start(1000);
    let mut pointer_sample = 0_u64;
    while started.elapsed() < Duration::from_secs(22) {
        let frame_start = Instant::now();
        if hover {
            let end = frame_start.duration_since(started).as_millis() as u64;
            while pointer_sample <= end {
                let phase = pointer_sample as f32 / 1000.0 * std::f32::consts::TAU * 0.6;
                let at = Point::new(600.0 + 150.0 * phase.sin(), 400.0 + 120.0 * (phase * 0.7).cos());
                page.pointer(Some(at), started + Duration::from_millis(pointer_sample));
                pointer_sample += 1;
            }
        }
        player.status.position = frame_start.duration_since(started).as_secs_f64();
        if radio {
            let mut audio = folia_core::AudioFrame::SILENT;
            audio.has_spectrum = true;
            audio.bass = 160.0;
            audio.spectrum.fill(128);
            page.sample_audio(&audio, frame_start);
            drop(page.update(LatticeMessage::Frame, None));
        }
        let mut ui = UserInterface::build(
            page.view(&i18n, &theme, &appearance, &player, Size::new(1200.0, 800.0)),
            Size::new(1200.0, 800.0),
            std::mem::take(&mut cache),
            &mut renderer,
        );
        ui.draw(
            &mut renderer,
            &iced::Theme::Light,
            &iced::advanced::renderer::Style { text_color: Color::BLACK },
            iced::mouse::Cursor::Unavailable,
        );
        cache = ui.into_cache();
        let draw_ms = frame_start.elapsed().as_secs_f64() * 1000.0;
        let submitted = renderer.present(Some(Color::WHITE), wgpu::TextureFormat::Rgba8Unorm, &view, &viewport);
        device.poll(wgpu::PollType::Wait { submission_index: Some(submitted), timeout: None }).unwrap();
        let total = frame_start.elapsed();
        if started.elapsed() >= Duration::from_secs(2) {
            samples.push(total.as_secs_f64() * 1000.0);
            present_samples.push(draw_ms);
        }
        std::thread::sleep(Duration::from_secs_f64(1.0 / 60.0).saturating_sub(total));
    }
    let history = monitor.history();
    monitor.stop();
    samples.sort_by(f64::total_cmp);
    present_samples.sort_by(f64::total_cmp);
    let memory = history.points.iter().filter(|p| p.uptime_sec >= 2).collect::<Vec<_>>();
    let cpu = memory.iter().map(|p| p.cpu_percent as f64).sum::<f64>() / memory.len().max(1) as f64;
    println!(
        "LATTICE_BENCH frames={} draw_p50_ms={:.3} draw_p95_ms={:.3} frame_p50_ms={:.3} frame_p95_ms={:.3} frame_max_ms={:.3} cpu_percent={:.2} working_set_mb={} private_mb={:?}",
        samples.len(),
        present_samples[present_samples.len() / 2],
        present_samples[present_samples.len() * 95 / 100],
        samples[samples.len() / 2],
        samples[samples.len() * 95 / 100],
        samples.last().unwrap(),
        cpu,
        memory.iter().map(|p| p.working_set_mb).max().unwrap_or(0),
        memory.iter().filter_map(|p| p.private_mb).max()
    );
}

fn silent_wall_init(root: &std::path::Path, seconds: u32) -> folia_ui::Init {
    let path = root.join("silence.wav");
    let bytes = 48_000_u32 * seconds * 2;
    let mut wav = Vec::with_capacity(bytes as usize + 44);
    wav.extend_from_slice(b"RIFF");
    wav.extend_from_slice(&(bytes + 36).to_le_bytes());
    wav.extend_from_slice(b"WAVEfmt ");
    wav.extend_from_slice(&16_u32.to_le_bytes());
    for value in [1_u16, 1] {
        wav.extend_from_slice(&value.to_le_bytes());
    }
    for value in [48_000_u32, 96_000] {
        wav.extend_from_slice(&value.to_le_bytes());
    }
    for value in [2_u16, 16] {
        wav.extend_from_slice(&value.to_le_bytes());
    }
    wav.extend_from_slice(b"data");
    wav.extend_from_slice(&bytes.to_le_bytes());
    wav.resize(bytes as usize + 44, 0);
    std::fs::write(&path, wav).unwrap();
    std::fs::write(path.with_extension("lrc"), "[00:00.00]一行持续移动的歌词\n[00:10.00]下一行歌词\n").unwrap();
    let mut init = super::player_fixture::init(root);
    init.appearance.frame_rate = folia_store::FrameRate::Fps60;
    init.open_files = vec![path];
    init
}

#[test]
#[cfg(feature = "stress")]
#[ignore = "release settings-to-GPU frame limit audit"]
fn settings_frame_limit_caps_actual_gpu_presents() {
    use iced::futures::{StreamExt, stream};
    assert!(!cfg!(debug_assertions));
    folia_ui::install_fonts();
    let runtime = tokio::runtime::Builder::new_multi_thread().enable_all().build().unwrap();
    let _context = runtime.enter();
    let temp = tempfile::tempdir().unwrap();
    let init = silent_wall_init(temp.path(), 90);
    let store = init.store.clone();
    let (_registration, device, mut renderer) = renderer();
    let (mut shell, initial) = Shell::new(init);
    finish_wall_tasks(&mut shell, &renderer, initial);
    std::thread::sleep(Duration::from_millis(400));
    for message in [
        Message::SplashFrame,
        Message::Playback(folia_ui::PlaybackMessage::Tick),
        Message::Home(Wall::Destination),
        Message::Lattice(0, folia_ui::LatticeMessage::Wall(folia_ui::WallMessage::Viewport(Size::new(1200.0, 800.0)))),
        Message::Home(Wall::Settings),
        Message::Settings(folia_ui::SettingsMessage::Section(folia_ui::SettingsSection::Graphics)),
    ] {
        let task = shell.update(message);
        finish_wall_tasks(&mut shell, &renderer, task);
    }
    let mut cache = Cache::default();
    for _ in 0..4 {
        wall_frame(&mut shell, &mut renderer, &mut cache);
        std::thread::sleep(Duration::from_millis(150));
    }
    let target = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("actual-present-rate"),
        size: wgpu::Extent3d { width: 1200, height: 800, depth_or_array_layers: 1 },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        view_formats: &[],
    });
    let view = target.create_view(&Default::default());
    let viewport = iced::advanced::graphics::Viewport::with_physical_size(Size::new(1200, 800), 1.0);
    for rate in [30u32, 45, 60, 90, 120, 0] {
        if rate == 0 {
            let task = shell.update(Message::FrameRate(folia_store::FrameRate::Display));
            finish_wall_tasks(&mut shell, &renderer, task);
            std::thread::sleep(Duration::from_millis(350));
        } else {
            super::picker::click(
                &mut shell,
                &mut renderer,
                &mut cache,
                Size::new(1200.0, 800.0),
                &format!("{rate} FPS"),
            );
        }
        assert_eq!(AppearanceSettings::load(&store).unwrap().frame_rate.fps(), (rate > 0).then_some(rate));
        for phase in ["settled", "landing"] {
            if phase == "landing" {
                let task = shell.update(Message::Settings(folia_ui::SettingsMessage::Tab("graphicsMotion")));
                finish_wall_tasks(&mut shell, &renderer, task);
            }
            let mut senders = Vec::new();
            let streams = iced::advanced::subscription::into_recipes(shell.subscription())
                .into_iter()
                .map(|recipe| {
                    let (tx, rx) = iced::futures::channel::mpsc::channel(4);
                    senders.push(tx);
                    recipe.stream(rx.boxed())
                })
                .collect::<Vec<_>>();
            let mut messages = stream::select_all(streams);
            let events = std::thread::spawn(move || {
                let window = iced::window::Id::unique();
                while senders.iter().any(|sender| !sender.is_closed()) {
                    std::thread::sleep(Duration::from_secs_f64(1.0 / 144.0));
                    for sender in &mut senders {
                        let _ = sender.try_send(iced::advanced::subscription::Event::Interaction {
                            window,
                            event: Event::Window(iced::window::Event::RedrawRequested(Instant::now())),
                            status: iced::event::Status::Ignored,
                        });
                    }
                }
            });
            let started = Instant::now();
            let deadline = started + Duration::from_millis(1500);
            let mut presents = 0;
            while let Ok(Some(message)) = pollster::block_on(tokio::time::timeout(
                deadline.saturating_duration_since(Instant::now()),
                messages.next(),
            )) {
                let task = shell.update(message);
                finish_wall_tasks(&mut shell, &renderer, task);
                let mut ui = UserInterface::build(
                    shell.view(),
                    Size::new(1200.0, 800.0),
                    std::mem::take(&mut cache),
                    &mut renderer,
                );
                ui.draw(
                    &mut renderer,
                    &shell.theme(),
                    &iced::advanced::renderer::Style { text_color: Color::BLACK },
                    iced::mouse::Cursor::Unavailable,
                );
                cache = ui.into_cache();
                renderer.present(Some(Color::WHITE), wgpu::TextureFormat::Rgba8Unorm, &view, &viewport);
                presents += 1;
                if Instant::now() >= deadline {
                    break;
                }
            }
            let fps = presents as f64 / started.elapsed().as_secs_f64();
            drop(messages);
            events.join().unwrap();
            device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
            println!("ACTUAL_FPS setting={rate} phase={phase} presents={presents} fps={fps:.2}");
            assert!(
                fps > if rate == 0 { 125.0 } else { f64::from(rate) * 0.6 },
                "the fixture must be actively rendering"
            );
            assert!(
                fps <= if rate == 0 { 150.0 } else { f64::from(rate) * 1.06 + 1.0 },
                "settings animation bypassed the frame limit: {fps}"
            );
        }
    }
}
