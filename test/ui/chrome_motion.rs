// Panel fades, help keys, the capsule's centre anchor and the subtitle's presence spring.

use folia_core::Theme;
use folia_ui::{
    CardDirection, CardKey, ChromeSpring, Faded, HELP_LIST, SUBTITLE_PRESENCE_SPRING, Tokens, WallKey, capsule_box,
    capsule_width, subtitle_bottom, wall_key_of,
};
use iced::keyboard::{Key, Modifiers, key::Named};
use iced::{Color, Size};

#[test]
#[cfg_attr(feature = "ci", ignore = "needs a renderer for the real drag strip")]
fn title_drag_keeps_tracking_after_leaving_the_strip() {
    use folia_ui::{Message, Shell};
    use iced::advanced::renderer::Headless;
    use iced::futures::StreamExt;
    use iced_runtime::user_interface::{Cache, UserInterface};
    folia_ui::install_fonts();
    let temp = tempfile::tempdir().unwrap();
    let (mut shell, initial) = Shell::new(super::player_fixture::init(temp.path()));
    super::player_fixture::complete(&mut shell, initial);
    std::thread::sleep(std::time::Duration::from_millis(400));
    drop(shell.update(Message::SplashFrame));
    std::thread::sleep(std::time::Duration::from_millis(350));
    let window = iced::window::Id::unique();
    drop(shell.update(Message::WindowReady(Some(window))));
    drop(shell.update(Message::Pointer(window, Some(iced::Point::new(300.0, 16.0)))));
    let mut renderer =
        pollster::block_on(<iced::Renderer as Headless>::new(folia_ui::INTERFACE_FONT, iced::Pixels(16.0), None))
            .unwrap();
    let mut ui = UserInterface::build(shell.view(), Size::new(1200.0, 800.0), Cache::default(), &mut renderer);
    let mut messages = Vec::new();
    ui.update(
        &[iced::Event::Mouse(iced::mouse::Event::ButtonPressed(iced::mouse::Button::Left))],
        iced::mouse::Cursor::Available(iced::Point::new(300.0, 16.0)),
        &mut renderer,
        &mut iced::advanced::clipboard::Null,
        &mut messages,
    );
    assert!(!messages.is_empty(), "the title strip must receive the press");
    drop(ui);
    for message in messages {
        drop(shell.update(message));
    }
    drop(shell.update(Message::PointerHeld(true)));
    let drags = |task| {
        pollster::block_on(async {
            let Some(mut stream) = iced_runtime::task::into_stream(task) else { return 0 };
            let mut count = 0;
            while let Some(action) = stream.next().await {
                if matches!(action, iced_runtime::Action::Window(iced_runtime::window::Action::Drag(_))) {
                    count += 1;
                }
            }
            count
        })
    };
    assert_eq!(
        drags(shell.update(Message::Pointer(window, Some(iced::Point::new(310.0, 90.0))))),
        1,
        "a fast move outside the title strip lost the drag"
    );
    drop(shell.update(Message::PointerHeld(false)));
    assert_eq!(
        drags(shell.update(Message::Pointer(window, Some(iced::Point::new(320.0, 100.0))))),
        0,
        "release must disarm the drag"
    );
}

#[test]
#[cfg_attr(feature = "ci", ignore = "needs a renderer for the wall gesture")]
fn wall_drag_keeps_the_press_across_overlays_and_rebuilds() {
    use folia_ui::{LibraryMessage, Message, Shell, WallMessage};
    use iced::advanced::renderer::Headless;
    use iced_runtime::user_interface::{Cache, UserInterface};
    folia_ui::install_fonts();
    let temp = tempfile::tempdir().unwrap();
    let mut init = super::player_fixture::init(temp.path());
    init.general.home_tab = folia_store::HomeTab::Local;
    let (mut app, initial) = Shell::new(init);
    super::player_fixture::complete(&mut app, initial);
    drop(app.update(Message::Library(LibraryMessage::Loaded(Ok(
        (1..=32).map(|id| folia_core::Track { id, title: format!("Song {id}"), ..Default::default() }).collect(),
    )))));
    std::thread::sleep(std::time::Duration::from_millis(400));
    drop(app.update(Message::SplashFrame));
    std::thread::sleep(std::time::Duration::from_millis(350));
    let id = iced::window::Id::unique();
    drop(app.update(Message::WindowReady(Some(id))));
    let mut renderer =
        pollster::block_on(<iced::Renderer as Headless>::new(folia_ui::INTERFACE_FONT, iced::Pixels(16.0), None))
            .unwrap();
    let mut cache = Cache::default();
    let mut event = |app: &mut Shell, at: iced::Point, event: iced::Event| {
        let mut ui =
            UserInterface::build(app.view(), Size::new(1200.0, 800.0), std::mem::take(&mut cache), &mut renderer);
        let mut messages = Vec::new();
        ui.update(
            &[event],
            iced::mouse::Cursor::Available(at),
            &mut renderer,
            &mut iced::advanced::clipboard::Null,
            &mut messages,
        );
        cache = ui.into_cache();
        let pans = messages
            .iter()
            .filter(|message| matches!(message, Message::Library(LibraryMessage::Wall(WallMessage::Pan(_)))))
            .count();
        for message in messages {
            drop(app.update(message));
        }
        drop(app.update(Message::Pointer(id, Some(at))));
        pans
    };
    let at = iced::Point::new(300.0, 400.0);
    event(&mut app, at, iced::Event::Window(iced::window::Event::RedrawRequested(std::time::Instant::now())));
    std::thread::sleep(std::time::Duration::from_millis(700));
    event(&mut app, at, iced::Event::Mouse(iced::mouse::Event::ButtonPressed(iced::mouse::Button::Left)));
    drop(app.update(Message::PointerHeld(true)));
    for (x, y) in [
        (320.0, 400.0),
        (580.0, 400.0),
        (680.0, 400.0),
        (900.0, 400.0),
        (920.0, 400.0),
        (900.0, 20.0),
        (1205.0, 400.0),
        (300.0, 400.0),
        (320.0, 400.0),
    ] {
        let at = iced::Point::new(x, y);
        assert_eq!(
            event(&mut app, at, iced::Event::Mouse(iced::mouse::Event::CursorMoved { position: at })),
            1,
            "held wall drag lost after the seam entered or left the viewport at {at:?}"
        );
    }
    event(&mut app, at, iced::Event::Mouse(iced::mouse::Event::ButtonReleased(iced::mouse::Button::Left)));
    drop(app.update(Message::PointerHeld(false)));
    let next = iced::Point::new(350.0, 400.0);
    assert_eq!(event(&mut app, next, iced::Event::Mouse(iced::mouse::Event::CursorMoved { position: next })), 0);
    event(&mut app, next, iced::Event::Mouse(iced::mouse::Event::ButtonPressed(iced::mouse::Button::Left)));
    drop(app.update(Message::PointerHeld(true)));
    assert_eq!(event(&mut app, at, iced::Event::Mouse(iced::mouse::Event::CursorMoved { position: at })), 1);
}

fn token_colors(tokens: &Tokens) -> [Color; 20] {
    [
        tokens.win,
        tokens.side,
        tokens.card,
        tokens.menu,
        tokens.pv,
        tokens.raise,
        tokens.knob,
        tokens.t1,
        tokens.t2,
        tokens.t3,
        tokens.line,
        tokens.line2,
        tokens.fill,
        tokens.fill2,
        tokens.track,
        tokens.accent,
        tokens.on_accent,
        tokens.muted,
        tokens.hover,
        tokens.shadow,
    ]
}

fn every_color(paint: &Faded) -> Vec<Color> {
    let mut colors = vec![
        paint.text(),
        paint.accent(),
        paint.ink(1.0),
        paint.ink(0.58),
        paint.tint(Color::BLACK, 0.22),
        paint.tint(Color::WHITE, 1.0),
        paint.wash(0.07, 0.10),
    ];
    colors.extend(token_colors(&paint.tokens()));
    let tip = paint.tip()(&iced::Theme::Dark);
    colors.extend(tip.text_color);
    colors.push(tip.border.color);
    if let Some(iced::Background::Color(color)) = tip.background {
        colors.push(color);
    }
    colors
}

#[test]
fn a_panel_faded_to_nothing_paints_nothing() {
    for daylight in [false, true] {
        let tokens = Tokens::new(&Theme::MIDNIGHT, daylight);
        let text = Color::from_rgb(0.9, 0.9, 0.9);
        let accent = Color::from_rgb(0.4, 0.6, 1.0);
        let gone = Faded::new(tokens, text, accent, daylight, 0.0);
        for paint in [gone, gone.faded(0.35), Faded::new(tokens, text, accent, daylight, 1.0).faded(0.0)] {
            for color in every_color(&paint) {
                assert_eq!(color.a, 0.0, "{color:?} still shows with daylight {daylight}");
            }
        }
        // The inert lights pill: 35% of whatever the panel is at, not a fixed 35%.
        let half = Faded::new(tokens, text, accent, daylight, 0.5);
        assert!((half.faded(0.35).tokens().accent.a - 0.175 * tokens.accent.a).abs() < 1e-6);
        assert!((half.wash(0.05, 0.06).a - if daylight { 0.025 } else { 0.03 }).abs() < 1e-6);
        let open = Faded::new(tokens, text, accent, daylight, 1.0);
        assert_eq!(open.text(), text);
        assert_eq!(token_colors(&open.tokens()), token_colors(&tokens));
    }
}

fn press(key: Key, modifiers: Modifiers) -> Option<WallKey> {
    wall_key_of(&key, modifiers, false)
}

fn probe(label: &str) -> (Key, Modifiers, WallKey) {
    let none = Modifiers::empty();
    match label {
        "libraryBravais.helpSearch" => (Key::Character("/".into()), none, WallKey::Search),
        "libraryBravais.helpMove" => {
            (Key::Named(Named::ArrowRight), none, WallKey::Card(CardKey::Move(CardDirection::Right)))
        }
        "libraryBravais.helpOpen" | "libraryBravais.helpPlay" => {
            (Key::Named(Named::Enter), none, WallKey::Card(CardKey::Enter))
        }
        "libraryBravais.helpEnqueue" => (Key::Named(Named::Enter), Modifiers::SHIFT, WallKey::Card(CardKey::Enqueue)),
        "libraryBravais.helpBack" => (Key::Named(Named::Escape), none, WallKey::Card(CardKey::Escape)),
        "libraryBravais.helpShuffle" | "libraryBravais.helpLocate" => {
            (Key::Character(":".into()), none, WallKey::Chord)
        }
        "libraryBravais.helpLattice" => (Key::Character("b".into()), Modifiers::COMMAND, WallKey::Destination),
        "libraryBravais.helpTabs" => (Key::Named(Named::F6), none, WallKey::CycleTab(false)),
        "libraryBravais.helpCommands" => (Key::Character("k".into()), Modifiers::COMMAND, WallKey::Commands),
        other => panic!("no probe for {other}"),
    }
}

#[test]
fn every_help_row_names_a_key_with_that_action() {
    for row in HELP_LIST {
        let (key, modifiers, action) = probe(row.label);
        assert_eq!(press(key, modifiers), Some(action), "{}", row.label);
    }
    let labels: Vec<_> = HELP_LIST.iter().map(|row| (row.label, row.kbd)).collect();
    assert_eq!(
        labels,
        [
            ("libraryBravais.helpSearch", "/"),
            ("libraryBravais.helpMove", "↑ ↓ ← →"),
            ("libraryBravais.helpOpen", "Enter"),
            ("libraryBravais.helpPlay", "Enter"),
            ("libraryBravais.helpEnqueue", "Shift + Enter"),
            ("libraryBravais.helpBack", "ESC"),
            ("libraryBravais.helpTabs", "F6"),
            ("libraryBravais.helpLocate", ": + C"),
            ("libraryBravais.helpShuffle", ": + R"),
            ("libraryBravais.helpLattice", if cfg!(target_os = "macos") { "⌘ + B" } else { "Ctrl + B" }),
            ("libraryBravais.helpCommands", if cfg!(target_os = "macos") { "⌘ + K" } else { "Ctrl + K" }),
        ]
    );
    assert_eq!(press(Key::Named(Named::F6), Modifiers::empty()), Some(WallKey::CycleTab(false)));
}

#[test]
fn wall_keys_follow_the_original_modifier_rules() {
    let slash = || Key::Character("/".into());
    // `resolveBravaisKey`: Shift + Esc still steps back, Tab ignores Ctrl, Alt and Meta.
    assert_eq!(press(Key::Named(Named::Escape), Modifiers::SHIFT), Some(WallKey::Card(CardKey::Escape)));
    assert_eq!(press(Key::Named(Named::Tab), Modifiers::SHIFT), Some(WallKey::Focus { backwards: true }));
    for chord in [Modifiers::CTRL, Modifiers::ALT, Modifiers::LOGO] {
        assert_eq!(press(Key::Named(Named::Tab), chord), None);
        assert_eq!(press(slash(), chord), None);
    }
    assert_eq!(press(Key::Named(Named::ArrowUp), Modifiers::SHIFT), None);
    // Repeats move but do not open, step back or search.
    assert_eq!(wall_key_of(&Key::Named(Named::Enter), Modifiers::empty(), true), None);
    assert_eq!(wall_key_of(&Key::Named(Named::Escape), Modifiers::empty(), true), None);
    assert_eq!(wall_key_of(&slash(), Modifiers::empty(), true), None);
    assert_eq!(
        wall_key_of(&Key::Named(Named::ArrowLeft), Modifiers::empty(), true),
        Some(WallKey::Card(CardKey::Move(CardDirection::Left)))
    );
}

#[test]
fn the_capsule_content_keeps_its_size_on_the_frame_centre() {
    let window = Size::new(1200.0, 800.0);
    for t in [0.0, 0.2, 0.5, 0.8, 1.04, 1.0] {
        let open = capsule_box(window, false, t, true);
        assert_eq!(open.content, Size::new(480.0, 72.0));
        assert!((open.frame.width - capsule_width(480.0, 1200.0, t)).abs() < 1e-4);
        assert!((open.frame.height - (48.0 + 24.0 * t)).abs() < 1e-4);
        let closed = capsule_box(window, false, t, false);
        assert_eq!(closed.content, Size::new(288.0, 48.0));
        for boxed in [open, closed] {
            let origin = boxed.content_origin();
            assert!((origin.x + boxed.content.width / 2.0 - boxed.frame.width / 2.0).abs() < 1e-4);
            assert!((origin.y + boxed.content.height / 2.0 - boxed.frame.height / 2.0).abs() < 1e-4);
        }
    }
    // Narrow windows stack the expanded row to 127px.
    assert_eq!(capsule_box(Size::new(500.0, 800.0), false, 0.3, true).content.height, 127.0);
}

/// framer-motion's underdamped spring from rest, unit mass.
fn framer_spring(from: f64, to: f64, t: f64, stiffness: f64, damping: f64) -> f64 {
    let omega = stiffness.sqrt();
    let zeta = damping / (2.0 * omega);
    let omega_d = omega * (1.0 - zeta * zeta).sqrt();
    let delta = to - from;
    to - (-zeta * omega * t).exp()
        * (zeta * omega * delta / omega_d * (omega_d * t).sin() + delta * (omega_d * t).cos())
}

#[test]
fn the_subtitle_makes_room_on_the_original_spring() {
    assert_eq!((SUBTITLE_PRESENCE_SPRING.stiffness, SUBTITLE_PRESENCE_SPRING.damping), (280.0, 28.0));
    assert!((subtitle_bottom(32.0, 1.0) - 112.0).abs() < 1e-4);
    assert!((subtitle_bottom(32.0, 0.0) - 32.0).abs() < 1e-4);
    assert!((subtitle_bottom(60.0, 1.0) - 140.0).abs() < 1e-4);
    let hiding = ChromeSpring::settled(1.0, 0, SUBTITLE_PRESENCE_SPRING).retarget(1_000, 0.0);
    let showing = ChromeSpring::settled(0.0, 0, SUBTITLE_PRESENCE_SPRING).retarget(1_000, 1.0);
    for ms in [0_u64, 16, 50, 100, 160, 260, 400, 600] {
        let t = ms as f64 / 1000.0;
        for (spring, from, to) in [(hiding, 1.0, 0.0), (showing, 0.0, 1.0)] {
            let presence = spring.value(1_000 + ms);
            let expected = framer_spring(from, to, t, 280.0, 28.0);
            assert!((f64::from(presence) - expected).abs() < 1e-5, "{ms}ms: {presence} vs {expected}");
            let bottom = subtitle_bottom(48.0, presence);
            assert!((f64::from(bottom) - (32.0 + expected * (16.0 + 80.0))).abs() < 1e-3);
        }
    }
    // It is not a jump: 50ms in the subtitle is still well above the base line.
    assert!(subtitle_bottom(32.0, hiding.value(1_050)) > 80.0);
    assert!(hiding.running(1_600));
    assert_eq!(hiding.value(1_700), 0.0);
    assert!(!hiding.running(1_700));
}
