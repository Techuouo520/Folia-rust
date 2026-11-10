// Panel fades, help keys, the capsule's centre anchor and the subtitle's presence spring.

use folia_core::Theme;
use folia_ui::{
    CardDirection, CardKey, ChromeSpring, Faded, HELP_LIST, SUBTITLE_PRESENCE_SPRING, Tokens, WallKey, capsule_box,
    capsule_width, subtitle_bottom, wall_key_of,
};
use iced::keyboard::{Key, Modifiers, key::Named};
use iced::{Color, Size};

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
        ]
    );
    // F6 has no tab cycling in this build, so its row is left out.
    assert_eq!(press(Key::Named(Named::F6), Modifiers::empty()), None);
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
