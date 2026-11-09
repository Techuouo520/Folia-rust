// The playlist picker over the window, from `PlaylistSelectionDialog` and `TextInputDialog`.

use folia_i18n::I18n;
use iced::widget::{button, column, container, mouse_area, opaque, row, scrollable, space, text, text_input, tooltip};
use iced::{Alignment, Background, Border, Color, Element, Length, Padding, Size, Theme as IcedTheme};
use lucide_static_svg::Icon;

use crate::icons::icon;
use crate::settings_kit::{self, Tokens};

/// A playlist the playing song can go into.
#[derive(Debug, Clone, PartialEq)]
pub struct PlaylistChoice {
    /// The local playlist's row id, or `None` for an online one.
    pub local: Option<i64>,
    /// The online platform and playlist id.
    pub online: Option<(String, String)>,
    pub name: String,
    pub detail: String,
}

#[derive(Debug, Clone)]
pub enum Message {
    Close,
    /// A row, by its place in the list the window was given.
    Pick(usize),
    /// The "new playlist" prompt in front of the list.
    Create,
    Name(String),
    Confirm,
    CancelCreate,
}

/// What the dialog asks the shell to do.
#[derive(Debug, Clone)]
pub enum Event {
    None,
    Pick(usize),
    Create(String),
}

/// `AddToPlaylistHost`'s open flag, the picker and the name prompt.
#[derive(Debug, Clone, Default)]
pub struct Dialog {
    open: bool,
    creating: bool,
    name: String,
}

pub struct Look<'a> {
    pub i18n: &'a I18n,
    pub tokens: Tokens,
    pub daylight: bool,
    pub window: Size,
    pub playlists: &'a [PlaylistChoice],
    pub can_create: bool,
}

/// The name field, focused when the prompt opens (`data-autofocus`).
pub const NAME_INPUT: &str = "playlist-dialog-name";

const SHEET_M: f32 = 600.0;
const SHEET_S: f32 = 400.0;
const MAX_HEIGHT: f32 = 720.0;
const HEADER_H: f32 = 48.0;
const ROW_H: f32 = 56.0;
const BODY_TOP: f32 = 12.0;
const BODY_BOTTOM: f32 = 20.0;
const DESCRIPTION_H: f32 = 35.0;
const CREATE_H: f32 = 52.0;

impl Dialog {
    pub fn open(&mut self) {
        self.open = true;
        self.creating = false;
        self.name.clear();
    }

    pub fn is_open(&self) -> bool {
        self.open
    }

    pub fn close(&mut self) {
        self.open = false;
        self.creating = false;
        self.name.clear();
    }

    pub fn update(&mut self, message: Message) -> Event {
        match message {
            Message::Close => {
                self.close();
                Event::None
            }
            Message::Pick(index) => {
                self.close();
                Event::Pick(index)
            }
            Message::Create => {
                self.creating = true;
                self.name.clear();
                Event::None
            }
            Message::Name(name) => {
                self.name = name;
                Event::None
            }
            Message::Confirm => {
                let name = self.name.trim().to_owned();
                if name.is_empty() {
                    return Event::None;
                }
                self.close();
                Event::Create(name)
            }
            Message::CancelCreate => {
                self.close();
                Event::None
            }
        }
    }

    pub fn view<'a>(&self, look: Look<'a>) -> Option<Element<'a, Message>> {
        if !self.open {
            return None;
        }
        let card = if self.creating { creating(self, &look) } else { picker(&look) };
        // `opaque`: a press on the card's empty space stops here instead of reaching the scrim and closing.
        let card = container(opaque(card))
            .width(Length::Fill)
            .height(Length::Fill)
            .align_x(Alignment::Center)
            .align_y(Alignment::Center);
        Some(layer(&look, card.into()))
    }
}

/// The dim layer that takes the pointer, and the centred card.
fn layer<'a>(look: &Look<'a>, card: Element<'a, Message>) -> Element<'a, Message> {
    let scrim = if look.daylight {
        Color { a: 0.36, ..Color::from_rgb8(0x18, 0x12, 0x0c) }
    } else {
        Color { a: 0.52, ..Color::BLACK }
    };
    let dim = opaque(
        mouse_area(container(space()).width(Length::Fill).height(Length::Fill).style(move |_theme: &IcedTheme| {
            container::Style { background: Some(Background::Color(scrim)), ..container::Style::default() }
        }))
        .on_press(Message::Close),
    );
    iced::widget::stack![dim, card].into()
}

fn picker<'a>(look: &Look<'a>) -> Element<'a, Message> {
    let tokens = look.tokens;
    let width = (look.window.width - 32.0).clamp(200.0, SHEET_M);
    let max_h = (look.window.height - 32.0).clamp(160.0, MAX_HEIGHT);
    let rows = look.playlists.len();
    let empty = rows == 0;
    let body_extra = if empty { 0.0 } else { rows as f32 * ROW_H };
    let fixed = HEADER_H + BODY_TOP + DESCRIPTION_H + BODY_BOTTOM + if look.can_create { CREATE_H } else { 0.0 };
    let card_h = (fixed + if empty { 82.0 } else { body_extra }).min(max_h);

    let description = text(look.i18n.t("home.playlists"))
        .size(13)
        .line_height(iced::widget::text::LineHeight::Absolute(iced::Pixels(21.0)))
        .color(tokens.t2)
        .width(Length::Fill);
    let mut body =
        column![].spacing(0).padding(Padding { top: BODY_TOP, right: 24.0, bottom: BODY_BOTTOM, left: 24.0 });
    body = body.push(container(description).height(Length::Fixed(DESCRIPTION_H)).align_y(Alignment::Start));
    if look.can_create {
        body = body.push(create_button(look, tokens));
    }
    let list_h = card_h - fixed;
    body = body.push(list(look, list_h, empty));

    container(column![header(look, "localMusic.addToPlaylist"), body].spacing(0))
        .width(Length::Fixed(width))
        .height(Length::Fixed(card_h))
        .style(card_style(tokens))
        .into()
}

fn creating<'a>(dialog: &Dialog, look: &Look<'a>) -> Element<'a, Message> {
    let tokens = look.tokens;
    let width = (look.window.width - 32.0).clamp(200.0, SHEET_S);
    let height = (look.window.height - 32.0).clamp(160.0, 208.0);
    let description = text(look.i18n.t("localMusic.enterPlaylistName"))
        .size(13)
        .line_height(iced::widget::text::LineHeight::Absolute(iced::Pixels(21.0)))
        .color(tokens.t2);
    let body = column![
        container(description).height(Length::Fixed(DESCRIPTION_H)).align_y(Alignment::Start),
        field_input(look, &dialog.name, tokens),
    ]
    .spacing(0)
    .padding(Padding { top: BODY_TOP, right: 24.0, bottom: 20.0, left: 24.0 });
    let rule = container(space()).width(Length::Fill).height(Length::Fixed(1.0)).style(move |_theme: &IcedTheme| {
        container::Style { background: Some(tokens.line.into()), ..container::Style::default() }
    });
    let ready = !dialog.name.trim().is_empty();
    let footer = container(
        row![
            sheet_button(tokens, look.i18n.t("localMusic.cancel"), false, Some(Message::CancelCreate)),
            sheet_button(tokens, look.i18n.t("options.save"), true, ready.then_some(Message::Confirm)),
        ]
        .spacing(8)
        .align_y(Alignment::Center),
    )
    .width(Length::Fill)
    .height(Length::Fixed(56.0))
    .align_x(Alignment::End)
    .align_y(Alignment::Center)
    .padding(Padding { top: 0.0, right: 16.0, bottom: 0.0, left: 16.0 });
    container(column![header(look, "localMusic.createPlaylist"), body, rule, footer].spacing(0))
        .width(Length::Fixed(width))
        .height(Length::Fixed(height))
        .style(card_style(tokens))
        .into()
}

fn field_input<'a>(look: &Look<'a>, name: &str, tokens: Tokens) -> Element<'a, Message> {
    text_input(look.i18n.t("localMusic.enterPlaylistName"), name)
        .id(iced::widget::Id::new(NAME_INPUT))
        .on_input(Message::Name)
        .on_submit(Message::Confirm)
        .size(13)
        .padding(Padding { top: 8.0, right: 12.0, bottom: 8.0, left: 12.0 })
        .style(move |_theme: &IcedTheme, _status| iced::widget::text_input::Style {
            background: Background::Color(tokens.fill),
            border: Border { radius: 8.0.into(), width: 1.0, color: Color::TRANSPARENT },
            icon: tokens.t3,
            placeholder: tokens.t3,
            value: tokens.t1,
            selection: tokens.fill2,
        })
        .into()
}

fn header<'a>(look: &Look<'a>, title_key: &'a str) -> Element<'a, Message> {
    let tokens = look.tokens;
    let title = text(look.i18n.t(title_key))
        .size(16)
        .font(settings_kit::SEMIBOLD)
        .line_height(iced::widget::text::LineHeight::Absolute(iced::Pixels(32.0)))
        .color(tokens.t1);
    let close = button(container(icon(Icon::X, 16.0, tokens.t2)).center(Length::Fixed(32.0)))
        .padding(0)
        .on_press(Message::Close)
        .style(move |_theme: &IcedTheme, status| {
            let hot = matches!(status, button::Status::Hovered | button::Status::Pressed);
            button::Style {
                background: hot.then_some(Background::Color(tokens.fill)),
                text_color: if hot { tokens.t1 } else { tokens.t2 },
                border: Border { radius: 8.0.into(), ..Border::default() },
                ..button::Style::default()
            }
        });
    let close = tooltip(close, text(look.i18n.t("ui.close")).size(12), tooltip::Position::Top)
        .style(settings_kit::faded_box(1.0));
    container(row![title, space().width(Length::Fill), close].spacing(12).align_y(Alignment::Start))
        .width(Length::Fill)
        .height(Length::Fixed(HEADER_H))
        .align_y(Alignment::Start)
        .padding(Padding { top: 16.0, right: 16.0, bottom: 0.0, left: 24.0 })
        .into()
}

fn create_button<'a>(look: &Look<'a>, tokens: Tokens) -> Element<'a, Message> {
    button(
        row![
            icon(Icon::Plus, 16.0, tokens.t1),
            text(look.i18n.t("localMusic.createPlaylist")).size(13).color(tokens.t1),
        ]
        .spacing(8)
        .align_y(Alignment::Center),
    )
    .width(Length::Fill)
    .height(Length::Fixed(40.0))
    .padding(0)
    .on_press(Message::Create)
    .style(move |_theme: &IcedTheme, status| {
        let hot = matches!(status, button::Status::Hovered | button::Status::Pressed);
        button::Style {
            background: Some(Background::Color(if hot { tokens.fill2 } else { tokens.fill })),
            border: Border { radius: 12.0.into(), ..Border::default() },
            ..button::Style::default()
        }
    })
    .into()
}

fn list<'a>(look: &Look<'a>, height: f32, empty: bool) -> Element<'a, Message> {
    let tokens = look.tokens;
    if empty {
        return container(text(look.i18n.t("localMusic.noPlaylistsFound")).size(13).color(tokens.t3))
            .width(Length::Fill)
            .height(Length::Fixed(height))
            .center_x(Length::Fill)
            .center_y(Length::Fixed(height))
            .into();
    }
    let mut rows = column![].spacing(0);
    for (index, choice) in look.playlists.iter().enumerate() {
        rows = rows.push(row_button(tokens, choice, index));
    }
    let content: Element<'a, Message> = scrollable(rows).height(Length::Fill).into();
    container(content)
        .width(Length::Fill)
        .height(Length::Fixed(height))
        .style(move |_theme: &IcedTheme| container::Style {
            background: Some(tokens.card.into()),
            border: Border { radius: 12.0.into(), width: 1.0, color: tokens.line },
            ..container::Style::default()
        })
        .clip(true)
        .into()
}

fn row_button<'a>(tokens: Tokens, choice: &'a PlaylistChoice, index: usize) -> Element<'a, Message> {
    let words = column![
        text(choice.name.clone())
            .size(14)
            .line_height(iced::widget::text::LineHeight::Absolute(iced::Pixels(20.0)))
            .color(tokens.t1)
            .wrapping(iced::widget::text::Wrapping::None),
        text(choice.detail.clone())
            .size(12)
            .line_height(iced::widget::text::LineHeight::Absolute(iced::Pixels(18.0)))
            .color(tokens.t2)
            .wrapping(iced::widget::text::Wrapping::None),
    ]
    .spacing(0);
    button(
        row![container(words).width(Length::Fill), icon(Icon::Plus, 16.0, tokens.t3)]
            .spacing(12)
            .align_y(Alignment::Center),
    )
    .width(Length::Fill)
    .height(Length::Fixed(ROW_H))
    .padding(Padding { top: 8.0, right: 16.0, bottom: 8.0, left: 16.0 })
    .on_press(Message::Pick(index))
    .style(move |_theme: &IcedTheme, status| {
        let hot = matches!(status, button::Status::Hovered | button::Status::Pressed);
        button::Style { background: hot.then_some(Background::Color(tokens.fill)), ..button::Style::default() }
    })
    .into()
}

fn sheet_button<'a>(tokens: Tokens, label: &'a str, primary: bool, on_press: Option<Message>) -> Element<'a, Message> {
    let enabled = on_press.is_some();
    let text_color = if primary { tokens.on_accent } else { tokens.t1 };
    let ink = if enabled { text_color } else { Color { a: text_color.a * 0.4, ..text_color } };
    button(container(text(label).size(13).color(ink)).center_y(Length::Fixed(32.0)))
        .height(Length::Fixed(32.0))
        .padding(Padding { top: 0.0, right: 12.0, bottom: 0.0, left: 12.0 })
        .on_press_maybe(on_press)
        .style(move |_theme: &IcedTheme, status| {
            let hot = enabled && matches!(status, button::Status::Hovered | button::Status::Pressed);
            let base = if primary { tokens.accent } else { tokens.fill };
            let fill = if enabled { base } else { Color { a: base.a * 0.4, ..base } };
            button::Style {
                background: Some(Background::Color(if hot { Color { a: fill.a * 0.9, ..fill } } else { fill })),
                border: Border { radius: 8.0.into(), ..Border::default() },
                ..button::Style::default()
            }
        })
        .into()
}

fn card_style(tokens: Tokens) -> impl Fn(&IcedTheme) -> container::Style {
    move |_theme| container::Style {
        background: Some(tokens.win.into()),
        border: Border { radius: 16.0.into(), width: 1.0, color: tokens.line2 },
        shadow: iced::Shadow {
            color: Color { a: if tokens.win.r > 0.5 { 0.26 } else { 0.6 }, ..Color::BLACK },
            offset: iced::Vector::new(0.0, 36.0),
            blur_radius: 90.0,
        },
        text_color: Some(tokens.t1),
        ..container::Style::default()
    }
}
