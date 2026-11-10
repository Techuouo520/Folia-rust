// Capsule hide curve and the Space play/pause key.

use std::time::{Duration, Instant};

use folia_ui::{PlayerBar, space_toggle};
use folia_visualizer::{Ease, tween};
use iced::Event;
use iced::event::Status;
use iced::keyboard::key::{Code, Named};
use iced::keyboard::{Event as KeyEvent, Key, Location, Modifiers};

fn space(repeat: bool) -> Event {
    Event::Keyboard(KeyEvent::KeyPressed {
        key: Key::Named(Named::Space),
        modified_key: Key::Named(Named::Space),
        physical_key: iced::keyboard::key::Physical::Code(Code::Space),
        location: Location::Standard,
        modifiers: Modifiers::default(),
        text: None,
        repeat,
    })
}

#[test]
fn hiding_follows_ease_out_for_260ms() {
    let start = Instant::now();
    let mut bar = PlayerBar::new(start);
    bar.set_shown(false, start);
    assert!((bar.shown(start) - 1.0).abs() < 1e-3);
    assert!(bar.animating(start));

    let mid = start + Duration::from_millis(130);
    let expected = tween(1.0, 0.0, 130.0, folia_ui::CAPSULE_FADE_MS as f64, Ease::OUT) as f32;
    assert!((bar.shown(mid) - expected).abs() < 1e-3);
    assert!(expected > 0.0 && expected < 0.5);
    assert!(bar.animating(mid));

    let end = start + Duration::from_millis(folia_ui::CAPSULE_FADE_MS);
    assert!(bar.shown(end).abs() < 1e-3);
    assert!(!bar.animating(end));
}

#[test]
fn space_toggles_only_when_the_key_is_free() {
    let free = space_toggle(space(false), Status::Ignored);
    assert!(free.is_some_and(|message| format!("{message:?}") == "Playback(Toggle)"));
    assert!(space_toggle(space(false), Status::Captured).is_none());
    assert!(space_toggle(space(true), Status::Ignored).is_none());
    let escape = Event::Keyboard(KeyEvent::KeyPressed {
        key: Key::Named(Named::Escape),
        modified_key: Key::Named(Named::Escape),
        physical_key: iced::keyboard::key::Physical::Code(Code::Escape),
        location: Location::Standard,
        modifiers: Modifiers::default(),
        text: None,
        repeat: false,
    });
    assert!(space_toggle(escape, Status::Ignored).is_none());
}
