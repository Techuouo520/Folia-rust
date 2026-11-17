// Full theme editor with independent AI/custom drafts, all theme fields and the shared live preview.

use folia_core::{Rgb, Theme, WordColor};
use folia_i18n::I18n;
use folia_store::{DualTheme, StoredTheme};
use iced::widget::{button, column, container, mouse_area, opaque, row, scrollable, space, stack, text_editor};
use iced::{Alignment, Border, Color, Element, Length, Shadow, Vector};
use lucide_static_svg::Icon;

use crate::settings_kit::{self as kit, Group, Row, Tokens};
use crate::typeface::{text, text_input};
use crate::workbench::{self, Workbench};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Colors,
    Details,
    Content,
    Ai,
}

#[derive(Debug, Clone)]
pub enum Message {
    Close,
    Target(bool),
    Side(bool),
    Tab(Tab),
    Color(usize),
    Swatch(Rgb),
    Hex(String),
    Name(bool, String),
    Description(bool, text_editor::Action),
    AddWord,
    Word(usize, String),
    WordColor(usize, String),
    RemoveWord(usize),
    IconDraft(String),
    AddIcon,
    RemoveIcon(usize),
    Json(text_editor::Action),
    Import,
    CopyJson,
    CopyPrompt,
    Reset,
    Save,
    Frame,
}

pub enum Event {
    None,
    Close,
    Copy(String),
    Save { custom: bool, theme: Box<DualTheme>, song: Option<String> },
}

pub struct ThemePark {
    drafts: [DualTheme; 2],
    original: [DualTheme; 2],
    custom: bool,
    light: bool,
    tab: Tab,
    color: usize,
    hex: String,
    icon_draft: String,
    json: text_editor::Content,
    descriptions: [text_editor::Content; 2],
    error: Option<&'static str>,
    song: Option<String>,
    prompt: String,
    pub preview: Workbench,
    pub preview_theme: Theme,
}

impl ThemePark {
    pub fn open(
        song_theme: Option<&DualTheme>,
        custom_theme: Option<&DualTheme>,
        custom: bool,
        light: bool,
        song: Option<String>,
        prompt: String,
    ) -> Self {
        let drafts =
            [song_theme.cloned().unwrap_or_else(folia_ai::default_theme), custom_theme.cloned().unwrap_or_default()];
        let mut park = Self {
            original: drafts.clone(),
            drafts,
            custom,
            light,
            tab: Tab::Colors,
            color: 2,
            hex: String::new(),
            icon_draft: String::new(),
            json: text_editor::Content::new(),
            descriptions: [text_editor::Content::new(), text_editor::Content::new()],
            error: None,
            song,
            prompt,
            preview: Workbench::open(),
            preview_theme: Theme::DAYLIGHT,
        };
        park.refresh();
        park.sync_descriptions();
        park
    }

    fn draft(&self) -> &DualTheme {
        &self.drafts[usize::from(self.custom)]
    }
    fn draft_mut(&mut self) -> &mut DualTheme {
        &mut self.drafts[usize::from(self.custom)]
    }
    fn side(&self) -> &StoredTheme {
        if self.light { &self.draft().light } else { &self.draft().dark }
    }
    fn side_mut(&mut self) -> &mut StoredTheme {
        if self.light { &mut self.draft_mut().light } else { &mut self.draft_mut().dark }
    }

    fn refresh(&mut self) {
        self.preview_theme = self.side().theme(self.preview_theme.intensity);
        let color = self.colors()[self.color];
        self.hex = hex(color);
    }

    fn colors(&self) -> [Rgb; 4] {
        let side = self.side();
        [side.background, side.primary, side.accent, side.secondary]
    }

    fn valid(&self) -> bool {
        [&self.draft().light.name, &self.draft().dark.name]
            .iter()
            .all(|name| !name.trim().is_empty() && name.trim().encode_utf16().count() <= 32)
    }

    pub fn update(&mut self, message: Message) -> Event {
        self.error = None;
        match message {
            Message::Close => return Event::Close,
            Message::Target(custom) => {
                self.custom = custom;
                self.sync_descriptions();
            }
            Message::Side(light) => self.light = light,
            Message::Tab(tab) => self.tab = tab,
            Message::Color(color) => self.color = color.min(3),
            Message::Swatch(color) => self.set_color(color),
            Message::Hex(value) => {
                self.hex = value;
                if let Some(color) = folia_core::parse_color(&self.hex) {
                    self.set_color(color);
                    self.preview_theme = self.side().theme(self.preview_theme.intensity);
                }
                return Event::None;
            }
            Message::Name(light, value) => {
                let side = if light { &mut self.draft_mut().light } else { &mut self.draft_mut().dark };
                side.name = limited(&value, 32);
            }
            Message::Description(light, action) => {
                let index = usize::from(!light);
                self.descriptions[index].perform(action);
                let text = self.descriptions[index].text();
                let value = limited(&text, 120);
                if text != value {
                    self.descriptions[index] = text_editor::Content::with_text(&value);
                }
                let side = if light { &mut self.draft_mut().light } else { &mut self.draft_mut().dark };
                side.description = Some(value);
            }
            Message::AddWord if self.draft().dark.word_colors.len() < 40 => {
                let color = hex(self.side().accent);
                let draft = self.draft_mut();
                for side in [&mut draft.light, &mut draft.dark] {
                    side.word_colors.push(WordColor { word: String::new(), color: color.clone() });
                }
            }
            Message::Word(index, value) => self.edit_word(index, Some(limited(&value, 256)), None),
            Message::WordColor(index, color) => self.edit_word(index, None, Some(color)),
            Message::RemoveWord(index) => {
                let draft = self.draft_mut();
                for side in [&mut draft.light, &mut draft.dark] {
                    if index < side.word_colors.len() {
                        side.word_colors.remove(index);
                    }
                }
            }
            Message::IconDraft(value) => self.icon_draft = limited(&value, 80),
            Message::AddIcon => self.add_icon(),
            Message::RemoveIcon(index) => {
                let draft = self.draft_mut();
                for side in [&mut draft.light, &mut draft.dark] {
                    if index < side.lyrics_icons.len() {
                        side.lyrics_icons.remove(index);
                    }
                }
            }
            Message::Json(action) => self.json.perform(action),
            Message::Import => match folia_ai::parse_theme(&self.json.text()) {
                Ok(theme) => {
                    *self.draft_mut() = theme;
                    self.sync_descriptions();
                    self.json = text_editor::Content::new();
                }
                Err(_) => self.error = Some("options.invalidJsonFormat"),
            },
            Message::CopyJson => match folia_ai::theme_json(self.draft()) {
                Ok(json) => return Event::Copy(json),
                Err(error) => {
                    tracing::warn!(error = &error as &dyn std::error::Error, "theme JSON unavailable");
                    self.error = Some("options.invalidJsonFormat");
                }
            },
            Message::CopyPrompt => return Event::Copy(self.prompt.clone()),
            Message::Reset => {
                if self.custom {
                    let defaults = DualTheme::default();
                    let draft = self.draft_mut();
                    for (side, default) in [(&mut draft.light, defaults.light), (&mut draft.dark, defaults.dark)] {
                        side.background = default.background;
                        side.primary = default.primary;
                        side.accent = default.accent;
                        side.secondary = default.secondary;
                    }
                } else {
                    self.drafts[0] = self.original[0].clone();
                }
                self.sync_descriptions();
            }
            Message::Save if self.valid() && (self.custom || self.song.is_some()) => {
                let theme = folia_ai::theme_json(self.draft()).and_then(|json| folia_ai::parse_theme(&json));
                match theme {
                    Ok(theme) => {
                        return Event::Save { custom: self.custom, theme: Box::new(theme), song: self.song.clone() };
                    }
                    Err(_) => self.error = Some("options.invalidJsonFormat"),
                }
            }
            _ => {}
        }
        self.refresh();
        Event::None
    }

    fn set_color(&mut self, color: Rgb) {
        let index = self.color;
        let side = self.side_mut();
        match index {
            0 => side.background = color,
            1 => side.primary = color,
            2 => side.accent = color,
            _ => side.secondary = color,
        }
    }

    fn sync_descriptions(&mut self) {
        self.descriptions = [
            text_editor::Content::with_text(self.draft().light.description.as_deref().unwrap_or("")),
            text_editor::Content::with_text(self.draft().dark.description.as_deref().unwrap_or("")),
        ];
    }

    fn edit_word(&mut self, index: usize, word: Option<String>, color: Option<String>) {
        let draft = self.draft_mut();
        for side in [&mut draft.light, &mut draft.dark] {
            if let Some(entry) = side.word_colors.get_mut(index) {
                if let Some(word) = &word {
                    entry.word.clone_from(word);
                }
                if let Some(color) = &color {
                    entry.color.clone_from(color);
                }
            }
        }
    }

    fn add_icon(&mut self) {
        let candidate = self.icon_draft.trim();
        let normalized: String =
            candidate.chars().filter(char::is_ascii_alphanumeric).flat_map(char::to_lowercase).collect();
        let valid = Icon::all().iter().any(|icon| {
            icon.name().chars().filter(char::is_ascii_alphanumeric).flat_map(char::to_lowercase).eq(normalized.chars())
        });
        if !valid {
            self.error = Some("theme.lyricsIconUnknown");
            return;
        }
        if self.draft().dark.lyrics_icons.len() >= 12 {
            return;
        }
        let name = candidate.to_owned();
        let draft = self.draft_mut();
        for side in [&mut draft.light, &mut draft.dark] {
            if !side.lyrics_icons.contains(&name) {
                side.lyrics_icons.push(name.clone());
            }
        }
        self.icon_draft.clear();
    }

    pub fn view<'a>(
        &'a self,
        mut look: workbench::Look<'a>,
        app_theme: &'a Theme,
        daylight: bool,
    ) -> Element<'a, crate::Message> {
        let tokens = Tokens::new(app_theme, daylight);
        let i18n = look.i18n;
        let size = look.window;
        look.theme = &self.preview_theme;
        look.daylight = self.light;
        let preview = self.preview.preview_only(look);
        let target = kit::segmented(
            tokens,
            vec![(false, i18n.t("theme.targetAiTheme").into()), (true, i18n.t("theme.targetCustomTheme").into())],
            self.custom,
            None,
            |custom| app(Message::Target(custom)),
        );
        let header = row![
            kit::icon_button(tokens, Icon::ChevronLeft, 18.0, app(Message::Close)),
            text(i18n.t("theme.themePark")).size(20).font(crate::typeface::adapt(kit::SEMIBOLD)).color(tokens.t1),
            target,
            space().width(Length::Fill),
            action(tokens, i18n.t("ui.resetToDefaultTheme"), Some(Message::Reset)),
            action(
                tokens,
                i18n.t(if self.custom { "options.saveAndApplyCustomTheme" } else { "options.aiThemeQuickEditSave" }),
                (self.valid() && (self.custom || self.song.is_some())).then_some(Message::Save)
            ),
        ]
        .spacing(8)
        .padding([12, 16])
        .align_y(Alignment::Center);
        let tabs = [
            (Tab::Colors, "theme.tabColors"),
            (Tab::Details, "theme.tabDetails"),
            (Tab::Content, "theme.tabContent"),
            (Tab::Ai, "theme.tabAi"),
        ];
        let index = tabs.iter().position(|(tab, _)| *tab == self.tab).unwrap_or(0);
        let tabs = kit::tabs(
            tokens,
            tabs.into_iter().map(|(tab, key)| (i18n.t(key).into(), app(Message::Tab(tab)))).collect(),
            Some(index),
            Some(index as f32),
            8.0,
            24.0,
        );
        let controls = self.controls(i18n, tokens, look.cover);
        let controls = scrollable(container(controls).padding([0, 24]))
            .direction(kit::scrollbar())
            .style(kit::scroll_style(tokens))
            .height(Length::Fill);
        let editor = column![tabs, controls].height(Length::Fill);
        let body: Element<'a, crate::Message> = if size.width < 1024.0 {
            scrollable(column![
                container(preview).height((size.height * 0.4).max(150.0)),
                container(editor).height(420)
            ])
            .direction(kit::scrollbar())
            .style(kit::scroll_style(tokens))
            .into()
        } else {
            row![preview, container(editor).width(420)].height(Length::Fill).into()
        };
        let window = container(column![header, body])
            .width((size.width - 24.0).clamp(320.0, 1240.0))
            .height((size.height - 24.0).clamp(260.0, 780.0))
            .style(move |_| container::Style {
                background: Some(tokens.win.into()),
                border: Border { color: tokens.line2, width: 1.0, radius: 16.0.into() },
                shadow: Shadow {
                    color: Color { a: 0.5, ..Color::BLACK },
                    offset: Vector::new(0.0, 36.0),
                    blur_radius: 90.0,
                },
                ..Default::default()
            });
        let veil = container(space()).width(Length::Fill).height(Length::Fill).style(move |_| container::Style {
            background: Some(Color { a: if daylight { 0.36 } else { 0.52 }, ..Color::BLACK }.into()),
            ..Default::default()
        });
        stack![opaque(mouse_area(veil).on_press(app(Message::Close))), container(opaque(window)).center(Length::Fill)]
            .into()
    }

    fn controls<'a>(&'a self, i18n: &'a I18n, tokens: Tokens, swatches: &'a [Rgb]) -> Element<'a, crate::Message> {
        let groups = match self.tab {
            Tab::Colors => return self.color_controls(i18n, tokens, swatches),
            Tab::Details => self.details(i18n, tokens),
            Tab::Content => self.content(i18n, tokens),
            Tab::Ai => self.ai_controls(i18n, tokens),
        };
        let mut body =
            column(groups.into_iter().enumerate().map(|(index, group)| kit::group(tokens, group, index == 0)));
        if let Some(error) = self.error {
            body = body.push(text(i18n.t(error)).size(12).color(tokens.t2));
        }
        body.padding(iced::Padding::default().bottom(24.0)).into()
    }

    fn color_controls<'a>(&'a self, i18n: &'a I18n, tokens: Tokens, swatches: &[Rgb]) -> Element<'a, crate::Message> {
        let side = kit::segmented(
            tokens,
            vec![(true, i18n.t("options.lightTheme").into()), (false, i18n.t("options.darkTheme").into())],
            self.light,
            None,
            |light| app(Message::Side(light)),
        );
        let labels = ["theme.bgColor", "theme.primaryColor", "theme.accentColor", "theme.secondaryColor"];
        let wells = row(self.colors().into_iter().enumerate().map(|(index, color)| {
            let color = folia_visualizer::color(color, 1.0);
            let tile = container(space()).width(62).height(48).style(move |_| container::Style {
                background: Some(color.into()),
                border: Border {
                    width: if self.color == index { 2.0 } else { 1.0 },
                    color: if self.color == index { tokens.accent } else { tokens.line },
                    radius: 8.0.into(),
                },
                ..Default::default()
            });
            button(column![tile, text(i18n.t(labels[index])).size(11).color(tokens.t2)])
                .padding(2)
                .style(|_, _| button::Style::default())
                .on_press(app(Message::Color(index)))
                .into()
        }))
        .spacing(8);
        let picker = crate::color_picker::view(
            self.colors()[self.color],
            |color| app(Message::Swatch(color)),
            app(Message::Frame),
        );
        let field = text_input("#RRGGBB", &self.hex)
            .on_input(|value| app(Message::Hex(value)))
            .size(13)
            .padding(8)
            .style(move |_, _| kit::input_style(tokens));
        let swatches = row(swatches.iter().copied().take(6).map(|color| {
            button(space().width(30).height(24))
                .style(move |_, _| kit::rounded_button(folia_visualizer::color(color, 1.0), 5.0))
                .on_press(app(Message::Swatch(color)))
                .into()
        }))
        .spacing(8);
        column![side, wells, picker, field, swatches].spacing(16).padding([20, 0]).into()
    }

    fn details<'a>(&'a self, i18n: &'a I18n, tokens: Tokens) -> Vec<Group<'a, crate::Message>> {
        [(true, &self.draft().light), (false, &self.draft().dark)]
            .into_iter()
            .map(|(light, side)| {
                let name = text_input(i18n.t("options.themeName"), &side.name)
                    .on_input(move |value| app(Message::Name(light, value)))
                    .style(move |_, _| kit::input_style(tokens))
                    .size(13)
                    .padding(8)
                    .width(168)
                    .into();
                let description = text_editor(&self.descriptions[usize::from(!light)])
                    .font(crate::typeface::adapt(crate::typeface::SYSTEM))
                    .placeholder(i18n.t("theme.themeDescriptionPlaceholder"))
                    .on_action(move |action| app(Message::Description(light, action)))
                    .style(move |_, _| kit::editor_style(tokens))
                    .size(13)
                    .padding(8)
                    .height(84)
                    .into();
                Group {
                    title: i18n.t(if light { "options.lightTheme" } else { "options.darkTheme" }).into(),
                    bare: false,
                    rows: vec![
                        option(i18n, "options.themeName", name),
                        Row {
                            label: i18n.t("theme.themeDescription").into(),
                            hint: None,
                            sub: false,
                            below: Some(description),
                            control: kit::value(
                                tokens,
                                format!("{} / 120", side.description.as_deref().unwrap_or("").encode_utf16().count()),
                            ),
                        },
                    ],
                    foot: (!self.valid()).then(|| i18n.t("theme.themeNameRequired").into()),
                }
            })
            .collect()
    }

    fn content<'a>(&'a self, i18n: &'a I18n, tokens: Tokens) -> Vec<Group<'a, crate::Message>> {
        let mut words = vec![option(
            i18n,
            "theme.wordColors",
            action(
                tokens,
                i18n.t("theme.addWordColor"),
                (self.draft().dark.word_colors.len() < 40).then_some(Message::AddWord),
            ),
        )];
        for (index, entry) in self.draft().dark.word_colors.iter().enumerate() {
            let controls = row![
                text_input(i18n.t("theme.wordColorWordPlaceholder"), &entry.word)
                    .on_input(move |value| app(Message::Word(index, value)))
                    .style(move |_, _| kit::input_style(tokens))
                    .size(13)
                    .padding(8),
                text_input("#RRGGBB", &entry.color)
                    .on_input(move |value| app(Message::WordColor(index, value)))
                    .style(move |_, _| kit::input_style(tokens))
                    .size(12)
                    .padding(8)
                    .width(92),
                kit::icon_button(tokens, Icon::Trash, 15.0, app(Message::RemoveWord(index)))
            ]
            .spacing(6)
            .into();
            words.push(Row {
                label: String::new(),
                hint: None,
                sub: false,
                below: Some(controls),
                control: space().into(),
            });
        }
        let mut icons = vec![Row {
            label: String::new(),
            hint: None,
            sub: false,
            below: Some(
                row![
                    text_input(i18n.t("theme.lyricsIconPlaceholder"), &self.icon_draft)
                        .on_input(|value| app(Message::IconDraft(value)))
                        .style(move |_, _| kit::input_style(tokens))
                        .size(13)
                        .padding(8),
                    kit::icon_button(tokens, Icon::Plus, 16.0, app(Message::AddIcon))
                ]
                .spacing(8)
                .into(),
            ),
            control: space().into(),
        }];
        for (index, name) in self.draft().dark.lyrics_icons.iter().enumerate() {
            icons.push(Row {
                label: name.clone(),
                hint: None,
                sub: false,
                below: None,
                control: kit::icon_button(tokens, Icon::X, 14.0, app(Message::RemoveIcon(index))),
            });
        }
        vec![
            Group {
                title: i18n.t("theme.wordColors").into(),
                bare: true,
                rows: words,
                foot: Some(i18n.t("theme.wordColorsDesc").into()),
            },
            Group {
                title: i18n.t("theme.lyricsIcons").into(),
                bare: false,
                rows: icons,
                foot: Some(i18n.t("theme.lyricsIconsDesc").into()),
            },
        ]
    }

    fn ai_controls<'a>(&'a self, i18n: &'a I18n, tokens: Tokens) -> Vec<Group<'a, crate::Message>> {
        let editor = text_editor(&self.json)
            .font(crate::typeface::adapt(crate::typeface::SYSTEM))
            .on_action(|action| app(Message::Json(action)))
            .size(12)
            .height(200)
            .style(move |_, _| kit::editor_style(tokens))
            .into();
        let mut import = option(
            i18n,
            "aiHelp.importJsonTitle",
            action(tokens, i18n.t("theme.importJsonToDraft"), Some(Message::Import)),
        );
        import.below = Some(editor);
        vec![
            Group {
                title: String::new(),
                bare: true,
                rows: vec![
                    option(
                        i18n,
                        "aiHelp.copyPromptTitle",
                        action(tokens, i18n.t("aiHelp.copyPrompt"), Some(Message::CopyPrompt)),
                    ),
                    import,
                ],
                foot: Some(i18n.t("theme.importJsonToDraftHint").into()),
            },
            Group {
                title: String::new(),
                bare: true,
                rows: vec![option(
                    i18n,
                    "theme.exportDraftJson",
                    action(tokens, i18n.t("options.copyThemeJson"), Some(Message::CopyJson)),
                )],
                foot: None,
            },
        ]
    }
}

fn app(message: Message) -> crate::Message {
    crate::Message::ThemePark(message)
}
fn hex(color: Rgb) -> String {
    format!("#{:02X}{:02X}{:02X}", color.r, color.g, color.b)
}
fn limited(value: &str, limit: usize) -> String {
    value
        .chars()
        .scan(0, |count, ch| {
            *count += ch.len_utf16();
            (*count <= limit).then_some(ch)
        })
        .collect()
}
fn option<'a>(i18n: &I18n, label: &str, control: Element<'a, crate::Message>) -> Row<'a, crate::Message> {
    Row { label: i18n.t(label).into(), hint: None, sub: false, below: None, control }
}
fn action<'a>(tokens: Tokens, label: &'a str, message: Option<Message>) -> Element<'a, crate::Message> {
    button(text(label).size(13).color(tokens.t1))
        .padding([7, 10])
        .style(move |_, _| kit::rounded_button(tokens.fill, 8.0))
        .on_press_maybe(message.map(app))
        .into()
}
