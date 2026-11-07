// The bottom-right wall tools button and its panel, from `WallToolsButton`.

use std::time::Instant;

use folia_core::Track;
use folia_i18n::I18n;
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
use crate::settings_kit::Faded;

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
        body = body.push(island_face(home, Face::new(host, home.island(now)), &geo, now));
        body = body.push(space().height(Length::Fixed(STACK_GAP)));
    }
    body = body.push(panel_face(home, Face::new(host, open), &geo, now));
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
                plate_fill(host),
                plate_shadow(host),
                alpha,
            ));
        }
    }
    layers.push(plate(
        host,
        Rectangle { x, y: geo.panel_top, width: geo.panel_w, height: geo.panel_h },
        PANEL_RADIUS,
        plate_fill(host),
        plate_shadow(host),
        open,
    ));
    stack(layers).into()
}

fn plate_fill(host: &Host<'_>) -> [f32; 4] {
    crate::settings_kit::tools_glass(host.daylight)
}

fn plate_shadow(host: &Host<'_>) -> [f32; 4] {
    if host.daylight { [42.0 / 255.0, 35.0 / 255.0, 27.0 / 255.0, 0.18] } else { [0.0, 0.0, 0.0, 0.42] }
}

/// `fade` is the panel's open amount. It covers the blur and the fill together (`backdrop-filter: blur(24px)`).
fn plate<'a>(
    host: &Host<'a>,
    rect: Rectangle,
    radius: f32,
    fill: [f32; 4],
    shadow: [f32; 4],
    fade: f32,
) -> Element<'a, Message> {
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
        backdrop: 24.0,
        fade,
        saturate: 1.0,
    };
    let patch = shader(plate).width(Length::Fixed((x1 - x0).max(1.0))).height(Length::Fixed((y1 - y0).max(1.0)));
    container(patch)
        .width(Length::Fill)
        .height(Length::Fill)
        .padding(Padding { top: y0, right: 0.0, bottom: 0.0, left: x0 })
        .into()
}

/// What a panel face reads of the host: its words and state, and its colours only through the panel's fade.
#[derive(Clone, Copy)]
struct Face<'a> {
    i18n: &'a I18n,
    paint: Faded,
    has_track: bool,
    fm: bool,
    volume: f32,
    muted: bool,
    queue: &'a [Track],
    queue_index: Option<usize>,
}

impl<'a> Face<'a> {
    fn new(host: &Host<'a>, alpha: f32) -> Self {
        Self {
            i18n: host.i18n,
            paint: Faded::new(host.tokens, host.text, host.accent, host.daylight, alpha),
            has_track: host.has_track,
            fm: host.fm,
            volume: host.volume,
            muted: host.muted,
            queue: host.queue,
            queue_index: host.queue_index,
        }
    }
}

fn island_face<'a>(home: &'a HomeChrome, face: Face<'a>, geo: &Geometry, now: Instant) -> Element<'a, Message> {
    let paint = face.paint;
    let border = paint.wash(0.07, 0.08);
    let count = face.queue.len();
    let list: Element<'a, Message> = if count == 0 {
        container(text(face.i18n.t("queue.empty").to_owned()).size(12).color(paint.ink(0.45)))
            .width(Length::Fill)
            .height(Length::Fixed(geo.list_h))
            .align_x(Alignment::Center)
            .align_y(Alignment::Center)
            .into()
    } else {
        let look = queue_island::Look {
            i18n: face.i18n,
            tokens: paint.tokens(),
            ink: paint.text(),
            daylight: paint.daylight(),
            actions_on_current: false,
            reorder: true,
            row: queue_island::ISLAND_ROW,
        };
        home.queue.view(look, face.queue, face.queue_index, geo.list_h, now).map(Message::ToolsQueue)
    };
    let body = column![island_header(home, face, count), list].width(Length::Fill).height(Length::Fill);
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

fn island_header<'a>(home: &HomeChrome, face: Face<'a>, count: usize) -> Element<'a, Message> {
    let line = face.paint.wash(0.07, 0.08);
    let count_text = count.to_string();
    let title = face.i18n.format("wallTools.queueCount", &[("count", count_text.as_str())]);
    let title = text(title)
        .size(12)
        .font(crate::typeface::font(iced::font::Weight::Bold))
        .color(face.paint.ink(0.72))
        .width(Length::Fill)
        .wrapping(iced::widget::text::Wrapping::None);
    let mut bar = row![title].align_y(Alignment::Center).width(Length::Fill);
    // The playing item stays after a clear, so with it alone there is nothing left to clear.
    if count > 1 {
        bar = bar.push(clear_button(home, face));
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

fn clear_button<'a>(home: &HomeChrome, face: Face<'a>) -> Element<'a, Message> {
    let paint = face.paint;
    let armed = home.queue_clear_armed();
    let label = if armed { face.i18n.t("wallTools.queueClearConfirm") } else { face.i18n.t("wallTools.queueClear") };
    if armed {
        let on_accent = paint.tokens().on_accent;
        let accent = paint.tokens().accent;
        let glyph = icon(Icon::Trash, 15.0, on_accent);
        button(
            row![glyph, text(label).size(11).font(crate::typeface::font(iced::font::Weight::Bold)).color(on_accent)]
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
        let glyph = icon(Icon::Trash, 15.0, paint.ink(0.72));
        button(container(glyph).center(Length::Fixed(28.0)))
            .width(Length::Fixed(28.0))
            .height(Length::Fixed(28.0))
            .padding(0)
            .on_press(Message::ToolsQueueClear)
            .style(move |_theme: &Theme, status| {
                let hot = matches!(status, button::Status::Hovered | button::Status::Pressed);
                button::Style {
                    background: hot.then_some(Background::Color(paint.wash(0.07, 0.10))),
                    border: Border { radius: 9.0.into(), ..Border::default() },
                    ..button::Style::default()
                }
            })
            .into()
    }
}

fn panel_face<'a>(home: &'a HomeChrome, face: Face<'a>, geo: &Geometry, now: Instant) -> Element<'a, Message> {
    let border = face.paint.wash(0.07, 0.08);
    let mut inner = column![
        quick_row(home, face, geo.panel_w),
        space().height(Length::Fixed(QUICK_GAP)),
        volume_row(face),
        space().height(Length::Fixed(HEADING_GAP)),
        heading(face, "libraryBravais.toolsAppearance"),
        space().height(Length::Fixed(ROW_GAP)),
        look_row(face),
        space().height(Length::Fixed(ROW_GAP)),
        tint_row(face),
        space().height(Length::Fixed(HELP_MARGIN)),
        help_section(face),
    ];
    if home.help_shown(now) {
        inner = inner.push(space().height(Length::Fixed(HELP_LIST_MARGIN))).push(help_list(
            face,
            geo.panel_w,
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

fn quick_row<'a>(home: &HomeChrome, face: Face<'a>, panel_w: f32) -> Element<'a, Message> {
    let cell = ((panel_w - 20.0) - 4.0 * 5.0) / 6.0;
    let (i18n, paint, fm) = (face.i18n, face.paint, face.fm);
    let queue_open = home.queue_island_open();
    let shuffle_tip =
        tooltip_text(i18n.t("wallTools.shuffle"), Some(i18n.t("wallTools.shuffleUnavailable")), Some(": + R"));
    let theme_tip = tooltip_text(i18n.t("wallTools.theme"), Some(i18n.t("wallTools.themeUnavailable")), None);
    let queue_status = if fm {
        i18n.t("wallTools.queueUnavailableInFm")
    } else if queue_open {
        i18n.t("wallTools.queueHide")
    } else {
        i18n.t("wallTools.queueShow")
    };
    let queue_tip = tooltip_text(i18n.t("wallTools.queue"), Some(queue_status), None);
    let lattice_status = if fm { i18n.t("status.latticeUnavailableInFm") } else { i18n.t("status.latticeUnavailable") };
    // No Lattice and no Ctrl/Cmd + B in this build: the label and the reason only.
    let lattice_tip = tooltip_text(i18n.t("libraryBravais.toolsLattice"), Some(lattice_status), None);
    let player_tip = tooltip_text(
        i18n.t("wallTools.openPlayer"),
        (!face.has_track).then_some(i18n.t("wallTools.openPlayerUnavailable")),
        None,
    );
    let settings_tip = i18n.t("wallTools.openSettings").to_owned();
    row![
        quick(Icon::Shuffle, cell, paint, None, shuffle_tip),
        quick(Icon::Sparkles, cell, paint, None, theme_tip),
        quick(Icon::ListMusic, cell, paint, (!fm).then_some(Message::ToolsQueueToggle), queue_tip),
        quick(Icon::PanelsTopLeft, cell, paint, None, lattice_tip),
        quick(Icon::Disc3, cell, paint, face.has_track.then_some(Message::Player), player_tip),
        quick(Icon::Settings, cell, paint, Some(Message::Settings), settings_tip),
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

fn heading<'a>(face: Face<'a>, key: &'static str) -> Element<'a, Message> {
    column![
        rule(face.paint.wash(0.09, 0.10)),
        container(
            text(face.i18n.t(key).to_owned())
                .size(10)
                .font(crate::typeface::font(iced::font::Weight::Bold))
                .color(face.paint.ink(0.52)),
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
fn look_row<'a>(face: Face<'a>) -> Element<'a, Message> {
    let value = face.i18n.t("options.libraryWallLookSolid");
    action_row(face, Icon::Blinds, face.i18n.t("options.libraryWallLook"), Some(value), None, false)
}

/// `海报聚焦叠色`: the toggle needs `latticePosterTintEnabled`, absent here, so it stays off and disabled.
fn tint_row<'a>(face: Face<'a>) -> Element<'a, Message> {
    action_row(face, Icon::Layers, face.i18n.t("options.latticePosterTint"), None, Some(false), false)
}

fn action_row<'a>(
    face: Face<'a>,
    glyph: Icon,
    label: &str,
    value: Option<&str>,
    toggle: Option<bool>,
    enabled: bool,
) -> Element<'a, Message> {
    let paint = face.paint.faded(if enabled { 1.0 } else { 0.35 });
    let ink = paint.text();
    let icon = icon(glyph, 17.0, ink);
    let label = text(label.to_owned()).size(13).font(crate::typeface::font(iced::font::Weight::Normal)).color(ink);
    let mut content = row![icon, label].spacing(11).align_y(Alignment::Center).width(Length::Fill);
    if let Some(value) = value {
        content = content.push(text(value.to_owned()).size(12).font(semibold()).color(paint.ink(0.6)));
    }
    if let Some(checked) = toggle {
        content = content.push(toggle_face(paint, checked));
    }
    button(content)
        .width(Length::Fill)
        .height(Length::Fixed(ACTION_H))
        .padding(Padding { top: 0.0, right: 12.0, bottom: 0.0, left: 12.0 })
        .on_press_maybe(enabled.then_some(Message::Frame))
        .style(move |_theme: &Theme, status| {
            let hot = enabled && matches!(status, button::Status::Hovered | button::Status::Pressed);
            button::Style {
                background: hot.then_some(Background::Color(paint.wash(0.07, 0.10))),
                border: Border { radius: 12.0.into(), ..Border::default() },
                ..button::Style::default()
            }
        })
        .into()
}

fn toggle_face<'a>(paint: Faded, checked: bool) -> Element<'a, Message> {
    let fill = checked.then_some(paint.tokens().accent);
    let ring = paint.ink(if checked { 1.0 } else { 0.42 });
    let knob = if checked { paint.tokens().on_accent } else { paint.text() };
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
        border: Border { radius: 9.0.into(), width: 1.0, color: ring },
        ..container::Style::default()
    })
    .into()
}

fn help_section<'a>(face: Face<'a>) -> Element<'a, Message> {
    let content = row![lights_toggle(face), space().width(Length::Fill), help_trigger(face)]
        .height(Length::Fixed(32.0))
        .align_y(Alignment::Center)
        .width(Length::Fill);
    column![
        rule(face.paint.wash(0.09, 0.10)),
        container(content).width(Length::Fill).height(Length::Fixed(HELP_SECTION_H - 1.0)).padding(Padding {
            top: 6.0,
            right: 0.0,
            bottom: 0.0,
            left: 0.0
        })
    ]
    .into()
}

fn lights_toggle<'a>(face: Face<'a>) -> Element<'a, Message> {
    // No lighting backend yet: the wall is lit, so the on segment is current and the control is inert at 35%.
    let paint = face.paint.faded(0.35);
    let segment = |label: &str, active: bool| {
        let (fill, color) =
            if active { (Some(paint.tokens().accent), paint.tokens().on_accent) } else { (None, paint.ink(0.58)) };
        container(text(label.to_owned()).size(10.5).font(crate::typeface::font(iced::font::Weight::Bold)).color(color))
            .width(Length::Fixed(44.0))
            .align_x(Alignment::Center)
            .padding(Padding { top: 4.0, right: 9.0, bottom: 4.0, left: 9.0 })
            .style(move |_theme: &Theme| container::Style {
                background: fill.map(Background::Color),
                border: Border { radius: 999.0.into(), ..Border::default() },
                ..container::Style::default()
            })
    };
    let face_row =
        row![segment(face.i18n.t("home.latticeLightsOn"), true), segment(face.i18n.t("home.latticeLightsOff"), false)]
            .spacing(2);
    container(face_row)
        .padding(3)
        .style(move |_theme: &Theme| container::Style {
            background: Some(Background::Color(paint.wash(0.05, 0.06))),
            border: Border { radius: 999.0.into(), width: 1.0, color: paint.ink(0.18) },
            ..container::Style::default()
        })
        .into()
}

fn help_trigger<'a>(face: Face<'a>) -> Element<'a, Message> {
    let paint = face.paint;
    let glyph = icon(Icon::CircleQuestionMark, 17.0, paint.text());
    let button = button(container(glyph).center(Length::Fill))
        .width(Length::Fixed(32.0))
        .height(Length::Fixed(32.0))
        .padding(0)
        .on_press(Message::ToolsHelp)
        .style(move |_theme: &Theme, status| {
            let hot = matches!(status, button::Status::Hovered | button::Status::Pressed);
            button::Style {
                background: hot.then_some(Background::Color(paint.wash(0.07, 0.10))),
                border: Border { radius: 12.0.into(), ..Border::default() },
                ..button::Style::default()
            }
        });
    tooltip(button, text(face.i18n.t("home.latticeHelp").to_owned()).size(12), Position::Top)
        .delay(TOOLTIP_DELAY)
        .style(paint.tip())
        .into()
}

fn help_list<'a>(face: Face<'a>, panel_w: f32, visible: f32, amount: f32) -> Element<'a, Message> {
    let rows = face.paint.faded(amount);
    let inner_w = (panel_w - 20.0).max(1.0);
    let full = HELP_PAD + HELP_ROWS as f32 * HELP_ROW_H;
    let body = column(
        super::HELP_LIST
            .iter()
            .enumerate()
            .map(|(index, row)| help_row(face.i18n, rows, row.label, row.kbd, index > 0))
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
        Element::new(crate::design_size::DesignSize { content, size: Size::new(inner_w, visible.min(full)) });
    let shown = visible.min(full) * amount;
    let fill = face.paint.wash(0.05, 0.06);
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

fn help_row<'a>(i18n: &'a I18n, paint: Faded, key: &'static str, kbd: &str, border: bool) -> Element<'a, Message> {
    let ink = paint.text();
    let ring = paint.ink(0.36);
    let content = row![
        text(i18n.t(key).to_owned()).size(12).color(paint.ink(0.76)),
        space().width(Length::Fill),
        container(text(kbd.to_owned()).size(10).font(semibold()).color(ink))
            .padding(Padding { top: 2.0, right: 6.0, bottom: 2.0, left: 6.0 })
            .style(move |_theme: &Theme| container::Style {
                border: Border { radius: 6.0.into(), width: 1.0, color: ring },
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
    if border { column![rule(paint.wash(0.06, 0.07)), body].into() } else { body.into() }
}

fn volume_row<'a>(face: Face<'a>) -> Element<'a, Message> {
    let paint = face.paint;
    let ink = paint.text();
    let shown = if face.muted { 0.0 } else { face.volume };
    // `volumeIcon`: by the shown value, 0 / under half / the rest.
    let icon_kind = if shown <= 0.0 {
        Icon::VolumeX
    } else if shown < 0.5 {
        Icon::Volume1
    } else {
        Icon::Volume2
    };
    let icon_color = if face.muted { paint.accent() } else { ink };
    let label = if face.muted {
        face.i18n.t("wallTools.muted").to_owned()
    } else {
        format!("{}%", (face.volume * 100.0).round() as i32)
    };
    let track = paint.ink(0.18);
    let accent = paint.accent();
    let hover = paint.tint(ink, 0.10);
    let handle_ring = paint.tint(Color::BLACK, 0.22);
    let row = row![
        button(container(icon(icon_kind, 17.0, icon_color)).center(Length::Fill))
            .width(Length::Fixed(30.0))
            .height(Length::Fixed(30.0))
            .padding(0)
            .on_press(Message::Mute)
            .style(move |_theme: &Theme, status| {
                let hot = matches!(status, button::Status::Hovered | button::Status::Pressed);
                button::Style {
                    background: Some(Background::Color(if hot { hover } else { Color::TRANSPARENT })),
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
                    border_color: handle_ring,
                },
            }
        ),
        text(label).size(12).color(paint.ink(0.66)).font(semibold()).width(Length::Fixed(38.0)).align_x(Alignment::End),
    ]
    .spacing(8)
    .align_y(Alignment::Center)
    .padding(Padding { top: 0.0, right: 12.0, bottom: 0.0, left: 4.0 })
    .height(Length::Fixed(VOLUME_H));
    let fill = paint.wash(0.05, 0.06);
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

fn quick<'a>(lucide: Icon, cell: f32, paint: Faded, on_press: Option<Message>, tip: String) -> Element<'a, Message> {
    let enabled = on_press.is_some();
    let dim = paint.faded(if enabled { 1.0 } else { 0.35 });
    let button = button(container(icon(lucide, 18.0, dim.text())).center(Length::Fill))
        .width(Length::Fixed(cell))
        .height(Length::Fixed(QUICK_H))
        .padding(0)
        .on_press_maybe(on_press)
        .style(move |_theme: &Theme, status| {
            let hot = enabled && matches!(status, button::Status::Hovered | button::Status::Pressed);
            let fill = if hot { dim.wash(0.10, 0.13) } else { dim.wash(0.05, 0.06) };
            button::Style {
                background: Some(Background::Color(fill)),
                border: Border { radius: 12.0.into(), ..Border::default() },
                ..button::Style::default()
            }
        });
    tooltip(button, text(tip).size(12), Position::Top).delay(TOOLTIP_DELAY).style(paint.tip()).into()
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
