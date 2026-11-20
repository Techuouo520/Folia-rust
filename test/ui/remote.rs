// Remote persistence, lyrics, monitor geometry and shared queue commands through the public shell.

use folia_core::{Line, QueueSnapshot, SubtitleContentMode, Track};
use folia_render::desktop_window::{Capabilities, Geometry};
use folia_store::{PlaybackSession, RemoteSettings, RemoteSource};
use folia_ui::{
    Message, Shell,
    remote::{self, Message as Remote, QueueAction},
};
use iced::{Point, Rectangle, Size};

#[test]
fn queue_anchors_and_restoration_respect_negative_monitors_and_dpi() {
    let area = Rectangle::new(Point::new(-1920.0, 0.0), Size::new(1920.0, 1040.0));
    let card = Rectangle::new(Point::new(-1000.0, 800.0), Size::new(602.0, 184.0));
    let expansion = remote::queue_expansion(card, Some(area), 340.0, None);
    assert!(expansion.up);
    assert_eq!(expansion.height, 340.0);
    assert!(!remote::queue_expansion(Rectangle { y: 100.0, ..card }, Some(area), 340.0, None).up);
    assert_eq!(remote::queue_expansion(card, None, 340.0, None), remote::Expansion { up: false, height: 340.0 });
    let geometry = Geometry {
        position: None,
        size: Size::new(602.0, 184.0),
        scale: 1.5,
        work_areas: vec![area],
        current_work_area: Some(area),
        capabilities: Capabilities { position: true, ..Default::default() },
    };
    let mut settings = RemoteSettings { position: Some([-1500, 200]), ..Default::default() };
    assert_eq!(remote::restored_position(&settings, &geometry), Some(Point::new(-1500.0, 200.0)));
    settings.position = Some([9000, 9000]);
    let restored = remote::restored_position(&settings, &geometry).unwrap();
    assert!(area.contains(restored));
    assert!(restored.x + settings.size[0] * 1.5 <= 0.0);
    assert_eq!(
        restored.x + 174.0 * 1.5 + (602.0 - 174.0) * 1.5 * 0.5,
        -960.0,
        "the compact card, not its empty sleeve, is centered"
    );
}

#[test]
fn remote_rows_follow_sources_without_duplicate_translation_or_interludes() {
    let line = Line {
        text: "原文".into(),
        translation: Some("translation".into()),
        romanization: Some("roman".into()),
        start: 1.0,
        end: 4.0,
        ..Default::default()
    };
    let next = Line {
        text: "next".into(),
        translation: Some("next translation".into()),
        start: 9.0,
        end: 11.0,
        ..Default::default()
    };
    let mut settings = RemoteSettings::default().appearance;
    let rows = remote::lyric_rows(&settings, SubtitleContentMode::Both, &line, Some(&next));
    assert_eq!(rows.romanization, Some("roman"));
    assert_eq!(rows.sub, Some("translation"));
    settings.lyric_rows[0] = RemoteSource::Translation;
    let rows = remote::lyric_rows(&settings, SubtitleContentMode::Translation, &line, Some(&next));
    assert_eq!(rows.main, "translation");
    assert!(!rows.timed);
    assert_eq!(rows.sub, Some("next"));
    settings.lyric_pairs = 2;
    let rows = remote::lyric_rows(&settings, SubtitleContentMode::Translation, &line, Some(&next));
    assert_eq!(rows.sub, None);
    assert_eq!(rows.upcoming, Some(("next", Some("next translation"))));
    let lines = [line, next];
    assert!(remote::line_at(&lines, 0.0).0.is_none());
    let (line, next, singing) = remote::line_at(&lines, 7.0);
    assert_eq!(line.unwrap().text, "原文");
    assert_eq!(next.unwrap().text, "next");
    assert!(!singing);
}

#[test]
fn remote_saves_are_serial_and_close_waits_for_latest_settings() {
    let temp = tempfile::tempdir().unwrap();
    let init = super::player_fixture::init(temp.path());
    let store = init.store.clone();
    store
        .save(&RemoteSettings {
            main_border: false,
            main_chrome: folia_store::RemoteChrome::Hidden,
            ..Default::default()
        })
        .unwrap();
    let (mut shell, task) = Shell::new(init);
    super::session::complete(&mut shell, task);
    let unlock = shell.update(Message::Remote(Remote::Unlock));
    super::session::complete(&mut shell, unlock);
    let unlocked: RemoteSettings = store.load().unwrap();
    assert!(!unlocked.main_border, "unlock changed the main window border preference");
    assert_eq!(unlocked.main_chrome, folia_store::RemoteChrome::Hidden);
    let first = shell.update(Message::Remote(Remote::ClickThrough(true)));
    drop(shell.update(Message::Remote(Remote::ClickThrough(false))));
    drop(shell.update(Message::Remote(Remote::Pairs(2))));
    let close = shell.update(Message::WindowClosing);
    assert_eq!(super::session::complete(&mut shell, close), 0);
    assert_eq!(super::session::complete(&mut shell, first), 1);
    let saved: RemoteSettings = store.load().unwrap();
    assert!(!saved.click_through);
    assert_eq!(saved.appearance.lyric_pairs, 2);
}

#[tokio::test(flavor = "multi_thread")]
async fn remote_queue_commands_resolve_stale_rows_and_leave_the_player_shared() {
    let temp = tempfile::tempdir().unwrap();
    let init = super::player_fixture::init(temp.path());
    let store = init.store.clone();
    store
        .save(&PlaybackSession {
            queue: QueueSnapshot {
                items: (1..=4)
                    .map(|id| Track { id, title: format!("Song {id}"), duration: 100.0, ..Default::default() })
                    .collect(),
                current: Some(0),
                ..Default::default()
            },
            position: 10.0,
            ..Default::default()
        })
        .unwrap();
    let (mut shell, task) = Shell::new(init);
    super::session::complete(&mut shell, task);
    let window = open_remote(&mut shell);
    drop(
        shell.update(Message::RemoteInput(
            window,
            Box::new(Message::Playback(folia_ui::PlaybackMessage::Seeking(42.0))),
        )),
    );
    drop(
        shell
            .update(Message::RemoteInput(window, Box::new(Message::Playback(folia_ui::PlaybackMessage::Volume(0.35))))),
    );
    let blur = shell.update(Message::Remote(Remote::Event(
        window,
        iced::Event::Window(iced::window::Event::Unfocused),
        iced::event::Status::Ignored,
    )));
    super::session::complete(&mut shell, blur);
    assert_eq!(store.load::<folia_store::PlaybackSettings>().unwrap().volume, 0.35);
    for action in [
        Remote::QueueAction(QueueAction::End, 1, 2),
        Remote::QueueAction(QueueAction::Next, 1, 2),
        Remote::QueueAction(QueueAction::Remove, 1, 4),
    ] {
        let task = shell.update(Message::Remote(action));
        super::session::complete(&mut shell, task);
    }
    assert!(shell.title().contains("Song 1"));
    let task = shell.update(Message::WindowClosing);
    super::session::complete(&mut shell, task);
    let saved: PlaybackSession = store.load().unwrap();
    assert_eq!(saved.queue.items.iter().map(|track| track.id).collect::<Vec<_>>(), [1, 2, 3]);
    assert_eq!(saved.position, 42.0);
}

fn open_remote(shell: &mut Shell) -> iced::window::Id {
    use iced::futures::StreamExt;
    let task = shell.update(Message::Remote(Remote::Open));
    let mut stream = iced_runtime::task::into_stream(task).unwrap();
    pollster::block_on(async {
        while let Some(action) = stream.next().await {
            if let iced_runtime::Action::Window(iced_runtime::window::Action::Open(id, settings, _)) = action {
                assert!(!settings.exit_on_close_request && settings.transparent && !settings.decorations);
                assert_eq!(settings.size, Size::new(602.0, 184.0));
                return id;
            }
        }
        panic!("remote did not open a native window")
    })
}

#[test]
#[cfg_attr(feature = "ci", ignore = "needs a GPU for offscreen remote layout")]
fn remote_card_alpha_and_controls_route_without_dragging_or_closing_the_app() {
    use iced::advanced::renderer::Headless;
    use iced_runtime::user_interface::{Cache, UserInterface};
    if super::player_fixture::run_gpu_child(
        "remote::remote_card_alpha_and_controls_route_without_dragging_or_closing_the_app",
    ) {
        return;
    }
    folia_ui::install_fonts();
    let temp = tempfile::tempdir().unwrap();
    let init = super::player_fixture::init(temp.path());
    let store = init.store.clone();
    store
        .save(&PlaybackSession {
            queue: QueueSnapshot {
                items: vec![Track {
                    id: 1,
                    title: "我的心动被什么打了岔".into(),
                    artist: Some("徐梓淳, 五毛".into()),
                    duration: 201.0,
                    ..Default::default()
                }],
                current: Some(0),
                ..Default::default()
            },
            ..Default::default()
        })
        .unwrap();
    let (mut shell, task) = Shell::new(init);
    super::session::complete(&mut shell, task);
    let id = open_remote(&mut shell);
    let mut renderer =
        pollster::block_on(<iced::Renderer as Headless>::new(folia_ui::INTERFACE_FONT, iced::Pixels(16.0), None))
            .unwrap();
    let mut cache = Cache::default();
    let mut pixels = Vec::new();
    for _ in 0..2 {
        let mut ui = UserInterface::build(
            shell.view_window(id),
            Size::new(602.0, 184.0),
            std::mem::take(&mut cache),
            &mut renderer,
        );
        ui.draw(
            &mut renderer,
            &shell.theme(),
            &iced::advanced::renderer::Style { text_color: iced::Color::BLACK },
            iced::mouse::Cursor::Unavailable,
        );
        pixels = renderer.screenshot(
            &iced::advanced::graphics::Viewport::with_physical_size(Size::new(602, 184), 1.0),
            iced::Color::TRANSPARENT,
        );
        cache = ui.into_cache();
    }
    assert_eq!(pixels[(90 * 602 + 20) * 4 + 3], 0, "collapsed sleeve must be transparent");
    assert_eq!(pixels[(100 * 602 + 300) * 4 + 3], 255, "default card is opaque");
    if let Some(path) = std::env::var_os("FOLIA_REMOTE_PREVIEW") {
        image::save_buffer(path, &pixels, 602, 184, image::ColorType::Rgba8).unwrap();
    }
    for (point, expected) in [
        (Point::new(517.0, 26.0), "background"),
        (Point::new(543.0, 155.0), "queue"),
        (Point::new(387.0, 155.0), "play"),
        (Point::new(387.0, 130.0), "seek"),
    ] {
        let mut ui = UserInterface::build(
            shell.view_window(id),
            Size::new(602.0, 184.0),
            std::mem::take(&mut cache),
            &mut renderer,
        );
        let mut messages = Vec::new();
        ui.update(
            &[
                iced::Event::Mouse(iced::mouse::Event::ButtonPressed(iced::mouse::Button::Left)),
                iced::Event::Mouse(iced::mouse::Event::ButtonReleased(iced::mouse::Button::Left)),
            ],
            iced::mouse::Cursor::Available(point),
            &mut renderer,
            &mut iced::advanced::clipboard::Null,
            &mut messages,
        );
        let messages: Vec<_> = messages
            .into_iter()
            .map(|message| match message {
                Message::RemoteInput(window, message) => {
                    assert_eq!(window, id);
                    *message
                }
                other => panic!("unscoped remote input: {other:?}"),
            })
            .collect();
        assert!(
            !messages.iter().any(|message| matches!(message, Message::Remote(Remote::Drag))),
            "{expected} also dragged the window"
        );
        assert!(
            messages.iter().any(|message| match expected {
                "background" => matches!(message, Message::Remote(Remote::Background)),
                "queue" => matches!(message, Message::Remote(Remote::Queue)),
                "seek" => matches!(message, Message::Playback(folia_ui::PlaybackMessage::Seeking(_))),
                _ => matches!(message, Message::Playback(folia_ui::PlaybackMessage::Toggle)),
            }),
            "{expected}: {messages:?}"
        );
        cache = ui.into_cache();
    }
    let close = shell.update(Message::Remote(Remote::Event(
        id,
        iced::Event::Window(iced::window::Event::CloseRequested),
        iced::event::Status::Ignored,
    )));
    assert_eq!(super::session::complete(&mut shell, close), 0);
    assert!(shell.title().contains("我的心动"));
    let reopened = open_remote(&mut shell);
    assert_ne!(id, reopened);
    let late = shell.update(Message::RemoteInput(id, Box::new(Message::Remote(Remote::TextShade(true)))));
    super::session::complete(&mut shell, late);
    assert!(!store.load::<RemoteSettings>().unwrap().appearance.text_shade);
}
