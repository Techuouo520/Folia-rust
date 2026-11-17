// Native connection form and song-theme preferences used by the existing settings cards.

use folia_i18n::I18n;
use folia_store::{AiProtocol, ThemeGenerationSource};
use iced::widget::{button, row, slider, space};
use iced::{Alignment, Element, Length};

use crate::settings_kit::{self as kit, Choice, Group, Row, Tokens};
use crate::song_theme::{Message, SongThemes};
use crate::typeface::{text, text_input};

fn change(message: Message) -> crate::Message {
    crate::Message::SongTheme(message)
}

fn action<'a>(tokens: Tokens, label: &'a str, message: Option<crate::Message>) -> Element<'a, crate::Message> {
    button(text(label).size(13).color(tokens.t1))
        .padding([7, 12])
        .style(move |_, _| kit::rounded_button(tokens.fill, 8.0))
        .on_press_maybe(message)
        .into()
}

fn option<'a>(
    i18n: &I18n,
    label: &str,
    hint: Option<&str>,
    control: Element<'a, crate::Message>,
) -> Row<'a, crate::Message> {
    Row {
        label: i18n.t(label).to_owned(),
        hint: hint.map(|key| i18n.t(key).to_owned()),
        sub: false,
        below: None,
        control,
    }
}

pub fn song_group<'a>(
    state: &'a SongThemes,
    i18n: &'a I18n,
    tokens: Tokens,
    has_custom: bool,
) -> Group<'a, crate::Message> {
    if !state.is_ready() {
        return loading(i18n);
    }
    let preferences = &state.preferences;
    let toggle = |enabled, message| kit::toggle(tokens, if enabled { 1.0 } else { 0.0 }, change(message));
    let mut rows = vec![option(
        i18n,
        "options.autoSwitchSongTheme",
        Some("options.autoSwitchSongThemeDesc"),
        toggle(preferences.auto_switch, Message::AutoSwitch(!preferences.auto_switch)),
    )];
    if preferences.auto_switch {
        let mut row = option(
            i18n,
            "options.autoGenerateSongTheme",
            Some(if preferences.generation_source == ThemeGenerationSource::Cover {
                "options.autoGenerateSongThemeCoverDesc"
            } else {
                "options.autoGenerateSongThemeDesc"
            }),
            toggle(preferences.auto_generate, Message::AutoGenerate(!preferences.auto_generate)),
        );
        row.sub = true;
        rows.push(row);
    }
    let sources = vec![
        (ThemeGenerationSource::Ai, i18n.t("options.themeGenerationSourceAi").to_owned()),
        (ThemeGenerationSource::Cover, i18n.t("options.themeGenerationSourceCover").to_owned()),
    ];
    let disabled = if state.has_key { &[][..] } else { &[ThemeGenerationSource::Ai][..] };
    rows.push(option(
        i18n,
        "options.themeGenerationSource",
        Some(if preferences.generation_source == ThemeGenerationSource::Cover {
            "options.themeGenerationSourceCoverDesc"
        } else {
            "options.themeGenerationSourceAiDesc"
        }),
        kit::segmented_available(tokens, sources, preferences.generation_source, disabled, |source| {
            change(Message::Source(source))
        }),
    ));
    if !state.has_key {
        rows.push(option(
            i18n,
            "options.themeGenerationSourceAiUnavailable",
            None,
            action(
                tokens,
                i18n.t("options.configureAiApiKey"),
                Some(crate::Message::Settings(crate::settings::Message::Section(crate::settings::Section::Ai))),
            ),
        ));
    }
    let custom = if has_custom {
        toggle(preferences.prefer_custom, Message::PreferCustom(!preferences.prefer_custom))
    } else {
        kit::toggle_fixed(tokens, 0.0)
    };
    rows.push(option(i18n, "options.preferCustomTheme", Some("options.preferCustomThemeDesc"), custom));
    Group { title: i18n.t("options.settingsHeadingSongTheme").to_owned(), bare: false, rows, foot: None }
}

pub fn connection_group<'a>(state: &'a SongThemes, i18n: &'a I18n, tokens: Tokens) -> Group<'a, crate::Message> {
    if !state.is_ready() {
        return loading(i18n);
    }
    let protocols = [
        (AiProtocol::Gemini, "options.aiProvider_gemini"),
        (AiProtocol::Openai, "options.aiProvider_custom"),
        (AiProtocol::Anthropic, "options.aiProvider_anthropic"),
    ];
    let choice = |(value, key)| Choice { value, label: i18n.t(key).to_owned() };
    let selected = protocols.iter().copied().find(|(value, _)| *value == state.draft.protocol).unwrap_or(protocols[0]);
    let protocol = kit::select(tokens, protocols.into_iter().map(choice).collect(), choice(selected), |value| {
        change(Message::Protocol(value.value))
    });
    let input = |value, placeholder, message: fn(String) -> Message, secure| {
        let field = text_input(placeholder, value)
            .style(move |_, _| kit::input_style(tokens))
            .secure(secure)
            .size(13)
            .padding([8, 10])
            .width(Length::Fixed(280.0));
        let field = if state.connection_busy { field } else { field.on_input(move |value| change(message(value))) };
        field.into()
    };
    let same = folia_ai::same_endpoint(&state.connection, &state.draft);
    let key_hint = if state.has_key && same { "options.aiApiKeySavedPlaceholder" } else { "options.aiApiKey" };
    let mut key_row = option(
        i18n,
        "options.aiApiKey",
        (state.has_key && same).then_some("options.aiApiKeySavedDesc"),
        input(&state.key_draft, i18n.t(key_hint), Message::Key, true),
    );
    if state.has_key && !same && state.key_draft.trim().is_empty() {
        key_row.hint = Some(if state.connection.protocol != state.draft.protocol {
            let provider = protocols
                .iter()
                .find(|(value, _)| *value == state.connection.protocol)
                .map(|(_, key)| i18n.t(key))
                .unwrap_or_default();
            i18n.format("options.aiApiKeyOtherProviderDesc", &[("provider", provider)])
        } else {
            i18n.format("options.aiApiKeyEndpointMismatchDesc", &[("endpoint", &state.connection.endpoint)])
        });
    }
    let temperature = row![
        slider(0.0..=2.0, state.draft.temperature, |value| change(Message::Temperature(value)))
            .step(0.1_f32)
            .width(150)
            .style(move |_, _| kit::slider_style(tokens)),
        kit::value(tokens, format!("{:.1}", state.draft.temperature))
    ]
    .spacing(12)
    .align_y(Alignment::Center)
    .into();
    let toggle = kit::toggle(
        tokens,
        if state.draft.use_system_proxy { 1.0 } else { 0.0 },
        change(Message::Proxy(!state.draft.use_system_proxy)),
    );
    let buttons = row![
        action(tokens, i18n.t("options.aiApiKeyClear"), (!state.connection_busy).then(|| change(Message::ClearKey))),
        action(
            tokens,
            i18n.t("options.aiTestConnection"),
            (!state.connection_busy).then(|| change(Message::TestConnection))
        ),
        action(tokens, i18n.t("options.save"), (!state.connection_busy).then(|| change(Message::SaveConnection))),
    ]
    .spacing(8)
    .into();
    let mut rows = vec![
        option(i18n, "options.aiProvider", None, protocol),
        option(i18n, "options.aiApiUrl", None, input(&state.draft.endpoint, "https://", Message::Endpoint, false)),
        option(
            i18n,
            "options.aiModel",
            None,
            input(&state.draft.model, i18n.t("options.aiModelPlaceholder"), Message::Model, false),
        ),
        key_row,
        option(i18n, "options.useSystemProxyAI", Some("options.useSystemProxyAIDesc"), toggle),
        Row { label: String::new(), hint: None, sub: false, below: None, control: buttons },
    ];
    if folia_ai::temperature_supported(&state.draft) {
        rows.insert(4, option(i18n, "options.openaiApiTemperature", None, temperature));
    }
    if state.connection_busy || state.connection_result.is_some() {
        let key = match (state.testing_connection, state.connection_busy, state.connection_result) {
            (true, true, _) => "options.aiTestRunning",
            (true, false, Some(true)) => "options.aiTestSuccess",
            (true, false, _) => "options.aiTestFailed",
            (false, true, _) => "options.settingsSaving",
            (false, false, Some(true)) => "options.settingsSaved",
            (false, false, _) => "options.aiSaveFailed",
        };
        rows.push(option(i18n, key, None, space().into()));
    }
    Group { title: i18n.t("options.aiServiceProviders").to_owned(), bare: true, rows, foot: None }
}

fn loading(i18n: &I18n) -> Group<'_, crate::Message> {
    Group { title: String::new(), bare: true, rows: vec![option(i18n, "ui.loading", None, space().into())], foot: None }
}
