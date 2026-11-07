// The account tab's audio quality select from `CustomSelect.tsx`: a compact trigger and a menu that opens above it.

use std::time::Instant;

use folia_core::AudioQuality;
use folia_i18n::I18n;
use folia_render::Plate;
use iced::widget::{button, column, container, mouse_area, row, shader, stack, svg, text};
use iced::{Alignment, Border, Color, Element, Length, Vector};
use lucide_static_svg::Icon;

use crate::icons::icon;
use crate::motion::{CSS_TRANSITION, Tween};
use crate::settings_kit::Tokens;

/// The menu's `transition: 150ms easeOut`; the chevron turns over 200ms.
const MENU_MS: f64 = 150.0;
const CHEVRON_MS: f64 = 200.0;
/// The compact trigger: 32px high, 12px and 8px of padding, a 13px label and a 16px chevron at 70%.
const TRIGGER_HEIGHT: f32 = 32.0;
const TRIGGER_MIN_WIDTH: f32 = 136.0;
const TRIGGER_MAX_WIDTH: f32 = 240.0;
const TRIGGER_FONT: f32 = 13.0;
const TRIGGER_GAP: f32 = 8.0;
const TRIGGER_LEFT: f32 = 12.0;
const TRIGGER_RIGHT: f32 = 8.0;
const CHEVRON: f32 = 16.0;
const CHEVRON_OPACITY: f32 = 0.70;
const MENU_GAP: f32 = 4.0;
const VIEWPORT_GUTTER: f32 = 8.0;
const MENU_MIN_WIDTH: f32 = 160.0;
const MENU_MAX_WIDTH: f32 = 320.0;
const MENU_MAX_HEIGHT: f32 = 240.0;
const MENU_MIN_PREFERRED_HEIGHT: f32 = 160.0;
const ROW_HEIGHT: f32 = 30.0;
const ROW_GAP: f32 = 2.0;
/// `p-1`: 4px all round, 8px together.
const MENU_PADDING: f32 = 4.0;
/// `px-2.5`.
const ROW_PADDING: f32 = 10.0;
const CHECK: f32 = 14.0;
/// The 8px under the label and the 14px check the selected row holds.
const ROW_EXTRA: f32 = 22.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Message {
    Toggle,
    Close,
    Hover(Option<usize>),
    Pick(AudioQuality),
}

/// What the menu needs from the panel: its words, colours, levels and the chosen one.
#[derive(Clone, Copy)]
pub struct Look<'a> {
    pub i18n: &'a I18n,
    pub tokens: Tokens,
    pub daylight: bool,
    pub levels: &'a [AudioQuality],
    pub value: AudioQuality,
    pub now: Instant,
}

/// One row of the menu.
struct Row<'a> {
    i18n: &'a I18n,
    tokens: Tokens,
    daylight: bool,
    level: AudioQuality,
    chosen: bool,
    index: usize,
    alpha: f32,
}

/// Where the trigger stands in the panel's face, and how much window there is around it.
#[derive(Debug, Clone, Copy)]
pub struct Place {
    pub window_height: f32,
    /// Where the face starts in the window, above the trigger's face-relative edges.
    pub window_top: f32,
    pub trigger_top: f32,
    pub trigger_bottom: f32,
    pub trigger_right: f32,
    /// Where the face starts inside the layer the menu is drawn in.
    pub origin: (f32, f32),
}

pub struct Select {
    open: bool,
    hovered: Option<usize>,
    pop: Tween,
    chevron: Tween,
}

impl Select {
    pub fn new(now: Instant) -> Self {
        Self {
            open: false,
            hovered: None,
            pop: Tween::new(0.0, MENU_MS, folia_visualizer::Ease::OUT, now),
            chevron: Tween::new(0.0, CHEVRON_MS, CSS_TRANSITION, now),
        }
    }

    pub fn is_open(&self) -> bool {
        self.open
    }

    pub fn animating(&self, now: Instant) -> bool {
        self.pop.running(now) || self.chevron.running(now)
    }

    /// Returns the level picked, if the message picked one.
    pub fn update(
        &mut self,
        message: Message,
        levels: &[AudioQuality],
        value: AudioQuality,
        now: Instant,
    ) -> Option<AudioQuality> {
        match message {
            Message::Toggle => {
                if self.open {
                    self.close(now);
                } else {
                    self.open = true;
                    self.hovered = levels.iter().position(|level| *level == value);
                    self.pop.set_with(1.0, MENU_MS, folia_visualizer::Ease::OUT, now);
                    self.chevron.set(1.0, now);
                }
                None
            }
            Message::Close => {
                self.close(now);
                None
            }
            Message::Hover(index) => {
                self.hovered = index;
                None
            }
            Message::Pick(level) => {
                self.close(now);
                Some(level)
            }
        }
    }

    /// Closes the menu at once with the panel, without waiting for its exit.
    pub fn dismiss(&mut self, now: Instant) {
        self.open = false;
        self.hovered = None;
        self.pop = Tween::new(0.0, MENU_MS, folia_visualizer::Ease::OUT, now);
        self.chevron = Tween::new(0.0, CHEVRON_MS, CSS_TRANSITION, now);
    }

    fn close(&mut self, now: Instant) {
        self.open = false;
        self.hovered = None;
        self.pop.set_with(0.0, MENU_MS, folia_visualizer::Ease::OUT, now);
        self.chevron.set(0.0, now);
    }

    /// The compact trigger: 32px of `--st-fill`, 13px text and a 16px chevron, as wide as the value needs.
    pub fn trigger<'a>(
        &self,
        i18n: &'a I18n,
        tokens: Tokens,
        ink: Color,
        value: AudioQuality,
        now: Instant,
    ) -> Element<'a, crate::panel::Message> {
        let label = i18n.t(value.label_key());
        let width = (label_width(label) + TRIGGER_LEFT + TRIGGER_RIGHT + TRIGGER_GAP + CHEVRON)
            .clamp(TRIGGER_MIN_WIDTH, TRIGGER_MAX_WIDTH);
        let chevron = svg(svg::Handle::from_memory(Icon::ChevronDown.svg_bytes()))
            .width(Length::Fixed(CHEVRON))
            .height(Length::Fixed(CHEVRON))
            .rotation(iced::Radians(std::f32::consts::PI * self.chevron.value(now)))
            .style(move |_, _| svg::Style { color: Some(Color { a: 1.0, ..tokens.t1 }) })
            .opacity(ink.a * CHEVRON_OPACITY);
        let words = container(text(label).size(TRIGGER_FONT).color(ink).wrapping(iced::widget::text::Wrapping::None))
            .width(Length::Fill)
            .clip(true);
        let body = row![words, chevron].spacing(TRIGGER_GAP).align_y(Alignment::Center);
        let open = self.open;
        button(
            container(body).height(Length::Fixed(TRIGGER_HEIGHT)).align_y(Alignment::Center).padding(iced::Padding {
                top: 0.0,
                right: TRIGGER_RIGHT,
                bottom: 0.0,
                left: TRIGGER_LEFT,
            }),
        )
        .width(Length::Fixed(width))
        .height(Length::Fixed(TRIGGER_HEIGHT))
        .padding(0)
        .on_press(crate::panel::Message::QualitySelect(Message::Toggle))
        .style(move |_, status| {
            let hot = matches!(status, button::Status::Hovered | button::Status::Pressed);
            button::Style {
                background: Some((if open || hot { tokens.fill2 } else { tokens.fill }).into()),
                border: Border { radius: 8.0.into(), ..Border::default() },
                ..button::Style::default()
            }
        })
        .into()
    }

    /// The open menu, above the trigger where it fits, ending at the trigger's right edge.
    pub fn menu<'a>(&self, look: Look<'a>, place: Place) -> Option<Element<'a, crate::panel::Message>> {
        let Look { i18n, tokens, daylight, levels, value, now } = look;
        let pop = self.pop.value(now);
        if pop <= 0.0 || levels.is_empty() {
            return None;
        }
        let content = levels.iter().map(|level| label_width(i18n.t(level.label_key()))).fold(0.0, f32::max);
        let room = (place.origin.0 + place.trigger_right - VIEWPORT_GUTTER).max(MENU_MIN_WIDTH);
        let width = (content + ROW_EXTRA + 2.0 * ROW_PADDING + 2.0 * MENU_PADDING + 2.0)
            .clamp(MENU_MIN_WIDTH, MENU_MAX_WIDTH)
            .min(room);
        let natural = levels.len() as f32 * (ROW_HEIGHT + ROW_GAP) - ROW_GAP + 2.0 * MENU_PADDING + 2.0;
        let height = natural.min(MENU_MAX_HEIGHT);
        let above_room = place.window_top + place.trigger_top - MENU_GAP - VIEWPORT_GUTTER;
        let below_room = place.window_height - (place.window_top + place.trigger_bottom) - MENU_GAP - VIEWPORT_GUTTER;
        let above = above_room >= height || (below_room < MENU_MIN_PREFERRED_HEIGHT && above_room > below_room);
        let top = if above { place.trigger_top - MENU_GAP - height } else { place.trigger_bottom + MENU_GAP };
        let left = (place.trigger_right - width).max(VIEWPORT_GUTTER - place.origin.0);
        let alpha = pop;

        let mut options = column![].spacing(ROW_GAP);
        for (index, level) in levels.iter().enumerate() {
            let row = Row { i18n, tokens, daylight, level: *level, chosen: *level == value, index, alpha };
            options = options.push(self.option(row));
        }
        let body = column![options].padding(MENU_PADDING).width(Length::Fixed(width)).height(Length::Fixed(height));
        let plate = plate(tokens, width, height, alpha);
        let layer: Element<'a, crate::panel::Message> = stack![
            plate.element,
            container(body).padding(iced::Padding { top: plate.inset.1, left: plate.inset.0, ..iced::Padding::ZERO })
        ]
        .into();
        // Grows out of the trigger's right corner: the hidden state drops 8px and sits at 96%.
        let scale = 0.96 + 0.04 * pop;
        let lift = (if above { 8.0 } else { -8.0 }) * (1.0 - pop);
        let fix_x = (1.0 - scale) * width / 2.0;
        let fix_y = if above { (1.0 - scale) * height / 2.0 } else { -(1.0 - scale) * height / 2.0 };
        let layer: Element<'a, crate::panel::Message> =
            iced::widget::float(layer).translate(move |_, _| Vector::new(fix_x, lift + fix_y)).scale(scale).into();
        let top = (place.origin.1 + top - plate.inset.1).max(0.0);
        let left = (place.origin.0 + left - plate.inset.0).max(0.0);
        let placed = container(layer).padding(iced::Padding { top, left, ..iced::Padding::ZERO });
        Some(container(placed).width(Length::Fill).height(Length::Fill).into())
    }

    fn option<'a>(&self, row: Row<'a>) -> Element<'a, crate::panel::Message> {
        let Row { i18n, tokens, daylight, level, chosen, index, alpha } = row;
        let hovered = self.hovered == Some(index);
        let fade = move |color: Color| Color { a: color.a * alpha, ..color };
        let selected = Color { a: tokens.accent.a * alpha * if daylight { 0.071 } else { 0.094 }, ..tokens.accent };
        let background = if chosen {
            selected
        } else if hovered {
            Color { a: tokens.hover.a * alpha, ..tokens.hover }
        } else {
            Color::TRANSPARENT
        };
        let mut line = row![
            container(text(i18n.t(level.label_key())).size(TRIGGER_FONT).color(fade(tokens.t1)))
                .width(Length::Fill)
                .clip(true)
        ]
        .height(Length::Fill)
        .align_y(Alignment::Center)
        .spacing(8);
        if chosen {
            line = line.push(icon(Icon::Check, CHECK, fade(tokens.accent)));
        }
        let press = button(line)
            .width(Length::Fill)
            .height(Length::Fixed(ROW_HEIGHT))
            .padding(iced::Padding { top: 0.0, right: ROW_PADDING, bottom: 0.0, left: ROW_PADDING })
            .on_press(crate::panel::Message::QualitySelect(Message::Pick(level)))
            .style(move |_, status| {
                let hot = matches!(status, button::Status::Hovered | button::Status::Pressed);
                button::Style {
                    background: Some((if hot && !chosen { fade(tokens.hover) } else { background }).into()),
                    border: Border { radius: 6.0.into(), ..Border::default() },
                    ..button::Style::default()
                }
            });
        mouse_area(press)
            .on_enter(crate::panel::Message::QualitySelect(Message::Hover(Some(index))))
            .on_exit(crate::panel::Message::QualitySelect(Message::Hover(None)))
            .into()
    }
}

/// The width of a label at the trigger's 13px.
fn label_width(label: &str) -> f32 {
    folia_visualizer::measure(label, TRIGGER_FONT, iced::Font::DEFAULT, f32::INFINITY).width.ceil()
}

struct MenuPlate<'a> {
    element: Element<'a, crate::panel::Message>,
    /// How far the menu sits inside the plate widget, left and top.
    inset: (f32, f32),
}

/// The compact menu's opaque surface: `rounded-[10px]`, `0 14px 36px rgba(0,0,0,0.36)` and a 14% hairline.
fn plate<'a>(tokens: Tokens, width: f32, height: f32, alpha: f32) -> MenuPlate<'a> {
    let reach = 54.0;
    let top = reach - 14.0;
    let menu = tokens.menu;
    let ring = Color { a: 0.14 * tokens.t1.a, ..tokens.t1 };
    let plate = Plate {
        radius: 10.0,
        fill_origin: [reach, top],
        fill_size: [width, height],
        fill: [menu.r, menu.g, menu.b, menu.a * alpha],
        shadow: [0.0, 0.0, 0.0, 0.36 * alpha * menu.a],
        shadow_blur: 36.0,
        shadow_y: 14.0,
        shadow_spread: 0.0,
        ring: [ring.r, ring.g, ring.b, ring.a * alpha],
        backdrop: 0.0,
        fade: 1.0,
        saturate: 1.0,
    };
    let element = shader(plate)
        .width(Length::Fixed(width + 2.0 * reach))
        .height(Length::Fixed(height + top + reach + 14.0))
        .into();
    MenuPlate { element, inset: (reach, top) }
}
