// The lyric-style workbench opened by More settings: a preview, the style shelf, and the style's own rows.

use std::time::Instant;

use folia_core::{Lyrics, Theme};
use folia_i18n::I18n;
use folia_visualizer::{Item, Manifest, PREVIEW_LOOP, Pictures, RESERVED_LOOP, Scene, Style, Values, lyric_view};
use iced::widget::{
    button, column, container, mouse_area, opaque, row, scrollable, slider, space, stack, text, tooltip,
};
use iced::{Alignment, Background, Border, Color, Element, Length, Padding, Shadow, Size, Vector};
use lucide_static_svg::Icon;

use crate::icons::{Variant, icon, variant};
use crate::settings_kit::{self as kit, Group, Row, Tokens};

const WINDOW_W: f32 = 1240.0;
const WINDOW_H: f32 = 780.0;
const COLUMN: f32 = 420.0;
const HEADER: f32 = 64.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Source {
    Builtin,
    Live,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sample {
    Default,
    Reserved,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tab {
    Lyrics,
    Background,
    Font,
    Subtitle,
    Global,
}

impl Tab {
    const ALL: [Tab; 5] = [Tab::Lyrics, Tab::Background, Tab::Font, Tab::Subtitle, Tab::Global];

    fn key(self) -> &'static str {
        match self {
            Tab::Lyrics => "options.previewVisualizerSettings",
            Tab::Background => "options.previewBackgroundSettings",
            Tab::Font => "options.previewCommonSettings",
            Tab::Subtitle => "options.previewSubtitleSettings",
            Tab::Global => "options.previewGlobalSettings",
        }
    }
}

#[derive(Clone, Debug)]
pub enum Message {
    Close,
    Tab(Tab),
    Source(Source),
    Sample(Sample),
    Pause,
    Style(Style),
    Slider(&'static str, f32),
    Commit,
    Toggle(&'static str, bool),
    Member(&'static str, &'static str, bool),
    Choice(&'static str, String),
    Button(&'static str),
    Reset,
    Frame,
    Font(f32),
}

pub enum Effect {
    None,
    Close,
    /// Keep the tuning for the preview, and write the file on [`Effect::Save`].
    Draft,
    Save,
    Style(Style),
    Font(f32),
    Button(&'static str),
}

/// What the open workbench draws from the shell.
#[derive(Clone, Copy)]
pub struct Look<'a> {
    pub i18n: &'a I18n,
    pub theme: &'a Theme,
    pub daylight: bool,
    pub window: Size,
    pub style: Style,
    pub font_scale: f32,
    pub live: Option<(&'a Lyrics, f64)>,
    pub audio: &'a folia_core::AudioFrame,
}

pub struct Workbench {
    tab: Tab,
    source: Source,
    sample: Sample,
    paused: bool,
    origin: Instant,
    frozen: f64,
    pictures: Pictures,
    pub values: Values,
    notice: Option<&'static str>,
}

impl Workbench {
    pub(crate) fn note(&mut self, key: &'static str) {
        self.notice = Some(key);
    }

    pub fn open() -> Self {
        Self {
            tab: Tab::Lyrics,
            source: Source::Builtin,
            sample: Sample::Default,
            paused: false,
            origin: Instant::now(),
            frozen: 0.0,
            pictures: Pictures::default(),
            values: Values::default(),
            notice: None,
        }
    }

    pub fn update(&mut self, message: Message, manifest: Manifest) -> Effect {
        match message {
            Message::Close => Effect::Close,
            Message::Tab(tab) => {
                self.tab = tab;
                Effect::None
            }
            Message::Source(source) => {
                self.frozen = self.clock(Instant::now());
                self.source = source;
                self.origin = Instant::now();
                Effect::None
            }
            Message::Sample(sample) => {
                self.sample = sample;
                Effect::None
            }
            Message::Pause => {
                let now = Instant::now();
                if self.paused {
                    self.origin =
                        now.checked_sub(std::time::Duration::from_secs_f64(self.frozen.max(0.0))).unwrap_or(now);
                    self.paused = false;
                } else {
                    self.frozen = self.clock(now);
                    self.paused = true;
                }
                Effect::None
            }
            Message::Style(style) => {
                self.tab = Tab::Lyrics;
                Effect::Style(style)
            }
            Message::Slider(key, value) => {
                if manifest.locked(key) {
                    return Effect::None;
                }
                self.values.set_number(manifest, key, value);
                Effect::Draft
            }
            Message::Commit => Effect::Save,
            Message::Toggle(key, on) => {
                if manifest.locked(key) {
                    return Effect::None;
                }
                self.values.set_flag(manifest, key, on);
                Effect::Save
            }
            Message::Member(key, option, on) => {
                let _ = self.values.toggle_member(manifest, key, option, on);
                Effect::Save
            }
            Message::Choice(key, value) => {
                if manifest.locked(key) {
                    return Effect::None;
                }
                let allowed = manifest.items().any(|item| match item {
                    Item::Select { key: item_key, options, .. } => {
                        item_key == key
                            && options.iter().any(|choice| {
                                choice.value == value
                                    && choice.requires.is_none_or(|path| self.values.path(path).is_some())
                            })
                    }
                    _ => false,
                });
                if allowed {
                    self.values.set_choice(manifest, key, &value);
                    Effect::Save
                } else {
                    Effect::None
                }
            }
            Message::Button(key) => Effect::Button(key),
            Message::Reset => match self.tab {
                Tab::Lyrics => {
                    self.values.reset(manifest);
                    self.notice = manifest.reset_key;
                    Effect::Save
                }
                Tab::Font => {
                    self.sample = Sample::Default;
                    Effect::Font(1.0)
                }
                Tab::Background | Tab::Subtitle | Tab::Global => Effect::None,
            },
            Message::Frame => Effect::None,
            Message::Font(scale) => Effect::Font(scale.clamp(0.85, 1.4)),
        }
    }

    pub fn ticking(&self) -> bool {
        self.source == Source::Builtin && !self.paused
    }

    fn clock(&self, now: Instant) -> f64 {
        if self.paused {
            return self.frozen;
        }
        now.saturating_duration_since(self.origin).as_secs_f64() % self.loop_seconds()
    }

    fn loop_seconds(&self) -> f64 {
        if self.sample == Sample::Reserved { RESERVED_LOOP } else { PREVIEW_LOOP }
    }

    pub fn view<'a>(&'a self, look: Look<'a>) -> Element<'a, crate::Message> {
        let Look { i18n, theme, daylight, window, style, font_scale, live, .. } = look;
        let tokens = Tokens::new(theme, daylight);
        let now = Instant::now();
        let (lyrics, time, empty) = self.shown(now, live);
        let header = self.header(i18n, tokens);
        let stage = self.stage(look, tokens, lyrics, time, empty);
        let column = self.column(i18n, tokens, theme, daylight, style, font_scale);
        let body = row![stage, container(column).width(COLUMN).height(Length::Fill)].height(Length::Fill);
        let inner = column![header, body].width(Length::Fill).height(Length::Fill);
        let width = WINDOW_W.min(window.width - 24.0).max(360.0);
        let height = WINDOW_H.min(window.height - 24.0).max(280.0);
        let panel = container(inner).width(width).height(height).style(move |_| container::Style {
            background: Some(tokens.win.into()),
            border: Border { color: tokens.line2, width: 1.0, radius: 16.0.into() },
            shadow: Shadow {
                color: Color { a: 0.5, ..Color::BLACK },
                offset: Vector::new(0.0, 36.0),
                blur_radius: 90.0,
            },
            ..container::Style::default()
        });
        let panel = container(opaque(panel))
            .width(Length::Fill)
            .height(Length::Fill)
            .center_x(Length::Fill)
            .center_y(Length::Fill);
        let veil = if daylight { 0.36 } else { 0.52 };
        let backdrop = container(space()).width(Length::Fill).height(Length::Fill).style(move |_| container::Style {
            background: Some(Color { a: veil, ..Color::BLACK }.into()),
            ..container::Style::default()
        });
        let backdrop = opaque(mouse_area(backdrop).on_press(crate::Message::Workbench(Message::Close)));
        stack![backdrop, panel].into()
    }

    fn shown<'a>(&'a self, now: Instant, live: Option<(&'a Lyrics, f64)>) -> (&'a Lyrics, f64, bool) {
        if self.source == Source::Live
            && let Some((lyrics, time)) = live
        {
            let shown = if self.paused { self.frozen } else { time };
            return (lyrics, shown, false);
        }
        let lyrics = if self.sample == Sample::Reserved { reserved() } else { builtin() };
        (lyrics, self.clock(now), self.source == Source::Live)
    }

    fn header<'a>(&'a self, i18n: &'a I18n, tokens: Tokens) -> Element<'a, crate::Message> {
        let back = button(icon(Icon::ChevronLeft, 18.0, tokens.t2))
            .width(32)
            .height(32)
            .padding(0)
            .on_press(crate::Message::Workbench(Message::Close))
            .style(move |_, status| button::Style {
                background: (status == button::Status::Hovered).then_some(Background::Color(tokens.fill)),
                border: Border { radius: 8.0.into(), ..Border::default() },
                ..button::Style::default()
            });
        let title = text(i18n.t("options.lyricsStyleSettings")).size(20).font(kit::SEMIBOLD).color(tokens.t1);
        container(row![back, title].spacing(8).align_y(Alignment::Center))
            .height(HEADER)
            .padding(Padding { left: 16.0, right: 20.0, ..Padding::ZERO })
            .align_y(Alignment::Center)
            .into()
    }

    fn stage<'a>(
        &'a self,
        look: Look<'a>,
        tokens: Tokens,
        lyrics: &'a Lyrics,
        time: f64,
        empty: bool,
    ) -> Element<'a, crate::Message> {
        let Look { i18n, theme, style, font_scale, .. } = look;
        let scene = Scene {
            style,
            lyrics,
            time,
            theme,
            font_scale,
            seed: "lumiere",
            stage: None,
            tuning: self.values.clone(),
            backdrop: true,
            title: lyrics.title.clone().unwrap_or_default(),
            artist: lyrics.artist.clone().unwrap_or_default(),
            album: String::new(),
            portrait: None,
            preview: true,
            audio: look.audio,
        };
        let preview =
            container(lyric_view(scene)).width(Length::Fill).height(Length::Fill).clip(true).style(move |_| {
                container::Style {
                    background: Some(Background::Color(folia_visualizer::color(theme.background, 1.0))),
                    border: Border { color: tokens.line, width: 1.0, radius: 12.0.into() },
                    ..container::Style::default()
                }
            });
        let note = empty.then(|| -> Element<'a, crate::Message> {
            container(text(i18n.t("options.previewSourceLiveEmpty")).size(13).color(Color::WHITE)).padding(8).into()
        });
        let bar = self.preview_bar(i18n, tokens);
        let picture = stack![preview, container(bar).width(Length::Fill).align_x(Alignment::End).padding(12)];
        let picture = match note {
            Some(note) => picture.push(container(note).padding(12)),
            None => picture,
        };
        let shelf = self.shelf(look, tokens);
        column![picture, shelf]
            .spacing(16)
            .padding(Padding { left: 16.0, right: 16.0, bottom: 24.0, top: 0.0 })
            .width(Length::Fill)
            .into()
    }

    fn preview_bar<'a>(&'a self, i18n: &'a I18n, _tokens: Tokens) -> Element<'a, crate::Message> {
        let plate = Color { a: 0.45, ..Color::BLACK };
        let ink = Color::WHITE;
        let builtin = self.source == Source::Builtin;
        let source = row![
            source_button(i18n.t("options.previewSourceBuiltin"), builtin, Message::Source(Source::Builtin), ink),
            source_button(i18n.t("options.previewSourceLive"), !builtin, Message::Source(Source::Live), ink),
        ];
        let source = container(source).padding(2).style(move |_| container::Style {
            background: Some(plate.into()),
            border: Border { radius: 8.0.into(), ..Border::default() },
            ..container::Style::default()
        });
        // VisPlayground: `grid h-8 w-8 place-items-center rounded-full bg-black/45 hover:bg-black/65 text-white`,
        // a filled lucide Play/Pause at 14px, 16px right of the source switch, with a tooltip.
        let (mark, tip) = if self.paused {
            (Variant::FilledPlay, "options.resumePreview")
        } else {
            (Variant::FilledPause, "options.pausePreview")
        };
        let pause = button(container(variant(mark, 14.0, ink)).center(32))
            .padding(0)
            .on_press(crate::Message::Workbench(Message::Pause))
            .style(move |_, status| button::Style {
                background: Some(Background::Color(Color {
                    a: if matches!(status, button::Status::Hovered | button::Status::Pressed) { 0.65 } else { 0.45 },
                    ..Color::BLACK
                })),
                border: Border { radius: 16.0.into(), ..Border::default() },
                ..button::Style::default()
            });
        let pause = tooltip(pause, text(i18n.t(tip)).size(12), tooltip::Position::Bottom).style(container::rounded_box);
        row![source, pause].spacing(16).align_y(Alignment::Center).into()
    }

    fn shelf<'a>(&'a self, look: Look<'a>, tokens: Tokens) -> Element<'a, crate::Message> {
        let Look { i18n, theme, window, style: current, font_scale, audio, .. } = look;
        let columns: usize = if window.width >= 1024.0 { 7 } else { 4 };
        let count = text(Style::ALL.len().to_string()).size(13).color(tokens.t3);
        let heading = row![text(i18n.t("options.lyricStyles")).size(13).font(kit::SEMIBOLD).color(tokens.t2), count,]
            .spacing(6)
            .align_y(Alignment::Center);
        let lyrics = builtin();
        let mut grid = column![].spacing(12);
        for chunk in Style::ALL.chunks(columns) {
            let mut line = row![].spacing(12);
            for style in chunk {
                let tuning = if *style == current { self.values.clone() } else { Values::default() };
                line = line.push(tile(TileIn {
                    i18n,
                    tokens,
                    theme,
                    lyrics,
                    style: *style,
                    font_scale,
                    chosen: *style == current,
                    pictures: &self.pictures,
                    tuning,
                    audio,
                }));
            }
            // A short last row keeps the grid's column width, as the original CSS grid does.
            for _ in chunk.len()..columns {
                line = line.push(space().width(Length::Fill));
            }
            grid = grid.push(line);
        }
        let grid =
            scrollable(grid).height(Length::Fixed(250.0)).direction(kit::scrollbar()).style(kit::scroll_style(tokens));
        column![heading, grid].spacing(8).into()
    }

    fn column<'a>(
        &'a self,
        i18n: &'a I18n,
        tokens: Tokens,
        _theme: &Theme,
        _daylight: bool,
        style: Style,
        font_scale: f32,
    ) -> Element<'a, crate::Message> {
        let tabs = Tab::ALL
            .into_iter()
            .map(|tab| (i18n.t(tab.key()).to_owned(), crate::Message::Workbench(Message::Tab(tab))))
            .collect();
        let chosen = Tab::ALL.iter().position(|tab| *tab == self.tab);
        let reset_on = matches!(self.tab, Tab::Lyrics | Tab::Font);
        let reset = button(text(i18n.t("ui.default")).size(13).color(if reset_on { tokens.t1 } else { tokens.t3 }))
            .padding(Padding { left: 8.0, right: 8.0, ..Padding::ZERO })
            .style(|_, _| button::Style::default());
        let reset = if reset_on { reset.on_press(crate::Message::Workbench(Message::Reset)) } else { reset };
        let tabs = row![kit::tabs(tokens, tabs, chosen, chosen.map(|index| index as f32), 8.0, 16.0), reset]
            .align_y(Alignment::Center);
        let page = self.page(i18n, tokens, style, font_scale);
        let page = scrollable(page).height(Length::Fill).direction(kit::scrollbar()).style(kit::scroll_style(tokens));
        let mut column = column![tabs];
        if let Some(key) = self.notice {
            column = column.push(text(i18n.t(key)).size(12).color(tokens.t2));
        }
        column.push(page).into()
    }

    fn page<'a>(
        &'a self,
        i18n: &'a I18n,
        tokens: Tokens,
        style: Style,
        font_scale: f32,
    ) -> Element<'a, crate::Message> {
        match self.tab {
            Tab::Lyrics => style_page(i18n, tokens, style.manifest(), &self.values),
            Tab::Background => notice(i18n, tokens, "options.backgroundUnported"),
            Tab::Font => font_page(i18n, tokens, self.sample, font_scale),
            Tab::Subtitle => notice(i18n, tokens, "options.subtitleFixed"),
            Tab::Global => global_page(i18n, tokens),
        }
    }
}

fn item_label(item: Item) -> &'static str {
    match item {
        Item::Slider { label_key, .. }
        | Item::Switch { label_key, .. }
        | Item::Member { label_key, .. }
        | Item::Select { label_key, .. }
        | Item::Button { label_key, .. } => label_key,
    }
}

fn disabled_modes<'a>(tokens: Tokens, modes: Vec<String>) -> Element<'a, crate::Message> {
    let mut row = row![].padding(2).spacing(0);
    for (index, label) in modes.into_iter().enumerate() {
        let on = index == 0;
        let label =
            text(label).size(13).color(if on { tokens.t3 } else { Color { a: tokens.t3.a * 0.7, ..tokens.t3 } });
        row = row.push(container(label).padding(Padding { left: 10.0, right: 10.0, top: 6.0, bottom: 6.0 }));
    }
    container(row)
        .style(move |_| container::Style {
            background: Some(tokens.fill.into()),
            border: Border { radius: 8.0.into(), ..Border::default() },
            ..container::Style::default()
        })
        .into()
}

fn style_page<'a>(
    i18n: &'a I18n,
    tokens: Tokens,
    manifest: Manifest,
    values: &'a Values,
) -> Element<'a, crate::Message> {
    if manifest.groups.is_empty() {
        return container(text(i18n.t("options.lyricStyleNoOptions")).size(13).color(tokens.t3))
            .padding(56)
            .center_x(Length::Fill)
            .into();
    }
    let mut page = column![];
    for (index, group) in manifest.groups.iter().enumerate() {
        let mut rows = Vec::new();
        for item in group.items {
            if manifest.hidden(values, item.key()) {
                continue;
            }
            rows.push(item_row(i18n, tokens, manifest, values, *item));
        }
        page = page.push(kit::group(
            tokens,
            Group { title: i18n.t(group.title_key).to_owned(), bare: false, rows },
            index == 0,
        ));
        if let Some(foot) = group.foot_key {
            page = page.push(text(i18n.t(foot)).size(12).color(tokens.t3));
        }
    }
    page.padding(Padding { left: 24.0, right: 12.0, bottom: 40.0, top: 0.0 }).into()
}

fn item_row<'a>(
    i18n: &'a I18n,
    tokens: Tokens,
    manifest: Manifest,
    values: &'a Values,
    item: Item,
) -> Row<'a, crate::Message> {
    let locked = manifest.locked(item.key());
    let control = match item {
        Item::Slider { key, step, .. } if locked => {
            kit::value(tokens, format_value(values.number(manifest, key), step))
        }
        Item::Slider { key, min, max, step, .. } => {
            let value = values.number(manifest, key);
            let shown = format_value(value, step);
            row![
                slider(min..=max, value, move |next| crate::Message::Workbench(Message::Slider(key, next)))
                    .step(step)
                    .width(160)
                    .on_release(crate::Message::Workbench(Message::Commit))
                    .style(move |_, _| kit::slider_style(tokens)),
                kit::value(tokens, shown),
            ]
            .spacing(8)
            .align_y(Alignment::Center)
            .into()
        }
        Item::Switch { key, .. } if locked => {
            kit::toggle_fixed(tokens, if values.flag(manifest, key) { 1.0 } else { 0.0 })
        }
        Item::Switch { key, .. } => {
            let on = values.flag(manifest, key);
            kit::toggle(tokens, if on { 1.0 } else { 0.0 }, crate::Message::Workbench(Message::Toggle(key, !on)))
        }
        Item::Button { key, path_key, .. } => {
            let enabled = path_key.is_some() || manifest.paths.iter().any(|path| values.path(path).is_some());
            let label = text(i18n.t(item_label(item))).size(13).color(if enabled { tokens.t1 } else { tokens.t3 });
            let button = button(label).padding([6, 10]).style(|_, _| button::Style::default());
            if enabled {
                button.on_press(crate::Message::Workbench(Message::Button(key))).into()
            } else {
                button.into()
            }
        }
        Item::Member { key, option, .. } => {
            let on = values.member(manifest, key, option);
            kit::toggle(
                tokens,
                if on { 1.0 } else { 0.0 },
                crate::Message::Workbench(Message::Member(key, option, !on)),
            )
        }
        Item::Select { key, options, .. } => {
            let current = values.choice(manifest, key);
            let choices = options.iter().map(|choice| (choice.value, i18n.t(choice.label_key).to_owned())).collect();
            kit::segmented(tokens, choices, current, None, |value| {
                crate::Message::Workbench(Message::Choice(key, value.to_owned()))
            })
        }
    };
    let hint = match item {
        Item::Button { path_key: Some(path), .. } => values
            .path(path)
            .and_then(|path| std::path::Path::new(path).file_name())
            .map(|name| name.to_string_lossy().into_owned()),
        Item::Slider { hint_key: Some(hint), .. }
        | Item::Switch { hint_key: Some(hint), .. }
        | Item::Select { hint_key: Some(hint), .. } => Some(i18n.t(hint).to_owned()),
        _ => None,
    };
    let label = i18n.t(item_label(item)).to_owned();
    Row { label, hint, sub: false, control }
}

fn font_page<'a>(i18n: &'a I18n, tokens: Tokens, sample: Sample, font_scale: f32) -> Element<'a, crate::Message> {
    let samples = vec![
        (Sample::Default, i18n.t("options.previewTextDefault").to_owned()),
        (Sample::Reserved, i18n.t("options.previewTextReserved").to_owned()),
    ];
    let sample_row = Row {
        label: i18n.t("options.previewText").to_owned(),
        hint: None,
        sub: false,
        control: kit::segmented(tokens, samples, sample, None, |sample| {
            crate::Message::Workbench(Message::Sample(sample))
        }),
    };
    let scale = Row {
        label: i18n.t("options.fontSize").to_owned(),
        hint: None,
        sub: false,
        control: row![
            slider(0.85_f32..=1.4, font_scale, |value| crate::Message::Workbench(Message::Font(value)))
                .step(0.05_f32)
                .width(160)
                .style(move |_, _| kit::slider_style(tokens)),
            kit::value(tokens, format!("{}%", (font_scale * 100.0).round())),
        ]
        .spacing(8)
        .align_y(Alignment::Center)
        .into(),
    };
    kit::group(
        tokens,
        Group { title: i18n.t("options.previewCommonSettings").to_owned(), bare: true, rows: vec![sample_row, scale] },
        true,
    )
}

fn global_page<'a>(i18n: &'a I18n, tokens: Tokens) -> Element<'a, crate::Message> {
    let modes = vec![
        ("off", i18n.t("options.smartBeatOff").to_owned()),
        ("cached", i18n.t("options.smartBeatCached").to_owned()),
        ("on", i18n.t("options.smartBeatOn").to_owned()),
    ];
    let mode = Row {
        label: i18n.t("options.smartBeatMode").to_owned(),
        hint: Some(i18n.t("options.smartBeatDesc_off").to_owned()),
        sub: false,
        control: disabled_modes(tokens, modes.into_iter().map(|(_, label)| label).collect()),
    };
    let locked = text(i18n.t("options.smartBeatUnavailable")).size(12).color(tokens.t3);
    column![
        kit::group(
            tokens,
            Group { title: i18n.t("options.smartBeat").to_owned(), bare: false, rows: vec![mode] },
            true
        ),
        locked,
    ]
    .padding(Padding { left: 24.0, right: 12.0, bottom: 40.0, top: 0.0 })
    .into()
}

fn notice<'a>(i18n: &'a I18n, tokens: Tokens, key: &'static str) -> Element<'a, crate::Message> {
    container(text(i18n.t(key)).size(13).color(tokens.t2))
        .padding(Padding { top: 28.0, left: 24.0, right: 16.0, bottom: 16.0 })
        .into()
}

struct TileIn<'a> {
    i18n: &'a I18n,
    tokens: Tokens,
    theme: &'a Theme,
    lyrics: &'a Lyrics,
    style: Style,
    font_scale: f32,
    chosen: bool,
    pictures: &'a Pictures,
    tuning: Values,
    audio: &'a folia_core::AudioFrame,
}

fn tile<'a>(tile: TileIn<'a>) -> Element<'a, crate::Message> {
    let TileIn { i18n, tokens, theme, lyrics, style, font_scale, chosen, pictures, tuning, audio } = tile;
    let scene = Scene {
        style,
        lyrics,
        time: folia_visualizer::POSTER_TIME,
        theme,
        font_scale,
        seed: "style-picture-lumiere",
        stage: Some(Size::new(folia_visualizer::STAGE_WIDTH, folia_visualizer::STAGE_HEIGHT)),
        tuning,
        backdrop: true,
        title: lyrics.title.clone().unwrap_or_default(),
        artist: lyrics.artist.clone().unwrap_or_default(),
        album: String::new(),
        portrait: None,
        preview: true,
        audio,
    };
    let picture = container(iced::widget::canvas(pictures.frozen(scene)).width(Length::Fill).height(Length::Fill))
        .width(Length::Fill)
        .height(Length::Fill)
        .clip(true)
        .style(move |_| container::Style {
            background: Some(Background::Color(tokens.pv)),
            border: Border { color: tokens.line, width: 1.0, radius: 8.0.into() },
            ..container::Style::default()
        });
    let frame = container(picture).width(Length::Fill).style(move |_| container::Style {
        border: Border {
            color: if chosen { tokens.accent } else { Color::TRANSPARENT },
            width: if chosen { 2.0 } else { 0.0 },
            radius: 11.0.into(),
        },
        ..container::Style::default()
    });
    let name = text(i18n.t(style.label_key())).size(12).color(if chosen { tokens.t1 } else { tokens.t2 });
    let name = if chosen { name.font(kit::SEMIBOLD) } else { name };
    let body = column![frame.height(Length::Fixed(64.0)), container(name).center_x(Length::Fill)]
        .spacing(6)
        .width(Length::Fill);
    button(body)
        .padding(3)
        .width(Length::Fill)
        .on_press(crate::Message::Workbench(Message::Style(style)))
        .style(|_, _| button::Style::default())
        .into()
}

fn source_button<'a>(label: &'a str, on: bool, message: Message, ink: Color) -> Element<'a, crate::Message> {
    let label = text(label).size(13).color(Color { a: if on { 1.0 } else { 0.72 }, ..ink });
    let button = button(container(label).center_y(28).padding(Padding { left: 10.0, right: 10.0, ..Padding::ZERO }))
        .padding(0)
        .style(move |_, _| button::Style {
            background: on.then_some(Background::Color(Color { a: 0.22, ..Color::WHITE })),
            border: Border { radius: 6.0.into(), ..Border::default() },
            ..button::Style::default()
        });
    if on { button.into() } else { button.on_press(crate::Message::Workbench(message)).into() }
}

fn format_value(value: f32, step: f32) -> String {
    if step >= 1.0 {
        format!("{}", value.round() as i32)
    } else if step >= 0.1 {
        format!("{value:.1}")
    } else {
        format!("{value:.2}")
    }
}

fn builtin() -> &'static Lyrics {
    use std::sync::OnceLock;
    static LYRICS: OnceLock<Lyrics> = OnceLock::new();
    LYRICS.get_or_init(folia_visualizer::preview_lyrics)
}

fn reserved() -> &'static Lyrics {
    use std::sync::OnceLock;
    static LYRICS: OnceLock<Lyrics> = OnceLock::new();
    LYRICS.get_or_init(folia_visualizer::preview_reserved)
}
