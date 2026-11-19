// The original seam title click rules, including diagonal drags and keyboard activation.

use folia_ui::seam_title_activates;
use iced::Point;

#[test]
#[cfg_attr(feature = "ci", ignore = "needs a GPU")]
fn collapsed_seam_has_a_clickable_restore_tab() {
    use iced::advanced::renderer::Headless;
    use iced::{Color, Event, Pixels, Size};
    use iced_runtime::user_interface::{Cache, UserInterface};
    use std::time::Instant;
    folia_ui::install_fonts();
    let mut renderer =
        pollster::block_on(<iced::Renderer as Headless>::new(folia_ui::INTERFACE_FONT, Pixels(16.0), None)).unwrap();
    let i18n = folia_i18n::I18n::new(folia_core::Language::ZhCn).unwrap();
    let home = folia_ui::HomeChrome::new(1200.0);
    let theme = folia_core::Theme::MIDNIGHT;
    for (horizontal, before, width) in [(false, true, 1200), (false, false, 1200), (true, true, 500)] {
        let host = folia_ui::HomeHost {
            wall: Default::default(),
            lattice: false,
            can_save: false,
            can_generate: false,
            generating: false,
            i18n: &i18n,
            text: Color::WHITE,
            background: Color::BLACK,
            accent: Color::WHITE,
            daylight: false,
            has_track: false,
            busy: false,
            explore_refresh: None,
            randomize_on_entry: false,
            volume: 1.0,
            muted: false,
            tokens: folia_ui::Tokens::new(&theme, false),
            queue: &[],
            queue_index: None,
            fm: false,
            window_width: width as f32,
            window_height: 800.0,
            seam: folia_ui::Seam { center: 600.0, width: 0.0, collapsed: false, before, horizontal },
            spine: false,
            horizontal,
            opacity: 0.0,
            bottom: 20.0,
            status: None,
            show_tab: true,
            tab_hidden: true,
            tab: 1.0,
            tabs: Vec::new(),
            current: folia_store::HomeTab::Local,
            account: None,
            layer: None,
            poster_ground: None,
            poster_tilted: false,
            list_hover: None,
            search: None,
            modal: false,
        };
        let root = folia_ui::home_overlay(&home, host, Instant::now());
        let mut ui = UserInterface::build(root, Size::new(width as f32, 800.0), Cache::default(), &mut renderer);
        let mut label = RestoreLabel::default();
        ui.operate(&renderer, &mut label);
        assert_eq!(label.0.len(), 1, "the restore action must be explicitly labelled");
        let bounds = label.0[0];
        assert!(bounds.x >= 0.0 && bounds.x + bounds.width <= width as f32);
        let cursor = iced::mouse::Cursor::Available(bounds.center());
        let mut messages = Vec::new();
        for event in [
            Event::Mouse(iced::mouse::Event::ButtonPressed(iced::mouse::Button::Left)),
            Event::Mouse(iced::mouse::Event::ButtonReleased(iced::mouse::Button::Left)),
        ] {
            ui.update(&[event], cursor, &mut renderer, &mut iced::advanced::clipboard::Null, &mut messages);
        }
        ui.draw(
            &mut renderer,
            &iced::Theme::Dark,
            &iced::advanced::renderer::Style { text_color: Color::WHITE },
            cursor,
        );
        let pixels = renderer.screenshot(
            &iced::advanced::graphics::Viewport::with_physical_size(Size::new(width, 800), 1.0),
            Color::from_rgb8(40, 80, 100),
        );
        assert!(
            pixels.as_chunks::<4>().0.iter().enumerate().any(|(index, pixel)| {
                bounds.contains(Point::new((index % width as usize) as f32, (index / width as usize) as f32))
                    && pixel[0] > 180
                    && pixel[1] > 180
                    && pixel[2] > 180
            }),
            "the restore label must be painted"
        );
        assert!(
            messages.iter().any(|message| format!("{message:?}") == "Tab"),
            "before={before} messages={messages:?}"
        );
    }
}

#[derive(Default)]
struct RestoreLabel(Vec<iced::Rectangle>);

impl iced::advanced::widget::Operation for RestoreLabel {
    fn traverse(&mut self, operate: &mut dyn FnMut(&mut dyn iced::advanced::widget::Operation)) {
        operate(self);
    }

    fn text(&mut self, _id: Option<&iced::widget::Id>, bounds: iced::Rectangle, text: &str) {
        if text == "展开信息条" {
            self.0.push(bounds);
        }
    }
}

#[test]
fn seam_title_pointer_clicks_use_the_original_four_pixel_radius() {
    let down = Some(Point::new(20.0, 30.0));
    assert!(seam_title_activates(false, down, Point::new(24.0, 30.0), false));
    assert!(seam_title_activates(false, down, Point::new(20.0, 34.0), false));
    assert!(!seam_title_activates(false, down, Point::new(24.01, 30.0), false));
    assert!(!seam_title_activates(false, down, Point::new(23.0, 33.0), false));
}

#[test]
fn seam_title_selection_blocks_pointer_activation_even_without_a_recorded_press() {
    assert!(!seam_title_activates(false, Some(Point::ORIGIN), Point::ORIGIN, true));
    assert!(!seam_title_activates(false, None, Point::ORIGIN, true));
    assert!(seam_title_activates(false, None, Point::new(500.0, 500.0), false));
}

#[test]
fn seam_title_keyboard_activation_ignores_pointer_history_and_selection() {
    assert!(seam_title_activates(true, Some(Point::ORIGIN), Point::new(500.0, 500.0), true));
    assert!(seam_title_activates(true, None, Point::ORIGIN, false));
}
