// Custom source settings run imports and persistence off the UI thread and show only localized failure summaries.

use std::path::PathBuf;

use folia_i18n::I18n;
use folia_omni::{
    Omni,
    lx::{SourceSettings, UpdateAlert},
};
use folia_store::Store;
use iced::widget::{button, column, container, mouse_area, opaque, row, scrollable, stack, text, text_input};
use iced::{Alignment, Border, Color, Element, Length, Task};

use crate::settings_kit::{self as kit, Group, Row, Tokens};

#[derive(Debug, Clone)]
pub enum Message {
    Import,
    ImportUrl,
    UrlChanged(String),
    PasteUrl,
    ClipboardUrl(Option<String>),
    Enable(String, bool),
    Move(String, usize),
    Delete(String),
    Finished(SourceSettings, Option<&'static str>, Vec<(String, UpdateAlert)>),
    OpenUpdate(String),
    DismissUpdates,
    Poll,
}

#[derive(Default)]
pub struct Sources {
    url: String,
    settings: SourceSettings,
    error: Option<&'static str>,
    busy: bool,
    alerts: Vec<(String, UpdateAlert)>,
}

impl Sources {
    pub fn restore(&mut self, omni: Option<Omni>, store: Store, data: PathBuf) -> Task<crate::Message> {
        let Some(omni) = omni else { return Task::none() };
        self.busy = true;
        run(omni, move |omni| omni.restore_sources(store, data).map(|_| ()))
    }

    pub fn update(&mut self, message: Message, omni: Option<Omni>) -> Task<crate::Message> {
        match message {
            Message::Poll => {
                if let Some(omni) = omni {
                    self.alerts.extend(omni.source_update_alerts());
                }
                return Task::none();
            }
            Message::Finished(settings, error, alerts) => {
                self.settings = settings;
                self.error = error;
                self.alerts.extend(alerts);
                self.busy = false;
                return Task::none();
            }
            Message::DismissUpdates => {
                self.alerts.clear();
                return Task::none();
            }
            Message::OpenUpdate(url) => {
                return Task::future(crate::background::blocking(move || {
                    if let Err(err) = open::that(url) {
                        tracing::warn!(error = &err as &dyn std::error::Error, "cannot open source update page");
                    }
                }))
                .discard();
            }
            _ if self.busy => return Task::none(),
            _ => {}
        }
        match message {
            Message::UrlChanged(url) => {
                self.url = url;
                self.error = None;
                return Task::none();
            }
            Message::PasteUrl => {
                return iced::clipboard::read().map(|value| crate::Message::Sources(Message::ClipboardUrl(value)));
            }
            Message::ClipboardUrl(value) => {
                match value.filter(|text| !text.trim().is_empty()) {
                    Some(text) => {
                        self.url = text.trim().to_owned();
                        self.error = None;
                    }
                    None => self.error = Some("options.sourceClipboardEmpty"),
                }
                return Task::none();
            }
            _ => {}
        }
        let Some(omni) = omni else { return Task::none() };
        self.busy = true;
        self.error = None;
        if matches!(message, Message::ImportUrl) {
            let url = self.url.clone();
            return Task::perform(
                async move {
                    let result = omni.import_source_url(&url).await.map(|_| ());
                    finished(&omni, result)
                },
                crate::Message::Sources,
            );
        }
        run(omni, move |omni| match message {
            Message::Import => {
                if let Some(path) = rfd::FileDialog::new().add_filter("JavaScript", &["js"]).pick_file() {
                    omni.import_source(&path)?;
                }
                Ok(())
            }
            Message::Enable(id, enabled) => omni.enable_source(&id, enabled).map(|_| ()),
            Message::Move(id, index) => omni.move_source(&id, index).map(|_| ()),
            Message::Delete(id) => omni.remove_source(&id).map(|_| ()),
            _ => Ok(()),
        })
    }

    pub fn polls(&self) -> bool {
        !self.settings.enabled.is_empty()
    }

    pub fn has_alert(&self) -> bool {
        !self.alerts.is_empty()
    }

    pub fn group<'a>(
        &'a self,
        tokens: Tokens,
        group: Group<'a, crate::Message>,
        first: bool,
    ) -> Element<'a, crate::Message> {
        let draggable = if !self.busy && self.alerts.is_empty() { 2..2 + self.settings.scripts.len() } else { 0..0 };
        kit::group_with_rows(tokens, group, first, |rows| {
            crate::drag_to_reorder(
                rows,
                draggable,
                tokens.accent,
                tokens.win,
                move |from, to| {
                    crate::Message::Sources(
                        self.settings
                            .scripts
                            .get(from.saturating_sub(2))
                            .map_or(Message::Poll, |source| Message::Move(source.id.clone(), to.saturating_sub(2))),
                    )
                },
                |y| {
                    crate::Message::Settings(crate::settings::Message::Wheel(iced::mouse::ScrollDelta::Pixels {
                        x: 0.0,
                        y,
                    }))
                },
            )
        })
    }

    pub fn alert<'a>(&self, i18n: &'a I18n, tokens: Tokens) -> Option<Element<'a, crate::Message>> {
        let (id, alert) = self.alerts.first()?;
        let name = self.settings.scripts.iter().find(|s| &s.id == id).map(|s| s.metadata.name.as_str()).unwrap_or("");
        let mut buttons =
            row![control(i18n, tokens, "options.sourceUpdateDismiss", Message::DismissUpdates, true)].spacing(8);
        if let Some(url) = &alert.update_url {
            buttons =
                buttons.push(control(i18n, tokens, "options.sourceUpdateOpen", Message::OpenUpdate(url.clone()), true));
        }
        let card = container(
            column![
                text(i18n.format("options.sourceUpdate", &[("name", name)])).size(18).color(tokens.t1),
                scrollable(text(alert.log.clone()).size(14).color(tokens.t2)).height(Length::Shrink),
                buttons,
            ]
            .spacing(16),
        )
        .padding(24)
        .width(520)
        .max_height(480)
        .style(move |_| container::Style {
            background: Some(tokens.win.into()),
            border: Border { radius: 16.0.into(), color: tokens.line, width: 1.0 },
            ..Default::default()
        });
        let scrim = mouse_area(container(iced::widget::space()).width(Length::Fill).height(Length::Fill).style(|_| {
            container::Style { background: Some(Color::from_rgba(0.0, 0.0, 0.0, 0.5).into()), ..Default::default() }
        }))
        .on_press(crate::Message::Sources(Message::DismissUpdates));
        Some(
            stack![
                scrim,
                container(opaque(card))
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .align_x(Alignment::Center)
                    .align_y(Alignment::Center)
            ]
            .into(),
        )
    }

    pub fn groups<'a>(&'a self, i18n: &'a I18n, tokens: Tokens) -> Vec<Group<'a, crate::Message>> {
        let input = text_input(i18n.t("options.sourceUrlPlaceholder"), &self.url)
            .on_input_maybe((!self.busy).then_some(|value| crate::Message::Sources(Message::UrlChanged(value))))
            .on_submit_maybe(
                (!self.busy && !self.url.trim().is_empty()).then_some(crate::Message::Sources(Message::ImportUrl)),
            )
            .size(13)
            .padding([9, 10])
            .style(move |_, status| text_input::Style {
                background: tokens.fill.into(),
                border: Border {
                    radius: 8.0.into(),
                    width: 1.0,
                    color: if matches!(status, text_input::Status::Focused { .. }) {
                        tokens.accent
                    } else {
                        tokens.line
                    },
                },
                icon: tokens.t2,
                placeholder: tokens.t3,
                value: tokens.t1,
                selection: tokens.fill2,
            });
        let mut rows = vec![
            Row {
                label: i18n.t("options.sourceImportUrl").into(),
                hint: Some(i18n.t(if self.busy { "options.sourceWorking" } else { "options.sourceUrlHint" }).into()),
                sub: false,
                below: Some(
                    row![
                        input,
                        control(
                            i18n,
                            tokens,
                            "options.sourceImportButton",
                            Message::ImportUrl,
                            !self.busy && !self.url.trim().is_empty()
                        )
                    ]
                    .spacing(8)
                    .into(),
                ),
                control: control(i18n, tokens, "options.sourcePasteUrl", Message::PasteUrl, !self.busy),
            },
            Row {
                label: i18n.t("options.sourceImport").into(),
                hint: Some(i18n.t(if self.busy { "options.sourceWorking" } else { "options.sourceImportHint" }).into()),
                sub: false,
                below: None,
                control: control(i18n, tokens, "options.sourceImportButton", Message::Import, !self.busy),
            },
        ];
        for source in &self.settings.scripts {
            let enabled = self.settings.enabled.contains(&source.id);
            let platforms = source
                .sources
                .iter()
                .map(
                    |(id, platform)| {
                        if platform.name.is_empty() { id.clone() } else { format!("{} ({id})", platform.name) }
                    },
                )
                .collect::<Vec<_>>()
                .join(", ");
            let details = i18n.format(
                "options.sourceDetails",
                &[
                    ("version", &source.metadata.version),
                    ("author", &source.metadata.author),
                    ("platforms", &platforms),
                ],
            );
            let status = i18n.t(if enabled { "options.sourceEnabled" } else { "options.sourceDisabled" });
            let hint = format!("{status} · {details}");
            let enable = Message::Enable(source.id.clone(), !enabled);
            let controls = row![
                control(
                    i18n,
                    tokens,
                    if enabled { "options.sourceDisable" } else { "options.sourceEnable" },
                    enable,
                    !self.busy
                ),
                control(i18n, tokens, "options.sourceDelete", Message::Delete(source.id.clone()), !self.busy),
            ]
            .spacing(6)
            .into();
            rows.push(Row {
                label: source.metadata.name.clone(),
                hint: Some(hint),
                sub: false,
                below: None,
                control: controls,
            });
        }
        for (id, alert) in &self.alerts {
            let name =
                self.settings.scripts.iter().find(|s| &s.id == id).map(|s| s.metadata.name.as_str()).unwrap_or("");
            let control = if let Some(url) = &alert.update_url {
                control(i18n, tokens, "options.sourceUpdateOpen", Message::OpenUpdate(url.clone()), true)
            } else {
                control(i18n, tokens, "options.sourceUpdateDismiss", Message::DismissUpdates, true)
            };
            rows.push(Row {
                label: i18n.format("options.sourceUpdate", &[("name", name)]),
                hint: Some(alert.log.clone()),
                sub: false,
                below: None,
                control,
            });
        }
        vec![Group {
            title: i18n.t("options.customSources").into(),
            bare: true,
            rows,
            foot: self.error.map(|key| i18n.t(key).to_owned()).or_else(|| {
                Some(
                    i18n.t(if self.settings.scripts.is_empty() {
                        "options.sourceEmpty"
                    } else {
                        "options.sourcePriorityHint"
                    })
                    .to_owned(),
                )
            }),
        }]
    }
}

fn control<'a>(
    i18n: &'a I18n,
    tokens: Tokens,
    key: &str,
    message: Message,
    enabled: bool,
) -> Element<'a, crate::Message> {
    let primary = enabled && matches!(message, Message::ImportUrl);
    button(text(i18n.t(key).to_owned()).size(12).color(if primary { tokens.on_accent } else { tokens.t1 }))
        .padding([7, 10])
        .style(move |_, _| kit::rounded_button(if primary { tokens.accent } else { tokens.fill }, 8.0))
        .on_press_maybe(enabled.then_some(crate::Message::Sources(message)))
        .into()
}

fn run(
    omni: Omni,
    work: impl FnOnce(&Omni) -> Result<(), folia_omni::lx::SourceError> + Send + 'static,
) -> Task<crate::Message> {
    Task::perform(crate::background::blocking(move || finished(&omni, work(&omni))), |result| {
        crate::Message::Sources(result.unwrap_or_else(|| {
            Message::Finished(SourceSettings::default(), Some("options.sourceErrorScript"), Vec::new())
        }))
    })
}

fn finished(omni: &Omni, result: Result<(), folia_omni::lx::SourceError>) -> Message {
    let error = result.err().map(|err| {
        tracing::warn!(error = &err as &dyn std::error::Error, "custom source operation failed");
        err.label_key()
    });
    Message::Finished(omni.custom_sources(), error, omni.source_update_alerts())
}
