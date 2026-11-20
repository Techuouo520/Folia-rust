// Native scene work window, shared controls and the cinema's selectable seat map.

use crate::icons::icon;
use crate::settings_kit::{self as kit, Group, Row, Tokens};
use crate::typeface::text;
use folia_core::{RoomOutput, SceneId, SceneQuality, Theme};
use folia_i18n::I18n;
use folia_store::SceneSettings;
use folia_visualizer::cinema;
use iced::widget::{button, column, container, mouse_area, opaque, row, scrollable, space, stack, tooltip};
use iced::{Alignment, Border, Color, Element, Length, Padding, Shadow, Size, Vector};
use lucide_static_svg::Icon;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Tab {
    #[default]
    Scene,
    Sound,
}

#[derive(Debug, Clone)]
pub enum Message {
    Close,
    Tab(Tab),
    Select(Option<SceneId>),
    Quality(SceneQuality),
    Seat(Option<String>),
    HouseLights(bool),
    Curtains(bool),
    Sound(bool),
    Output(RoomOutput),
    Reset,
}

#[derive(Default)]
pub struct State {
    tab: Tab,
}

#[derive(Clone, Copy)]
pub struct Look<'a> {
    pub i18n: &'a I18n,
    pub theme: &'a Theme,
    pub daylight: bool,
    pub window: Size,
    pub sound_available: bool,
    pub sound_loading: bool,
    pub sound_error: Option<&'a str>,
}

impl State {
    pub fn open() -> Self {
        Self::default()
    }

    pub fn update(&mut self, message: &Message) {
        match message {
            Message::Tab(tab) => self.tab = *tab,
            Message::Select(_) => self.tab = Tab::Scene,
            _ => {}
        }
    }

    pub fn view<'a>(&self, settings: &'a SceneSettings, look: Look<'a>) -> Element<'a, Message> {
        let tokens = Tokens::new(look.theme, look.daylight);
        let width = (look.window.width - 24.0).clamp(280.0, 1240.0);
        let height = (look.window.height - 24.0).clamp(240.0, 780.0);
        let wide = look.window.width >= 1024.0;
        let column_width = if wide { 420.0 } else { width };
        let page = self.page(settings, look, tokens, column_width);
        let options: Element<'a, Message> = if wide {
            column![self.tabs(settings, look, tokens), scroll(page, look.daylight).height(Length::Fill)]
                .width(column_width)
                .height(Length::Fill)
                .into()
        } else {
            column![self.tabs(settings, look, tokens), page].width(Length::Fill).into()
        };
        let stage_height = if wide { height - 64.0 } else { (look.window.height * 0.64).max(380.0) };
        let stage = stage(settings, look, tokens, stage_height, if wide { width - column_width } else { width });
        let body: Element<'a, Message> = if wide {
            row![stage, container(options).width(column_width).height(Length::Fill)].height(Length::Fill).into()
        } else {
            scroll(column![stage, options].into(), look.daylight).height(Length::Fill).into()
        };
        let back = tooltip(
            kit::icon_button(tokens, Icon::ChevronLeft, 18.0, Message::Close),
            text(look.i18n.t("ui.back")).size(12),
            tooltip::Position::Bottom,
        )
        .style(kit::faded_box(1.0));
        let header = container(
            row![
                back,
                text(look.i18n.t("ui.scene")).size(20).font(crate::typeface::adapt(kit::SEMIBOLD)).color(tokens.t1)
            ]
            .spacing(8)
            .align_y(Alignment::Center),
        )
        .height(64)
        .padding([0, 16])
        .align_y(Alignment::Center);
        let card = container(column![header, body]).width(width).height(height).style(move |_| container::Style {
            background: Some(tokens.win.into()),
            border: Border { radius: 16.0.into(), width: 1.0, color: tokens.line2 },
            shadow: Shadow {
                color: Color { a: 0.5, ..Color::BLACK },
                offset: Vector::new(0.0, 36.0),
                blur_radius: 90.0,
            },
            ..Default::default()
        });
        let card = container(opaque(card)).center(Length::Fill);
        let scrim = Color { a: if look.daylight { 0.36 } else { 0.52 }, ..Color::BLACK };
        let behind = opaque(
            mouse_area(
                container(space())
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .style(move |_| container::Style { background: Some(scrim.into()), ..Default::default() }),
            )
            .on_press(Message::Close),
        );
        stack![behind, card].into()
    }

    fn tabs<'a>(&self, settings: &SceneSettings, look: Look<'a>, tokens: Tokens) -> Element<'a, Message> {
        let selected = usize::from(self.tab == Tab::Sound);
        let tabs = kit::tabs(
            tokens,
            vec![
                (look.i18n.t("ui.sceneSettings").to_owned(), Message::Tab(Tab::Scene)),
                (look.i18n.t("ui.soundTracing").to_owned(), Message::Tab(Tab::Sound)),
            ],
            Some(selected),
            Some(selected as f32),
            8.0,
            16.0,
        );
        let reset = self.tab == Tab::Scene
            && settings.scene_id.is_some()
            && settings.scene_options.cinema != Default::default();
        let reset = button(text(look.i18n.t("ui.default")).size(13).color(if reset { tokens.t2 } else { tokens.t3 }))
            .on_press_maybe(reset.then_some(Message::Reset))
            .padding([8, 12])
            .style(button::text);
        row![container(tabs).width(Length::Fill), reset].align_y(Alignment::Center).into()
    }

    fn page<'a>(
        &self,
        settings: &'a SceneSettings,
        look: Look<'a>,
        tokens: Tokens,
        width: f32,
    ) -> Element<'a, Message> {
        let content = match self.tab {
            Tab::Scene if settings.scene_id.is_some() => scene_controls(settings, look, tokens, width - 68.0),
            Tab::Scene => container(text(look.i18n.t("ui.sceneNoSettings")).size(13).color(tokens.t3))
                .padding([56, 0])
                .width(Length::Fill)
                .align_x(Alignment::Center)
                .into(),
            Tab::Sound => sound_controls(settings, look, tokens),
        };
        container(content)
            .width(Length::Fill)
            .padding(Padding { left: 24.0, right: 12.0, bottom: 40.0, ..Padding::ZERO })
            .into()
    }
}

fn scroll<'a>(content: Element<'a, Message>, daylight: bool) -> iced::widget::Scrollable<'a, Message> {
    scrollable(content).direction(crate::scrollbar::direction()).style(crate::scrollbar::style(daylight))
}

fn stage<'a>(
    settings: &SceneSettings,
    look: Look<'a>,
    tokens: Tokens,
    height: f32,
    width: f32,
) -> Element<'a, Message> {
    let chosen = settings.scene_id.is_some();
    let picture = column![
        icon(if chosen { Icon::Clapperboard } else { Icon::CircleOff }, 48.0, tokens.t2),
        text(look.i18n.t(if chosen { "ui.sceneCinema" } else { "ui.sceneNone" })).size(14).color(tokens.t2)
    ]
    .spacing(12)
    .align_x(Alignment::Center);
    let preview = container(picture).center(Length::Fill).style(move |_| container::Style {
        background: Some(tokens.pv.into()),
        border: Border { radius: 12.0.into(), width: 1.0, color: tokens.line },
        ..Default::default()
    });
    let columns = if look.window.width >= 1024.0 { 7.0 } else { 4.0 };
    let tile_width = ((width - 48.0 - (columns - 1.0) * 12.0) / columns).max(48.0);
    let mut choices = row![].spacing(12);
    for value in [None, Some(SceneId::Cinema)] {
        let active = settings.scene_id == value;
        let glyph = if value.is_some() { Icon::Clapperboard } else { Icon::CircleOff };
        let tile = container(icon(glyph, 28.0, tokens.accent)).center(Length::Fill).style(move |_| container::Style {
            background: Some(tokens.pv.into()),
            border: Border {
                radius: 10.0.into(),
                width: if active { 2.0 } else { 1.0 },
                color: if active { tokens.accent } else { tokens.line2 },
            },
            ..Default::default()
        });
        let choice = column![
            container(tile).width(tile_width).height(tile_width * 0.72),
            text(look.i18n.t(if value.is_some() { "ui.sceneCinema" } else { "ui.sceneNone" }))
                .size(12)
                .color(tokens.t2)
        ]
        .spacing(6)
        .align_x(Alignment::Center);
        choices = choices.push(button(choice).padding(0).on_press(Message::Select(value)).style(button::text));
    }
    let shelf = column![
        row![
            text(look.i18n.t("ui.scene")).size(13).font(crate::typeface::adapt(kit::SEMIBOLD)).color(tokens.t2),
            text("2").size(13).color(tokens.t3)
        ]
        .spacing(6),
        choices
    ]
    .spacing(8);
    container(column![preview, shelf].spacing(16))
        .height(height)
        .width(Length::Fill)
        .padding(Padding { left: 24.0, right: 24.0, bottom: 24.0, ..Padding::ZERO })
        .into()
}

fn setting<'a>(i18n: &I18n, label: &str, control: Element<'a, Message>) -> Row<'a, Message> {
    Row { note: None, label: i18n.t(label).to_owned(), hint: None, sub: false, below: None, control }
}

fn scene_controls<'a>(
    settings: &SceneSettings,
    look: Look<'a>,
    tokens: Tokens,
    map_width: f32,
) -> Element<'a, Message> {
    let options = &settings.scene_options.cinema;
    let choices = SceneQuality::ALL
        .into_iter()
        .zip(["ui.sceneQuality_low", "ui.sceneQuality_medium", "ui.sceneQuality_high"])
        .map(|(quality, key)| (quality, look.i18n.t(key).to_owned()))
        .collect();
    let selected = SceneQuality::ALL.iter().position(|quality| *quality == options.quality).map(|index| index as f32);
    let mut rows = vec![
        setting(
            look.i18n,
            "ui.sceneQuality",
            kit::segmented(tokens, choices, options.quality, selected, Message::Quality),
        ),
        setting(
            look.i18n,
            "ui.sceneHouseLights",
            kit::toggle(tokens, f32::from(options.house_lights), Message::HouseLights(!options.house_lights)),
        ),
        setting(
            look.i18n,
            "ui.sceneCurtains",
            kit::toggle(tokens, f32::from(options.curtains_on_pause), Message::Curtains(!options.curtains_on_pause)),
        ),
    ];
    let mut seats = setting(look.i18n, "ui.sceneSeat", space().into());
    seats.below = Some(seat_map(look.i18n, tokens, options.viewpoint.as_deref(), map_width.max(180.0)));
    rows.push(seats);
    kit::group(tokens, Group { title: look.i18n.t("ui.sceneSettings").to_owned(), bare: true, rows, foot: None }, true)
}

fn sound_controls<'a>(settings: &SceneSettings, look: Look<'a>, tokens: Tokens) -> Element<'a, Message> {
    let supported = settings.scene_id.is_some();
    let usable = supported && look.sound_available;
    let on = usable && settings.scene_room_sound;
    let toggle =
        if usable { kit::toggle(tokens, f32::from(on), Message::Sound(!on)) } else { kit::toggle_fixed(tokens, 0.0) };
    let mut tracing = setting(look.i18n, "ui.soundTracing", toggle);
    tracing.hint = if !supported {
        Some(look.i18n.t("ui.soundTracingUnsupported").to_owned())
    } else if !look.sound_available {
        Some(look.i18n.t("ui.soundTracingUnavailable").to_owned())
    } else if look.sound_loading {
        Some(look.i18n.t("ui.soundTracingLoading").to_owned())
    } else if look.sound_error.is_some() {
        Some(look.i18n.t("ui.soundTracingError").to_owned())
    } else {
        None
    };
    if let Some(error) = look.sound_error {
        tracing.control = tooltip(tracing.control, text(error).size(12).width(320), tooltip::Position::Top)
            .style(kit::faded_box(1.0))
            .into();
    }
    let mut rows = vec![tracing];
    if on {
        let choices = [
            (RoomOutput::Headphones, "ui.soundTracingOutput_headphones"),
            (RoomOutput::Speakers, "ui.soundTracingOutput_speakers"),
        ]
        .into_iter()
        .map(|(output, key)| (output, look.i18n.t(key).to_owned()))
        .collect();
        let mut output = setting(
            look.i18n,
            "ui.soundTracingOutput",
            kit::segmented(
                tokens,
                choices,
                settings.scene_room_output,
                Some(f32::from(settings.scene_room_output == RoomOutput::Speakers)),
                Message::Output,
            ),
        );
        output.sub = true;
        rows.push(output);
    }
    kit::group(tokens, Group { title: look.i18n.t("ui.soundTracing").to_owned(), bare: true, rows, foot: None }, true)
}

fn seat_map<'a>(i18n: &'a I18n, tokens: Tokens, viewpoint: Option<&str>, width: f32) -> Element<'a, Message> {
    let seats = cinema::seats();
    let chosen = cinema::selected_seat(viewpoint);
    let (mut min_x, mut max_x, mut min_z, mut max_z) =
        (f32::INFINITY, f32::NEG_INFINITY, f32::INFINITY, f32::NEG_INFINITY);
    for seat in seats {
        min_x = min_x.min(seat.eye[0]);
        max_x = max_x.max(seat.eye[0]);
        min_z = min_z.min(seat.eye[2]);
        max_z = max_z.max(seat.eye[2]);
    }
    let scale = (width - 16.0) / (max_x - min_x);
    let gap = seats
        .windows(2)
        .map(|pair| pair[1].eye[0] - pair[0].eye[0])
        .filter(|gap| *gap > 0.001)
        .fold(max_x - min_x, f32::min);
    let size = (gap * scale * 0.8).min((width - 16.0) * 0.08);
    let height = (max_z - min_z) * scale + 16.0;
    let mut map = stack![space().width(width).height(height)];
    for seat in seats {
        let selected = seat.id == chosen.id;
        let occupied = seat.occupied;
        let fill = if selected {
            tokens.accent
        } else if occupied {
            Color { a: tokens.fill.a * 0.5, ..tokens.fill }
        } else {
            tokens.fill2
        };
        let seat_button = button(space().width(size).height(size))
            .padding(0)
            .on_press_maybe((!occupied).then(|| Message::Seat(Some(seat.id.clone()))))
            .style(move |_, status| button::Style {
                background: Some(
                    if !occupied && status == button::Status::Hovered && !selected { tokens.t3 } else { fill }.into(),
                ),
                border: Border { radius: 3.0.into(), ..Default::default() },
                ..Default::default()
            });
        let mut label =
            i18n.format("ui.sceneSeatName", &[("row", &seat.row.to_string()), ("number", &seat.number.to_string())]);
        if occupied {
            label.push_str(&format!(" · {}", i18n.t("ui.sceneSeatTaken")));
        }
        if seat.preferred {
            label.push_str(&format!(" · {}", i18n.t("ui.sceneSeatBest")));
        }
        let padding = if seat.preferred { 2.0 } else { 0.0 };
        let preferred = seat.preferred;
        let seat_button = container(seat_button).padding(padding).style(move |_| container::Style {
            border: Border { width: if preferred { 1.0 } else { 0.0 }, color: tokens.accent, radius: 4.0.into() },
            ..Default::default()
        });
        let seat_button = tooltip(seat_button, text(label).size(11), tooltip::Position::Top).style(kit::faded_box(1.0));
        map = map.push(container(seat_button).padding(Padding {
            left: 8.0 + (seat.eye[0] - min_x) * scale - size / 2.0 - padding,
            top: 8.0 + (seat.eye[2] - min_z) * scale - size / 2.0 - padding,
            ..Padding::ZERO
        }));
    }
    let screen = container(space()).width(width * 0.75).height(4).style(move |_| container::Style {
        background: Some(tokens.t3.into()),
        border: Border { radius: 2.0.into(), ..Default::default() },
        ..Default::default()
    });
    let mut legend = row![].spacing(16).align_y(Alignment::Center);
    for (key, fill, outline) in [
        ("ui.sceneSeatYours", tokens.accent, false),
        ("ui.sceneSeatBest", Color::TRANSPARENT, true),
        ("ui.sceneSeatTaken", Color { a: tokens.fill.a * 0.5, ..tokens.fill }, false),
    ] {
        let mark = container(space()).width(8).height(8).style(move |_| container::Style {
            background: Some(fill.into()),
            border: Border { radius: 2.0.into(), width: f32::from(outline), color: tokens.accent },
            ..Default::default()
        });
        legend =
            legend.push(row![mark, text(i18n.t(key)).size(11).color(tokens.t3)].spacing(4).align_y(Alignment::Center));
    }
    column![screen, text(i18n.t("ui.sceneSeatScreen")).size(11).color(tokens.t3), map, legend.wrap()]
        .spacing(8)
        .align_x(Alignment::Center)
        .into()
}
