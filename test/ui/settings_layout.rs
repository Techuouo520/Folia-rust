// Drive real settings views through the same repeated-redraw loop used by iced_winit.
use super::player_fixture;
use folia_ui::{HomeMessage, Message, SettingsMessage, SettingsSection, Shell};
use iced::advanced::renderer::Headless;
use iced_runtime::user_interface::{Cache, UserInterface};
#[test]
#[cfg_attr(feature = "ci", ignore = "requires a hardware GPU for layout integration")]
fn settings_redraw_settles_while_switching_pages() {
    if player_fixture::run_gpu_child("settings_layout::settings_redraw_settles_while_switching_pages") {
        return;
    }
    folia_ui::install_fonts();
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    let (mut app, initial) = Shell::new(player_fixture::init(root));
    drop(initial);
    drop(app.update(Message::WindowResized(iced::Size::new(1920.0, 1080.0))));
    drop(app.update(Message::Home(HomeMessage::Settings)));
    let mut renderer =
        pollster::block_on(<iced::Renderer as Headless>::new(iced::Font::DEFAULT, iced::Pixels(16.), None)).unwrap();
    let mut cache = Cache::default();
    let mut warnings = 0;
    let mut max_rounds = 0;
    for section in [
        SettingsSection::Developer,
        SettingsSection::Appearance,
        SettingsSection::General,
        SettingsSection::Graphics,
        SettingsSection::Player,
        SettingsSection::Developer,
    ] {
        drop(app.update(Message::Settings(SettingsMessage::Section(section))));
        let mut section_warnings = 0;
        for _ in 0..24 {
            drop(app.update(Message::UiFrame));
            let event = iced::Event::Window(iced::window::Event::RedrawRequested(std::time::Instant::now()));
            for retry in 0..8 {
                let mut ui = UserInterface::build(app.view(), iced::Size::new(1920., 1080.), cache, &mut renderer);
                let mut messages = vec![];
                let (state, _) = ui.update(
                    std::slice::from_ref(&event),
                    iced::mouse::Cursor::Unavailable,
                    &mut renderer,
                    &mut iced::advanced::clipboard::Null,
                    &mut messages,
                );
                let changed = state.has_layout_changed();
                cache = ui.into_cache();
                max_rounds = max_rounds.max(retry + 1);
                if messages.is_empty() && !changed {
                    break;
                }
                if retry == 2 {
                    warnings += 1;
                    section_warnings += 1;
                }
                for message in messages {
                    drop(app.update(message));
                }
                // Layout work takes time, but retrying one redraw must not advance its animation.
                std::thread::sleep(std::time::Duration::from_millis(2));
            }
            std::thread::sleep(std::time::Duration::from_millis(8));
        }
        println!("{section:?}: repeated_layout_frames={section_warnings}");
    }
    println!("total_warning_equivalent_frames={warnings} max_update_passes={max_rounds}");
    assert_eq!(warnings, 0, "scroll and measurement feedback kept advancing the layout within one redraw");
    assert!(max_rounds <= 3);
}

#[test]
#[cfg_attr(feature = "ci", ignore = "requires a hardware GPU for layout integration")]
fn delayed_shell_frames_finish_settings_transitions() {
    if player_fixture::run_gpu_child("settings_layout::delayed_shell_frames_finish_settings_transitions") {
        return;
    }
    folia_ui::install_fonts();
    let mut renderer =
        pollster::block_on(<iced::Renderer as Headless>::new(iced::Font::DEFAULT, iced::Pixels(16.), None)).unwrap();
    let temp = tempfile::tempdir().unwrap();
    let (mut app, initial) = Shell::new(player_fixture::init(temp.path()));
    drop(initial);
    drop(app.update(Message::WindowResized(iced::Size::new(1200., 800.))));
    for message in
        [Message::Home(HomeMessage::Settings), Message::Settings(SettingsMessage::Section(SettingsSection::Graphics))]
    {
        drop(app.update(message));
        std::thread::sleep(std::time::Duration::from_millis(400));
        drop(app.update(Message::UiFrame));
        let automatic = text_bounds(&app, &mut renderer);
        drop(app.update(Message::Settings(SettingsMessage::Frame)));
        let settled = text_bounds(&app, &mut renderer);
        assert!(!automatic.is_empty());
        assert_eq!(automatic.len(), settled.len());
        for ((name, left), (other, right)) in automatic.iter().zip(&settled) {
            assert_eq!(name, other);
            assert!((left.y - right.y).abs() < 0.1, "late frame left {name} at {left:?}, expected {right:?}");
        }
    }
}

fn text_bounds(app: &Shell, renderer: &mut iced::Renderer) -> Vec<(String, iced::Rectangle)> {
    #[derive(Default)]
    struct Labels(Vec<(String, iced::Rectangle)>);
    impl iced::advanced::widget::Operation for Labels {
        fn traverse(&mut self, operate: &mut dyn FnMut(&mut dyn iced::advanced::widget::Operation)) {
            operate(self);
        }
        fn text(&mut self, _: Option<&iced::widget::Id>, bounds: iced::Rectangle, text: &str) {
            self.0.push((text.to_owned(), bounds));
        }
    }
    let mut ui = UserInterface::build(app.view(), iced::Size::new(1200., 800.), Cache::default(), renderer);
    let mut labels = Labels::default();
    ui.operate(renderer, &mut labels);
    labels.0
}
