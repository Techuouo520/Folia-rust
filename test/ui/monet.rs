// Monet on the live stage: the hanger drag publishes one offset per gesture, each picture keeps its own drag,
// clicking a lyric line plays from it (but never in a preview), and the shared bottom subtitle stays off under Monet.

use folia_core::{Line, Lyrics, Theme};
use folia_visualizer::styles::monet::{self, Event as MonetEvent, PortraitOffset};
use folia_visualizer::{Scene, SceneState, Style, Values};
use iced::widget::canvas::{Event, Program};
use iced::{Point, Rectangle, Size, mouse};

const BOUNDS: Rectangle = Rectangle { x: 0.0, y: 0.0, width: 1920.0, height: 1032.0 };

fn scene<'a>(lyrics: &'a Lyrics, theme: &'a Theme) -> Scene<'a> {
    Scene {
        style: Style::Monet,
        lyrics,
        time: 5.0,
        theme,
        font_scale: 1.0,
        seed: "monet",
        stage: None,
        tuning: Values::default(),
        backdrop: true,
        title: "长歌".into(),
        artist: "folia".into(),
        album: String::new(),
        portrait: None,
        typography: folia_visualizer::Typography::default(),
        preview: false,
        audio: &folia_core::AudioFrame::SILENT,
    }
}

fn lines() -> Lyrics {
    let line = |start: f64, end: f64, text: &str| Line { start, end, text: text.to_owned(), ..Line::default() };
    Lyrics {
        lines: vec![
            line(0.0, 2.0, "first line"),
            line(2.0, 4.0, "second line"),
            line(4.0, 6.0, "third line"),
            line(6.0, 8.0, "fourth line"),
        ],
        ..Lyrics::default()
    }
}

/// Feeds one event and returns what the canvas published.
fn send(scene: &Scene<'_>, state: &mut SceneState, event: Event, at: Point) -> Option<MonetEvent> {
    let stage = folia_visualizer::Stage::new(scene.clone());
    let action = Program::<MonetEvent>::update(&stage, state, &event, BOUNDS, mouse::Cursor::Available(at))?;
    action.into_inner().0
}

fn press() -> Event {
    Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left))
}

fn release() -> Event {
    Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left))
}

fn moved(at: Point) -> Event {
    Event::Mouse(mouse::Event::CursorMoved { position: at })
}

/// Click to edit, then press, drag `dx` in three moves, let go. Returns every published offset.
fn gesture(scene: &Scene<'_>, state: &mut SceneState, dx: f32) -> Vec<f32> {
    let hanger = monet::layout(scene, BOUNDS.size(), 0.0).hanger.unwrap();
    let grab = hanger.center();
    let mut published = Vec::new();
    published.extend(send(scene, state, press(), grab));
    published.extend(send(scene, state, release(), grab));
    assert!(published.is_empty(), "entering edit mode published {published:?}");
    published.extend(send(scene, state, press(), grab));
    for step in 1..=3 {
        let at = Point::new(grab.x + dx * step as f32 / 3.0, grab.y);
        published.extend(send(scene, state, moved(at), at));
    }
    let end = Point::new(grab.x + dx, grab.y);
    published.extend(send(scene, state, release(), end));
    published
        .into_iter()
        .filter_map(|event| match event {
            MonetEvent::Portrait(PortraitOffset(offset)) => Some(offset),
            MonetEvent::Seek(_) => None,
        })
        .collect()
}

#[test]
fn hanger_drag_publishes_once_per_picture() {
    let lyrics = Lyrics::default();
    let theme = Theme::DAYLIGHT;
    let scene = scene(&lyrics, &theme);
    let mut player = SceneState::default();
    let mut workbench = SceneState::default();
    assert_eq!(gesture(&scene, &mut player, -40.0), vec![-40.0]);
    // The other picture neither saw that drag nor holds its preview: its own drag starts from the saved 0.
    assert_eq!(send(&scene, &mut workbench, release(), Point::ORIGIN), None);
    assert_eq!(gesture(&scene, &mut workbench, -20.0), vec![-20.0]);
    // Past the range the offset stops at -150.
    let mut third = SceneState::default();
    assert_eq!(gesture(&scene, &mut third, -400.0), vec![-150.0]);
}

#[test]
fn small_pictures_ignore_the_hanger() {
    let lyrics = Lyrics::default();
    let theme = Theme::DAYLIGHT;
    let thumbnail = Scene { stage: Some(Size::new(1200.0, 800.0)), ..scene(&lyrics, &theme) };
    let mut state = SceneState::default();
    let hanger = monet::layout(&scene(&lyrics, &theme), BOUNDS.size(), 0.0).hanger.unwrap();
    let stage = folia_visualizer::Stage::new(thumbnail);
    assert!(
        Program::<MonetEvent>::update(&stage, &mut state, &press(), BOUNDS, mouse::Cursor::Available(hanger.center()))
            .is_none()
    );
}

#[test]
fn monet_hides_the_shared_subtitle() {
    assert!(!Style::Monet.shared_subtitle());
    assert!(!Style::Still.shared_subtitle());
    assert!(Style::Classic.shared_subtitle());
    assert!(Style::Lumiere.shared_subtitle());
}

#[test]
fn a_click_on_a_lyric_line_publishes_that_line_start() {
    let lyrics = lines();
    let theme = Theme::DAYLIGHT;
    let scene = Scene { time: 5.0, ..scene(&lyrics, &theme) };
    let boxes = monet::layout(&scene, BOUNDS.size(), 0.0);
    let rail_boxes = folia_visualizer::styles::monet::rail_boxes(&scene, &boxes);
    assert!(!rail_boxes.is_empty());
    for (index, rect) in rail_boxes {
        let at = rect.center();
        assert_eq!(monet::line_at(&scene, BOUNDS.size(), at), Some(lyrics.lines[index].start));
    }
}

#[test]
fn a_click_on_a_line_ends_manual_scrolling() {
    let lyrics = lines();
    let theme = Theme::DAYLIGHT;
    let scene = Scene { time: 5.0, ..scene(&lyrics, &theme) };
    let boxes = monet::layout(&scene, BOUNDS.size(), 0.0);
    let rail_boxes = folia_visualizer::styles::monet::rail_boxes(&scene, &boxes);
    let (index, rect) = rail_boxes[0];
    let at = rect.center();
    let published = monet::line_at(&scene, BOUNDS.size(), at).map(MonetEvent::Seek);
    assert_eq!(published, Some(MonetEvent::Seek(lyrics.lines[index].start)));
}

#[test]
fn a_preview_never_seeks() {
    let lyrics = lines();
    let theme = Theme::DAYLIGHT;
    let preview = Scene { preview: true, ..scene(&lyrics, &theme) };
    let frame = Scene { preview: false, ..scene(&lyrics, &theme) };
    let mut state = SceneState::default();
    let boxes = monet::layout(&frame, BOUNDS.size(), 0.0);
    let rail_boxes = folia_visualizer::styles::monet::rail_boxes(&frame, &boxes);
    let at = rail_boxes[0].1.center();
    // A preview draws the same lines but its press is ignored.
    assert!(send(&preview, &mut state, press(), at).is_none());
    assert_eq!(send(&frame, &mut state, press(), at), Some(MonetEvent::Seek(lyrics.lines[rail_boxes[0].0].start)));
}
