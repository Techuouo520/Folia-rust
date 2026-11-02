// The bottom-right wall tools button and its panel, from `WallToolsButton`.

use std::time::Instant;

use folia_render::Plate;
use iced::widget::slider::{Handle, HandleShape, Rail};
use iced::widget::{
    button, column, container, mouse_area, pin, row, scrollable, shader, space, stack, text, tooltip, tooltip::Position,
};
use iced::{Alignment, Background, Border, Color, Element, Length, Padding, Rectangle, Size, Theme};
use lucide_static_svg::Icon;

use super::{HELP_PAD, HELP_ROW_H, HELP_ROWS, HomeChrome, Host, Message};
use crate::icons::icon;
use crate::queue_island;

const BUTTON: f32 = 48.0;
const GAP: f32 = 16.0;
const STACK_GAP: f32 = 8.0;
const PANEL_RADIUS: f32 = 24.0;
const ISLAND_RADIUS: f32 = 20.0;
const ISLAND_HEADER: f32 = 36.0;
const ISLAND_BORDER: f32 = 2.0;
const PANEL_PAD: f32 = 10.0;
const QUICK_H: f32 = 40.0;
const QUICK_GAP: f32 = 6.0;
const VOLUME_H: f32 = 38.0;
const HEADING_GAP: f32 = 6.0;
const HEADING_H: f32 = 21.0;
const ROW_GAP: f32 = 2.0;
const ACTION_H: f32 = 38.0;
const HELP_MARGIN: f32 = 6.0;
const HELP_SECTION_H: f32 = 39.0;
const HELP_LIST_MARGIN: f32 = 6.0;
/// `WallToolsQueueIsland`'s smallest height and the top reserve over the stack.
const ISLAND_MIN: f32 = 112.0;
const TOP_RESERVE: f32 = 48.0;
const BLUR: f32 = 64.0;
const SHADOW_Y: f32 = 24.0;
/// A CSS blur radius is two sigmas; the Gaussian is spent three sigmas out.
const REACH: f32 = BLUR * 1.5;
const TOOLTIP_DELAY: std::time::Duration = std::time::Duration::from_millis(500);

/// The panel without its optional help list: padding, quick row, volume, heading, look, tint and the help bar.
fn panel_base() -> f32 {
    2.0 * PANEL_PAD
        + QUICK_H
        + QUICK_GAP
        + VOLUME_H
        + HEADING_GAP
        + HEADING_H
        + ROW_GAP
        + ACTION_H
        + ROW_GAP
        + ACTION_H
        + HELP_MARGIN
        + HELP_SECTION_H
}

struct Geometry {
    panel_w: f32,
    panel_h: f32,
    panel_top: f32,
    island: bool,
    island_h: f32,
    island_top: f32,
    list_h: f32,
    help_h: f32,
}

fn geometry(home: &HomeChrome, host: &Host<'_>, now: Instant) -> Geometry {
    let edge = edge(host);
    let panel_w = (host.window_width - 32.0).clamp(1.0, 296.0);
    let island = home.island_shown(now);
    let room = (host.window_height - edge - BUTTON - GAP - TOP_RESERVE).max(0.0);
    let help_h = help_visible(host, island, room);
    let help = if home.help_shown(now) { HELP_LIST_MARGIN + help_h * home.help(now) } else { 0.0 };
    let panel_h = panel_base() + help;
    let ideal = queue_island::list_height(host.queue.len()) + ISLAND_HEADER + ISLAND_BORDER;
    let island_h = if island { ideal.min((room - STACK_GAP - panel_h).max(ISLAND_MIN)) } else { 0.0 };
    let list_h = if island { (island_h - ISLAND_HEADER - ISLAND_BORDER).max(1.0) } else { 0.0 };
    let panel_top = host.window_height - edge - BUTTON - GAP - panel_h;
    let island_top = panel_top - STACK_GAP - island_h;
    Geometry { panel_w, panel_h, panel_top, island, island_h, island_top, list_h, help_h }
}

fn edge(host: &Host<'_>) -> f32 {
    if host.window_width < 640.0 { 16.0 } else { 24.0 }
}

fn panel_width(host: &Host<'_>) -> f32 {
    (host.window_width - 32.0).clamp(1.0, 296.0)
}

/// `max-height: max(150px, calc(100vh - 600px))`, further capped so the panel and the smallest island fit.
fn help_visible(host: &Host<'_>, island: bool, room: f32) -> f32 {
    let full = HELP_PAD + HELP_ROWS as f32 * HELP_ROW_H;
    let base = (host.window_height - 600.0).max(150.0);
    let cap = if island { (room - STACK_GAP - ISLAND_MIN - (panel_base() + HELP_LIST_MARGIN)).max(48.0) } else { full };
    full.min(base).min(cap)
}

pub fn dock<'a>(home: &'a HomeChrome, host: &Host<'a>, now: Instant) -> Element<'a, Message> {
    let open = home.tools(now);
    let edge = edge(host);
    let button = tool_button(host, open > 0.5);
    if open <= 0.02 {
        return pin_corner(button, edge);
    }
    let geo = geometry(home, host, now);
    let mut body = column![].width(Length::Fixed(geo.panel_w)).align_x(Alignment::End);
    if geo.island {
        body = body.push(island_face(home, host, &geo, now));
        body = body.push(space().height(Length::Fixed(STACK_GAP)));
    }
    body = body.push(panel_face(home, host, open, &geo, now));
    body = body.push(space().height(Length::Fixed(GAP)));
    body = body.push(button);
    // Presses on the panel padding stay here. The dismiss layer is underneath.
    let column = mouse_area(body).on_press(Message::Frame).interaction(iced::mouse::Interaction::Idle);
    stack![plates(home, host, open, &geo, now), pin_corner(column, edge)]
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

/// `WallBackButton` concealed: 40px round at `top: 20px; left: 24px` of the wall (`left: 16px` below 640px).
/// Hidden it is `opacity: 0; translateX(-6px) scale(0.92)` and takes no pointer; hover grows it to 105%.
pub fn back<'a>(host: &Host<'a>, shown: f32, hover: f32) -> Element<'a, Message> {
    if shown <= 0.0 {
        return space().into();
    }
    let fill = if host.daylight { Color { a: 0.05, ..Color::BLACK } } else { Color { a: 0.08, ..Color::WHITE } };
    let fill = Color { a: fill.a * shown, ..fill };
    let face = container(icon(Icon::ChevronLeft, 20.0, Color { a: host.text.a * shown, ..host.text }))
        .center(Length::Fixed(40.0))
        .style(move |_theme: &Theme| container::Style {
            background: Some(Background::Color(fill)),
            border: Border { radius: 20.0.into(), ..Border::default() },
            shadow: iced::Shadow {
                color: Color { a: 0.24 * shown, ..Color::BLACK },
                offset: iced::Vector::new(0.0, 10.0),
                blur_radius: 30.0,
            },
            ..container::Style::default()
        });
    let revealed = host.has_track && shown > 0.5;
    let button = button(face)
        .padding(0)
        .on_press_maybe(revealed.then_some(Message::Player))
        .style(|_theme: &Theme, _status| button::Style::default());
    let button = mouse_area(button).on_enter(Message::BackHover(true)).on_exit(Message::BackHover(false));
    let scale = (0.92 + 0.08 * shown) * (1.0 + 0.05 * hover);
    let shift = iced::Vector::new(-6.0 * (1.0 - shown), 0.0);
    let button: Element<'a, Message> = iced::widget::float(button).translate(move |_, _| shift).scale(scale).into();
    let (top, left) = if host.window_width < 640.0 { (14.0, 16.0) } else { (20.0, 24.0) };
    container(button)
        .width(Length::Fill)
        .height(Length::Fill)
        .align_x(Alignment::Start)
        .align_y(Alignment::Start)
        .padding(Padding { top, right: 0.0, bottom: 0.0, left })
        .into()
}

fn pin_corner<'a>(body: impl Into<Element<'a, Message>>, edge: f32) -> Element<'a, Message> {
    container(body)
        .width(Length::Fill)
        .height(Length::Fill)
        .align_x(Alignment::End)
        .align_y(Alignment::End)
        .padding(Padding { top: 0.0, right: edge, bottom: edge, left: 0.0 })
        .into()
}

/// The two glass plates (island above panel), each with its shadow room.
fn plates<'a>(home: &HomeChrome, host: &Host<'a>, open: f32, geo: &Geometry, now: Instant) -> Element<'a, Message> {
    let x = host.window_width - edge(host) - geo.panel_w;
    let mut layers: Vec<Element<'a, Message>> = Vec::new();
    if geo.island {
        let alpha = home.island(now);
        if alpha > 0.005 {
            layers.push(plate(
                host,
                Rectangle { x, y: geo.island_top, width: geo.panel_w, height: geo.island_h },
                ISLAND_RADIUS,
                plate_fill(host, alpha),
                plate_shadow(host, alpha),
            ));
        }
    }
    layers.push(plate(
        host,
        Rectangle { x, y: geo.panel_top, width: geo.panel_w, height: geo.panel_h },
        PANEL_RADIUS,
        plate_fill(host, open),
        plate_shadow(host, open),
    ));
    stack(layers).into()
}

fn plate_fill(host: &Host<'_>, alpha: f32) -> [f32; 4] {
    if host.daylight { [1.0, 1.0, 1.0, 0.62 * alpha] } else { [0.0, 0.0, 0.0, 0.40 * alpha] }
}

fn plate_shadow(host: &Host<'_>, alpha: f32) -> [f32; 4] {
    if host.daylight { [42.0 / 255.0, 35.0 / 255.0, 27.0 / 255.0, 0.18 * alpha] } else { [0.0, 0.0, 0.0, 0.42 * alpha] }
}

fn plate<'a>(host: &Host<'a>, rect: Rectangle, radius: f32, fill: [f32; 4], shadow: [f32; 4]) -> Element<'a, Message> {
    let right = rect.x + rect.width;
    let bottom = rect.y + rect.height;
    let x0 = (rect.x - REACH).max(0.0);
    let y0 = (rect.y - (REACH - SHADOW_Y).max(0.0)).max(0.0);
    let x1 = (right + REACH).min(host.window_width.max(1.0));
    let y1 = (bottom + REACH + SHADOW_Y).min(host.window_height.max(1.0));
    let plate = Plate {
        radius,
        fill_origin: [rect.x - x0, rect.y - y0],
        fill_size: [rect.width, rect.height],
        fill,
        shadow,
        shadow_blur: BLUR,
        shadow_y: SHADOW_Y,
        shadow_spread: 0.0,
        ring: [0.0; 4],
    };
    let patch = shader(plate).width(Length::Fixed((x1 - x0).max(1.0))).height(Length::Fixed((y1 - y0).max(1.0)));
    container(patch)
        .width(Length::Fill)
        .height(Length::Fill)
        .padding(Padding { top: y0, right: 0.0, bottom: 0.0, left: x0 })
        .into()
}

fn island_face<'a>(home: &'a HomeChrome, host: &Host<'a>, geo: &Geometry, now: Instant) -> Element<'a, Message> {
    let alpha = home.island(now);
    let ink = Color { a: host.text.a * alpha, ..host.text };
    let border = if host.daylight {
        Color { a: 0.07 * alpha, ..Color::BLACK }
    } else {
        Color { r: 1.0, g: 1.0, b: 1.0, a: 0.08 * alpha }
    };
    let count = host.queue.len();
    let list: Element<'a, Message> = if count == 0 {
        container(text(host.i18n.t("queue.empty").to_owned()).size(12).color(Color { a: ink.a * 0.45, ..ink }))
            .width(Length::Fill)
            .height(Length::Fixed(geo.list_h))
            .align_x(Alignment::Center)
            .align_y(Alignment::Center)
            .into()
    } else {
        let look =
            queue_island::Look { i18n: host.i18n, tokens: host.tokens.faded(alpha), ink, daylight: host.daylight };
        home.queue.view(look, host.queue, host.queue_index, geo.list_h, now).map(Message::ToolsQueue)
    };
    let body = column![island_header(home, host, alpha, count), list].width(Length::Fill).height(Length::Fill);
    container(body)
        .width(Length::Fixed(geo.panel_w))
        .height(Length::Fixed(geo.island_h))
        .clip(true)
        .style(move |_theme: &Theme| container::Style {
            border: Border { radius: ISLAND_RADIUS.into(), width: 1.0, color: border },
            ..container::Style::default()
        })
        .into()
}

fn island_header<'a>(home: &HomeChrome, host: &Host<'a>, alpha: f32, count: usize) -> Element<'a, Message> {
    let ink = Color { a: host.text.a * alpha, ..host.text };
    let line = if host.daylight {
        Color { a: 0.07 * alpha, ..Color::BLACK }
    } else {
        Color { r: 1.0, g: 1.0, b: 1.0, a: 0.08 * alpha }
    };
    let count_text = count.to_string();
    let title = host.i18n.format("wallTools.queueCount", &[("count", count_text.as_str())]);
    let title = text(title)
        .size(12)
        .font(crate::typeface::font(iced::font::Weight::Bold))
        .color(Color { a: ink.a * 0.72, ..ink })
        .width(Length::Fill)
        .wrapping(iced::widget::text::Wrapping::None);
    let mut bar = row![title].align_y(Alignment::Center).width(Length::Fill);
    // The playing item stays after a clear, so with it alone there is nothing left to clear.
    if count > 1 {
        bar = bar.push(clear_button(home, host, ink, alpha));
    }
    column![
        container(bar).width(Length::Fill).height(Length::Fixed(ISLAND_HEADER - 1.0)).padding(Padding {
            top: 0.0,
            right: 6.0,
            bottom: 0.0,
            left: 14.0
        }),
        rule(line)
    ]
    .into()
}

fn clear_button<'a>(home: &HomeChrome, host: &Host<'a>, ink: Color, alpha: f32) -> Element<'a, Message> {
    let armed = home.queue_clear_armed();
    let label = if armed { host.i18n.t("wallTools.queueClearConfirm") } else { host.i18n.t("wallTools.queueClear") };
    if armed {
        let on_accent = Color { a: alpha, ..host.tokens.on_accent };
        let accent = Color { a: alpha, ..host.tokens.accent };
        let glyph = icon(Icon::Trash, 15.0, on_accent);
        button(
            row![glyph, text(label).size(11).font(semibold()).color(on_accent)]
                .spacing(6)
                .align_y(Alignment::Center)
                .padding(Padding { top: 0.0, right: 7.0, bottom: 0.0, left: 7.0 }),
        )
        .height(Length::Fixed(28.0))
        .padding(0)
        .on_press(Message::ToolsQueueClear)
        .style(move |_theme: &Theme, _status| button::Style {
            background: Some(Background::Color(accent)),
            border: Border { radius: 9.0.into(), ..Border::default() },
            ..button::Style::default()
        })
        .into()
    } else {
        let daylight = host.daylight;
        let glyph = icon(Icon::Trash, 15.0, Color { a: ink.a * 0.72, ..ink });
        button(container(glyph).center(Length::Fixed(28.0)))
            .width(Length::Fixed(28.0))
            .height(Length::Fixed(28.0))
            .padding(0)
            .on_press(Message::ToolsQueueClear)
            .style(move |_theme: &Theme, status| {
                let hot = matches!(status, button::Status::Hovered | button::Status::Pressed);
                let fill = if daylight {
                    Color { a: 0.07 * alpha, ..Color::BLACK }
                } else {
                    Color { r: 1.0, g: 1.0, b: 1.0, a: 0.10 * alpha }
                };
                button::Style {
                    background: hot.then_some(Background::Color(fill)),
                    border: Border { radius: 9.0.into(), ..Border::default() },
                    ..button::Style::default()
                }
            })
            .into()
    }
}

fn panel_face<'a>(
    home: &'a HomeChrome,
    host: &Host<'a>,
    open: f32,
    geo: &Geometry,
    now: Instant,
) -> Element<'a, Message> {
    let border = if host.daylight {
        Color { a: 0.07 * open, ..Color::BLACK }
    } else {
        Color { r: 1.0, g: 1.0, b: 1.0, a: 0.08 * open }
    };
    let ink = Color { a: host.text.a * open, ..host.text };
    let mut inner = column![
        quick_row(home, host, ink, geo.panel_w),
        space().height(Length::Fixed(QUICK_GAP)),
        volume_row(host, ink),
        space().height(Length::Fixed(HEADING_GAP)),
        heading(host, ink, "libraryBravais.toolsAppearance"),
        space().height(Length::Fixed(ROW_GAP)),
        look_row(host, ink),
        space().height(Length::Fixed(ROW_GAP)),
        tint_row(host, ink),
        space().height(Length::Fixed(HELP_MARGIN)),
        help_section(host, ink),
    ];
    if home.help_shown(now) {
        inner = inner.push(space().height(Length::Fixed(HELP_LIST_MARGIN))).push(help_list(
            host,
            open,
            geo.help_h,
            home.help(now),
        ));
    }
    container(inner)
        .width(Length::Fixed(geo.panel_w))
        .padding(PANEL_PAD)
        .style(move |_theme: &Theme| container::Style {
            border: Border { radius: PANEL_RADIUS.into(), width: 1.0, color: border },
            ..container::Style::default()
        })
        .into()
}

fn quick_row<'a>(home: &HomeChrome, host: &Host<'a>, ink: Color, panel_w: f32) -> Element<'a, Message> {
    let cell = ((panel_w - 20.0) - 4.0 * 5.0) / 6.0;
    let fm = host.fm;
    let queue_open = home.queue_island_open();
    let shuffle_tip = tooltip_text(
        host.i18n.t("wallTools.shuffle"),
        Some(host.i18n.t("wallTools.shuffleUnavailable")),
        Some(": + R"),
    );
    let theme_tip = tooltip_text(host.i18n.t("wallTools.theme"), Some(host.i18n.t("wallTools.themeUnavailable")), None);
    let queue_status = if fm {
        host.i18n.t("wallTools.queueUnavailableInFm")
    } else if queue_open {
        host.i18n.t("wallTools.queueHide")
    } else {
        host.i18n.t("wallTools.queueShow")
    };
    let queue_tip = tooltip_text(host.i18n.t("wallTools.queue"), Some(queue_status), None);
    let lattice_status =
        if fm { host.i18n.t("status.latticeUnavailableInFm") } else { host.i18n.t("status.latticeUnavailable") };
    // No Lattice and no Ctrl/Cmd + B in this build: the label and the reason only.
    let lattice_tip = tooltip_text(host.i18n.t("libraryBravais.toolsLattice"), Some(lattice_status), None);
    let player_tip = tooltip_text(
        host.i18n.t("wallTools.openPlayer"),
        (!host.has_track).then_some(host.i18n.t("wallTools.openPlayerUnavailable")),
        None,
    );
    let settings_tip = host.i18n.t("wallTools.openSettings").to_owned();
    row![
        quick(Icon::Shuffle, cell, ink, host.daylight, None, shuffle_tip),
        quick(Icon::Sparkles, cell, ink, host.daylight, None, theme_tip),
        quick(Icon::ListMusic, cell, ink, host.daylight, (!fm).then_some(Message::ToolsQueueToggle), queue_tip,),
        quick(Icon::PanelsTopLeft, cell, ink, host.daylight, None, lattice_tip),
        quick(Icon::Disc3, cell, ink, host.daylight, host.has_track.then_some(Message::Player), player_tip),
        quick(Icon::Settings, cell, ink, host.daylight, Some(Message::Settings), settings_tip),
    ]
    .spacing(4)
    .height(Length::Fixed(QUICK_H))
    .into()
}

fn tooltip_text(label: &str, status: Option<&str>, kbd: Option<&str>) -> String {
    let mut parts = vec![label.to_owned()];
    if let Some(status) = status {
        parts.push(status.to_owned());
    }
    if let Some(kbd) = kbd {
        parts.push(kbd.to_owned());
    }
    parts.join(" · ")
}

fn heading<'a>(host: &Host<'a>, ink: Color, key: &'static str) -> Element<'a, Message> {
    let line = if host.daylight {
        Color { a: 0.09 * ink.a, ..Color::BLACK }
    } else {
        Color { r: 1.0, g: 1.0, b: 1.0, a: 0.10 * ink.a }
    };
    column![
        rule(line),
        container(
            text(host.i18n.t(key).to_owned())
                .size(10)
                .font(crate::typeface::font(iced::font::Weight::Bold))
                .color(Color { a: ink.a * 0.52, ..ink }),
        )
        .width(Length::Fill)
        .height(Length::Fixed(HEADING_H - 1.0))
        .padding(Padding { top: 7.0, right: 12.0, bottom: 0.0, left: 12.0 })
    ]
    .into()
}

fn rule<'a>(color: Color) -> Element<'a, Message> {
    container(space())
        .width(Length::Fill)
        .height(Length::Fixed(1.0))
        .style(move |_theme: &Theme| container::Style {
            background: Some(Background::Color(color)),
            ..container::Style::default()
        })
        .into()
}

/// `透光`: value cycled by the wall look store, which this build does not have yet; shown as its current solid state.
fn look_row<'a>(host: &Host<'a>, ink: Color) -> Element<'a, Message> {
    let value = host.i18n.t("options.libraryWallLookSolid");
    action_row(host, ink, Icon::Blinds, host.i18n.t("options.libraryWallLook"), Some(value), None, false)
}

/// `海报聚焦叠色`: the toggle needs `latticePosterTintEnabled`, absent here, so it stays off and disabled.
fn tint_row<'a>(host: &Host<'a>, ink: Color) -> Element<'a, Message> {
    action_row(host, ink, Icon::Layers, host.i18n.t("options.latticePosterTint"), None, Some(false), false)
}

fn action_row<'a>(
    host: &Host<'a>,
    ink: Color,
    glyph: Icon,
    label: &str,
    value: Option<&str>,
    toggle: Option<bool>,
    enabled: bool,
) -> Element<'a, Message> {
    let alpha = if enabled { 1.0 } else { 0.35 };
    let ink = Color { a: ink.a * alpha, ..ink };
    let daylight = host.daylight;
    let icon = icon(glyph, 17.0, ink);
    let label = text(label.to_owned()).size(13).font(crate::typeface::font(iced::font::Weight::Normal)).color(ink);
    let mut content = row![icon, label].spacing(11).align_y(Alignment::Center).width(Length::Fill);
    if let Some(value) = value {
        content = content.push(text(value.to_owned()).size(12).font(semibold()).color(Color { a: ink.a * 0.6, ..ink }));
    }
    if let Some(checked) = toggle {
        content = content.push(toggle_face(host, ink, checked, alpha));
    }
    button(content)
        .width(Length::Fill)
        .height(Length::Fixed(ACTION_H))
        .padding(Padding { top: 0.0, right: 12.0, bottom: 0.0, left: 12.0 })
        .on_press_maybe(enabled.then_some(Message::Frame))
        .style(move |_theme: &Theme, status| {
            let hot = enabled && matches!(status, button::Status::Hovered | button::Status::Pressed);
            let fill = if daylight {
                Color { a: 0.07 * ink.a, ..Color::BLACK }
            } else {
                Color { r: 1.0, g: 1.0, b: 1.0, a: 0.10 * ink.a }
            };
            button::Style {
                background: hot.then_some(Background::Color(fill)),
                border: Border { radius: 12.0.into(), ..Border::default() },
                ..button::Style::default()
            }
        })
        .into()
}

fn toggle_face<'a>(host: &Host<'a>, ink: Color, checked: bool, alpha: f32) -> Element<'a, Message> {
    let (fill, opacity) = if checked { (Some(host.tokens.accent), 1.0) } else { (None, 0.42) };
    let fill = fill.map(|color| Color { a: color.a * alpha, ..color });
    let knob = if checked { Color { a: alpha, ..host.tokens.on_accent } } else { ink };
    container(container(space()).width(Length::Fixed(12.0)).height(Length::Fixed(12.0)).style(move |_| {
        container::Style {
            background: Some(Background::Color(knob)),
            border: Border { radius: 6.0.into(), ..Border::default() },
            ..container::Style::default()
        }
    }))
    .width(Length::Fixed(30.0))
    .height(Length::Fixed(18.0))
    .padding(2)
    .align_x(if checked { Alignment::End } else { Alignment::Start })
    .style(move |_theme: &Theme| container::Style {
        background: fill.map(Background::Color),
        border: Border { radius: 9.0.into(), width: 1.0, color: Color { a: ink.a * opacity, ..ink } },
        ..container::Style::default()
    })
    .into()
}

fn help_section<'a>(host: &Host<'a>, ink: Color) -> Element<'a, Message> {
    let line = if host.daylight {
        Color { a: 0.09 * ink.a, ..Color::BLACK }
    } else {
        Color { r: 1.0, g: 1.0, b: 1.0, a: 0.10 * ink.a }
    };
    let content = row![lights_toggle(host, ink), space().width(Length::Fill), help_trigger(host, ink)]
        .height(Length::Fixed(32.0))
        .align_y(Alignment::Center)
        .width(Length::Fill);
    column![
        rule(line),
        container(content).width(Length::Fill).height(Length::Fixed(HELP_SECTION_H - 1.0)).padding(Padding {
            top: 6.0,
            right: 0.0,
            bottom: 0.0,
            left: 0.0
        })
    ]
    .into()
}

fn lights_toggle<'a>(host: &Host<'a>, ink: Color) -> Element<'a, Message> {
    // No lighting backend yet: the wall is lit, so the on segment is current and the control is inert.
    let alpha = 0.35;
    let ink = Color { a: ink.a * alpha, ..ink };
    let segment = |label: &str, active: bool| {
        let (fill, color) = if active {
            (Some(Color { a: alpha, ..host.tokens.accent }), Color { a: alpha, ..host.tokens.on_accent })
        } else {
            (None, Color { a: ink.a * 0.58, ..ink })
        };
        container(text(label.to_owned()).size(10.5).font(semibold()).color(color))
            .width(Length::Fixed(44.0))
            .align_x(Alignment::Center)
            .padding(Padding { top: 4.0, right: 9.0, bottom: 4.0, left: 9.0 })
            .style(move |_theme: &Theme| container::Style {
                background: fill.map(Background::Color),
                border: Border { radius: 999.0.into(), ..Border::default() },
                ..container::Style::default()
            })
    };
    let daylight = host.daylight;
    let face =
        row![segment(host.i18n.t("home.latticeLightsOn"), true), segment(host.i18n.t("home.latticeLightsOff"), false)]
            .spacing(2);
    container(face)
        .padding(3)
        .style(move |_theme: &Theme| container::Style {
            background: Some(Background::Color(if daylight {
                Color { a: 0.05 * alpha, ..Color::BLACK }
            } else {
                Color { r: 1.0, g: 1.0, b: 1.0, a: 0.06 * alpha }
            })),
            border: Border { radius: 999.0.into(), width: 1.0, color: Color { a: ink.a * 0.18, ..ink } },
            ..container::Style::default()
        })
        .into()
}

fn help_trigger<'a>(host: &Host<'a>, ink: Color) -> Element<'a, Message> {
    let daylight = host.daylight;
    let glyph = icon(Icon::CircleQuestionMark, 17.0, ink);
    let button = button(container(glyph).center(Length::Fill))
        .width(Length::Fixed(32.0))
        .height(Length::Fixed(32.0))
        .padding(0)
        .on_press(Message::ToolsHelp)
        .style(move |_theme: &Theme, status| {
            let hot = matches!(status, button::Status::Hovered | button::Status::Pressed);
            let fill = if daylight {
                Color { a: 0.07 * ink.a, ..Color::BLACK }
            } else {
                Color { r: 1.0, g: 1.0, b: 1.0, a: 0.10 * ink.a }
            };
            button::Style {
                background: hot.then_some(Background::Color(fill)),
                border: Border { radius: 12.0.into(), ..Border::default() },
                ..button::Style::default()
            }
        });
    tooltip(button, text(host.i18n.t("home.latticeHelp").to_owned()).size(12), Position::Top)
        .delay(TOOLTIP_DELAY)
        .style(container::rounded_box)
        .into()
}

const HELP_LABELS: [(&str, &str); HELP_ROWS] = [
    ("libraryBravais.helpSearch", "/"),
    ("libraryBravais.helpMove", "← ↑ ↓ →"),
    ("libraryBravais.helpOpen", "Enter"),
    ("libraryBravais.helpPlay", "Enter"),
    ("libraryBravais.helpEnqueue", "Shift + Enter"),
    ("libraryBravais.helpBack", "Esc"),
];

fn help_list<'a>(host: &Host<'a>, open: f32, visible: f32, amount: f32) -> Element<'a, Message> {
    let fade = open * amount;
    let ink = Color { a: host.text.a * fade, ..host.text };
    let inner_w = (panel_width(host) - 20.0).max(1.0);
    let full = HELP_PAD + HELP_ROWS as f32 * HELP_ROW_H;
    let body = column(
        HELP_LABELS
            .iter()
            .enumerate()
            .map(|(index, (label, kbd))| help_row(host, ink, label, kbd, index > 0))
            .collect::<Vec<_>>(),
    )
    .width(Length::Fill);
    let content: Element<'a, Message> = if full > visible {
        let padding = Padding { top: 7.0, right: 12.0, bottom: 7.0, left: 12.0 };
        scrollable(container(body).padding(padding).width(Length::Fill))
            .direction(crate::settings_kit::scrollbar())
            .width(Length::Fill)
            .height(Length::Fixed(visible))
            .into()
    } else {
        container(body)
            .padding(Padding { top: 7.0, right: 12.0, bottom: 7.0, left: 12.0 })
            .width(Length::Fill)
            .height(Length::Fixed(full))
            .into()
    };
    let content: Element<'a, Message> =
        Element::new(super::seam::SeamLayout { content, size: Size::new(inner_w, visible.min(full)) });
    let shown = visible.min(full) * amount;
    let fill = if host.daylight {
        Color { a: 0.05 * open, ..Color::BLACK }
    } else {
        Color { r: 1.0, g: 1.0, b: 1.0, a: 0.06 * open }
    };
    container(pin(content).width(Length::Fill).height(Length::Fill))
        .width(Length::Fill)
        .height(Length::Fixed(shown.max(0.0)))
        .clip(true)
        .style(move |_theme: &Theme| container::Style {
            background: Some(Background::Color(fill)),
            border: Border { radius: 14.0.into(), ..Border::default() },
            ..container::Style::default()
        })
        .into()
}

fn help_row<'a>(host: &Host<'a>, ink: Color, key: &'static str, kbd: &str, border: bool) -> Element<'a, Message> {
    let line = if host.daylight {
        Color { a: 0.06 * ink.a, ..Color::BLACK }
    } else {
        Color { r: 1.0, g: 1.0, b: 1.0, a: 0.07 * ink.a }
    };
    let content = row![
        text(host.i18n.t(key).to_owned()).size(12).color(Color { a: ink.a * 0.76, ..ink }),
        space().width(Length::Fill),
        container(text(kbd.to_owned()).size(10).font(semibold()).color(ink))
            .padding(Padding { top: 2.0, right: 6.0, bottom: 2.0, left: 6.0 })
            .style(move |_theme: &Theme| container::Style {
                border: Border { radius: 6.0.into(), width: 1.0, color: Color { a: ink.a * 0.36, ..ink } },
                ..container::Style::default()
            })
    ]
    .align_y(Alignment::Center)
    .width(Length::Fill);
    let body = container(content).width(Length::Fill).height(Length::Fixed(if border {
        HELP_ROW_H - 1.0
    } else {
        HELP_ROW_H
    }));
    if border { column![rule(line), body].into() } else { body.into() }
}

fn volume_row<'a>(host: &Host<'a>, ink: Color) -> Element<'a, Message> {
    let shown = if host.muted { 0.0 } else { host.volume };
    // `volumeIcon`: by the shown value, 0 / under half / the rest.
    let icon_kind = if shown <= 0.0 {
        Icon::VolumeX
    } else if shown < 0.5 {
        Icon::Volume1
    } else {
        Icon::Volume2
    };
    let icon_color = if host.muted { Color { a: ink.a, ..host.accent } } else { ink };
    let label = if host.muted {
        super::tr(host, "wallTools.muted").to_owned()
    } else {
        format!("{}%", (host.volume * 100.0).round() as i32)
    };
    let track = Color { a: 0.18 * ink.a, ..ink };
    let accent = Color { a: ink.a, ..host.accent };
    let value_color = Color { a: 0.66 * ink.a, ..ink };
    let row = row![
        button(container(icon(icon_kind, 17.0, icon_color)).center(Length::Fill))
            .width(Length::Fixed(30.0))
            .height(Length::Fixed(30.0))
            .padding(0)
            .on_press(Message::Mute)
            .style(move |_theme: &Theme, status| {
                let hot = matches!(status, button::Status::Hovered | button::Status::Pressed);
                button::Style {
                    background: Some(Background::Color(if hot {
                        Color { a: 0.10, ..ink }
                    } else {
                        Color::TRANSPARENT
                    })),
                    border: Border { radius: 9.0.into(), ..Border::default() },
                    ..button::Style::default()
                }
            }),
        iced::widget::slider(0.0..=1.0, shown, Message::Volume).step(0.01_f32).on_release(Message::VolumeDone).style(
            move |_theme: &Theme, _status| iced::widget::slider::Style {
                rail: Rail {
                    backgrounds: (Background::Color(accent), Background::Color(track)),
                    width: 4.0,
                    border: Border { radius: 2.0.into(), width: 0.0, color: Color::TRANSPARENT },
                },
                handle: Handle {
                    shape: HandleShape::Circle { radius: 7.0 },
                    background: Background::Color(ink),
                    border_width: 1.0,
                    border_color: Color { a: 0.22, ..Color::BLACK },
                },
            }
        ),
        text(label).size(12).color(value_color).font(semibold()).width(Length::Fixed(38.0)).align_x(Alignment::End),
    ]
    .spacing(8)
    .align_y(Alignment::Center)
    .padding(Padding { top: 0.0, right: 12.0, bottom: 0.0, left: 4.0 })
    .height(Length::Fixed(VOLUME_H));
    let fill = if host.daylight {
        Color { a: 0.05 * ink.a, ..Color::BLACK }
    } else {
        Color { r: 1.0, g: 1.0, b: 1.0, a: 0.06 * ink.a }
    };
    container(row)
        .style(move |_theme: &Theme| container::Style {
            background: Some(Background::Color(fill)),
            border: Border { radius: 12.0.into(), ..Border::default() },
            ..container::Style::default()
        })
        .into()
}

fn tool_button<'a>(host: &Host<'a>, open: bool) -> Element<'a, Message> {
    let (fill, icon_color) = if host.daylight {
        (Color { r: 1.0, g: 1.0, b: 1.0, a: 0.7 }, host.text)
    } else {
        (Color { a: 0.4, ..Color::BLACK }, Color::WHITE)
    };
    let icon = if open { Icon::X } else { Icon::Settings2 };
    round_button(icon, 20.0, BUTTON, fill, icon_color, Some(Message::Tools))
}

fn quick<'a>(
    lucide: Icon,
    cell: f32,
    ink: Color,
    daylight: bool,
    on_press: Option<Message>,
    tip: String,
) -> Element<'a, Message> {
    let enabled = on_press.is_some();
    let alpha = if enabled { 1.0 } else { 0.35 };
    let color = Color { a: ink.a * alpha, ..ink };
    let button = button(container(icon(lucide, 18.0, color)).center(Length::Fill))
        .width(Length::Fixed(cell))
        .height(Length::Fixed(QUICK_H))
        .padding(0)
        .on_press_maybe(on_press)
        .style(move |_theme: &Theme, status| {
            let hot = enabled && matches!(status, button::Status::Hovered | button::Status::Pressed);
            let idle = if daylight { 0.05 } else { 0.06 };
            let hover = if daylight { 0.10 } else { 0.13 };
            button::Style {
                background: Some(Background::Color(Color {
                    a: if hot { hover } else { idle } * alpha,
                    ..if daylight { Color::BLACK } else { Color::WHITE }
                })),
                border: Border { radius: 12.0.into(), ..Border::default() },
                ..button::Style::default()
            }
        });
    tooltip(button, text(tip).size(12), Position::Top).delay(TOOLTIP_DELAY).style(container::rounded_box).into()
}

fn round_button<'a>(
    lucide: Icon,
    icon_px: f32,
    box_px: f32,
    fill: Color,
    icon_color: Color,
    on_press: Option<Message>,
) -> Element<'a, Message> {
    let pad = ((box_px - icon_px) * 0.5).max(0.0);
    button(icon(lucide, icon_px, icon_color))
        .padding(pad)
        .on_press_maybe(on_press)
        .style(move |_theme: &Theme, status| {
            let hot = matches!(status, button::Status::Hovered | button::Status::Pressed);
            button::Style {
                background: Some(Background::Color(Color {
                    a: if hot { (fill.a + 0.08).min(1.0) } else { fill.a },
                    ..fill
                })),
                border: Border { radius: (box_px * 0.5).into(), ..Border::default() },
                shadow: iced::Shadow {
                    color: Color { a: 0.10, ..Color::BLACK },
                    offset: iced::Vector::new(0.0, 10.0),
                    blur_radius: 15.0,
                },
                ..button::Style::default()
            }
        })
        .into()
}

fn semibold() -> iced::Font {
    crate::typeface::font(iced::font::Weight::Semibold)
}
