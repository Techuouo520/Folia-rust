// The sheets the player panel opens over the window: the audio effects window, the theme quick editor and
// the lyric file window. Each keeps its own draft; the shell applies what a sheet hands it.

use crate::typeface::text;
use crate::typeface::text_input;
use folia_core::{EQUALIZER_MAX_GAIN_DB, EQUALIZER_MIN_GAIN_DB, EqualizerSettings, Rgb, SOUND_PRESETS};
use folia_i18n::I18n;
use folia_store::DualTheme;
use iced::widget::{button, column, container, mouse_area, row, space};
use iced::{Alignment, Background, Border, Color, Element, Length, Size, Theme as IcedTheme};
use lucide_static_svg::Icon;

use crate::icons::icon;
use crate::settings_kit::{self, Faded};
use crate::typeface;

#[derive(Debug, Clone)]
pub enum Message {
    EqualizerToggle(bool),
    EqualizerPreset(String),
    EqualizerBand(usize, f32),
    EqualizerBandDone,
    EqualizerEffect(usize, f32),
    EqualizerEffectDone,
    EqualizerReset,
    EqualizerClose,
    EditorSide(bool),
    EditorColor(usize),
    EditorSwatch(Rgb),
    EditorName(String),
    EditorSave,
    EditorClose,
    LyricClose,
    LyricImport,
    LyricExport,
}

/// What a sheet hands the shell.
#[derive(Debug, Clone)]
pub enum Event {
    None,
    Equalizer(EqualizerSettings),
    SaveTheme(DualTheme),
    ImportLyrics,
    ExportLyrics,
}

#[derive(Debug, Clone, Default)]
pub struct Sheets {
    pub equalizer: bool,
    pub lyric: bool,
    pub editor: Option<Editor>,
    draft: Option<EqualizerSettings>,
}

#[derive(Debug, Clone)]
pub struct Editor {
    pub draft: DualTheme,
    pub light: bool,
    pub color: usize,
    pub name: String,
}

/// The editor's swatches: the two defaults' colours, then a spread of hues.
const SWATCHES: [u32; 12] = [
    0x09090b, 0xf4f4f5, 0xf5f5f4, 0x1c1917, 0xea580c, 0xdc2626, 0x16a34a, 0x2563eb, 0x9333ea, 0x0891b2, 0xd97706,
    0x64748b,
];

impl Sheets {
    pub fn open_equalizer(&mut self, settings: &EqualizerSettings) {
        self.draft = Some(settings.clone());
        self.equalizer = true;
        self.editor = None;
        self.lyric = false;
    }

    pub fn open_editor(&mut self, theme: Option<&DualTheme>, daylight: bool) {
        let draft = theme.cloned().unwrap_or_default();
        self.editor = Some(Editor { name: draft.light.name.clone(), draft, light: daylight, color: 2 });
        self.equalizer = false;
        self.lyric = false;
    }

    pub fn open_lyric_file(&mut self) {
        self.lyric = true;
        self.equalizer = false;
        self.editor = None;
    }

    pub fn close_all(&mut self) {
        self.equalizer = false;
        self.lyric = false;
        self.editor = None;
    }

    pub fn any_open(&self) -> bool {
        self.equalizer || self.lyric || self.editor.is_some()
    }

    /// Applies a sheet's message; `saved` is the settings already stored.
    pub fn update(&mut self, message: Message, saved: &EqualizerSettings) -> Event {
        let draft = self.draft.get_or_insert_with(|| saved.clone());
        match message {
            Message::EqualizerToggle(on) => {
                draft.enabled = on;
                Event::Equalizer(draft.clone().clamped())
            }
            Message::EqualizerPreset(id) => {
                if let Some(preset) = folia_core::preset(&id) {
                    draft.enabled = true;
                    draft.preset = id;
                    draft.gains = preset.gains;
                    draft.effects = preset.effects;
                } else if let Some(slot) = custom_slot(&id) {
                    draft.enabled = true;
                    draft.preset = id;
                    draft.gains = draft.custom_slots[slot].gains;
                    draft.effects = draft.custom_slots[slot].effects;
                }
                Event::Equalizer(draft.clone().clamped())
            }
            Message::EqualizerBand(index, value) => {
                if let Some(gain) = draft.gains.get_mut(index) {
                    *gain = value.clamp(EQUALIZER_MIN_GAIN_DB, EQUALIZER_MAX_GAIN_DB);
                }
                draft.enabled = true;
                Event::None
            }
            Message::EqualizerBandDone => {
                let slot = custom_index(&draft.preset).unwrap_or(0);
                draft.preset =
                    if custom_index(&draft.preset).is_some() { draft.preset.clone() } else { "custom1".to_owned() };
                draft.custom_slots[slot].gains = draft.gains;
                Event::Equalizer(draft.clone().clamped())
            }
            Message::EqualizerEffect(index, value) => {
                if let Some(effect) = effect_field(&mut draft.effects, index) {
                    *effect = value;
                }
                draft.enabled = true;
                Event::None
            }
            Message::EqualizerEffectDone => {
                let slot = custom_index(&draft.preset).unwrap_or(0);
                draft.preset =
                    if custom_index(&draft.preset).is_some() { draft.preset.clone() } else { "custom1".to_owned() };
                draft.custom_slots[slot].effects = draft.effects;
                Event::Equalizer(draft.clone().clamped())
            }
            Message::EqualizerReset => {
                let slot = custom_index(&draft.preset).unwrap_or(0);
                draft.preset = format!("custom{}", slot + 1);
                draft.gains = SOUND_PRESETS[0].gains;
                draft.effects = SOUND_PRESETS[0].effects;
                draft.custom_slots[slot].gains = draft.gains;
                draft.custom_slots[slot].effects = draft.effects;
                Event::Equalizer(draft.clone().clamped())
            }
            Message::EqualizerClose => {
                self.equalizer = false;
                self.draft = None;
                Event::None
            }
            Message::LyricClose => {
                self.lyric = false;
                Event::None
            }
            Message::LyricImport => Event::ImportLyrics,
            Message::LyricExport => Event::ExportLyrics,
            Message::EditorClose => {
                self.editor = None;
                Event::None
            }
            Message::EditorSide(light) => {
                if let Some(editor) = &mut self.editor {
                    editor.light = light;
                }
                Event::None
            }
            Message::EditorColor(index) => {
                if let Some(editor) = &mut self.editor {
                    editor.color = index;
                }
                Event::None
            }
            Message::EditorSwatch(rgb) => {
                if let Some(editor) = &mut self.editor {
                    let side = if editor.light { &mut editor.draft.light } else { &mut editor.draft.dark };
                    match editor.color {
                        0 => side.background = rgb,
                        1 => side.primary = rgb,
                        2 => side.accent = rgb,
                        _ => side.secondary = rgb,
                    }
                }
                Event::None
            }
            Message::EditorName(name) => {
                if let Some(editor) = &mut self.editor {
                    editor.name = name;
                }
                Event::None
            }
            Message::EditorSave => {
                let Some(mut editor) = self.editor.take() else { return Event::None };
                let name = editor.name.trim().to_owned();
                if !name.is_empty() {
                    editor.draft.light.name = name.clone();
                    editor.draft.dark.name = name;
                }
                Event::SaveTheme(editor.draft)
            }
        }
    }
}

pub struct Look<'a> {
    pub i18n: &'a I18n,
    pub daylight: bool,
    pub window: Size,
    pub has_lyrics: bool,
    pub tokens: settings_kit::Tokens,
}

pub fn view<'a>(sheets: &'a Sheets, look: Look<'a>) -> Option<Element<'a, Message>> {
    if let Some(draft) = sheets.draft.as_ref().filter(|_| sheets.equalizer) {
        return Some(equalizer(draft, &look));
    }
    if let Some(editor) = &sheets.editor {
        return Some(theme_editor(editor, &look));
    }
    sheets.lyric.then(|| lyric_file(&look))
}

fn sheet<'a>(
    look: &Look<'a>,
    width: f32,
    height: f32,
    title: String,
    body: Element<'a, Message>,
    close: Message,
) -> Element<'a, Message> {
    let tokens = Faded::new(look.tokens, look.tokens.t1, look.tokens.accent, look.daylight, 1.0);
    let closing = mouse_area(container(space()).width(Length::Fill).height(Length::Fill)).on_release(close.clone());
    let head = row![
        text(title)
            .size(crate::typeface::size::SUBHEADING)
            .font(crate::typeface::adapt(typeface::font(iced::font::Weight::Semibold)))
            .color(look.tokens.t1),
        space().width(Length::Fill),
        button(container(icon(Icon::X, 18.0, look.tokens.t2)).center(Length::Fixed(32.0)))
            .padding(0)
            .on_press(close)
            .style(button::text),
    ]
    .align_y(Alignment::Center);
    let card = container(column![head, body].spacing(14))
        .width(Length::Fixed(width))
        .height(Length::Fixed(height))
        .padding(20)
        .style(move |_theme: &IcedTheme| container::Style {
            background: Some(tokens.win.into()),
            border: Border { radius: 16.0.into(), width: 1.0, color: tokens.line },
            shadow: iced::Shadow {
                color: Color { a: 0.5, ..Color::BLACK },
                offset: iced::Vector::new(0.0, 30.0),
                blur_radius: 80.0,
            },
            ..container::Style::default()
        });
    let dim =
        container(closing).width(Length::Fill).height(Length::Fill).style(|_theme: &IcedTheme| container::Style {
            background: Some(Background::Color(Color { a: 0.5, ..Color::BLACK })),
            ..container::Style::default()
        });
    // `opaque`: a press on the card's empty space stops here instead of reaching the scrim and closing.
    let centered = container(iced::widget::opaque(card))
        .width(Length::Fill)
        .height(Length::Fill)
        .align_x(Alignment::Center)
        .align_y(Alignment::Center);
    iced::widget::stack![dim, centered].into()
}

fn equalizer<'a>(draft: &'a EqualizerSettings, look: &Look<'a>) -> Element<'a, Message> {
    crate::equalizer_dialog::view(
        draft,
        &crate::equalizer_dialog::Look {
            i18n: look.i18n,
            tokens: look.tokens,
            daylight: look.daylight,
            window: look.window,
        },
    )
}

fn theme_editor<'a>(editor: &'a Editor, look: &Look<'a>) -> Element<'a, Message> {
    let i18n = look.i18n;
    let tokens = Faded::new(look.tokens, look.tokens.t1, look.tokens.accent, look.daylight, 1.0);
    let side = if editor.light { &editor.draft.light } else { &editor.draft.dark };
    let mut sides = row![].spacing(0);
    for (light, key) in [(true, "options.daylightModeLight"), (false, "options.daylightModeDark")] {
        let chosen = editor.light == light;
        sides = sides.push(
            button(
                container(text(i18n.t(key)).size(crate::typeface::size::CONTROL).color(if chosen {
                    tokens.t1
                } else {
                    tokens.t2
                }))
                .center_x(Length::Fill)
                .center_y(Length::Fill),
            )
            .width(Length::Fill)
            .height(Length::Fill)
            .padding(0)
            .on_press(Message::EditorSide(light))
            .style(move |_theme: &IcedTheme, _status| button::Style {
                background: chosen.then_some(Background::Color(tokens.raise)),
                border: Border { radius: 6.0.into(), ..Border::default() },
                ..button::Style::default()
            }),
        );
    }
    let sides =
        container(sides.height(Length::Fill)).width(Length::Fixed(200.0)).height(Length::Fixed(32.0)).padding(2).style(
            move |_theme: &IcedTheme| container::Style {
                background: Some(tokens.fill.into()),
                border: Border { radius: 8.0.into(), ..Border::default() },
                ..container::Style::default()
            },
        );
    let names = [
        "themePark.colors.background",
        "themePark.colors.primary",
        "themePark.colors.accent",
        "themePark.colors.secondary",
    ];
    let colors = [side.background, side.primary, side.accent, side.secondary];
    let mut rows = column![].spacing(6);
    for (index, name) in names.iter().enumerate() {
        rows = rows.push(
            row![
                text(i18n.t(name)).size(crate::typeface::size::CONTROL).color(tokens.t2).width(Length::Fixed(120.0)),
                button(container(space()).width(Length::Fixed(22.0)).height(Length::Fixed(22.0)))
                    .padding(0)
                    .style(move |_theme: &IcedTheme, _status| button::Style {
                        background: Some(Background::Color(Color::from_rgb8(
                            colors[index].r,
                            colors[index].g,
                            colors[index].b
                        ))),
                        border: Border {
                            radius: 6.0.into(),
                            width: if editor.color == index { 2.0 } else { 1.0 },
                            color: tokens.line
                        },
                        ..button::Style::default()
                    })
                    .on_press(Message::EditorColor(index)),
                text(format!("#{:02x}{:02x}{:02x}", colors[index].r, colors[index].g, colors[index].b))
                    .size(crate::typeface::size::CAPTION)
                    .color(tokens.t3),
            ]
            .spacing(10)
            .align_y(Alignment::Center),
        );
    }
    let mut swatches: Vec<Element<'a, Message>> = Vec::new();
    for value in SWATCHES {
        let rgb = Rgb::hex(value);
        swatches.push(
            button(container(space()).width(Length::Fixed(24.0)).height(Length::Fixed(24.0)))
                .padding(0)
                .on_press(Message::EditorSwatch(rgb))
                .style(move |_theme: &IcedTheme, _status| button::Style {
                    background: Some(Background::Color(Color::from_rgb8(rgb.r, rgb.g, rgb.b))),
                    border: Border { radius: 6.0.into(), width: 1.0, color: tokens.line },
                    ..button::Style::default()
                })
                .into(),
        );
    }
    let swatches = row(swatches).spacing(6).wrap();
    let name = text_input(i18n.t("themePark.namePlaceholder"), &editor.name)
        .on_input(Message::EditorName)
        .on_submit(Message::EditorSave)
        .size(crate::typeface::size::CONTROL)
        .padding(8)
        .style(move |_theme: &IcedTheme, _status| iced::widget::text_input::Style {
            background: Background::Color(tokens.fill),
            border: Border { radius: 8.0.into(), width: 1.0, color: tokens.line },
            icon: tokens.t3,
            placeholder: tokens.t3,
            value: tokens.t1,
            selection: tokens.fill2,
        });
    let buttons = row![
        quiet(tokens, i18n.t("ui.cancel"), Message::EditorClose),
        primary(tokens, i18n.t("options.customThemeQuickEditSave"), Message::EditorSave),
    ]
    .spacing(8)
    .align_y(Alignment::Center);
    let body = column![
        row![
            sides,
            space().width(Length::Fill),
            text(i18n.t("options.customTheme")).size(crate::typeface::size::HINT).color(tokens.t3)
        ]
        .align_y(Alignment::Center),
        rows,
        swatches,
        name,
        container(buttons).width(Length::Fill).align_x(Alignment::End),
    ]
    .spacing(14);
    sheet(look, 460.0, 430.0, i18n.t("options.customThemeQuickEditTitle").to_owned(), body.into(), Message::EditorClose)
}

fn lyric_file<'a>(look: &Look<'a>) -> Element<'a, Message> {
    let i18n = look.i18n;
    let tokens = Faded::new(look.tokens, look.tokens.t1, look.tokens.accent, look.daylight, 1.0);
    let import = row_button(
        tokens,
        i18n.t("lyricFile.importChoose"),
        Some(i18n.t("lyricFile.importFormats")),
        Message::LyricImport,
    );
    let export = row_button(
        tokens,
        i18n.t("lyricExport.formatLrc"),
        Some(i18n.t("lyricExport.formatLrcDesc")),
        Message::LyricExport,
    );
    let no_lyrics = text(i18n.t("lyricExport.noLyrics")).size(crate::typeface::size::HINT).color(tokens.t3);
    let body = column![
        text(i18n.t("lyricFile.description")).size(crate::typeface::size::HINT).color(tokens.t3),
        section_label(i18n.t("lyricFile.importHeading"), tokens),
        import,
        section_label(i18n.t("lyricFile.exportHeading"), tokens),
        export,
        if look.has_lyrics { Element::from(space().height(0)) } else { no_lyrics.into() },
        container(quiet(tokens, i18n.t("ui.close"), Message::LyricClose)).width(Length::Fill).align_x(Alignment::End),
    ]
    .spacing(10);
    sheet(look, 420.0, 330.0, i18n.t("lyricFile.title").to_owned(), body.into(), Message::LyricClose)
}

fn row_button<'a>(tokens: Faded, title: &'a str, hint: Option<&'a str>, message: Message) -> Element<'a, Message> {
    let mut body = column![text(title).size(crate::typeface::size::CONTROL).color(tokens.t1)].spacing(2);
    if let Some(hint) = hint {
        body = body.push(text(hint).size(crate::typeface::size::CAPTION).color(tokens.t3));
    }
    button(body)
        .width(Length::Fill)
        .padding(10)
        .on_press(message)
        .style(move |_theme: &IcedTheme, status| {
            let hot = matches!(status, button::Status::Hovered | button::Status::Pressed);
            button::Style {
                background: Some(Background::Color(if hot { tokens.fill2 } else { tokens.fill })),
                border: Border { radius: 8.0.into(), ..Border::default() },
                ..button::Style::default()
            }
        })
        .into()
}

fn quiet<'a>(tokens: Faded, label: &'a str, message: Message) -> Element<'a, Message> {
    button(text(label).size(crate::typeface::size::CONTROL).color(tokens.t2))
        .padding(iced::Padding { top: 8.0, right: 12.0, bottom: 8.0, left: 12.0 })
        .on_press(message)
        .style(move |_theme: &IcedTheme, status| {
            let hot = matches!(status, button::Status::Hovered | button::Status::Pressed);
            button::Style {
                background: hot.then_some(Background::Color(tokens.fill)),
                border: Border { radius: 8.0.into(), ..Border::default() },
                text_color: tokens.t2,
                ..button::Style::default()
            }
        })
        .into()
}

fn primary<'a>(tokens: Faded, label: &'a str, message: Message) -> Element<'a, Message> {
    button(text(label).size(crate::typeface::size::CONTROL).color(tokens.on_accent))
        .padding(iced::Padding { top: 8.0, right: 14.0, bottom: 8.0, left: 14.0 })
        .on_press(message)
        .style(move |_theme: &IcedTheme, _status| button::Style {
            background: Some(Background::Color(tokens.accent)),
            border: Border { radius: 8.0.into(), ..Border::default() },
            ..button::Style::default()
        })
        .into()
}

fn section_label<'a>(label: &'a str, tokens: Faded) -> Element<'a, Message> {
    text(label)
        .size(crate::typeface::size::HINT)
        .font(crate::typeface::adapt(typeface::font(iced::font::Weight::Semibold)))
        .color(tokens.t3)
        .into()
}

fn custom_index(preset: &str) -> Option<usize> {
    match preset {
        "custom1" => Some(0),
        "custom2" => Some(1),
        _ => None,
    }
}

fn custom_slot(preset: &str) -> Option<usize> {
    custom_index(preset)
}

fn effect_field(effects: &mut folia_core::AudioEffects, index: usize) -> Option<&mut f32> {
    match index {
        0 => Some(&mut effects.highpass),
        1 => Some(&mut effects.lowpass),
        2 => Some(&mut effects.drive),
        3 => Some(&mut effects.crush),
        4 => Some(&mut effects.wow),
        5 => Some(&mut effects.noise),
        6 => Some(&mut effects.width),
        7 => Some(&mut effects.space),
        _ => Some(&mut effects.punch),
    }
}
