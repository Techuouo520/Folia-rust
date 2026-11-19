// Searchable commands for the two wall surfaces and their execute-mode bindings.

use super::{Host, Message};
use crate::typeface::{text, text_input};
use iced::widget::{button, column, container, mouse_area, opaque, row, scrollable, space, stack};
use iced::{Alignment, Element, Length};

pub(super) fn view<'a>(host: &Host<'a>, query: &'a str) -> Element<'a, Message> {
    let wall = host.wall;
    let mut choices = vec![
        ("wallTools.shuffle", "R", (!host.fm && host.queue.len() > 1).then_some(Message::Shuffle)),
        ("wallTools.theme", "", host.can_generate.then_some(Message::GenerateTheme)),
        (
            "playlist.locatePlaying",
            "C",
            (host.has_track && (host.lattice || !host.fm)).then_some(Message::LocatePlaying),
        ),
        (if host.lattice { "wallTools.goHome" } else { "libraryBravais.toolsLattice" }, "", Some(Message::Destination)),
        ("wallTools.openPlayer", "", host.has_track.then_some(Message::Player)),
        ("wallTools.openSettings", "", Some(Message::Settings)),
    ];
    if !host.lattice {
        choices.extend([
            ("options.libraryWallLook", "P", Some(Message::WallLook(wall.look.next()))),
            (
                "wallTools.moreWindows",
                "",
                (wall.reserved() > 0 && wall.reserved() < 6).then(|| Message::Windows(wall.reserved() + 1)),
            ),
            ("wallTools.fewerWindows", "", (wall.reserved() > 1).then(|| Message::Windows(wall.reserved() - 1))),
        ]);
    } else {
        choices.push(("options.latticeWallForm", "", (!host.fm).then_some(Message::Radio(!wall.radio))));
        choices.push(("options.latticeAutoFocusOnSongChange", "", Some(Message::AutoFocus(!wall.auto_focus))));
        choices.push(("options.latticeVignette", "", Some(Message::Vignette(!wall.vignette))));
    }
    choices.extend([
        ("options.wallRandomizeOnEntry", "", Some(Message::RandomizeOnEntry)),
        ("options.latticePosterTint", "", Some(Message::Tint(!wall.tint))),
        ("home.latticeLights", "", Some(Message::Lights(!wall.lights_on))),
        ("options.latticeBadgeCurrentOnly", "", Some(Message::BadgeCurrentOnly(!wall.badge_current_only))),
    ]);
    let tokens = host.tokens;
    let mut rows = column![].spacing(3);
    let mut first = None;
    let filter = query.trim().to_lowercase();
    for (key, shortcut, action) in choices {
        let label = host.i18n.t(key);
        if !label.to_lowercase().contains(&filter) && (shortcut.is_empty() || !shortcut.eq_ignore_ascii_case(&filter)) {
            continue;
        }
        let action = action.map(|action| Message::RunCommand(Box::new(action)));
        if first.is_none() {
            first = action.clone();
        }
        rows = rows.push(
            button(row![text(label).size(14), space().width(Length::Fill), text(shortcut).size(12)])
                .padding(8)
                .width(Length::Fill)
                .on_press_maybe(action)
                .style(button::text),
        );
    }
    let input = text_input(host.i18n.t("commandPalette.placeholder"), query)
        .id("wall-commands")
        .on_input(Message::CommandQuery)
        .on_submit_maybe(first);
    let card = opaque(
        container(column![input, scrollable(rows).height(Length::Shrink)].spacing(8))
            .padding(12)
            .max_width(420)
            .max_height((host.window_height - 64.0).max(100.0))
            .style(move |_| container::Style {
                background: Some(tokens.win.into()),
                border: iced::Border { radius: 16.0.into(), width: 1.0, color: tokens.line },
                ..Default::default()
            }),
    );
    stack![
        mouse_area(space().width(Length::Fill).height(Length::Fill)).on_press(Message::Dismiss),
        container(card).width(Length::Fill).height(Length::Fill).align_x(Alignment::Center).align_y(Alignment::Center)
    ]
    .into()
}
