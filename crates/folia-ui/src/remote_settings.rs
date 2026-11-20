// Remote page controls, sharing the same settings and actions as the companion window.

use folia_i18n::I18n;
use folia_store::{RemoteSettings, RemoteSource};
use iced::Element;
use iced::widget::{button, row};

use crate::remote::Message as Remote;
use crate::settings_kit::{self as kit, Choice, Group, Row, Tokens};
use crate::{Message, typeface::text};

pub(crate) fn groups<'a>(
    settings: &'a RemoteSettings,
    taskbar_available: bool,
    i18n: &'a I18n,
    tokens: Tokens,
) -> Vec<Group<'a, Message>> {
    let appearance = &settings.appearance;
    let toggle =
        |label, value, action| item(i18n, label, kit::toggle(tokens, f32::from(value), Message::Remote(action)));
    let mut window = vec![
        item(
            i18n,
            "remote.open",
            button(text(i18n.t("remote.open"))).on_press(Message::Remote(Remote::Open)).style(button::text).into(),
        ),
        toggle(
            "options.hideRemoteControlTaskbarIcon",
            settings.hide_taskbar_icon,
            Remote::Taskbar(!settings.hide_taskbar_icon),
        ),
        toggle(
            "options.hideRemoteControlTitlebar",
            appearance.hide_titlebar,
            Remote::Titlebar(!appearance.hide_titlebar),
        ),
        toggle(
            "options.remoteControlClickThrough",
            settings.click_through,
            Remote::ClickThrough(!settings.click_through),
        ),
        toggle(
            "options.remoteControlExpandCover",
            appearance.expand_cover,
            Remote::ExpandCover(!appearance.expand_cover),
        ),
        toggle("options.remoteControlTextShade", appearance.text_shade, Remote::TextShade(!appearance.text_shade)),
        item(
            i18n,
            "remote.unlock",
            button(text(i18n.t("remote.unlock"))).on_press(Message::Remote(Remote::Unlock)).style(button::text).into(),
        ),
    ];
    window[3].hint = Some(i18n.t("remote.unlockHint").into());
    window[2].hint = Some(i18n.t("options.hideRemoteControlTitlebarHint").into());
    window[4].hint = Some(i18n.t("options.remoteControlExpandCoverHint").into());
    window[5].hint = Some(i18n.t("options.remoteControlTextShadeHint").into());
    if !taskbar_available {
        window[1].hint = Some(i18n.t("remote.taskbarUnavailable").into());
        window[1].control = kit::toggle_fixed(tokens, f32::from(settings.hide_taskbar_icon));
    }
    window.push(item(
        i18n,
        "remote.appearance",
        row![
            button(text(i18n.t("wallTools.copyPreset")))
                .on_press(Message::Remote(Remote::ExportAppearance))
                .style(button::text),
            button(text(i18n.t("wallTools.pastePreset")))
                .on_press(Message::Remote(Remote::ImportAppearance))
                .style(button::text),
        ]
        .spacing(8)
        .into(),
    ));
    let mut lyrics = vec![item(
        i18n,
        "options.remoteLyricPairs",
        kit::segmented(
            tokens,
            vec![(1, i18n.t("options.remoteLyricPairsOne").into()), (2, i18n.t("options.remoteLyricPairsTwo").into())],
            appearance.lyric_pairs,
            None,
            |count| Message::Remote(Remote::Pairs(count)),
        ),
    )];
    for (index, key) in
        ["options.remoteLyricRow1", "options.remoteLyricRow2", "options.remoteLyricRow3", "options.remoteLyricRow4"]
            .into_iter()
            .take(usize::from(appearance.lyric_pairs) * 2)
            .enumerate()
    {
        let sources: &[RemoteSource] = if index % 2 == 0 { &RemoteSource::MAIN } else { &RemoteSource::SUB };
        let choice = |value: RemoteSource| Choice { value, label: i18n.t(value.label()).to_owned() };
        let control = kit::select(
            tokens,
            sources.iter().copied().map(choice).collect(),
            choice(appearance.lyric_rows[index]),
            move |choice| Message::Remote(Remote::Source(index, choice.value)),
        );
        let mut line = item(i18n, key, control);
        line.sub = true;
        lyrics.push(line);
    }
    vec![
        Group { title: i18n.t("options.remoteControlWindowGroup").into(), bare: false, rows: window, foot: None },
        Group {
            title: i18n.t("options.remoteLyricsGroup").into(),
            bare: false,
            rows: lyrics,
            foot: Some(i18n.t("options.remoteLyricPairsDesc").into()),
        },
    ]
}

fn item<'a>(i18n: &I18n, key: &str, control: Element<'a, Message>) -> Row<'a, Message> {
    Row { label: i18n.t(key).into(), hint: None, note: None, sub: false, below: None, control }
}
