// The audio effect window, from `AudioEqualizerDialog`, `EqualizerBandGrid`, `Fader`, `PresetChips` and
// `AudioEffectGrid`.

use crate::typeface::text;
use folia_core::{EQUALIZER_FREQUENCIES, EQUALIZER_MAX_GAIN_DB, EQUALIZER_MIN_GAIN_DB, EqualizerSettings};
use folia_i18n::I18n;
use iced::advanced::mouse::{self, Cursor};
use iced::advanced::renderer::{self, Quad};
use iced::advanced::widget::{self, Widget};
use iced::advanced::{Clipboard, Layout, Shell, layout};
use iced::widget::canvas::{self, Frame, Path, Stroke};
use iced::widget::{button, column, container, mouse_area, opaque, row, slider, space, stack, tooltip};
use iced::{
    Alignment, Background, Border, Color, Element, Event, Font, Length, Padding, Point, Rectangle, Size, Theme, Vector,
};
use lucide_static_svg::Icon;

use crate::icons::icon;
use crate::panel_sheets::Message;
use crate::settings_kit::{self, Tokens};

const CARD_MAX_W: f32 = 880.0;
const CARD_MAX_H: f32 = 720.0;
const HEADER_H: f32 = 64.0;
const TEXT_ROW: f32 = 22.0;
const THUMB_INSET: f32 = 8.0;
const BAND_CARD_MIN: f32 = 236.0;
const EFFECT_CARD_H: f32 = 182.0;

pub struct Look<'a> {
    pub i18n: &'a I18n,
    pub tokens: Tokens,
    pub daylight: bool,
    pub window: Size,
}

pub fn view<'a>(draft: &'a EqualizerSettings, look: &Look<'a>) -> Element<'a, Message> {
    let tokens = look.tokens;
    let width = (look.window.width - 32.0).clamp(360.0, CARD_MAX_W);
    let height = (look.window.height - 32.0).clamp(320.0, CARD_MAX_H);
    let dim = if draft.enabled { tokens } else { tokens.faded(0.4) };
    let body = column![
        preset_row(draft, look, tokens),
        section_title(look.i18n.t("ui.equalizerGain"), None, tokens),
        band_card(draft, dim),
        section_title(look.i18n.t("ui.equalizerEffects"), Some(look.i18n.t("ui.equalizerEffectsHint")), tokens),
        effect_card(draft, look, dim),
    ]
    .spacing(0)
    .padding(Padding { top: 12.0, right: 24.0, bottom: 20.0, left: 24.0 });
    let card = container(column![header(draft, look, tokens), body].spacing(0))
        .width(Length::Fixed(width))
        .height(Length::Fixed(height))
        .style(card_style(tokens));
    // `opaque`: a press on the card's empty space stops here instead of reaching the scrim and closing.
    let centred = container(opaque(card))
        .width(Length::Fill)
        .height(Length::Fill)
        .align_x(Alignment::Center)
        .align_y(Alignment::Center);
    layer(look, centred.into())
}

fn layer<'a>(look: &Look<'a>, card: Element<'a, Message>) -> Element<'a, Message> {
    let scrim = if look.daylight {
        Color { a: 0.36, ..Color::from_rgb8(0x18, 0x12, 0x0c) }
    } else {
        Color { a: 0.52, ..Color::BLACK }
    };
    let dim = opaque(
        mouse_area(container(space()).width(Length::Fill).height(Length::Fill).style(move |_theme: &Theme| {
            container::Style { background: Some(Background::Color(scrim)), ..container::Style::default() }
        }))
        .on_press(Message::EqualizerClose),
    );
    stack![dim, card].into()
}

fn card_style(tokens: Tokens) -> impl Fn(&Theme) -> container::Style {
    move |_theme| container::Style {
        background: Some(tokens.win.into()),
        border: Border { radius: 16.0.into(), width: 1.0, color: tokens.line2 },
        shadow: iced::Shadow {
            color: Color { a: if tokens.win.r > 0.5 { 0.26 } else { 0.6 }, ..Color::BLACK },
            offset: Vector::new(0.0, 36.0),
            blur_radius: 90.0,
        },
        text_color: Some(tokens.t1),
        ..container::Style::default()
    }
}

fn header<'a>(draft: &EqualizerSettings, look: &Look<'a>, tokens: Tokens) -> Element<'a, Message> {
    let i18n = look.i18n;
    let title = column![
        text(i18n.t("ui.equalizerTitle"))
            .size(crate::typeface::size::DIALOG_TITLE)
            .font(crate::typeface::adapt(settings_kit::SEMIBOLD))
            .line_height(iced::widget::text::LineHeight::Absolute(iced::Pixels(32.0)))
            .color(tokens.t1),
        text(i18n.t("ui.equalizerDescription"))
            .size(crate::typeface::size::HINT)
            .line_height(iced::widget::text::LineHeight::Absolute(iced::Pixels(18.0)))
            .color(tokens.t2),
    ]
    .spacing(0);
    let switch = row![
        text(i18n.t(if draft.enabled { "ui.equalizerEnabled" } else { "ui.equalizerDisabled" }))
            .size(crate::typeface::size::HINT)
            .color(tokens.t2)
            .wrapping(iced::widget::text::Wrapping::None),
        toggle(tokens, draft.enabled),
    ]
    .spacing(12)
    .align_y(Alignment::Center);
    let close = button(container(icon(Icon::X, 16.0, tokens.t2)).center(Length::Fixed(32.0)))
        .padding(0)
        .on_press(Message::EqualizerClose)
        .style(move |_theme: &Theme, status| {
            let hot = matches!(status, button::Status::Hovered | button::Status::Pressed);
            button::Style {
                background: hot.then_some(Background::Color(tokens.fill)),
                text_color: if hot { tokens.t1 } else { tokens.t2 },
                border: Border { radius: 8.0.into(), ..Border::default() },
                ..button::Style::default()
            }
        });
    let close = tooltip(close, text(i18n.t("ui.close")).size(crate::typeface::size::HINT), tooltip::Position::Top)
        .style(settings_kit::faded_box(1.0));
    container(row![title, space().width(Length::Fill), switch, close].spacing(8).align_y(Alignment::Start))
        .width(Length::Fill)
        .height(Length::Fixed(HEADER_H))
        .align_y(Alignment::Start)
        .padding(Padding { top: 16.0, right: 16.0, bottom: 0.0, left: 24.0 })
        .into()
}

fn toggle<'a>(tokens: Tokens, on: bool) -> Element<'a, Message> {
    let knob_color = if on { tokens.on_accent } else { tokens.knob };
    let knob = container(space()).width(18).height(18).style(move |_theme: &Theme| container::Style {
        background: Some(knob_color.into()),
        border: Border { radius: 9.0.into(), ..Border::default() },
        shadow: iced::Shadow {
            color: Color { a: 0.3, ..Color::BLACK },
            offset: Vector::new(0.0, 1.0),
            blur_radius: 2.0,
        },
        ..container::Style::default()
    });
    button(
        container(knob)
            .width(40)
            .height(24)
            .padding(Padding { top: 3.0, right: 0.0, bottom: 0.0, left: if on { 19.0 } else { 3.0 } })
            .style(move |_theme: &Theme| container::Style {
                background: Some(if on { tokens.accent } else { tokens.track }.into()),
                border: Border { radius: 12.0.into(), ..Border::default() },
                ..container::Style::default()
            }),
    )
    .padding(0)
    .on_press(Message::EqualizerToggle(!on))
    .style(button::text)
    .into()
}

fn preset_row<'a>(draft: &EqualizerSettings, look: &Look<'a>, tokens: Tokens) -> Element<'a, Message> {
    let ids = [
        ("flat", "ui.equalizerPreset.flat"),
        ("lofi", "ui.equalizerPreset.lofi"),
        ("radio", "ui.equalizerPreset.radio"),
        ("hall", "ui.equalizerPreset.hall"),
        ("vocal", "ui.equalizerPreset.vocal"),
        ("bass", "ui.equalizerPreset.bass"),
        ("custom1", "ui.equalizerPreset.custom1"),
        ("custom2", "ui.equalizerPreset.custom2"),
    ];
    let mut chips = row![].spacing(6);
    for (id, key) in ids {
        chips = chips.push(preset_chip(look, tokens, id, key, draft.preset == id));
    }
    let chips = chips.wrap();
    let custom = matches!(draft.preset.as_str(), "custom1" | "custom2");
    let reset_ink = if custom { tokens.t2 } else { Color { a: tokens.t2.a * 0.4, ..tokens.t2 } };
    let reset = button(container(icon(Icon::RotateCcw, 16.0, reset_ink)).center(Length::Fixed(32.0)))
        .padding(0)
        .on_press_maybe(custom.then_some(Message::EqualizerReset))
        .style(move |_theme: &Theme, status| {
            let hot = custom && matches!(status, button::Status::Hovered | button::Status::Pressed);
            button::Style {
                background: hot.then_some(Background::Color(tokens.fill)),
                text_color: if custom { tokens.t2 } else { tokens.t3 },
                border: Border { radius: 8.0.into(), ..Border::default() },
                ..button::Style::default()
            }
        });
    let reset = tooltip(
        reset,
        text(look.i18n.t("ui.equalizerReset")).size(crate::typeface::size::HINT),
        tooltip::Position::Top,
    )
    .style(settings_kit::faded_box(1.0));
    row![container(chips).width(Length::Fill), reset]
        .spacing(8)
        .align_y(Alignment::Start)
        .height(Length::Fixed(32.0))
        .into()
}

fn preset_chip<'a>(look: &Look<'a>, tokens: Tokens, id: &'a str, key: &'a str, chosen: bool) -> Element<'a, Message> {
    button(container(text(look.i18n.t(key)).size(crate::typeface::size::CONTROL)).center_y(Length::Fixed(28.0)))
        .height(Length::Fixed(28.0))
        .padding(Padding { top: 0.0, right: 12.0, bottom: 0.0, left: 12.0 })
        .on_press(Message::EqualizerPreset(id.to_owned()))
        .style(move |_theme: &Theme, status| {
            let hot = matches!(status, button::Status::Hovered | button::Status::Pressed);
            let (fill, ink) = if chosen {
                (tokens.accent, tokens.on_accent)
            } else if hot {
                (tokens.fill2, tokens.t1)
            } else {
                (tokens.fill, tokens.t2)
            };
            button::Style {
                background: Some(Background::Color(fill)),
                text_color: ink,
                border: Border { radius: 8.0.into(), ..Border::default() },
                ..button::Style::default()
            }
        })
        .into()
}

fn section_title<'a>(title: &'a str, hint: Option<&'a str>, tokens: Tokens) -> Element<'a, Message> {
    let mut line = row![
        text(title)
            .size(crate::typeface::size::CONTROL)
            .font(crate::typeface::adapt(settings_kit::SEMIBOLD))
            .color(tokens.t2)
    ]
    .spacing(12)
    .align_y(Alignment::Center)
    .padding(Padding { top: 0.0, right: 4.0, bottom: 0.0, left: 4.0 })
    .height(Length::Fixed(24.0));
    if let Some(hint) = hint {
        line =
            line.push(space().width(Length::Fill)).push(text(hint).size(crate::typeface::size::HINT).color(tokens.t3));
    }
    column![container(line).padding(Padding { top: 20.0, right: 0.0, bottom: 8.0, left: 0.0 })]
        .height(Length::Fixed(52.0))
        .into()
}

fn band_card<'a>(draft: &EqualizerSettings, tokens: Tokens) -> Element<'a, Message> {
    let scale = column![
        text("+12 dB")
            .size(crate::typeface::size::CAPTION)
            .font(crate::typeface::adapt(Font::MONOSPACE))
            .color(tokens.t3),
        space().height(Length::Fill),
        text("0").size(crate::typeface::size::CAPTION).font(crate::typeface::adapt(Font::MONOSPACE)).color(tokens.t3),
        space().height(Length::Fill),
        text("−12 dB")
            .size(crate::typeface::size::CAPTION)
            .font(crate::typeface::adapt(Font::MONOSPACE))
            .color(tokens.t3),
    ]
    .align_x(Alignment::End)
    .height(Length::Fill)
    .padding(Padding { top: TEXT_ROW, right: 0.0, bottom: TEXT_ROW, left: 0.0 });
    let scale = container(scale).width(Length::Fixed(44.0)).height(Length::Fill);

    let mut bands = row![].spacing(0).height(Length::Fill);
    for index in 0..EQUALIZER_FREQUENCIES.len() {
        bands = bands.push(band(draft.gains[index], index, tokens));
    }
    let curve = canvas::Canvas::new(Curve { gains: draft.gains, line: tokens.line2, accent: tokens.accent })
        .width(Length::Fill)
        .height(Length::Fill);
    let grid = stack![curve, bands].width(Length::Fill).height(Length::Fill);
    let hz = container(
        text("Hz").size(crate::typeface::size::CAPTION).font(crate::typeface::adapt(Font::MONOSPACE)).color(tokens.t3),
    )
    .width(Length::Fixed(20.0))
    .height(Length::Fill)
    .align_y(Alignment::End)
    .align_x(Alignment::Start);
    container(row![scale, grid, hz].spacing(8).height(Length::Fill))
        .width(Length::Fill)
        .height(Length::Fill)
        .padding(Padding { top: 14.0, right: 16.0, bottom: 12.0, left: 16.0 })
        .style(move |_theme: &Theme| container::Style {
            background: Some(tokens.card.into()),
            border: Border { radius: 12.0.into(), width: 1.0, color: tokens.line },
            ..container::Style::default()
        })
        .into()
}

fn band<'a>(gain: f32, index: usize, tokens: Tokens) -> Element<'a, Message> {
    column![
        container(
            text(signed(gain))
                .size(crate::typeface::size::CAPTION)
                .font(crate::typeface::adapt(Font::MONOSPACE))
                .color(tokens.t2)
        )
        .height(Length::Fixed(16.0))
        .align_x(Alignment::Center),
        Element::new(Fader { index, value: gain, tokens }),
        container(
            text(band_label(index))
                .size(crate::typeface::size::CAPTION)
                .font(crate::typeface::adapt(Font::MONOSPACE))
                .color(tokens.t3)
        )
        .height(Length::Fixed(16.0))
        .align_x(Alignment::Center),
    ]
    .spacing(6)
    .width(Length::Fill)
    .height(Length::Fill)
    .align_x(Alignment::Center)
    .into()
}

fn band_label(index: usize) -> String {
    let hz = EQUALIZER_FREQUENCIES[index];
    if hz >= 1000.0 { format!("{:.0}k", hz / 1000.0) } else { format!("{hz:.0}") }
}

fn signed(gain: f32) -> String {
    let rounded = gain.round();
    if rounded > 0.0 {
        format!("+{rounded}")
    } else if rounded < 0.0 {
        format!("−{}", rounded.abs())
    } else {
        "0".to_owned()
    }
}

fn effect_card<'a>(draft: &EqualizerSettings, look: &Look<'a>, tokens: Tokens) -> Element<'a, Message> {
    let mut grid = column![].spacing(6).width(Length::Fill);
    for line in 0..3 {
        let mut cells = row![].spacing(28).width(Length::Fill);
        for column_index in 0..3 {
            let index = line * 3 + column_index;
            cells = cells.push(effect_cell(draft, look, tokens, index));
        }
        grid = grid.push(cells);
    }
    container(grid)
        .width(Length::Fill)
        .height(Length::Fixed(EFFECT_CARD_H))
        .padding(Padding { top: 12.0, right: 16.0, bottom: 14.0, left: 16.0 })
        .style(move |_theme: &Theme| container::Style {
            background: Some(tokens.card.into()),
            border: Border { radius: 12.0.into(), width: 1.0, color: tokens.line },
            ..container::Style::default()
        })
        .into()
}

fn effect_cell<'a>(draft: &EqualizerSettings, look: &Look<'a>, tokens: Tokens, index: usize) -> Element<'a, Message> {
    let control = EFFECTS[index];
    let value = effect_value(draft, index);
    let name = look.i18n.t(control.key);
    let mut line = row![
        text(name)
            .size(crate::typeface::size::CONTROL)
            .line_height(iced::widget::text::LineHeight::Absolute(iced::Pixels(20.0)))
            .color(tokens.t1),
    ]
    .spacing(6)
    .align_y(Alignment::Center);
    if control.noise {
        let neutral = (value - control.neutral).abs() < f32::EPSILON;
        let badge =
            container(
                text(look.i18n.t("ui.equalizerEffectNoiseTag"))
                    .size(crate::typeface::size::CAPTION)
                    .color(if neutral { tokens.t2 } else { tokens.on_accent }),
            )
            .padding(Padding { top: 0.0, right: 5.0, bottom: 0.0, left: 5.0 })
            .style(move |_theme: &Theme| container::Style {
                background: Some(if neutral { tokens.fill2 } else { tokens.accent }.into()),
                border: Border { radius: 4.0.into(), ..Border::default() },
                ..container::Style::default()
            });
        line = line.push(
            tooltip(
                badge,
                text(look.i18n.t("ui.equalizerEffectNoiseTagHint")).size(crate::typeface::size::HINT),
                tooltip::Position::Top,
            )
            .style(settings_kit::faded_box(1.0)),
        );
    }
    line = line.push(space().width(Length::Fill)).push(
        text(format_effect(&control, value))
            .size(crate::typeface::size::HINT)
            .font(crate::typeface::adapt(Font::MONOSPACE))
            .color(tokens.t2),
    );
    let position = control.position(value);
    let slider = slider(0.0..=1000.0, position, move |place| Message::EqualizerEffect(index, control.value_at(place)))
        .step(1.0_f32)
        .on_release(Message::EqualizerEffectDone)
        .width(Length::Fill)
        .style(move |_, _| settings_kit::slider_style(tokens));
    column![line, container(slider).padding(Padding { top: 4.0, right: 0.0, bottom: 0.0, left: 0.0 })]
        .spacing(0)
        .width(Length::Fill)
        .into()
}

#[derive(Debug, Clone, Copy)]
struct Effect {
    key: &'static str,
    min: f32,
    max: f32,
    step: f32,
    neutral: f32,
    log: bool,
    hz: bool,
    noise: bool,
}

impl Effect {
    /// `audioEffectToSliderPosition`: 0..1000 along the control's travel, logarithmic for the cutoffs.
    fn position(self, value: f32) -> f32 {
        let value = value.clamp(self.min, self.max);
        if self.log {
            let min = self.min.ln();
            let span = self.max.ln() - min;
            ((value.ln() - min) / span * 1000.0).round()
        } else {
            ((value - self.min) / (self.max - self.min) * 1000.0).round()
        }
    }

    /// `sliderPositionToAudioEffect`, rounded onto the control's step grid.
    fn value_at(self, position: f32) -> f32 {
        let ratio = (position / 1000.0).clamp(0.0, 1.0);
        if self.log {
            (self.min.ln() + ratio * (self.max.ln() - self.min.ln())).exp().round()
        } else {
            let raw = self.min + ratio * (self.max - self.min);
            (raw / self.step).round() * self.step
        }
    }
}

const EFFECTS: [Effect; 9] = [
    Effect {
        key: "ui.equalizerEffect.highpass",
        min: 20.0,
        max: 2000.0,
        step: 1.0,
        neutral: 20.0,
        log: true,
        hz: true,
        noise: false,
    },
    Effect {
        key: "ui.equalizerEffect.lowpass",
        min: 500.0,
        max: 20000.0,
        step: 1.0,
        neutral: 20000.0,
        log: true,
        hz: true,
        noise: false,
    },
    Effect {
        key: "ui.equalizerEffect.drive",
        min: 0.0,
        max: 1.0,
        step: 0.01,
        neutral: 0.0,
        log: false,
        hz: false,
        noise: false,
    },
    Effect {
        key: "ui.equalizerEffect.crush",
        min: 0.0,
        max: 1.0,
        step: 0.01,
        neutral: 0.0,
        log: false,
        hz: false,
        noise: true,
    },
    Effect {
        key: "ui.equalizerEffect.wow",
        min: 0.0,
        max: 1.0,
        step: 0.01,
        neutral: 0.0,
        log: false,
        hz: false,
        noise: false,
    },
    Effect {
        key: "ui.equalizerEffect.noise",
        min: 0.0,
        max: 1.0,
        step: 0.01,
        neutral: 0.0,
        log: false,
        hz: false,
        noise: true,
    },
    Effect {
        key: "ui.equalizerEffect.width",
        min: 0.0,
        max: 2.0,
        step: 0.01,
        neutral: 1.0,
        log: false,
        hz: false,
        noise: false,
    },
    Effect {
        key: "ui.equalizerEffect.space",
        min: 0.0,
        max: 1.0,
        step: 0.01,
        neutral: 0.0,
        log: false,
        hz: false,
        noise: false,
    },
    Effect {
        key: "ui.equalizerEffect.punch",
        min: 0.0,
        max: 1.0,
        step: 0.01,
        neutral: 0.0,
        log: false,
        hz: false,
        noise: false,
    },
];

fn effect_value(settings: &EqualizerSettings, index: usize) -> f32 {
    let effects = &settings.effects;
    [
        effects.highpass,
        effects.lowpass,
        effects.drive,
        effects.crush,
        effects.wow,
        effects.noise,
        effects.width,
        effects.space,
        effects.punch,
    ][index]
}

fn format_effect(control: &Effect, value: f32) -> String {
    if control.hz {
        if value >= 1000.0 { format!("{:.1} kHz", value / 1000.0) } else { format!("{value:.0} Hz") }
    } else {
        format!("{:.0}%", value * 100.0)
    }
}

/// `EqualizerBandGrid`'s curve: a cubic through the thumbs, over the same travel the faders use.
struct Curve {
    gains: [f32; 10],
    line: Color,
    accent: Color,
}

impl canvas::Program<Message> for Curve {
    type State = ();

    fn draw(
        &self,
        _state: &Self::State,
        renderer: &iced::Renderer,
        _theme: &Theme,
        bounds: Rectangle,
        _cursor: Cursor,
    ) -> Vec<canvas::Geometry> {
        let mut frame = Frame::new(renderer, bounds.size());
        let top = TEXT_ROW + THUMB_INSET;
        let travel = (bounds.height - 2.0 * top).max(1.0);
        let span = EQUALIZER_MAX_GAIN_DB - EQUALIZER_MIN_GAIN_DB;
        let y = |gain: f32| (EQUALIZER_MAX_GAIN_DB - gain) / span * travel;
        let x = |index: usize| (index as f32 + 0.5) / 10.0 * bounds.width;
        let base = Path::line(
            Point::new(0.5 / 10.0 * bounds.width, top + travel / 2.0),
            Point::new(9.5 / 10.0 * bounds.width, top + travel / 2.0),
        );
        frame.stroke(&base, Stroke::default().with_color(self.line).with_width(1.0));
        let points: Vec<(f32, f32)> = (0..10).map(|index| (x(index), top + y(self.gains[index]))).collect();
        let path = Path::new(|curve| {
            curve.move_to(Point::new(points[0].0, points[0].1));
            for pair in 1..points.len() {
                let (from_x, from_y) = points[pair - 1];
                let (to_x, to_y) = points[pair];
                let middle = (from_x + to_x) / 2.0;
                curve.bezier_curve_to(Point::new(middle, from_y), Point::new(middle, to_y), Point::new(to_x, to_y));
            }
        });
        frame.stroke(
            &path,
            Stroke::default().with_color(Color { a: self.accent.a * 0.5, ..self.accent }).with_width(1.5),
        );
        vec![frame.into_geometry()]
    }
}

/// `Fader`: a vertical track filled from the middle to a 16px thumb, as tall as its place.
struct Fader {
    index: usize,
    value: f32,
    tokens: Tokens,
}

#[derive(Debug, Default)]
struct FaderState {
    held: bool,
}

impl Fader {
    fn value_at(&self, y: f32, bounds: Rectangle) -> f32 {
        let travel = bounds.height - 16.0;
        let from_top = if travel > 0.0 { ((y - bounds.y - 8.0) / travel).clamp(0.0, 1.0) } else { 0.5 };
        let raw = EQUALIZER_MAX_GAIN_DB - from_top * (EQUALIZER_MAX_GAIN_DB - EQUALIZER_MIN_GAIN_DB);
        raw.round().clamp(EQUALIZER_MIN_GAIN_DB, EQUALIZER_MAX_GAIN_DB)
    }
}

impl Widget<Message, Theme, iced::Renderer> for Fader {
    fn tag(&self) -> widget::tree::Tag {
        widget::tree::Tag::of::<FaderState>()
    }

    fn state(&self) -> widget::tree::State {
        widget::tree::State::new(FaderState::default())
    }

    fn children(&self) -> Vec<widget::Tree> {
        Vec::new()
    }

    fn size(&self) -> Size<Length> {
        Size::new(Length::Fixed(24.0), Length::Fill)
    }

    fn layout(
        &mut self,
        _tree: &mut widget::Tree,
        _renderer: &iced::Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        layout::Node::new(limits.resolve(Length::Fixed(24.0), Length::Fill, Size::new(24.0, BAND_CARD_MIN)))
    }

    fn update(
        &mut self,
        tree: &mut widget::Tree,
        event: &Event,
        layout: Layout<'_>,
        cursor: Cursor,
        _renderer: &iced::Renderer,
        _clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
        _viewport: &Rectangle,
    ) {
        let state = tree.state.downcast_mut::<FaderState>();
        let bounds = layout.bounds();
        match event {
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) if cursor.is_over(bounds) => {
                state.held = true;
                shell.capture_event();
                if let Some(position) = cursor.position() {
                    shell.publish(Message::EqualizerBand(self.index, self.value_at(position.y, bounds)));
                }
            }
            Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) if state.held => {
                state.held = false;
                shell.publish(Message::EqualizerBandDone);
            }
            Event::Mouse(mouse::Event::CursorMoved { .. }) if state.held => {
                if let Some(position) = cursor.position() {
                    shell.publish(Message::EqualizerBand(self.index, self.value_at(position.y, bounds)));
                }
            }
            _ => {}
        }
    }

    fn draw(
        &self,
        tree: &widget::Tree,
        renderer: &mut iced::Renderer,
        _theme: &Theme,
        _style: &renderer::Style,
        layout: Layout<'_>,
        _cursor: Cursor,
        _viewport: &Rectangle,
    ) {
        use iced::advanced::Renderer as _;
        let state = tree.state.downcast_ref::<FaderState>();
        let bounds = layout.bounds();
        let height = bounds.height;
        let ratio =
            ((self.value - EQUALIZER_MIN_GAIN_DB) / (EQUALIZER_MAX_GAIN_DB - EQUALIZER_MIN_GAIN_DB)).clamp(0.0, 1.0);
        let track = Rectangle::new(Point::new(bounds.x + 10.0, bounds.y), Size::new(4.0, height));
        renderer.fill_quad(
            Quad { bounds: track, border: Border { radius: 2.0.into(), ..Border::default() }, ..Quad::default() },
            self.tokens.track,
        );
        let middle = bounds.y + 8.0 + (height - 16.0) / 2.0;
        let thumb = bounds.y + 8.0 + (1.0 - ratio) * (height - 16.0);
        let (from, to) = if middle <= thumb { (middle, thumb) } else { (thumb, middle) };
        let fill = Rectangle::new(Point::new(bounds.x + 10.0, from), Size::new(4.0, (to - from).max(0.0)));
        renderer.fill_quad(
            Quad { bounds: fill, border: Border { radius: 2.0.into(), ..Border::default() }, ..Quad::default() },
            self.tokens.accent,
        );
        let radius = if state.held { 9.2 } else { 8.0 };
        let centre = Point::new(bounds.x + bounds.width / 2.0, thumb);
        let circle = |radius: f32| {
            Rectangle::new(Point::new(centre.x - radius, centre.y - radius), Size::new(radius * 2.0, radius * 2.0))
        };
        renderer.fill_quad(
            Quad {
                bounds: Rectangle { y: circle(radius).y + 1.0, ..circle(radius) },
                border: Border { radius: radius.into(), ..Border::default() },
                ..Quad::default()
            },
            Color { a: 0.35, ..Color::BLACK },
        );
        renderer.fill_quad(
            Quad {
                bounds: circle(radius),
                border: Border { radius: radius.into(), width: 1.0, color: Color { a: 0.14, ..Color::BLACK } },
                ..Quad::default()
            },
            self.tokens.knob,
        );
    }

    fn mouse_interaction(
        &self,
        tree: &widget::Tree,
        layout: Layout<'_>,
        cursor: Cursor,
        _viewport: &Rectangle,
        _renderer: &iced::Renderer,
    ) -> mouse::Interaction {
        let state = tree.state.downcast_ref::<FaderState>();
        if state.held {
            mouse::Interaction::Grabbing
        } else if cursor.is_over(layout.bounds()) {
            mouse::Interaction::Pointer
        } else {
            mouse::Interaction::None
        }
    }
}
