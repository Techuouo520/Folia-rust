// The lyric-style workbench opened by More settings: a preview with its three hotspots, the style shelf, and the
// five settings tabs of the original `VisPlayground`: the style's tuning, backgrounds, fonts, subtitles and global.

use std::time::{Duration, Instant};

use folia_core::{Lyrics, Theme};
use folia_i18n::I18n;
use folia_store::{AppearanceSettings, FontSettings, FontStyle, RowSource, SubtitleContentMode};
use folia_visualizer::{
    Background, Item, Manifest, PREVIEW_LOOP, Pictures, RESERVED_LOOP, Scene, Style, Values, lyric_view,
};
use iced::widget::canvas::{self, Frame, Geometry, Text};
use iced::widget::scrollable::Viewport;
use iced::widget::{
    Id, button, canvas as canvas_widget, column, container, mouse_area, opaque, row, scrollable, slider, space, stack,
    text, text_input, tooltip,
};
use iced::{
    Alignment, Background as IcedBackground, Border, Color, Element, Font, Length, Padding, Point, Rectangle, Renderer,
    Shadow, Size, Task, Vector, mouse,
};
use lucide_static_svg::Icon;

use crate::fonts;
use crate::icons::{Variant, icon, variant};
use crate::settings_kit::{self as kit, Choice, Group, Row, Tokens};
use crate::smooth_scroll::{self, Scroll};

const WINDOW_W: f32 = 1240.0;
const WINDOW_H: f32 = 780.0;
const COLUMN: f32 = 420.0;
const HEADER: f32 = 64.0;
/// The style shelf and the settings body keep their own offsets.
const GRID: &str = "workbench-styles";
const PAGE: &str = "workbench-page";
const MEDIUM: Font = crate::typeface::font(iced::font::Weight::Medium);
const HOTSPOT_FADE: Duration = Duration::from_millis(200);

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

/// Which face the font picker edits.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FontTarget {
    Lyrics,
    Subtitle,
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
    Scale(FontTarget, f32),
    FontStyle(FontStyle),
    Picker(Option<FontTarget>),
    PickFont(String),
    ClearFont(FontTarget),
    FontSearch(String),
    Fallback(String),
    FallbackCommit,
    WeightAuto(FontTarget, bool),
    Weight(FontTarget, f32),
    Opacity(f32),
    BackgroundMode(Background),
    Vignette(bool),
    SubtitleRows(u8),
    SubtitleRow(usize, RowSource),
    SubtitleMode(SubtitleContentMode),
    SubtitleOpacity(f32),
    SubtitleBackground(bool),
    SubtitleUpcomingBlur(bool),
    SubtitleHide(bool),
    SubtitleInherits(bool),
    GridScrolled(Viewport),
    PageScrolled(Viewport),
    GridWheel(mouse::ScrollDelta),
    PageWheel(mouse::ScrollDelta),
}

/// What the open workbench asks the shell to do.
pub enum Effect {
    None,
    Close,
    /// Keep the tuning for the preview, and write the file on [`Effect::Save`].
    Draft,
    /// Write the lyric style's tuning and the appearance settings.
    Save,
    /// Keep the appearance for the preview; the file waits for release.
    Appearance,
    /// The settings body scrolled back to the top.
    ScrollTop,
    Style(Style),
    Button(&'static str),
}

/// What one message changed: an effect, and the appearance to keep when the message changed it.
pub struct Update {
    pub effect: Effect,
    pub appearance: Option<AppearanceSettings>,
}

impl Update {
    fn effect(effect: Effect) -> Self {
        Self { effect, appearance: None }
    }

    fn appearance(effect: Effect, appearance: &AppearanceSettings) -> Self {
        Self { effect, appearance: Some(appearance.clone()) }
    }
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
    pub appearance: &'a AppearanceSettings,
    pub fonts: &'a [String],
}

/// The font picker opened over the workbench.
struct Picker {
    target: FontTarget,
    search: String,
    fallback: String,
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
    grid: Scroll,
    page: Scroll,
    /// A wheel set an offset this update; the scrollables need the new offset now.
    scroll_dirty: bool,
    picker: Option<Picker>,
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
            grid: Scroll::default(),
            page: Scroll::default(),
            scroll_dirty: false,
            picker: None,
        }
    }

    pub fn update(&mut self, message: Message, manifest: Manifest, current: &AppearanceSettings) -> Update {
        let mut appearance = current.clone();
        let mut changed = false;
        let result = match message {
            Message::Close => Update::effect(Effect::Close),
            Message::Tab(tab) => {
                let changed = self.tab != tab;
                self.tab = tab;
                if changed { Update::effect(Effect::ScrollTop) } else { Update::effect(Effect::None) }
            }
            Message::Source(source) => {
                self.frozen = self.clock(Instant::now());
                self.source = source;
                self.origin = Instant::now();
                Update::effect(Effect::None)
            }
            Message::Sample(sample) => {
                self.sample = sample;
                Update::effect(Effect::None)
            }
            Message::Pause => {
                let now = Instant::now();
                if self.paused {
                    self.origin = now.checked_sub(Duration::from_secs_f64(self.frozen.max(0.0))).unwrap_or(now);
                    self.paused = false;
                } else {
                    self.frozen = self.clock(now);
                    self.paused = true;
                }
                Update::effect(Effect::None)
            }
            Message::Style(style) => {
                self.tab = Tab::Lyrics;
                Update::effect(Effect::Style(style))
            }
            Message::Slider(key, value) => {
                if manifest.locked(key) {
                    return Update::effect(Effect::None);
                }
                self.values.set_number(manifest, key, value);
                Update::effect(Effect::Draft)
            }
            Message::Commit => Update::effect(Effect::Save),
            Message::Toggle(key, on) => {
                if manifest.locked(key) {
                    return Update::effect(Effect::None);
                }
                self.values.set_flag(manifest, key, on);
                Update::effect(Effect::Save)
            }
            Message::Member(key, option, on) => {
                let _ = self.values.toggle_member(manifest, key, option, on);
                Update::effect(Effect::Save)
            }
            Message::Choice(key, value) => {
                if manifest.locked(key) {
                    return Update::effect(Effect::None);
                }
                let allowed = manifest.items().any(|item| match item {
                    Item::Select { key: item_key, options, .. } => {
                        item_key == key
                            && options.iter().any(|choice| {
                                choice.value == value
                                    && choice.requires.is_none_or(|path| self.values.path(path).is_some())
                            })
                    }
                    Item::Picture { key: item_key, options, .. } => {
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
                    Update::effect(Effect::Save)
                } else {
                    Update::effect(Effect::None)
                }
            }
            Message::Button(key) => Update::effect(Effect::Button(key)),
            Message::Reset => match self.tab {
                Tab::Lyrics => {
                    self.values.reset(manifest);
                    self.notice = manifest.reset_key;
                    Update::effect(Effect::Save)
                }
                Tab::Background => {
                    appearance.set_background_mode(None);
                    appearance.disable_visualizer_vignette = false;
                    changed = true;
                    Update::effect(Effect::Save)
                }
                Tab::Font => {
                    self.sample = Sample::Default;
                    appearance.lyrics_font = FontSettings::default();
                    appearance.lyrics_font_scale = 1.0;
                    appearance.visualizer_opacity = 1.0;
                    changed = true;
                    Update::effect(Effect::Save)
                }
                Tab::Subtitle => {
                    appearance.subtitle = Default::default();
                    changed = true;
                    Update::effect(Effect::Save)
                }
                Tab::Global => Update::effect(Effect::None),
            },
            Message::Frame => Update::effect(Effect::None),
            Message::GridScrolled(viewport) => {
                self.grid.scrolled(&viewport, Instant::now());
                Update::effect(Effect::None)
            }
            Message::PageScrolled(viewport) => {
                self.page.scrolled(&viewport, Instant::now());
                Update::effect(Effect::None)
            }
            Message::GridWheel(delta) => {
                self.grid.wheel(delta, Instant::now());
                self.scroll_dirty = true;
                Update::effect(Effect::None)
            }
            Message::PageWheel(delta) => {
                self.page.wheel(delta, Instant::now());
                self.scroll_dirty = true;
                Update::effect(Effect::None)
            }
            Message::Scale(target, value) => {
                match target {
                    FontTarget::Lyrics => appearance.lyrics_font_scale = value.clamp(0.85, 1.4),
                    FontTarget::Subtitle => appearance.subtitle.font_scale = value.clamp(0.85, 1.4),
                }
                changed = true;
                Update::effect(Effect::Appearance)
            }
            Message::FontStyle(style) => {
                appearance.lyrics_font.style = style;
                if style != FontStyle::Custom {
                    appearance.lyrics_font.family = None;
                }
                changed = true;
                Update::effect(Effect::Save)
            }
            Message::Picker(target) => {
                self.picker = target.map(|target| {
                    let font =
                        if target == FontTarget::Lyrics { &appearance.lyrics_font } else { &appearance.subtitle.font };
                    Picker { target, search: String::new(), fallback: font.fallback.join(", ") }
                });
                Update::effect(Effect::None)
            }
            Message::FontSearch(search) => {
                if let Some(picker) = self.picker.as_mut() {
                    picker.search = search;
                }
                Update::effect(Effect::None)
            }
            Message::ClearFont(target) => {
                let font = match target {
                    FontTarget::Lyrics => &mut appearance.lyrics_font,
                    FontTarget::Subtitle => &mut appearance.subtitle.font,
                };
                font.style = FontStyle::Sans;
                font.family = None;
                self.picker = None;
                changed = true;
                Update::effect(Effect::Save)
            }
            Message::PickFont(family) => {
                if let Some(picker) = self.picker.take() {
                    let font = match picker.target {
                        FontTarget::Lyrics => &mut appearance.lyrics_font,
                        FontTarget::Subtitle => &mut appearance.subtitle.font,
                    };
                    font.style = FontStyle::Custom;
                    font.family = Some(family);
                    changed = true;
                }
                Update::effect(Effect::Save)
            }
            Message::Fallback(text) => {
                if let Some(picker) = self.picker.as_mut() {
                    picker.fallback = text;
                }
                Update::effect(Effect::None)
            }
            Message::FallbackCommit => {
                if let Some(picker) = self.picker.as_ref() {
                    let stack = parse_fallback(&picker.fallback);
                    let font = match picker.target {
                        FontTarget::Lyrics => &mut appearance.lyrics_font,
                        FontTarget::Subtitle => &mut appearance.subtitle.font,
                    };
                    if font.fallback != stack {
                        font.fallback = stack;
                        changed = true;
                    }
                }
                Update::effect(Effect::Save)
            }
            Message::WeightAuto(target, follow) => {
                let font = match target {
                    FontTarget::Lyrics => &mut appearance.lyrics_font,
                    FontTarget::Subtitle => &mut appearance.subtitle.font,
                };
                if follow {
                    font.weight = None;
                } else if font.weight.is_none() {
                    font.weight = Some(400);
                }
                changed = true;
                Update::effect(Effect::Save)
            }
            Message::Weight(target, value) => {
                let font = match target {
                    FontTarget::Lyrics => &mut appearance.lyrics_font,
                    FontTarget::Subtitle => &mut appearance.subtitle.font,
                };
                font.weight = Some(fonts::stored_weight(value));
                changed = true;
                Update::effect(Effect::Appearance)
            }
            Message::Opacity(value) => {
                appearance.visualizer_opacity = value.clamp(0.2, 1.0);
                changed = true;
                Update::effect(Effect::Appearance)
            }
            Message::BackgroundMode(mode) => {
                appearance.set_background_mode(Some(mode.id()));
                changed = true;
                Update::effect(Effect::Save)
            }
            Message::Vignette(off) => {
                appearance.disable_visualizer_vignette = off;
                changed = true;
                Update::effect(Effect::Save)
            }
            Message::SubtitleRows(count) => {
                appearance.subtitle.layout = appearance.subtitle.layout.with_count(count);
                changed = true;
                Update::effect(Effect::Save)
            }
            Message::SubtitleRow(index, source) => {
                appearance.subtitle.layout = appearance.subtitle.layout.with_row(index, source);
                changed = true;
                Update::effect(Effect::Save)
            }
            Message::SubtitleMode(mode) => {
                appearance.subtitle.content_mode = mode;
                changed = true;
                Update::effect(Effect::Save)
            }
            Message::SubtitleOpacity(value) => {
                appearance.subtitle.opacity = value.clamp(0.2, 1.0);
                changed = true;
                Update::effect(Effect::Appearance)
            }
            Message::SubtitleBackground(on) => {
                appearance.subtitle.background = on;
                changed = true;
                Update::effect(Effect::Save)
            }
            Message::SubtitleUpcomingBlur(on) => {
                appearance.subtitle.upcoming_blur = on;
                changed = true;
                Update::effect(Effect::Save)
            }
            Message::SubtitleHide(hidden) => {
                appearance.subtitle.hide_translation = hidden;
                changed = true;
                Update::effect(Effect::Save)
            }
            Message::SubtitleInherits(inherits) => {
                appearance.subtitle.font_inherits_lyrics = inherits;
                changed = true;
                Update::effect(Effect::Save)
            }
        };
        if changed { Update::appearance(result.effect, &appearance) } else { result }
    }

    pub fn ticking(&self) -> bool {
        self.source == Source::Builtin && !self.paused || self.scrolling()
    }

    /// A smooth scroll is moving, so the shell keeps sending frames.
    pub fn scrolling(&self) -> bool {
        let now = Instant::now();
        self.grid.animating(now) || self.page.animating(now)
    }

    /// Advances both lists; called after every workbench update.
    pub fn scroll_frame(&mut self, now: Instant) -> Task<Message> {
        let grid = self.grid.frame(now);
        let page = self.page.frame(now);
        if grid.is_some() || page.is_some() || std::mem::take(&mut self.scroll_dirty) {
            Task::batch([self.grid.apply(Id::new(GRID)), self.page.apply(Id::new(PAGE))])
        } else {
            Task::none()
        }
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
        let Look { i18n, theme, daylight, window, style, font_scale, live, appearance, .. } = look;
        let tokens = Tokens::new(theme, daylight);
        let now = Instant::now();
        let (lyrics, time, empty) = self.shown(now, live);
        let header = self.header(i18n, tokens);
        let stage = self.stage(look, tokens, lyrics, time, empty);
        let column = self.column(i18n, tokens, theme, style, font_scale, appearance);
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
        let panel: Element<'a, crate::Message> = match &self.picker {
            Some(picker) => {
                let dialog = self.picker_view(i18n, tokens, appearance, picker, look.fonts);
                stack![
                    panel,
                    opaque(
                        container(dialog)
                            .width(Length::Fill)
                            .height(Length::Fill)
                            .center_x(Length::Fill)
                            .center_y(Length::Fill)
                            .style(move |_| container::Style {
                                background: Some(Color { a: 0.72, ..tokens.win }.into()),
                                ..container::Style::default()
                            })
                    )
                ]
                .into()
            }
            None => panel.into(),
        };
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
                background: (status == button::Status::Hovered).then_some(IcedBackground::Color(tokens.fill)),
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
                    background: Some(IcedBackground::Color(folia_visualizer::color(theme.background, 1.0))),
                    border: Border { color: tokens.line, width: 1.0, radius: 12.0.into() },
                    ..container::Style::default()
                }
            });
        let note = empty.then(|| -> Element<'a, crate::Message> {
            container(text(i18n.t("options.previewSourceLiveEmpty")).size(13).color(Color::WHITE)).padding(8).into()
        });
        let bar = self.preview_bar(i18n, tokens);
        let hotspots = canvas_widget(Hotspots {
            theme,
            active: self.tab,
            labels: HotspotLabels {
                background: i18n.t("options.previewBackgroundHotspot"),
                visualizer: i18n.t("options.previewVisualizerHotspot"),
                subtitle: i18n.t("options.previewSubtitleHotspot"),
            },
        })
        .width(Length::Fill)
        .height(Length::Fill);
        let picture = stack![
            preview,
            hotspots,
            container(bar)
                .width(Length::Fill)
                .height(Length::Fill)
                .align_x(Alignment::End)
                .align_y(Alignment::Start)
                .padding(12)
        ];
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
                background: Some(IcedBackground::Color(Color {
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
        let grid = scrollable(grid)
            .id(GRID)
            .height(Length::Fixed(250.0))
            .direction(kit::scrollbar())
            .on_scroll(|viewport| crate::Message::Workbench(Message::GridScrolled(viewport)))
            .style(kit::scroll_style(tokens));
        let grid = smooth_scroll::smooth(grid, |delta| crate::Message::Workbench(Message::GridWheel(delta)));
        column![heading, grid].spacing(8).into()
    }

    fn column<'a>(
        &'a self,
        i18n: &'a I18n,
        tokens: Tokens,
        theme: &'a Theme,
        style: Style,
        font_scale: f32,
        appearance: &'a AppearanceSettings,
    ) -> Element<'a, crate::Message> {
        let tabs = Tab::ALL
            .into_iter()
            .map(|tab| (i18n.t(tab.key()).to_owned(), crate::Message::Workbench(Message::Tab(tab))))
            .collect();
        let chosen = Tab::ALL.iter().position(|tab| *tab == self.tab);
        let reset_on = !matches!(self.tab, Tab::Global);
        let reset = button(text(i18n.t("ui.default")).size(13).color(if reset_on { tokens.t1 } else { tokens.t3 }))
            .padding(Padding { left: 8.0, right: 8.0, ..Padding::ZERO })
            .style(|_, _| button::Style::default());
        let reset = if reset_on { reset.on_press(crate::Message::Workbench(Message::Reset)) } else { reset };
        let tabs = row![kit::tabs(tokens, tabs, chosen, chosen.map(|index| index as f32), 8.0, 16.0), reset]
            .align_y(Alignment::Center);
        let page = self.page(i18n, tokens, theme, style, font_scale, appearance);
        let page = scrollable(page)
            .id(settings_scroll_id())
            .height(Length::Fill)
            .direction(kit::scrollbar())
            .on_scroll(|viewport| crate::Message::Workbench(Message::PageScrolled(viewport)))
            .style(kit::scroll_style(tokens));
        let page = smooth_scroll::smooth(page, |delta| crate::Message::Workbench(Message::PageWheel(delta)));
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
        theme: &'a Theme,
        style: Style,
        font_scale: f32,
        appearance: &'a AppearanceSettings,
    ) -> Element<'a, crate::Message> {
        match self.tab {
            Tab::Lyrics => style_page(i18n, tokens, theme, style.manifest(), &self.values),
            Tab::Background => background_page(i18n, tokens, theme, appearance),
            Tab::Font => font_page(i18n, tokens, theme, style, self.sample, font_scale, appearance),
            Tab::Subtitle => subtitle_page(i18n, tokens, theme, appearance),
            Tab::Global => global_page(i18n, tokens),
        }
    }

    fn picker_view<'a>(
        &'a self,
        i18n: &'a I18n,
        tokens: Tokens,
        appearance: &AppearanceSettings,
        picker: &Picker,
        fonts: &'a [String],
    ) -> Element<'a, crate::Message> {
        let font = match picker.target {
            FontTarget::Lyrics => &appearance.lyrics_font,
            FontTarget::Subtitle => &appearance.subtitle.font,
        };
        let title = i18n.t("options.chooseSystemFont").to_owned();
        let close = kit::icon_button(tokens, Icon::X, 16.0, crate::Message::Workbench(Message::Picker(None)));
        let heading = row![
            column![
                text(title).size(15).font(kit::SEMIBOLD).color(tokens.t1),
                text(i18n.t("options.chooseSystemFontDesc")).size(12).color(tokens.t3)
            ]
            .width(Length::Fill),
            close
        ]
        .align_y(Alignment::Start);
        let search = text_input(i18n.t("options.searchSystemFont"), &picker.search)
            .size(13)
            .padding([8, 10])
            .on_input(|text| crate::Message::Workbench(Message::FontSearch(text)))
            .style(move |_, _| text_input::Style {
                background: tokens.fill.into(),
                border: Border { color: tokens.line2, width: 1.0, radius: 8.0.into() },
                icon: tokens.t3,
                placeholder: tokens.t3,
                value: tokens.t1,
                selection: tokens.fill2,
            });
        let current = match font.style {
            FontStyle::Custom => font.family.clone().unwrap_or_else(|| i18n.t("options.systemFontInactive").to_owned()),
            FontStyle::Sans => i18n.t("options.fontSans").to_owned(),
            FontStyle::Serif => i18n.t("options.fontSerif").to_owned(),
            FontStyle::Mono => i18n.t("options.fontMono").to_owned(),
        };
        let current = column![
            text(i18n.t("options.currentFont")).size(12).color(tokens.t3),
            text(current).size(14).color(tokens.t1)
        ];
        let fallback = text_input(i18n.t("options.fontFallbackFamiliesPlaceholder"), &picker.fallback)
            .size(13)
            .padding([8, 10])
            .on_input(|text| crate::Message::Workbench(Message::Fallback(text)))
            .on_submit(crate::Message::Workbench(Message::FallbackCommit))
            .style(move |_, _| text_input::Style {
                background: tokens.fill.into(),
                border: Border { color: tokens.line2, width: 1.0, radius: 8.0.into() },
                icon: tokens.t3,
                placeholder: tokens.t3,
                value: tokens.t1,
                selection: tokens.fill2,
            });
        let fallback = column![text(i18n.t("options.fontFallbackFamilies")).size(12).color(tokens.t3), fallback];
        let wanted = picker.search.trim().to_lowercase();
        let mut list = column![].spacing(2);
        let mut shown = 0;
        for family in fonts {
            if !wanted.is_empty() && !family.to_lowercase().contains(&wanted) {
                continue;
            }
            shown += 1;
            let face = Font { family: iced::font::Family::Name(fonts::intern(family)), ..Font::DEFAULT };
            let label = text(family.as_str()).size(15).font(face).color(tokens.t1);
            list = list.push(
                button(label)
                    .width(Length::Fill)
                    .padding([7, 10])
                    .on_press(crate::Message::Workbench(Message::PickFont(family.clone())))
                    .style(move |_, status| button::Style {
                        background: (status == button::Status::Hovered).then_some(IcedBackground::Color(tokens.fill)),
                        border: Border { radius: 6.0.into(), ..Border::default() },
                        ..button::Style::default()
                    }),
            );
        }
        let body: Element<'a, crate::Message> = if shown == 0 {
            text(i18n.t("options.systemFontNoResults")).size(13).color(tokens.t3).into()
        } else {
            scrollable(list).height(Length::Fill).direction(kit::scrollbar()).style(kit::scroll_style(tokens)).into()
        };
        // "Back to the built-in font" clears the custom family.
        let clear = button(text(i18n.t("options.clearSystemFont")).size(13).color(tokens.t1))
            .padding([6, 10])
            .on_press(crate::Message::Workbench(Message::ClearFont(picker.target)))
            .style(|_, _| button::Style::default());
        let dialog = container(
            column![heading, search, current, fallback, container(body).height(Length::Fill), clear]
                .spacing(10)
                .width(Length::Fill),
        )
        .width(Length::Fixed(520.0))
        .height(Length::Fixed(560.0))
        .padding(20)
        .style(move |_| container::Style {
            background: Some(tokens.win.into()),
            border: Border { color: tokens.line2, width: 1.0, radius: 16.0.into() },
            shadow: Shadow {
                color: Color { a: 0.35, ..Color::BLACK },
                offset: Vector::new(0.0, 24.0),
                blur_radius: 60.0,
            },
            ..container::Style::default()
        });
        dialog.into()
    }
}

/// The right settings body, scrolled back to its top when a hotspot or a tab is chosen.
pub fn settings_scroll_id() -> iced::widget::Id {
    iced::widget::Id::new(PAGE)
}

fn item_label(item: Item) -> &'static str {
    match item {
        Item::Slider { label_key, .. }
        | Item::Switch { label_key, .. }
        | Item::Member { label_key, .. }
        | Item::Select { label_key, .. }
        | Item::Picture { label_key, .. }
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
    theme: &'a Theme,
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
            rows.push(item_row(i18n, tokens, theme, manifest, values, *item));
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
    theme: &'a Theme,
    manifest: Manifest,
    values: &'a Values,
    item: Item,
) -> Row<'a, crate::Message> {
    let locked = manifest.locked(item.key());
    let mut below = None;
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
        Item::Picture { key, options, .. } => {
            let current = values.choice(manifest, key);
            let choices = options
                .iter()
                .map(|choice| PictureOption {
                    value: choice.value,
                    label: i18n.t(choice.label_key).to_owned(),
                    picture: choice.picture,
                })
                .collect();
            below = Some(picture_grid(tokens, theme, choices, current, 4, move |value| {
                crate::Message::Workbench(Message::Choice(key, value.to_owned()))
            }));
            space().into()
        }
    };
    let hint = match item {
        Item::Button { path_key: Some(path), .. } => values
            .path(path)
            .and_then(|path| std::path::Path::new(path).file_name())
            .map(|name| name.to_string_lossy().into_owned()),
        Item::Slider { hint_key: Some(hint), .. }
        | Item::Switch { hint_key: Some(hint), .. }
        | Item::Select { hint_key: Some(hint), .. }
        | Item::Picture { hint_key: Some(hint), .. } => Some(i18n.t(hint).to_owned()),
        _ => None,
    };
    let label = i18n.t(item_label(item)).to_owned();
    Row { label, hint, sub: false, below, control }
}

fn background_page<'a>(
    i18n: &'a I18n,
    tokens: Tokens,
    theme: &'a Theme,
    appearance: &AppearanceSettings,
) -> Element<'a, crate::Message> {
    let current = Background::resolve(appearance.stored_background_mode());
    let choices: Vec<PictureOption> = Background::ALL
        .iter()
        .map(|mode| PictureOption {
            value: mode.id(),
            label: i18n.t(mode.label_key()).to_owned(),
            picture: background_picture(*mode),
        })
        .collect();
    let grid = picture_grid(tokens, theme, choices, current.id(), 3, |id| {
        crate::Message::Workbench(Message::BackgroundMode(Background::from_id(id).unwrap_or_default()))
    });
    let mode = Row {
        label: i18n.t("options.visualizerBackgroundMode").to_owned(),
        hint: None,
        sub: false,
        below: Some(grid),
        control: space().into(),
    };
    let vignette = Row {
        label: i18n.t("options.disableVisualizerVignette").to_owned(),
        hint: Some(i18n.t("options.disableVisualizerVignetteDesc").to_owned()),
        sub: false,
        below: None,
        control: kit::toggle(
            tokens,
            if appearance.disable_visualizer_vignette { 1.0 } else { 0.0 },
            crate::Message::Workbench(Message::Vignette(!appearance.disable_visualizer_vignette)),
        ),
    };
    column![
        kit::group(
            tokens,
            Group { title: i18n.t("options.visualizerBackgroundMode").to_owned(), bare: false, rows: vec![mode] },
            true,
        ),
        kit::group(
            tokens,
            Group {
                title: i18n.t("options.visualizerBackgroundModeCommon").to_owned(),
                bare: false,
                rows: vec![vignette],
            },
            false,
        ),
    ]
    .padding(Padding { left: 24.0, right: 12.0, bottom: 40.0, top: 0.0 })
    .into()
}

fn background_picture(mode: Background) -> fn(&mut Frame, Rectangle, &Theme) {
    match mode {
        Background::Common => draw_common_picture,
    }
}

fn draw_common_picture(frame: &mut Frame, rect: Rectangle, theme: &Theme) {
    Background::Common.picture(frame, rect, theme);
}

fn font_page<'a>(
    i18n: &'a I18n,
    tokens: Tokens,
    theme: &'a Theme,
    style: Style,
    sample: Sample,
    font_scale: f32,
    appearance: &AppearanceSettings,
) -> Element<'a, crate::Message> {
    let font = &appearance.lyrics_font;
    let samples = vec![
        (Sample::Default, i18n.t("options.previewTextDefault").to_owned()),
        (Sample::Reserved, i18n.t("options.previewTextReserved").to_owned()),
    ];
    let mut rows = vec![Row {
        label: i18n.t("options.previewText").to_owned(),
        hint: None,
        sub: false,
        below: None,
        control: kit::segmented(tokens, samples, sample, None, |sample| {
            crate::Message::Workbench(Message::Sample(sample))
        }),
    }];
    let auto = style.own_font_size_notice();
    rows.extend(font_tiles(i18n, tokens, theme, font));
    if font.style == FontStyle::Custom {
        rows.push(Row {
            label: i18n.t("options.currentFont").to_owned(),
            hint: font.family.clone(),
            sub: true,
            below: None,
            control: button(text(i18n.t("options.chooseSystemFont")).size(13).color(tokens.t1))
                .padding([6, 10])
                .on_press(crate::Message::Workbench(Message::Picker(Some(FontTarget::Lyrics))))
                .style(|_, _| button::Style::default())
                .into(),
        });
    }
    rows.push(Row {
        label: i18n.t("options.fontSize").to_owned(),
        hint: None,
        sub: false,
        below: None,
        control: if auto.is_some() {
            kit::value(tokens, format!("{}%", (font_scale * 100.0).round()))
        } else {
            row![
                slider(0.85_f32..=1.4, font_scale, |value| crate::Message::Workbench(Message::Scale(
                    FontTarget::Lyrics,
                    value
                )))
                .step(0.05_f32)
                .width(160)
                .on_release(crate::Message::Workbench(Message::Commit))
                .style(move |_, _| kit::slider_style(tokens)),
                kit::value(tokens, format!("{}%", (font_scale * 100.0).round())),
            ]
            .spacing(8)
            .align_y(Alignment::Center)
            .into()
        },
    });
    rows.push(weight_auto_row(tokens, i18n, FontTarget::Lyrics, font));
    if let Some(weight) = font.weight {
        rows.push(weight_row(tokens, i18n, FontTarget::Lyrics, weight));
    }
    rows.push(Row {
        label: i18n.t("options.visualizerOpacity").to_owned(),
        hint: None,
        sub: false,
        below: None,
        control: row![
            slider(0.2_f32..=1.0, appearance.visualizer_opacity, |value| {
                crate::Message::Workbench(Message::Opacity(value))
            })
            .step(0.05_f32)
            .width(160)
            .on_release(crate::Message::Workbench(Message::Commit))
            .style(move |_, _| kit::slider_style(tokens)),
            kit::value(tokens, format!("{}%", (appearance.visualizer_opacity * 100.0).round())),
        ]
        .spacing(8)
        .align_y(Alignment::Center)
        .into(),
    });
    let mut page = column![kit::group(
        tokens,
        Group { title: i18n.t("options.previewCommonSettings").to_owned(), bare: true, rows },
        true,
    )];
    if let Some(key) = auto {
        page = page.push(text(i18n.t(key)).size(12).color(tokens.t3));
    }
    page.padding(Padding { left: 24.0, right: 12.0, bottom: 40.0, top: 0.0 }).into()
}

/// The font tiles row of the original `FontTiles`, in its four faces.
fn font_tiles<'a>(
    i18n: &'a I18n,
    tokens: Tokens,
    theme: &'a Theme,
    font: &FontSettings,
) -> Vec<Row<'a, crate::Message>> {
    let custom_label = font.family.clone().unwrap_or_else(|| i18n.t("options.customFont").to_owned());
    let chosen = font.style;
    let choices = vec![
        FontChoice {
            value: FontStyle::Sans,
            label: i18n.t("options.fontSans").to_owned(),
            family: None,
            active: chosen,
        },
        FontChoice {
            value: FontStyle::Serif,
            label: i18n.t("options.fontSerif").to_owned(),
            family: None,
            active: chosen,
        },
        FontChoice {
            value: FontStyle::Mono,
            label: i18n.t("options.fontMono").to_owned(),
            family: None,
            active: chosen,
        },
        FontChoice {
            value: FontStyle::Custom,
            label: custom_label,
            family: font.family.as_deref().map(fonts::intern),
            active: chosen,
        },
    ];
    vec![Row {
        label: i18n.t("options.fontFamily").to_owned(),
        hint: None,
        sub: false,
        below: Some(font_grid(tokens, theme, choices)),
        control: space().into(),
    }]
}

fn weight_auto_row<'a>(
    tokens: Tokens,
    i18n: &'a I18n,
    target: FontTarget,
    font: &FontSettings,
) -> Row<'a, crate::Message> {
    Row {
        label: i18n.t("options.fontWeightAuto").to_owned(),
        hint: Some(i18n.t("options.fontWeightAutoDesc").to_owned()),
        sub: false,
        below: None,
        control: kit::toggle(
            tokens,
            if font.weight.is_none() { 1.0 } else { 0.0 },
            crate::Message::Workbench(Message::WeightAuto(target, font.weight.is_some())),
        ),
    }
}

fn weight_row<'a>(tokens: Tokens, i18n: &'a I18n, target: FontTarget, weight: u16) -> Row<'a, crate::Message> {
    Row {
        label: i18n.t("options.fontWeight").to_owned(),
        hint: None,
        sub: true,
        below: None,
        control: row![
            slider(100.0_f32..=900.0, f32::from(weight), move |value| crate::Message::Workbench(Message::Weight(
                target, value
            )))
            .step(10.0_f32)
            .width(160)
            .on_release(crate::Message::Workbench(Message::Commit))
            .style(move |_, _| kit::slider_style(tokens)),
            kit::value(tokens, weight.to_string()),
        ]
        .spacing(8)
        .align_y(Alignment::Center)
        .into(),
    }
}

fn subtitle_page<'a>(
    i18n: &'a I18n,
    tokens: Tokens,
    theme: &'a Theme,
    appearance: &AppearanceSettings,
) -> Element<'a, crate::Message> {
    let subtitle = &appearance.subtitle;
    let layout = &subtitle.layout;
    let count = layout.count() as u8;
    let rows = layout.rows();
    let mut subtitle_rows = vec![
        Row {
            label: i18n.t("options.hidePlayerTranslationSubtitle").to_owned(),
            hint: Some(i18n.t("options.hidePlayerTranslationSubtitleDesc").to_owned()),
            sub: false,
            below: None,
            control: kit::toggle(
                tokens,
                if subtitle.hide_translation { 1.0 } else { 0.0 },
                crate::Message::Workbench(Message::SubtitleHide(!subtitle.hide_translation)),
            ),
        },
        Row {
            label: i18n.t("options.subtitleOverlayRows").to_owned(),
            hint: Some(i18n.t("options.subtitleOverlayRowsDesc").to_owned()),
            sub: false,
            below: None,
            control: kit::segmented(
                tokens,
                vec![
                    (1u8, i18n.t("options.subtitleOverlayRowsOne").to_owned()),
                    (2u8, i18n.t("options.subtitleOverlayRowsTwo").to_owned()),
                    (3u8, i18n.t("options.subtitleOverlayRowsThree").to_owned()),
                ],
                count,
                None,
                |count| crate::Message::Workbench(Message::SubtitleRows(count)),
            ),
        },
    ];
    if count == 1 {
        let source = rows.first().copied().unwrap_or(RowSource::Translation);
        subtitle_rows.push(source_row(i18n, tokens, 0, source, true));
    } else {
        subtitle_rows.push(Row {
            label: i18n.t("options.subtitleOverlayRow1").to_owned(),
            hint: None,
            sub: true,
            below: None,
            control: kit::value(tokens, i18n.t("options.subtitleOverlaySourceCurrent").to_owned()),
        });
        subtitle_rows.push(source_row(i18n, tokens, 1, rows.get(1).copied().unwrap_or(RowSource::Next), false));
        if count == 3 {
            subtitle_rows.push(source_row(
                i18n,
                tokens,
                2,
                rows.get(2).copied().unwrap_or(RowSource::Translation),
                false,
            ));
        }
    }
    subtitle_rows.push(Row {
        label: i18n.t("options.subtitleContentMode").to_owned(),
        hint: Some(i18n.t("options.subtitleContentModeDesc").to_owned()),
        sub: false,
        below: None,
        control: kit::select(
            tokens,
            vec![
                Choice {
                    value: SubtitleContentMode::Translation,
                    label: i18n.t("options.subtitleContentTranslation").to_owned(),
                },
                Choice {
                    value: SubtitleContentMode::Romanization,
                    label: i18n.t("options.subtitleContentRomanization").to_owned(),
                },
                Choice { value: SubtitleContentMode::Both, label: i18n.t("options.subtitleContentBoth").to_owned() },
                Choice { value: SubtitleContentMode::None, label: i18n.t("options.subtitleContentNone").to_owned() },
            ],
            Choice { value: subtitle.content_mode, label: i18n.t(content_mode_key(subtitle.content_mode)).to_owned() },
            |choice| crate::Message::Workbench(Message::SubtitleMode(choice.value)),
        ),
    });
    subtitle_rows.push(Row {
        label: i18n.t("options.subtitleOverlayOpacity").to_owned(),
        hint: None,
        sub: false,
        below: None,
        control: row![
            slider(0.2_f32..=1.0, subtitle.opacity, |value| crate::Message::Workbench(Message::SubtitleOpacity(value)))
                .step(0.05_f32)
                .width(160)
                .on_release(crate::Message::Workbench(Message::Commit))
                .style(move |_, _| kit::slider_style(tokens)),
            kit::value(tokens, format!("{}%", (subtitle.opacity * 100.0).round())),
        ]
        .spacing(8)
        .align_y(Alignment::Center)
        .into(),
    });
    subtitle_rows.push(Row {
        label: i18n.t("options.subtitleOverlayBackground").to_owned(),
        hint: Some(i18n.t("options.subtitleOverlayBackgroundDesc").to_owned()),
        sub: false,
        below: None,
        control: kit::toggle(
            tokens,
            if subtitle.background { 1.0 } else { 0.0 },
            crate::Message::Workbench(Message::SubtitleBackground(!subtitle.background)),
        ),
    });
    subtitle_rows.push(Row {
        label: i18n.t("options.subtitleUpcomingLyricsBlur").to_owned(),
        hint: None,
        sub: false,
        below: None,
        control: kit::toggle(
            tokens,
            if subtitle.upcoming_blur { 1.0 } else { 0.0 },
            crate::Message::Workbench(Message::SubtitleUpcomingBlur(!subtitle.upcoming_blur)),
        ),
    });
    let mut font_rows = vec![Row {
        label: i18n.t("options.subtitleFontInheritsLyrics").to_owned(),
        hint: Some(i18n.t("options.subtitleFontInheritsLyricsDesc").to_owned()),
        sub: false,
        below: None,
        control: kit::toggle(
            tokens,
            if subtitle.font_inherits_lyrics { 1.0 } else { 0.0 },
            crate::Message::Workbench(Message::SubtitleInherits(!subtitle.font_inherits_lyrics)),
        ),
    }];
    if !subtitle.font_inherits_lyrics {
        let font = &subtitle.font;
        let tiles = font_tiles(i18n, tokens, theme, font);
        for tile in tiles {
            font_rows.push(Row { sub: true, ..tile });
        }
        if font.style == FontStyle::Custom {
            font_rows.push(Row {
                label: i18n.t("options.currentFont").to_owned(),
                hint: font.family.clone(),
                sub: true,
                below: None,
                control: button(text(i18n.t("options.chooseSystemFont")).size(13).color(tokens.t1))
                    .padding([6, 10])
                    .on_press(crate::Message::Workbench(Message::Picker(Some(FontTarget::Subtitle))))
                    .style(|_, _| button::Style::default())
                    .into(),
            });
        }
        font_rows.push(weight_auto_row(tokens, i18n, FontTarget::Subtitle, font));
        if let Some(weight) = font.weight {
            font_rows.push(weight_row(tokens, i18n, FontTarget::Subtitle, weight));
        }
    }
    font_rows.push(Row {
        label: i18n.t("options.subtitleFontScale").to_owned(),
        hint: None,
        sub: false,
        below: None,
        control: row![
            slider(0.85_f32..=1.4, subtitle.font_scale, |value| crate::Message::Workbench(Message::Scale(
                FontTarget::Subtitle,
                value
            )))
            .step(0.05_f32)
            .width(160)
            .on_release(crate::Message::Workbench(Message::Commit))
            .style(move |_, _| kit::slider_style(tokens)),
            kit::value(tokens, format!("{}%", (subtitle.font_scale * 100.0).round())),
        ]
        .spacing(8)
        .align_y(Alignment::Center)
        .into(),
    });
    column![
        kit::group(
            tokens,
            Group { title: i18n.t("options.previewSubtitleSettings").to_owned(), bare: true, rows: subtitle_rows },
            true,
        ),
        kit::group(
            tokens,
            Group { title: i18n.t("options.subtitleFontFamily").to_owned(), bare: false, rows: font_rows },
            false,
        ),
    ]
    .padding(Padding { left: 24.0, right: 12.0, bottom: 40.0, top: 0.0 })
    .into()
}

fn content_mode_key(mode: SubtitleContentMode) -> &'static str {
    match mode {
        SubtitleContentMode::Translation => "options.subtitleContentTranslation",
        SubtitleContentMode::Romanization => "options.subtitleContentRomanization",
        SubtitleContentMode::Both => "options.subtitleContentBoth",
        SubtitleContentMode::None => "options.subtitleContentNone",
    }
}

fn source_row<'a>(
    i18n: &'a I18n,
    tokens: Tokens,
    index: usize,
    current: RowSource,
    single: bool,
) -> Row<'a, crate::Message> {
    let options = if single {
        vec![
            Choice { value: RowSource::Current, label: i18n.t("options.subtitleOverlaySourceCurrent").to_owned() },
            Choice { value: RowSource::Translation, label: i18n.t("options.subtitleContentTranslation").to_owned() },
            Choice { value: RowSource::Romanization, label: i18n.t("options.subtitleContentRomanization").to_owned() },
        ]
    } else {
        vec![
            Choice { value: RowSource::Next, label: i18n.t("options.subtitleOverlaySourceNext").to_owned() },
            Choice { value: RowSource::Translation, label: i18n.t("options.subtitleContentTranslation").to_owned() },
            Choice { value: RowSource::Romanization, label: i18n.t("options.subtitleContentRomanization").to_owned() },
        ]
    };
    let label_key = ["options.subtitleOverlayRow1", "options.subtitleOverlayRow2", "options.subtitleOverlayRow3"]
        .get(index)
        .copied()
        .unwrap_or("options.subtitleOverlayRow1");
    let selected = options.iter().find(|choice| choice.value == current).cloned().unwrap_or_else(|| options[0].clone());
    Row {
        label: i18n.t(label_key).to_owned(),
        hint: None,
        sub: true,
        below: None,
        control: kit::select(tokens, options, selected, move |choice| {
            crate::Message::Workbench(Message::SubtitleRow(index, choice.value))
        }),
    }
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
        below: None,
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
    let picture = container(canvas_widget(pictures.frozen(scene)).width(Length::Fill).height(Length::Fill))
        .width(Length::Fill)
        .height(Length::Fill)
        .clip(true)
        .style(move |_| container::Style {
            background: Some(IcedBackground::Color(tokens.pv)),
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
            background: on.then_some(IcedBackground::Color(Color { a: 0.22, ..Color::WHITE })),
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

fn parse_fallback(text: &str) -> Vec<String> {
    text.split([',', '\n']).map(str::trim).filter(|family| !family.is_empty()).map(str::to_owned).collect()
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

/// One choice of a picture grid: its value, translated name, and the painter the style owns.
#[derive(Clone)]
struct PictureOption {
    value: &'static str,
    label: String,
    picture: fn(&mut Frame, Rectangle, &Theme),
}

/// The kit's `PictureGrid`: 16:10 pictures in `columns` columns, the chosen one ringed in the accent colour.
fn picture_grid<'a, F>(
    tokens: Tokens,
    theme: &'a Theme,
    options: Vec<PictureOption>,
    current: &'a str,
    columns: usize,
    on_pick: F,
) -> Element<'a, crate::Message>
where
    F: Fn(&'static str) -> crate::Message + Clone + 'a,
{
    let mut grid = column![].spacing(12);
    // A 16:10 frame: four tiles in the column are about 87px wide, three about 116px.
    let picture_height = if columns == 3 { 72.0 } else { 52.0 };
    for chunk in options.chunks(columns) {
        let mut line = row![].spacing(12);
        for option in chunk {
            let chosen = option.value == current;
            let picture = canvas_widget(PictureCanvas { picture: option.picture, theme })
                .width(Length::Fill)
                .height(Length::Fill);
            let frame = container(picture).width(Length::Fill).height(Length::Fixed(picture_height)).clip(true).style(
                move |_| container::Style {
                    background: Some(IcedBackground::Color(tokens.pv)),
                    border: Border {
                        color: if chosen { tokens.accent } else { tokens.line },
                        width: if chosen { 2.0 } else { 1.0 },
                        radius: 8.0.into(),
                    },
                    ..container::Style::default()
                },
            );
            let caption = text(option.label.clone())
                .size(12)
                .font(if chosen { kit::SEMIBOLD } else { Font::DEFAULT })
                .color(if chosen { tokens.t1 } else { tokens.t2 });
            let value = option.value;
            let press = on_pick.clone();
            let body = column![frame, container(caption).center_x(Length::Fill)].spacing(6).width(Length::Fill);
            line = line.push(
                button(body)
                    .padding(1)
                    .width(Length::Fill)
                    .on_press(press(value))
                    .style(|_, _| button::Style::default()),
            );
        }
        for _ in chunk.len()..columns {
            line = line.push(space().width(Length::Fill));
        }
        grid = grid.push(line);
    }
    grid.into()
}

/// A picture option's 16:10 frame, painted by its style.
struct PictureCanvas<'a> {
    picture: fn(&mut Frame, Rectangle, &Theme),
    theme: &'a Theme,
}

impl<Message> canvas::Program<Message> for PictureCanvas<'_> {
    type State = ();

    fn draw(
        &self,
        _state: &(),
        renderer: &Renderer,
        _theme: &iced::Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<Geometry> {
        let mut frame = Frame::new(renderer, bounds.size());
        (self.picture)(&mut frame, Rectangle::new(Point::ORIGIN, bounds.size()), self.theme);
        vec![frame.into_geometry()]
    }
}

/// One font tile: its built-in style, or a chosen custom family, and the label under it.
struct FontChoice {
    value: FontStyle,
    label: String,
    family: Option<&'static str>,
    /// The face the settings currently use.
    active: FontStyle,
}

/// The original `FontTiles`: "永Aa" drawn in each face, four to a line.
fn font_grid<'a>(tokens: Tokens, theme: &'a Theme, options: Vec<FontChoice>) -> Element<'a, crate::Message> {
    let mut grid = column![].spacing(12);
    for chunk in options.chunks(4) {
        let mut line = row![].spacing(12);
        for option in chunk {
            let font = match option.family {
                Some(family) => Font { family: iced::font::Family::Name(family), ..Font::DEFAULT },
                None => Font { family: fonts::builtin(option.value), ..Font::DEFAULT },
            };
            let chosen = option.active == option.value;
            let sample = canvas_widget(FontSample { theme, font }).width(Length::Fill).height(Length::Fill);
            let frame = container(sample).width(Length::Fill).height(Length::Fixed(52.0)).clip(true).style(move |_| {
                container::Style {
                    background: Some(IcedBackground::Color(tokens.pv)),
                    border: Border {
                        color: if chosen { tokens.accent } else { tokens.line },
                        width: if chosen { 2.0 } else { 1.0 },
                        radius: 8.0.into(),
                    },
                    ..container::Style::default()
                }
            });
            let caption = text(option.label.clone())
                .size(12)
                .font(if chosen { kit::SEMIBOLD } else { Font::DEFAULT })
                .color(if chosen { tokens.t1 } else { tokens.t2 });
            let value = option.value;
            let body = column![frame, container(caption).center_x(Length::Fill)].spacing(6).width(Length::Fill);
            line = line.push(
                button(body)
                    .padding(1)
                    .width(Length::Fill)
                    .on_press(crate::Message::Workbench(Message::FontStyle(value)))
                    .style(|_, _| button::Style::default()),
            );
        }
        for _ in chunk.len()..4 {
            line = line.push(space().width(Length::Fill));
        }
        grid = grid.push(line);
    }
    grid.into()
}

struct FontSample<'a> {
    theme: &'a Theme,
    font: Font,
}

impl<Message> canvas::Program<Message> for FontSample<'_> {
    type State = ();

    fn draw(
        &self,
        _state: &(),
        renderer: &Renderer,
        _theme: &iced::Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<Geometry> {
        let mut frame = Frame::new(renderer, bounds.size());
        frame.fill_text(Text {
            content: "永Aa".to_owned(),
            position: Point::new(bounds.width / 2.0, bounds.height / 2.0 - 9.0),
            color: folia_visualizer::color(self.theme.primary, 1.0),
            size: 18.0.into(),
            font: self.font,
            align_x: iced::widget::text::Alignment::Center,
            align_y: iced::alignment::Vertical::Top,
            ..Text::default()
        });
        vec![frame.into_geometry()]
    }
}

/// The original `VisPlaygroundPreviewHotspots`: three transparent regions over the sample that choose tabs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Hotspot {
    Background,
    Visualizer,
    Subtitle,
}

impl Hotspot {
    pub const ALL: [Hotspot; 3] = [Hotspot::Background, Hotspot::Visualizer, Hotspot::Subtitle];

    fn tab(self) -> Tab {
        match self {
            Hotspot::Background => Tab::Background,
            Hotspot::Visualizer => Tab::Lyrics,
            Hotspot::Subtitle => Tab::Subtitle,
        }
    }

    /// `left-4 right-4 top-4 h-[20%]` and the two others of `VisPlaygroundPreviewHotspots.tsx`.
    pub fn rect(self, size: Size) -> Rectangle {
        match self {
            Hotspot::Background => {
                Rectangle { x: 16.0, y: 16.0, width: (size.width - 32.0).max(0.0), height: size.height * 0.2 }
            }
            Hotspot::Visualizer => Rectangle {
                x: size.width * 0.1,
                y: size.height * 0.28,
                width: size.width * 0.8,
                height: size.height * 0.48,
            },
            Hotspot::Subtitle => Rectangle {
                x: 16.0,
                y: (size.height - 24.0 - size.height * 0.18).max(0.0),
                width: (size.width - 32.0).max(0.0),
                height: size.height * 0.18,
            },
        }
    }
}

/// The hotspot under `at`, the last drawn (subtitle, visualizer, background) first.
pub fn hotspot_at(size: Size, at: Point) -> Option<Hotspot> {
    Hotspot::ALL.iter().rev().copied().find(|hotspot| hotspot.rect(size).contains(at))
}

#[derive(Default)]
struct HotspotState {
    hover: Option<(Hotspot, Instant)>,
    leave: Option<(Hotspot, Instant)>,
}

struct Hotspots<'a> {
    theme: &'a Theme,
    active: Tab,
    labels: HotspotLabels<'a>,
}

struct HotspotLabels<'a> {
    background: &'a str,
    visualizer: &'a str,
    subtitle: &'a str,
}

impl<'a> Hotspots<'a> {
    fn alpha(&self, state: &HotspotState, hotspot: Hotspot, now: Instant) -> f32 {
        if let Some((shown, at)) = state.hover
            && shown == hotspot
        {
            return (now.saturating_duration_since(at).as_secs_f32() / HOTSPOT_FADE.as_secs_f32()).clamp(0.0, 1.0);
        }
        if let Some((shown, at)) = state.leave
            && shown == hotspot
        {
            return 1.0
                - (now.saturating_duration_since(at).as_secs_f32() / HOTSPOT_FADE.as_secs_f32()).clamp(0.0, 1.0);
        }
        0.0
    }
}

impl canvas::Program<crate::Message> for Hotspots<'_> {
    type State = HotspotState;

    fn draw(
        &self,
        state: &HotspotState,
        renderer: &Renderer,
        _theme: &iced::Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<Geometry> {
        let now = Instant::now();
        let mut frame = Frame::new(renderer, bounds.size());
        for hotspot in Hotspot::ALL {
            let alpha = self.alpha(state, hotspot, now);
            if alpha <= 0.0 {
                continue;
            }
            let rect = hotspot.rect(bounds.size());
            let active = self.active == hotspot.tab();
            let accent = folia_visualizer::color(self.theme.accent, alpha);
            let secondary = folia_visualizer::color(self.theme.secondary, 0.24 * alpha);
            let border = if active { accent } else { secondary };
            let radius = 24.0;
            if active {
                frame.fill(
                    &canvas::Path::rounded_rectangle(rect.position(), rect.size(), iced::border::Radius::from(radius)),
                    folia_visualizer::color(self.theme.accent, 0.08 * alpha),
                );
                let inset =
                    Rectangle { x: rect.x + 1.0, y: rect.y + 1.0, width: rect.width - 2.0, height: rect.height - 2.0 };
                frame.stroke(
                    &canvas::Path::rounded_rectangle(
                        inset.position(),
                        inset.size(),
                        iced::border::Radius::from(radius - 1.0),
                    ),
                    canvas::Stroke::default()
                        .with_width(1.0)
                        .with_color(folia_visualizer::color(self.theme.accent, 0.56 * alpha)),
                );
                for (reach, width, a) in [(8.0, 16.0, 0.02), (20.0, 24.0, 0.008)] {
                    let halo = Rectangle {
                        x: rect.x - reach,
                        y: rect.y - reach,
                        width: rect.width + reach * 2.0,
                        height: rect.height + reach * 2.0,
                    };
                    frame.stroke(
                        &canvas::Path::rounded_rectangle(
                            halo.position(),
                            halo.size(),
                            iced::border::Radius::from(radius + reach),
                        ),
                        canvas::Stroke::default()
                            .with_width(width)
                            .with_color(folia_visualizer::color(self.theme.accent, a * alpha)),
                    );
                }
            }
            frame.stroke(
                &canvas::Path::rounded_rectangle(rect.position(), rect.size(), iced::border::Radius::from(radius)),
                canvas::Stroke::default().with_width(1.0).with_color(border),
            );
            let label_text = match hotspot {
                Hotspot::Background => self.labels.background,
                Hotspot::Visualizer => self.labels.visualizer,
                Hotspot::Subtitle => self.labels.subtitle,
            };
            let width = folia_visualizer::measure(label_text, 12.0, MEDIUM, f32::INFINITY).width;
            let (x, y) = match hotspot {
                Hotspot::Background | Hotspot::Visualizer => (rect.x + 20.0, rect.y + 20.0),
                Hotspot::Subtitle => {
                    ((rect.x + rect.width / 2.0) - (width + 24.0) / 2.0, rect.y + rect.height - 16.0 - 24.0)
                }
            };
            let pill = Rectangle { x, y, width: width + 24.0, height: 24.0 };
            let fill = if active {
                folia_visualizer::color(self.theme.accent, 0.18 * alpha)
            } else {
                folia_visualizer::color(self.theme.background, 0.72 * alpha)
            };
            frame.fill(
                &canvas::Path::rounded_rectangle(pill.position(), pill.size(), iced::border::Radius::from(12.0)),
                fill,
            );
            frame.stroke(
                &canvas::Path::rounded_rectangle(pill.position(), pill.size(), iced::border::Radius::from(12.0)),
                canvas::Stroke::default().with_width(1.0).with_color(border),
            );
            frame.fill_text(Text {
                content: label_text.to_owned(),
                position: Point::new(pill.x + 12.0, pill.y + 4.0),
                color: if active {
                    folia_visualizer::color(self.theme.primary, alpha)
                } else {
                    folia_visualizer::color(self.theme.secondary, alpha)
                },
                size: 12.0.into(),
                font: MEDIUM,
                align_x: iced::widget::text::Alignment::Left,
                align_y: iced::alignment::Vertical::Top,
                ..Text::default()
            });
        }
        vec![frame.into_geometry()]
    }

    fn update(
        &self,
        state: &mut HotspotState,
        event: &canvas::Event,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Option<canvas::Action<crate::Message>> {
        match event {
            canvas::Event::Mouse(mouse::Event::CursorMoved { .. }) => {
                let over = cursor.position_in(bounds).and_then(|at| hotspot_at(bounds.size(), at));
                let now = Instant::now();
                if state.hover.map(|(shown, _)| shown) != over {
                    if let Some((shown, _)) = state.hover {
                        state.leave = Some((shown, now));
                    }
                    state.hover = over.map(|shown| (shown, now));
                    return Some(canvas::Action::request_redraw_at(now + Duration::from_millis(16)));
                }
                let fading = state.hover.is_some_and(|(_, at)| now.saturating_duration_since(at) < HOTSPOT_FADE)
                    || state.leave.is_some_and(|(_, at)| now.saturating_duration_since(at) < HOTSPOT_FADE);
                fading.then(|| canvas::Action::request_redraw_at(now + Duration::from_millis(16)))
            }
            canvas::Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
                let at = cursor.position_in(bounds)?;
                let hotspot = hotspot_at(bounds.size(), at)?;
                Some(canvas::Action::publish(crate::Message::Workbench(Message::Tab(hotspot.tab()))).and_capture())
            }
            _ => None,
        }
    }

    fn mouse_interaction(&self, _state: &HotspotState, bounds: Rectangle, cursor: mouse::Cursor) -> mouse::Interaction {
        match cursor.position_in(bounds).and_then(|at| hotspot_at(bounds.size(), at)) {
            Some(_) => mouse::Interaction::Pointer,
            None => mouse::Interaction::None,
        }
    }
}
