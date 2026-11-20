// The settings window, after the original `SettingsModal`: a side list of categories, a page of option cards, search.

use crate::typeface::text;
use crate::typeface::text_input;
use std::time::Instant;

use folia_core::{Language, LanguagePreference, Theme};
use folia_i18n::I18n;
use folia_store::{AppearanceSettings, FrameRate, ThemeMode};
use folia_visualizer::{Ease, Style, tween};
use iced::widget::scrollable::Viewport;
use iced::widget::{Id, button, column, container, mouse_area, opaque, row, scrollable, space, stack};
use iced::{Alignment, Border, Color, Element, Length, Padding, Shadow, Size, Task, Vector, mouse};
use lucide_static_svg::Icon;

use crate::icons::icon;
use crate::settings_kit::{self as kit, Choice, Group, Row, Tokens};
use crate::smooth_scroll::{self, Scroll};

/// `KIT_MOTION.move` and the window's own fade (`shellTransition`), in seconds.
const MOVE: f64 = 0.22;
const SHELL: f64 = 0.24;
/// `KIT_EASE.out`.
const KIT_OUT: Ease = Ease::new(0.22, 1.0, 0.36, 1.0);
const SIDE_WIDTH: f32 = 212.0;
const WIDE_WIDTH: f32 = 768.0;
const SCROLL: &str = "settings-page";
const THEMES: [Option<ThemeMode>; 3] = [Some(ThemeMode::Light), Some(ThemeMode::Dark), None];
const LIMITS: [FrameRate; 5] =
    [FrameRate::Fps30, FrameRate::Fps45, FrameRate::Fps60, FrameRate::Fps90, FrameRate::Fps120];
/// What switching the limit on picks, `FRAME_RATE_WHEN_SWITCHED_ON`.
const LIMIT_WHEN_SWITCHED_ON: FrameRate = FrameRate::Fps120;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Section {
    Remote,
    Appearance,
    Player,
    General,
    Integration,
    Storage,
    Graphics,
    Developer,
    Sources,
    Ai,
}

impl Section {
    fn label_key(self) -> &'static str {
        match self {
            Section::Remote => "options.settingsPageRemote",
            Section::Appearance => "options.settingsPageAppearance",
            Section::Player => "options.settingsPagePlayer",
            Section::General => "options.settingsPageGeneral",
            Section::Integration => "options.settingsPageIntegration",
            Section::Storage => "options.settingsPageStorage",
            Section::Graphics => "options.settingsPageGraphics",
            Section::Developer => "options.settingsPageDeveloper",
            Section::Sources => "options.customSources",
            Section::Ai => "options.settingsPageAi",
        }
    }

    fn icon(self) -> Icon {
        match self {
            Section::Remote => Icon::Radio,
            Section::Appearance => Icon::Contrast,
            Section::Player => Icon::MonitorPlay,
            Section::General => Icon::SlidersHorizontal,
            Section::Integration => Icon::Plug,
            Section::Storage => Icon::HardDrive,
            Section::Graphics => Icon::Gauge,
            Section::Developer => Icon::Terminal,
            Section::Sources => Icon::ListMusic,
            Section::Ai => Icon::Sparkles,
        }
    }
}

/// The side list's groups, `SETTINGS_NAV_GROUP_SPECS` without the categories Folia-rust has nothing on yet.
const NAV: [(&str, &[Section]); 3] = [
    ("options.settingsGroupInterface", &[Section::Appearance, Section::Player, Section::Remote]),
    (
        "options.settingsGroupApp",
        &[Section::General, Section::Graphics, Section::Sources, Section::Ai, Section::Integration, Section::Storage],
    ),
    ("options.settingsGroupAdvanced", &[Section::Developer]),
];

/// A group of one page: its anchor id, heading and whether it is drawn without the heading.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct GroupDef {
    id: &'static str,
    title: &'static str,
    bare: bool,
}

const LYRICS: GroupDef = GroupDef { id: "lyricsRenderer", title: "options.lyricsRenderer", bare: true };
const THEME: GroupDef = GroupDef { id: "themePresets", title: "options.settingsHeadingTheme", bare: false };
const LANGUAGE: GroupDef = GroupDef { id: "languageSettings", title: "options.languageSettings", bare: true };
const BAR: GroupDef = GroupDef { id: "bottomUiSettings", title: "options.settingsHeadingControlBar", bare: false };
const PERFORMANCE: GroupDef =
    GroupDef { id: "graphicsPerformance", title: "options.labPerformanceSection", bare: false };
const MOTION: GroupDef = GroupDef { id: "graphicsMotion", title: "options.reduceMotionSection", bare: false };
const DEVELOPER: GroupDef = GroupDef { id: "developerTools", title: "options.developerTools", bare: true };
const SOURCES: GroupDef = GroupDef { id: "customSources", title: "options.customSources", bare: true };
const WALL: GroupDef = GroupDef { id: "libraryWallLook", title: "libraryBravais.toolsAppearance", bare: false };
const SONG_THEME: GroupDef = GroupDef { id: "songTheme", title: "options.settingsHeadingSongTheme", bare: false };
const AI: GroupDef = GroupDef { id: "electronSettings", title: "options.aiServiceProviders", bare: true };
const SERVICES: GroupDef = GroupDef { id: "services", title: "options.settingsHeadingServices", bare: false };
const MEDIA: GroupDef = GroupDef { id: "mediaCache", title: "options.mediaCache", bare: false };

/// Every option the pages draw, in page order: what the search finds.
const REMOTE: GroupDef = GroupDef { id: "remoteControl", title: "options.remoteControlWindowGroup", bare: false };
const REMOTE_LYRICS: GroupDef = GroupDef { id: "remoteLyrics", title: "options.remoteLyricsGroup", bare: false };

const OPTIONS: &[(Section, GroupDef, &str)] = &[
    (Section::Remote, REMOTE, "options.hideRemoteControlTaskbarIcon"),
    (Section::Remote, REMOTE, "options.hideRemoteControlTitlebar"),
    (Section::Remote, REMOTE, "options.remoteControlClickThrough"),
    (Section::Remote, REMOTE, "options.remoteControlExpandCover"),
    (Section::Remote, REMOTE, "options.remoteControlTextShade"),
    (Section::Remote, REMOTE_LYRICS, "options.remoteLyricPairs"),
    (Section::Remote, REMOTE_LYRICS, "options.remoteLyricRow1"),
    (Section::Remote, REMOTE_LYRICS, "options.remoteLyricRow2"),
    (Section::Remote, REMOTE_LYRICS, "options.remoteLyricRow3"),
    (Section::Remote, REMOTE_LYRICS, "options.remoteLyricRow4"),
    (Section::Storage, MEDIA, "options.cacheDirectory"),
    (Section::Integration, SERVICES, "options.neteaseScrobble"),
    (Section::Ai, AI, "options.aiProvider"),
    (Section::Ai, AI, "options.aiApiUrl"),
    (Section::Ai, AI, "options.aiModel"),
    (Section::Ai, AI, "options.aiApiKey"),
    (Section::Ai, AI, "options.openaiApiTemperature"),
    (Section::Ai, AI, "options.useSystemProxyAI"),
    (Section::Sources, SOURCES, "options.sourceCustomOnly"),
    (Section::Sources, SOURCES, "options.sourceImportUrl"),
    (Section::Sources, SOURCES, "options.sourceImport"),
    (Section::Appearance, LYRICS, "options.lyricsAnimationAdjust"),
    (Section::Appearance, LYRICS, "ui.scene"),
    (Section::Appearance, THEME, "options.daylightMode"),
    (Section::Appearance, THEME, "options.themePresets"),
    (Section::Appearance, THEME, "options.openThemePark"),
    (Section::Appearance, SONG_THEME, "options.autoSwitchSongTheme"),
    (Section::Appearance, SONG_THEME, "options.autoGenerateSongTheme"),
    (Section::Appearance, SONG_THEME, "options.themeGenerationSource"),
    (Section::Appearance, SONG_THEME, "options.preferCustomTheme"),
    (Section::Appearance, WALL, "options.libraryWallLook"),
    (Section::Appearance, WALL, "options.libraryWallWindows"),
    (Section::Appearance, WALL, "options.bravaisSeamClear"),
    (Section::Appearance, WALL, "options.bravaisBackdropLyrics"),
    (Section::Appearance, WALL, "options.bravaisBackdropBlur"),
    (Section::Appearance, WALL, "options.latticePosterTint"),
    (Section::Appearance, WALL, "options.latticePosterTintCustomColor"),
    (Section::Appearance, WALL, "options.latticePosterTintColor"),
    (Section::Appearance, WALL, "options.latticePosterTintIntensity"),
    (Section::Appearance, WALL, "options.latticeWallForm"),
    (Section::Appearance, WALL, "options.latticeAutoFocusOnSongChange"),
    (Section::Appearance, WALL, "options.latticeVignette"),
    (Section::Appearance, WALL, "options.latticeBadgeCurrentOnly"),
    (Section::Appearance, WALL, "options.latticeRadioHideUpcoming"),
    (Section::Appearance, WALL, "wallTools.preset"),
    (Section::Player, BAR, "options.playerBottomBarOffset"),
    (Section::Player, BAR, "options.playerBottomBarReposition"),
    (Section::General, LANGUAGE, "options.appLanguage"),
    (Section::Graphics, PERFORMANCE, "options.visualizerFrameRate"),
    (Section::Graphics, PERFORMANCE, "options.visualizerFrameRateValue"),
    (Section::Graphics, PERFORMANCE, "options.enableStaticMode"),
    (Section::Graphics, PERFORMANCE, "options.enablePlayerPageNativeBlur"),
    (Section::Graphics, PERFORMANCE, "options.lyricRenderScale"),
    (Section::Graphics, PERFORMANCE, "options.lyricAntialias"),
    (Section::Graphics, PERFORMANCE, "options.backgroundThrottling"),
    (Section::Graphics, PERFORMANCE, "options.gpuPreference"),
    (Section::Graphics, MOTION, "options.reduceMotionFollowSystem"),
    (Section::Graphics, MOTION, "options.reduceMotionCollectionMorph"),
    (Section::Graphics, MOTION, "options.reduceMotionMonetBackground"),
    (Section::Graphics, MOTION, "options.reduceMotionUiMicroMotion"),
    (Section::Graphics, MOTION, "options.reduceMotionSettingsScroll"),
    (Section::Developer, DEVELOPER, "options.consoleLogCapture"),
    (Section::Developer, DEVELOPER, "options.debugLogsToFile"),
    (Section::Developer, DEVELOPER, "options.memoryMonitor"),
    (Section::Developer, DEVELOPER, "options.memoryMonitorInterval"),
];

#[derive(Debug, Clone)]
pub enum Message {
    Section(Section),
    Query(String),
    /// A search hit: the category, and the group to land on.
    Hit(Section, Option<&'static str>),
    Tab(&'static str),
    Scrolled(Viewport),
    Wheel(mouse::ScrollDelta),
    Measured(Section, &'static str, f32),
    AskNativeBlur,
    CancelNativeBlur,
    Escape,
    Close,
    Frame,
}

/// Parts whose mark glides to a new place.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Control {
    Theme,
    Limit,
    LimitValue,
    Tab,
    Graphics(&'static str),
}

#[derive(Debug, Clone, Copy)]
struct Motion {
    control: Control,
    from: f32,
    to: f32,
    at: Instant,
}

/// What the pages show and change; the shell owns the values.
pub struct Values<'a> {
    pub remote: &'a folia_store::RemoteSettings,
    pub remote_taskbar_available: bool,
    pub storage: &'a crate::storage::Storage,
    pub scrobble_enabled: bool,
    pub scrobble_ready: bool,
    pub sources: &'a crate::sources::Sources,
    pub language: LanguagePreference,
    pub resolved: Language,
    pub appearance: &'a AppearanceSettings,
    pub graphics: &'a folia_store::GraphicsSettings,
    pub applied_gpu: folia_store::GpuPreference,
    pub system_reduced_motion: bool,
    pub theme_settings: &'a folia_store::ThemeSettings,
    pub song_themes: &'a crate::song_theme::SongThemes,
    pub style: Option<Style>,
    pub scene: Option<folia_core::SceneId>,
    /// `playerBottomBarOffset` and its limit for this window.
    pub bar_offset: f32,
    pub bar_max: f32,
    /// A song is loaded, so the bar is on screen to be dragged.
    pub has_track: bool,
    /// The developer page's switches and the state of its two panels.
    pub developer: &'a crate::developer::Developer,
    pub developer_settings: &'a folia_store::DeveloperSettings,
    pub runtime_log: &'a folia_store::RuntimeLog,
    pub memory_log_path: &'a std::path::Path,
}

pub struct Settings {
    section: Section,
    query: String,
    tab: Option<&'static str>,
    opened: Instant,
    /// One animation sample per update; layout/scroll feedback must not advance the same frame.
    frame_time: Instant,
    closing: Option<Instant>,
    page_at: Instant,
    motions: Vec<Motion>,
    body: Scroll,
    group_sizes: Vec<(Section, &'static str, f32)>,
    pending_group: Option<&'static str>,
    landing: Option<(&'static str, Instant)>,
    picked_group: Option<(&'static str, Instant)>,
    scroll_view: Option<(f32, f32)>,
    asking_native_blur: bool,
}

impl Settings {
    pub fn open() -> Self {
        let now = Instant::now();
        Self {
            section: Section::Appearance,
            query: String::new(),
            tab: None,
            opened: now,
            frame_time: now,
            closing: None,
            page_at: now,
            motions: Vec::new(),
            body: Scroll::default(),
            group_sizes: Vec::new(),
            pending_group: None,
            landing: None,
            picked_group: None,
            scroll_view: None,
            asking_native_blur: false,
        }
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        if !matches!(message, Message::Scrolled(_) | Message::Measured(..)) || !self.animating() {
            self.frame_time = Instant::now();
        }
        let now = self.frame_time;
        match message {
            Message::AskNativeBlur => {
                self.asking_native_blur = true;
                Task::none()
            }
            Message::CancelNativeBlur => {
                self.asking_native_blur = false;
                Task::none()
            }
            Message::Escape | Message::Close if self.asking_native_blur => {
                self.asking_native_blur = false;
                Task::none()
            }
            Message::Section(section) => {
                self.show(section, None, now);
                self.scroll_to(None)
            }
            Message::Hit(section, group) => {
                self.query.clear();
                self.show(section, group, now);
                self.landing = group.map(|id| (id, now));
                self.picked_group = self.landing;
                self.scroll_to(group)
            }
            Message::Query(query) => {
                self.query = query;
                self.page_at = now;
                Task::none()
            }
            Message::Tab(id) => {
                self.landing = Some((id, now));
                self.picked_group = self.landing;
                self.land(id, now);
                self.scroll_to(Some(id))
            }
            Message::Scrolled(viewport) => {
                let offset = viewport.absolute_offset().y;
                self.scroll_view = Some((viewport.content_bounds().height, viewport.bounds().height));
                if self.body.scrolled(&viewport, now) {
                    self.picked_group = None;
                }
                self.follow_scroll(offset, now);
                Task::none()
            }
            Message::Wheel(delta) => {
                self.picked_group = None;
                self.pending_group = None;
                self.body.wheel(delta, now);
                if crate::motion::reduced(folia_store::MotionSurface::SettingsScroll) {
                    self.body.finish();
                }
                self.body.apply(Id::new(SCROLL))
            }
            Message::Measured(section, id, height) => {
                self.group_sizes.retain(|(owner, group, _)| *owner != section || *group != id);
                self.group_sizes.push((section, id, height));
                if section == self.section && self.pending_group.is_some() {
                    return self.scroll_to(self.pending_group);
                }
                Task::none()
            }
            Message::Escape if !self.query.is_empty() => {
                self.query.clear();
                Task::none()
            }
            Message::Escape | Message::Close => {
                self.closing.get_or_insert(now);
                Task::none()
            }
            Message::Frame => {
                if crate::motion::reduced(folia_store::MotionSurface::SettingsScroll) {
                    self.body.finish();
                }
                self.motions.retain(|motion| seconds(motion.at, now) < MOVE);
                let moved = self.body.frame(now);
                self.follow_scroll(self.body.offset(), now);
                match moved {
                    Some(_) => self.body.apply(Id::new(SCROLL)),
                    None => Task::none(),
                }
            }
        }
    }

    /// Glides the marks of whatever the change moved.
    pub fn changed(&mut self, before: &AppearanceSettings, after: &AppearanceSettings) {
        self.frame_time = Instant::now();
        let now = self.frame_time;
        let theme_index = |settings: &AppearanceSettings| {
            let mode = (!settings.follow_system_theme).then_some(settings.theme);
            THEMES.iter().position(|m| *m == mode).unwrap_or(0) as f32
        };
        let limited = |rate: FrameRate| if rate.fps().is_some() { 1.0 } else { 0.0 };
        let limit_index = |rate: FrameRate| LIMITS.iter().position(|r| *r == rate).map(|i| i as f32);
        if before.theme != after.theme || before.follow_system_theme != after.follow_system_theme {
            self.start(Control::Theme, theme_index(before), theme_index(after), now);
        }
        if limited(before.frame_rate) != limited(after.frame_rate) {
            self.start(Control::Limit, limited(before.frame_rate), limited(after.frame_rate), now);
        }
        if let (Some(from), Some(to)) = (limit_index(before.frame_rate), limit_index(after.frame_rate)) {
            self.start(Control::LimitValue, from, to, now);
        }
    }

    pub fn graphics_changed(&mut self, before: &folia_store::GraphicsSettings, after: &folia_store::GraphicsSettings) {
        self.frame_time = Instant::now();
        let controls = |settings: &folia_store::GraphicsSettings| {
            [
                ("options.enableStaticMode", settings.static_mode),
                ("options.enablePlayerPageNativeBlur", settings.enable_player_page_native_blur),
                ("options.backgroundThrottling", settings.background_throttling),
                ("options.reduceMotionFollowSystem", settings.motion.follow_system),
                ("options.reduceMotionCollectionMorph", settings.motion.collection_morph),
                ("options.reduceMotionMonetBackground", settings.motion.monet_background),
                ("options.reduceMotionUiMicroMotion", settings.motion.ui_micro_motion),
                ("options.reduceMotionSettingsScroll", settings.motion.settings_scroll),
            ]
        };
        for ((key, from), (_, to)) in controls(before).into_iter().zip(controls(after)) {
            if from != to {
                self.start(Control::Graphics(key), f32::from(from), f32::from(to), self.frame_time);
            }
        }
    }

    /// The window has faded out and can be dropped.
    pub fn finished(&self) -> bool {
        self.closing.is_some_and(|at| seconds(at, self.frame_time) >= SHELL)
    }

    /// Keep requesting frames until the sampled view reaches the end, even after a late redraw.
    pub fn animating(&self) -> bool {
        let now = self.frame_time;
        seconds(self.opened, now) < SHELL
            || self.closing.is_some()
            || seconds(self.page_at, now) < MOVE
            || self.motions.iter().any(|motion| seconds(motion.at, now) < MOVE)
            || self.body.animating(now)
            || self.landing.is_some_and(|(_, at)| now.saturating_duration_since(at).as_secs_f64() < 2.2)
    }

    /// The developer page is on screen, so its log panel needs refreshing while it sits still.
    pub fn shows_developer(&self) -> bool {
        self.query.is_empty() && self.section == Section::Developer
    }

    pub fn dismiss_native_blur(&mut self) {
        self.asking_native_blur = false;
    }

    /// Keeps the group's heading 12px below the viewport, matching the original anchor landing.
    fn scroll_to(&mut self, group: Option<&'static str>) -> Task<Message> {
        let groups = groups_of(self.section);
        let before = groups.iter().take_while(|g| Some(g.id) != group).count();
        let before = if group.is_some() && before < groups.len() { before } else { 0 };
        self.pending_group = group.filter(|_| groups[..before].iter().any(|g| self.measured_height(g).is_none()));
        let y: f32 = groups[..before].iter().enumerate().map(|(index, g)| self.group_height(g, index == 0)).sum();
        let margin = if before == 0 { kit::FIRST_GROUP_GAP } else { kit::GROUP_GAP };
        self.body.scroll_to(if group.is_some() { (y + margin - 12.0).max(0.0) } else { 0.0 }, self.frame_time);
        if crate::motion::reduced(folia_store::MotionSurface::SettingsScroll) {
            self.body.finish();
        }
        self.body.apply(Id::new(SCROLL))
    }

    fn show(&mut self, section: Section, group: Option<&'static str>, now: Instant) {
        self.landing = None;
        self.picked_group = None;
        self.scroll_view = None;
        if section != self.section || group.is_none() {
            self.page_at = now;
            self.motions.retain(|motion| motion.control != Control::Tab);
        }
        self.section = section;
        self.tab = group;
    }

    fn land(&mut self, id: &'static str, now: Instant) {
        let groups = groups_of(self.section);
        let index = |id: Option<&str>| groups.iter().position(|g| Some(g.id) == id).unwrap_or(0) as f32;
        let from = self.at(Control::Tab, index(self.tab), now);
        self.start(Control::Tab, from, index(Some(id)), now);
        self.tab = Some(id);
    }

    /// Moves the tab line to the group whose card the page has scrolled to.
    fn follow_scroll(&mut self, offset: f32, now: Instant) {
        if self.picked_group.is_some_and(|(_, at)| now.saturating_duration_since(at).as_secs_f64() < 0.8) {
            return;
        }
        let groups = groups_of(self.section);
        let (content, height) = self.scroll_view.unwrap_or((f32::INFINITY, 0.0));
        let bottom = offset > 0.0 && content - offset - height <= 2.0;
        let mut top = 0.0;
        let mut current = groups.first().map(|g| g.id);
        for (index, group) in groups.iter().enumerate() {
            let anchor = top + if index == 0 { kit::FIRST_GROUP_GAP } else { kit::GROUP_GAP };
            let end = top + self.group_height(group, index == 0);
            if bottom {
                current = Some(group.id);
                if self.picked_group.is_some_and(|(id, _)| id == group.id) && end > offset && anchor < offset + height {
                    break;
                }
            } else if anchor <= offset + height * 0.1 {
                current = Some(group.id);
            }
            top = end;
        }
        if let Some(id) = current.filter(|id| Some(*id) != self.tab.or(groups.first().map(|g| g.id))) {
            self.land(id, now);
        }
    }

    fn landing_highlight(&self, id: &str, now: Instant) -> f32 {
        let Some((_, at)) = self.landing.filter(|(target, _)| *target == id) else { return 0.0 };
        let progress = (now.saturating_duration_since(at).as_secs_f64() / 2.2).clamp(0.0, 1.0);
        if crate::motion::reduced(folia_store::MotionSurface::UiMicroMotion) {
            return if progress < 1.0 { 1.0 } else { 0.0 };
        }
        (1.0 - ((Ease::OUT.at(progress) - 0.7) / 0.3).clamp(0.0, 1.0)) as f32
    }

    fn start(&mut self, control: Control, from: f32, to: f32, now: Instant) {
        let from = self.at(control, from, now);
        self.motions.retain(|motion| motion.control != control);
        self.motions.push(Motion { control, from, to, at: now });
    }

    fn measured_height(&self, group: &GroupDef) -> Option<f32> {
        self.group_sizes
            .iter()
            .find(|(section, id, _)| *section == self.section && *id == group.id)
            .map(|(_, _, height)| *height)
    }

    fn group_height(&self, group: &GroupDef, first: bool) -> f32 {
        self.measured_height(group).unwrap_or_else(|| group_height(self.section, group, first))
    }

    /// Where `control`'s mark is drawn now; `rest` when it is not moving.
    fn at(&self, control: Control, rest: f32, now: Instant) -> f32 {
        let Some(motion) = self.motions.iter().find(|motion| motion.control == control) else { return rest };
        tween(f64::from(motion.from), f64::from(motion.to), seconds(motion.at, now), MOVE, KIT_OUT) as f32
    }

    /// How far the window has come in (1) or gone out (0).
    fn shown(&self, now: Instant) -> f32 {
        let opening = tween(0.0, 1.0, seconds(self.opened, now), SHELL, Ease::OUT);
        let closing = self.closing.map_or(1.0, |at| tween(1.0, 0.0, seconds(at, now), SHELL, Ease::OUT));
        opening.min(closing) as f32
    }

    pub fn view<'a>(
        &'a self,
        i18n: &'a I18n,
        theme: &Theme,
        daylight: bool,
        window: Size,
        values: Values<'a>,
    ) -> Element<'a, crate::Message> {
        let now = self.frame_time;
        let shown = self.shown(now);
        let tokens = Tokens::new(theme, daylight).faded(shown);
        let wide = window.width >= WIDE_WIDTH;
        let page_in = tween(0.0, 1.0, seconds(self.page_at, now), MOVE, KIT_OUT) as f32;

        let side = self.side_list(i18n, tokens, wide);
        let body = column![
            self.header(i18n, tokens, wide),
            self.content(i18n, tokens.faded(page_in), page_in, &values, wide, now)
        ]
        .width(Length::Fill)
        .height(Length::Fill);
        let inner: Element<'a, crate::Message> = if wide {
            row![
                side,
                container(space()).width(1).height(Length::Fill).style(move |_| kit::rounded(tokens.line, 0.0)),
                body
            ]
            .into()
        } else {
            column![side, kit::hairline(tokens), body].into()
        };

        let (pad_x, pad_y) = if window.width >= 640.0 { (20.0, 48.0) } else { (16.0, 32.0) };
        let width = if wide { 960.0_f32.min(window.width - pad_x * 2.0) } else { window.width - pad_x * 2.0 };
        let height = window.height * if wide { 0.86 } else { 0.9 };
        let shadow_alpha = 0.5 * shown;
        let panel = container(inner).width(width).height(height).style(move |_| container::Style {
            background: Some(tokens.win.into()),
            border: Border { color: tokens.line2, width: 1.0, radius: 16.0.into() },
            shadow: Shadow {
                color: Color { a: shadow_alpha, ..Color::BLACK },
                offset: Vector::new(0.0, 36.0),
                blur_radius: 90.0,
            },
            text_color: Some(tokens.t1),
            ..container::Style::default()
        });
        let lift = 20.0 * (1.0 - shown);
        let panel = container(opaque(panel))
            .width(Length::Fill)
            .height(Length::Fill)
            .padding(Padding { top: pad_y + lift, bottom: (pad_y - lift).max(0.0), left: pad_x, right: pad_x })
            .center_x(Length::Fill)
            .center_y(Length::Fill);
        let veil_alpha = if daylight { 0.32 } else { 0.5 } * shown;
        let backdrop = container(space()).width(Length::Fill).height(Length::Fill).style(move |_| container::Style {
            background: Some(Color { a: veil_alpha, ..Color::BLACK }.into()),
            ..container::Style::default()
        });
        let backdrop = opaque(mouse_area(backdrop).on_press(crate::Message::Settings(Message::Close)));
        let mut layers = stack![backdrop, panel];
        if self.asking_native_blur {
            layers = layers.push(crate::graphics::native_blur_question(i18n, tokens, window));
        }
        layers.into()
    }

    fn side_list<'a>(&'a self, i18n: &'a I18n, tokens: Tokens, wide: bool) -> Element<'a, crate::Message> {
        let search = self.search_field(i18n, tokens);
        let active = self.query.is_empty().then_some(self.section);
        if !wide {
            let strip =
                NAV.iter().flat_map(|(_, sections)| sections.iter()).map(|s| nav_item(i18n, tokens, *s, active, false));
            let strip = scrollable(row(strip).spacing(4))
                .direction(scrollable::Direction::Horizontal(scrollable::Scrollbar::hidden()))
                .width(Length::Fill);
            return container(column![search, space().height(8), strip])
                .padding(Padding { top: 12.0, right: 12.0, bottom: 8.0, left: 12.0 })
                .width(Length::Fill)
                .style(move |_| container::Style {
                    background: Some(tokens.side.into()),
                    border: Border {
                        radius: iced::border::Radius::default().top_left(16.0).top_right(16.0),
                        ..Border::default()
                    },
                    ..container::Style::default()
                })
                .into();
        }
        let mut nav = column![space().height(4)];
        for (group_key, sections) in NAV {
            let label = text(i18n.t(group_key)).size(crate::typeface::size::HINT).color(tokens.t3);
            nav = nav.push(container(label).padding(Padding { top: 14.0, right: 10.0, bottom: 4.0, left: 10.0 }));
            let items = sections.iter().map(|section| nav_item(i18n, tokens, *section, active, true));
            nav = nav.push(column(items).spacing(1));
        }
        container(column![search, nav])
            .width(SIDE_WIDTH)
            .height(Length::Fill)
            .padding(Padding { top: 14.0, right: 10.0, bottom: 10.0, left: 10.0 })
            .style(move |_| container::Style {
                background: Some(tokens.side.into()),
                border: Border {
                    radius: iced::border::Radius::default().top_left(16.0).bottom_left(16.0),
                    ..Border::default()
                },
                ..container::Style::default()
            })
            .into()
    }

    fn search_field<'a>(&'a self, i18n: &'a I18n, tokens: Tokens) -> Element<'a, crate::Message> {
        let input = text_input(i18n.t("options.settingsSearchPlaceholder"), &self.query)
            .on_input(|query| crate::Message::Settings(Message::Query(query)))
            .size(crate::typeface::size::CONTROL)
            .line_height(crate::typeface::leading::PX_18_0)
            .padding(Padding { top: 6.0, right: 30.0, bottom: 6.0, left: 32.0 })
            .style(move |_, status| text_input::Style {
                background: tokens.fill.into(),
                border: Border {
                    color: if matches!(status, text_input::Status::Focused { .. }) {
                        tokens.line2
                    } else {
                        Color::TRANSPARENT
                    },
                    width: 1.0,
                    radius: 8.0.into(),
                },
                icon: tokens.t3,
                placeholder: tokens.t3,
                value: tokens.t1,
                selection: Color { a: 0.3 * tokens.accent.a, ..tokens.accent },
            });
        let glass = container(icon(Icon::Search, 14.0, tokens.t3)).padding(Padding::ZERO.left(10.0)).center_y(32);
        let mut field = stack![input, glass];
        if !self.query.is_empty() {
            let clear = button(icon(Icon::X, 14.0, tokens.t3))
                .padding(0)
                .style(|_, _| button::Style::default())
                .on_press(crate::Message::Settings(Message::Query(String::new())));
            field = field.push(
                container(clear)
                    .width(Length::Fill)
                    .align_x(Alignment::End)
                    .padding(Padding::ZERO.right(10.0))
                    .center_y(32),
            );
        }
        field.into()
    }

    fn header<'a>(&'a self, i18n: &'a I18n, tokens: Tokens, wide: bool) -> Element<'a, crate::Message> {
        let gutter = Gutter::of(wide);
        let searching = !self.query.is_empty();
        let title = if searching { i18n.t("options.settingsSearchTitle") } else { i18n.t(self.section.label_key()) };
        let groups = groups_of(self.section);
        let has_tabs = !searching && groups.len() > 1;
        let close = kit::icon_button(tokens, Icon::X, 16.0, crate::Message::Settings(Message::Close));
        let title = crate::typeface::PAGE_TITLE.text(title);
        let plain = !searching && !has_tabs;
        let top = row![title, space().width(Length::Fill), close].align_y(Alignment::Center).padding(Padding {
            top: gutter.title_top,
            right: gutter.close_right,
            bottom: if plain { gutter.title_top - 4.0 } else { 0.0 },
            left: gutter.side,
        });
        if searching {
            let hits = search(i18n, &self.query).len();
            let summary = if hits > 0 {
                i18n.format("options.settingsSearchCount", &[("n", &hits.to_string())])
            } else {
                i18n.t("options.settingsSearchEmpty").to_owned()
            };
            let summary = container(text(summary).size(crate::typeface::size::CONTROL).color(tokens.t2))
                .height(36)
                .align_y(Alignment::Center);
            return column![
                top,
                container(summary).padding(Padding {
                    top: gutter.under_title,
                    left: gutter.side,
                    right: gutter.side,
                    bottom: 0.0
                }),
                kit::hairline(tokens)
            ]
            .into();
        }
        if !has_tabs {
            return column![top, kit::hairline(tokens)].into();
        }
        let chosen = groups.iter().position(|g| Some(g.id) == self.tab).unwrap_or(0);
        let mark = self.at(Control::Tab, chosen as f32, self.frame_time);
        let entries =
            groups.iter().map(|g| (i18n.t(g.title).to_owned(), crate::Message::Settings(Message::Tab(g.id)))).collect();
        column![top, kit::tabs(tokens, entries, Some(chosen), Some(mark), gutter.under_title + 4.0, gutter.side)].into()
    }

    fn content<'a>(
        &'a self,
        i18n: &'a I18n,
        tokens: Tokens,
        page_in: f32,
        values: &Values<'a>,
        wide: bool,
        now: Instant,
    ) -> Element<'a, crate::Message> {
        let rise = 6.0 * (1.0 - page_in);
        let body: Element<'a, crate::Message> = if self.query.is_empty() {
            let groups = self.page(i18n, tokens, values, now);
            let definitions = groups_of(self.section);
            column(groups.into_iter().enumerate().map(|(index, group)| {
                let highlight = self.landing_highlight(definitions[index].id, now);
                let content = if self.section == Section::Sources {
                    values.sources.group(tokens, group, index == 0, highlight)
                } else if self.section == Section::Appearance && index == 0 {
                    kit::group_with_rows(tokens, group, true, highlight, |rows| {
                        column(rows.into_iter().zip([crate::Message::OpenWorkbench, crate::Message::OpenScene]).map(
                            |(row, message)| {
                                mouse_area(row).on_press(message).interaction(mouse::Interaction::Pointer).into()
                            },
                        ))
                        .into()
                    })
                } else {
                    kit::landed_group(tokens, group, index == 0, highlight)
                };
                let definition = definitions[index];
                let section = self.section;
                crate::measured::measured(content, self.group_height(&definition, index == 0), move |height| {
                    crate::Message::Settings(Message::Measured(section, definition.id, height))
                })
            }))
            .into()
        } else {
            search_results(i18n, tokens, &self.query)
        };
        let gutter = Gutter::of(wide);
        let body = container(body).padding(Padding {
            top: rise,
            right: gutter.side,
            bottom: gutter.foot - rise,
            left: gutter.side,
        });
        let page = scrollable(body)
            .id(SCROLL)
            .height(Length::Fill)
            .direction(crate::settings_kit::scrollbar())
            .on_scroll(|viewport| crate::Message::Settings(Message::Scrolled(viewport)))
            .style(crate::settings_kit::scroll_style(tokens));
        smooth_scroll::smooth(page, |delta| crate::Message::Settings(Message::Wheel(delta)))
    }

    fn page<'a>(
        &'a self,
        i18n: &'a I18n,
        tokens: Tokens,
        values: &Values<'a>,
        now: Instant,
    ) -> Vec<Group<'a, crate::Message>> {
        let group =
            |def: GroupDef, rows| Group { title: i18n.t(def.title).to_owned(), bare: def.bare, rows, foot: None };
        let row = |label: &str, hint: Option<String>, sub: bool, control| Row {
            note: None,
            label: i18n.t(label).to_owned(),
            hint,
            sub,
            below: None,
            control,
        };
        match self.section {
            Section::Remote => {
                crate::remote_settings::groups(values.remote, values.remote_taskbar_available, i18n, tokens)
            }
            Section::Ai => vec![crate::ai_settings::connection_group(values.song_themes, i18n, tokens)],
            Section::Storage => vec![values.storage.group(i18n, tokens)],
            Section::Appearance => {
                let style = iced::widget::row![
                    kit::value(tokens, i18n.t(values.style.map_or("ui.visualizerRandom", Style::label_key)).to_owned()),
                    icon(Icon::ChevronRight, 16.0, tokens.t2)
                ]
                .spacing(8)
                .align_y(Alignment::Center)
                .into();
                let scene = iced::widget::row![
                    kit::value(
                        tokens,
                        i18n.t(if values.scene.is_some() { "ui.sceneCinema" } else { "ui.sceneNone" }).to_owned()
                    ),
                    icon(Icon::ChevronRight, 16.0, tokens.t2)
                ]
                .spacing(8)
                .align_y(Alignment::Center)
                .into();
                let themes = THEMES.into_iter().map(|mode| (mode, i18n.t(theme_key(mode)).to_owned())).collect();
                let theme_choice = (!values.appearance.follow_system_theme).then_some(values.appearance.theme);
                let theme_rest = THEMES.iter().position(|m| *m == theme_choice).unwrap_or(0) as f32;
                let mark = self.at(Control::Theme, theme_rest, now);
                let daylight = kit::segmented(tokens, themes, theme_choice, Some(mark), |mode| {
                    mode.map_or(crate::Message::FollowSystemTheme, crate::Message::Theme)
                });
                let source = kit::segmented_available(
                    tokens,
                    vec![
                        (folia_store::ThemeSource::Default, i18n.t("options.themePresetsDefault").to_owned()),
                        (folia_store::ThemeSource::Custom, i18n.t("options.customTheme").to_owned()),
                    ],
                    values.theme_settings.source,
                    if values.theme_settings.custom_dual_theme.is_none() {
                        &[folia_store::ThemeSource::Custom]
                    } else {
                        &[]
                    },
                    |source| crate::Message::Panel(crate::panel::Message::ThemeSource(source)),
                );
                let editor = kit::icon_button(tokens, Icon::ChevronRight, 16.0, crate::Message::OpenThemePark);
                vec![
                    group(
                        LYRICS,
                        vec![
                            row(
                                "options.lyricsAnimationAdjust",
                                Some(i18n.t("options.lyricsRendererDesc").to_owned()),
                                false,
                                style,
                            ),
                            row("ui.scene", None, false, scene),
                        ],
                    ),
                    group(
                        THEME,
                        vec![
                            row(
                                "options.daylightMode",
                                Some(i18n.t("options.followSystemThemeDesc").to_owned()),
                                false,
                                daylight,
                            ),
                            row("options.themePresets", None, false, source),
                            row("options.openThemePark", None, false, editor),
                        ],
                    ),
                    crate::ai_settings::song_group(
                        values.song_themes,
                        i18n,
                        tokens,
                        values.theme_settings.custom_dual_theme.is_some(),
                    ),
                    wall_group(values.appearance.wall, i18n, tokens),
                ]
            }
            Section::Player => {
                // `PlayerPage` control bar: the height with its reset, and dragging the bar on the player page.
                let base = folia_store::BOTTOM_BAR_BASE;
                let reset = if values.bar_offset > base {
                    kit::icon_button(tokens, Icon::RotateCcw, 15.0, crate::Message::BarOffsetReset)
                } else {
                    container(icon(Icon::RotateCcw, 15.0, Color { a: tokens.t2.a * 0.4, ..tokens.t2 }))
                        .center(32)
                        .into()
                };
                let slider = iced::widget::slider(
                    base..=values.bar_max.max(base + 1.0),
                    values.bar_offset,
                    crate::Message::BarOffset,
                )
                .on_release(crate::Message::BarOffsetDone)
                .step(1.0_f32)
                .width(Length::Fixed(160.0))
                .style(move |_, _| kit::slider_style(tokens));
                let height =
                    iced::widget::row![reset, slider, kit::value(tokens, format!("{}px", values.bar_offset.round()))]
                        .spacing(8)
                        .align_y(iced::Alignment::Center);
                let reposition = if values.has_track {
                    kit::icon_button(tokens, Icon::ChevronRight, 16.0, crate::Message::StartPositioning)
                } else {
                    container(icon(Icon::ChevronRight, 16.0, Color { a: tokens.t2.a * 0.4, ..tokens.t2 }))
                        .center(32)
                        .into()
                };
                let unavailable =
                    (!values.has_track).then(|| i18n.t("options.playerBottomBarRepositionUnavailable").to_owned());
                vec![group(
                    BAR,
                    vec![
                        row(
                            "options.playerBottomBarOffset",
                            Some(i18n.t("options.playerBottomBarOffsetDesc").to_owned()),
                            false,
                            height.into(),
                        ),
                        row("options.playerBottomBarReposition", unavailable, false, reposition),
                    ],
                )]
            }
            Section::Integration => {
                let enabled = values.scrobble_enabled;
                let ready = values.scrobble_ready;
                let control = if ready || enabled {
                    kit::toggle(
                        tokens,
                        f32::from(enabled),
                        crate::Message::Playback(crate::playback::Message::Scrobble(!enabled)),
                    )
                } else {
                    kit::toggle_fixed(tokens, 0.0)
                };
                let hint = i18n
                    .t(if ready { "options.neteaseScrobbleDesc" } else { "options.neteaseScrobbleSignInHint" })
                    .to_owned();
                vec![group(SERVICES, vec![row("options.neteaseScrobble", Some(hint), false, control)])]
            }
            Section::General => {
                let languages: Vec<_> = std::iter::once(LanguagePreference::System)
                    .chain(Language::ALL.map(LanguagePreference::Fixed))
                    .map(|p| language_choice(i18n, p))
                    .collect();
                let hint = (values.language == LanguagePreference::System).then(|| {
                    let current = i18n.t(language_key(LanguagePreference::Fixed(values.resolved)));
                    i18n.format("options.appLanguageSystemHint", &[("language", current)])
                });
                let picker = kit::select(
                    tokens,
                    languages,
                    language_choice(i18n, values.language),
                    |c: Choice<LanguagePreference>| crate::Message::LanguagePicked(c.value),
                );
                vec![group(LANGUAGE, vec![row("options.appLanguage", hint, false, picker)])]
            }
            Section::Graphics => {
                use crate::graphics::Message as Graphics;
                use folia_store::{GpuPreference, MotionSurface};
                let graphics = values.graphics;
                let toggle = |key, value: bool, message| {
                    kit::toggle(
                        tokens,
                        self.at(Control::Graphics(key), f32::from(value), now),
                        crate::Message::Graphics(message),
                    )
                };
                let rate = values.appearance.frame_rate;
                let limited = rate.fps().is_some();
                let next = if limited { FrameRate::Display } else { LIMIT_WHEN_SWITCHED_ON };
                let switch = kit::toggle(
                    tokens,
                    self.at(Control::Limit, if limited { 1.0 } else { 0.0 }, now),
                    crate::Message::FrameRate(next),
                );
                let mut rows = vec![
                    row(
                        "options.enableStaticMode",
                        Some(i18n.t("options.enableStaticModeDesc").to_owned()),
                        false,
                        toggle(
                            "options.enableStaticMode",
                            graphics.static_mode,
                            Graphics::Static(!graphics.static_mode),
                        ),
                    ),
                    row(
                        "options.visualizerFrameRate",
                        Some(i18n.t("options.visualizerFrameRateDesc").to_owned()),
                        false,
                        switch,
                    ),
                ];
                if folia_render::backend::native_blur_supported() {
                    let message = if graphics.enable_player_page_native_blur {
                        crate::Message::Graphics(Graphics::NativeBlur(false))
                    } else {
                        crate::Message::Settings(Message::AskNativeBlur)
                    };
                    rows.insert(
                        1,
                        row(
                            "options.enablePlayerPageNativeBlur",
                            Some(i18n.t("options.enablePlayerPageNativeBlurHint").to_owned()),
                            false,
                            kit::toggle(
                                tokens,
                                self.at(
                                    Control::Graphics("options.enablePlayerPageNativeBlur"),
                                    f32::from(graphics.enable_player_page_native_blur),
                                    now,
                                ),
                                message,
                            ),
                        ),
                    );
                }
                if let Some(index) = LIMITS.iter().position(|r| *r == rate) {
                    let choices =
                        LIMITS.into_iter().map(|r| (r, format!("{} FPS", r.fps().unwrap_or_default()))).collect();
                    let mark = self.at(Control::LimitValue, index as f32, now);
                    rows.push(row(
                        "options.visualizerFrameRateValue",
                        None,
                        true,
                        kit::segmented(tokens, choices, rate, Some(mark), crate::Message::FrameRate),
                    ));
                }
                rows.push(row(
                    "options.lyricRenderScale",
                    Some(i18n.t("options.lyricRenderScaleDesc").to_owned()),
                    false,
                    kit::segmented(
                        tokens,
                        [0.5, 0.75, 1.0, 1.5, 2.0].into_iter().map(|scale| (scale, format!("{scale}×"))).collect(),
                        graphics.lyric_render_quality.scale,
                        None,
                        |scale| crate::Message::Graphics(Graphics::RenderScale(scale)),
                    ),
                ));
                rows.push(row(
                    "options.lyricAntialias",
                    Some(i18n.t("options.lyricAntialiasDesc").to_owned()),
                    false,
                    kit::segmented(
                        tokens,
                        [
                            folia_core::LyricAntialias::Auto,
                            folia_core::LyricAntialias::On,
                            folia_core::LyricAntialias::Off,
                        ]
                        .into_iter()
                        .zip(["options.lyricAntialiasAuto", "options.lyricAntialiasOn", "options.lyricAntialiasOff"])
                        .map(|(value, key)| (value, i18n.t(key).to_owned()))
                        .collect(),
                        graphics.lyric_render_quality.antialias,
                        None,
                        |value| crate::Message::Graphics(Graphics::Antialias(value)),
                    ),
                ));
                rows.push(row(
                    "options.backgroundThrottling",
                    Some(i18n.t("options.backgroundThrottlingHint").to_owned()),
                    false,
                    toggle(
                        "options.backgroundThrottling",
                        graphics.background_throttling,
                        Graphics::BackgroundThrottling(!graphics.background_throttling),
                    ),
                ));
                let choices = GpuPreference::ALL
                    .into_iter()
                    .zip([
                        "options.gpuPreferenceAuto",
                        "options.gpuPreferenceHighPerformance",
                        "options.gpuPreferenceLowPower",
                    ])
                    .map(|(value, key)| (value, i18n.t(key).to_owned()))
                    .collect();
                rows.push(row(
                    "options.gpuPreference",
                    Some(i18n.t("options.gpuPreferenceHint").to_owned()),
                    false,
                    kit::segmented(tokens, choices, graphics.gpu_preference, None, |value| {
                        crate::Message::Graphics(Graphics::Gpu(value))
                    }),
                ));
                if folia_render::backend::power_preference_overridden() {
                    rows.push(row("options.gpuPreferenceCommandLineNotice", None, true, space().into()));
                } else if values.applied_gpu != graphics.gpu_preference {
                    rows.push(row("options.gpuPreferenceRestartNotice", None, true, space().into()));
                }
                if let Some(adapter) = folia_render::backend::active_adapter() {
                    rows.push(row("options.gpuPreferenceActive", None, true, kit::value(tokens, adapter)));
                }
                let motion = &graphics.motion;
                let mut motions = vec![row(
                    "options.reduceMotionFollowSystem",
                    Some(i18n.t("options.reduceMotionFollowSystemDesc").to_owned()),
                    false,
                    toggle(
                        "options.reduceMotionFollowSystem",
                        motion.follow_system,
                        Graphics::FollowSystem(!motion.follow_system),
                    ),
                )];
                if motion.follow_system && values.system_reduced_motion {
                    motions.push(row("options.reduceMotionForcedNotice", None, true, space().into()));
                }
                for (surface, key, hint) in [
                    (
                        MotionSurface::CollectionMorph,
                        "options.reduceMotionCollectionMorph",
                        "options.reduceMotionCollectionMorphDesc",
                    ),
                    (
                        MotionSurface::MonetBackground,
                        "options.reduceMotionMonetBackground",
                        "options.reduceMotionMonetBackgroundDesc",
                    ),
                    (
                        MotionSurface::UiMicroMotion,
                        "options.reduceMotionUiMicroMotion",
                        "options.reduceMotionUiMicroMotionDesc",
                    ),
                    (
                        MotionSurface::SettingsScroll,
                        "options.reduceMotionSettingsScroll",
                        "options.reduceMotionSettingsScrollDesc",
                    ),
                ] {
                    let enabled = motion.selected(surface);
                    motions.push(row(
                        key,
                        Some(i18n.t(hint).to_owned()),
                        false,
                        toggle(key, enabled, Graphics::Motion(surface, !enabled)),
                    ));
                }
                let mut motion_group = group(MOTION, motions);
                motion_group.foot = Some(i18n.t("options.reduceMotionSectionDesc").to_owned());
                vec![group(PERFORMANCE, rows), motion_group]
            }
            Section::Sources => values.sources.groups(i18n, tokens),
            Section::Developer => crate::developer::settings_groups(
                i18n,
                tokens,
                values.developer,
                values.developer_settings,
                values.runtime_log,
                values.memory_log_path,
            ),
        }
    }
}

/// The page's spacing: `md:` values from 768px up, the narrower ones below.
struct Gutter {
    side: f32,
    title_top: f32,
    close_right: f32,
    under_title: f32,
    foot: f32,
}

impl Gutter {
    fn of(wide: bool) -> Self {
        if wide {
            Self { side: 32.0, title_top: 20.0, close_right: 16.0, under_title: 4.0, foot: 64.0 }
        } else {
            Self { side: 16.0, title_top: 16.0, close_right: 12.0, under_title: 0.0, foot: 48.0 }
        }
    }
}

/// The groups of a category, in page order.
fn wall_group<'a>(wall: folia_store::WallAppearance, i18n: &'a I18n, tokens: Tokens) -> Group<'a, crate::Message> {
    use crate::Message::Home;
    use crate::home::Message as Change;
    use folia_store::WallLook;
    let row =
        |key, control| Row { note: None, label: i18n.t(key).to_owned(), control, hint: None, below: None, sub: false };
    let toggle = |key, on, message| row(key, kit::toggle(tokens, if on { 1.0 } else { 0.0 }, Home(message)));
    let look = kit::segmented(
        tokens,
        [WallLook::Solid, WallLook::Partial, WallLook::Clear]
            .map(|mode| (mode, i18n.t(crate::home::look_label(mode)).to_owned()))
            .into(),
        wall.look,
        None,
        |mode| Home(Change::WallLook(mode)),
    );
    let mut rows = vec![row("options.libraryWallLook", look)];
    if wall.look == WallLook::Partial {
        let count = wall.reserved();
        rows.push(row(
            "options.libraryWallWindows",
            kit::segmented(tokens, (1..=6).map(|n| (n, n.to_string())).collect(), count, None, |n| {
                Home(Change::Windows(n))
            }),
        ));
    }
    rows.push(toggle("options.bravaisSeamClear", wall.seam_clear, Change::SeamClear(!wall.seam_clear)));
    rows.push(toggle(
        "options.bravaisBackdropLyrics",
        wall.backdrop_lyrics,
        Change::BackdropLyrics(!wall.backdrop_lyrics),
    ));
    rows.push(toggle("options.bravaisBackdropBlur", wall.backdrop_blur, Change::BackdropBlur(!wall.backdrop_blur)));
    rows.push(toggle("options.latticePosterTint", wall.tint, Change::Tint(!wall.tint)));
    if wall.tint {
        rows.push(toggle(
            "options.latticePosterTintCustomColor",
            wall.custom_tint,
            Change::CustomTint(!wall.custom_tint),
        ));
        if wall.custom_tint {
            let color = wall.tint_color;
            let mut picker = row(
                "options.latticePosterTintColor",
                kit::value(tokens, format!("#{:02X}{:02X}{:02X}", color.r, color.g, color.b)),
            );
            picker.below = Some(crate::color_picker::view(
                color,
                |rgb| Home(Change::TintColor(rgb)),
                Home(Change::CommitAppearance),
            ));
            rows.push(picker);
        }
        let amount = iced::widget::slider(0.0..=1.0, wall.intensity, |n| Home(Change::Intensity(n)))
            .step(0.01_f32)
            .width(120)
            .on_release(Home(Change::CommitAppearance))
            .style(move |_, _| kit::slider_style(tokens));
        rows.push(row(
            "options.latticePosterTintIntensity",
            iced::widget::row![amount, kit::value(tokens, format!("{:.0}%", wall.intensity * 100.0))].spacing(8).into(),
        ));
    }
    let form = kit::segmented(
        tokens,
        [false, true]
            .map(|radio| {
                (
                    radio,
                    i18n.t(if radio { "options.latticeWallForm_radio" } else { "options.latticeWallForm_poster" })
                        .to_owned(),
                )
            })
            .into(),
        wall.radio,
        None,
        |radio| Home(Change::Radio(radio)),
    );
    rows.push(row("options.latticeWallForm", form));
    rows.push(toggle("options.latticeAutoFocusOnSongChange", wall.auto_focus, Change::AutoFocus(!wall.auto_focus)));
    rows.push(toggle("options.latticeVignette", wall.vignette, Change::Vignette(!wall.vignette)));
    rows.push(toggle(
        "options.latticeBadgeCurrentOnly",
        wall.badge_current_only,
        Change::BadgeCurrentOnly(!wall.badge_current_only),
    ));
    rows.push(toggle(
        "options.latticeRadioHideUpcoming",
        wall.hide_upcoming,
        Change::HideUpcoming(!wall.hide_upcoming),
    ));
    let preset = iced::widget::row![
        button(text(i18n.t("wallTools.copyPreset"))).on_press(Home(Change::ExportAppearance)).style(button::text),
        button(text(i18n.t("wallTools.pastePreset"))).on_press(Home(Change::ImportAppearance)).style(button::text)
    ]
    .spacing(6);
    rows.push(row("wallTools.preset", preset.into()));
    Group { title: i18n.t(WALL.title).to_owned(), bare: false, rows, foot: None }
}

fn groups_of(section: Section) -> Vec<GroupDef> {
    let mut groups: Vec<GroupDef> = Vec::new();
    for &(owner, group, _) in OPTIONS {
        if owner == section && !groups.contains(&group) {
            groups.push(group);
        }
    }
    groups
}

/// A group's height on the page, for following the scroll with the tab line: hint rows are a line taller.
fn group_height(section: Section, group: &GroupDef, first: bool) -> f32 {
    let rows = OPTIONS.iter().filter(|(owner, g, _)| *owner == section && g == group).count() as f32;
    let gap = if first { kit::FIRST_GROUP_GAP } else { kit::GROUP_GAP };
    let heading = if group.bare { 0.0 } else { kit::GROUP_HEADING };
    gap + heading + rows * kit::ROW_HINT_HEIGHT
}

/// One search hit: where it lands, what it is called and where it sits.
struct Hit {
    section: Section,
    group: Option<&'static str>,
    label: String,
    path: String,
}

/// `buildSettingsSearchEntries` matched against `query`: categories, their groups and options, in side-list order.
fn search(i18n: &I18n, query: &str) -> Vec<Hit> {
    let query = query.trim().to_lowercase();
    let mut entries = Vec::new();
    for (nav_key, sections) in NAV {
        for section in sections.iter().copied() {
            let page = i18n.t(section.label_key());
            entries.push(Hit { section, group: None, label: page.to_owned(), path: i18n.t(nav_key).to_owned() });
            let mut listed: Vec<&str> = Vec::new();
            for (_, group, label) in OPTIONS.iter().filter(|(owner, _, label)| {
                *owner == section
                    && (*label != "options.enablePlayerPageNativeBlur"
                        || folia_render::backend::native_blur_supported())
            }) {
                let title = i18n.t(group.title);
                if !listed.contains(&group.id) {
                    listed.push(group.id);
                    entries.push(Hit {
                        section,
                        group: Some(group.id),
                        label: title.to_owned(),
                        path: page.to_owned(),
                    });
                }
                entries.push(Hit {
                    section,
                    group: Some(group.id),
                    label: i18n.t(label).to_owned(),
                    path: format!("{page} \u{203a} {title}"),
                });
            }
        }
    }
    entries.retain(|hit| hit.label.to_lowercase().contains(&query));
    entries
}

fn search_results<'a>(i18n: &'a I18n, tokens: Tokens, query: &str) -> Element<'a, crate::Message> {
    let hits = search(i18n, query);
    if hits.is_empty() {
        return space().into();
    }
    let mut card = column![];
    for (index, hit) in hits.into_iter().enumerate() {
        if index > 0 {
            card = card.push(container(kit::hairline(tokens)).padding(Padding::ZERO.left(16.0)));
        }
        let line = row![
            text(hit.label).size(crate::typeface::size::BODY).color(tokens.t1).width(Length::Fill),
            kit::value(tokens, hit.path),
            icon(Icon::ChevronRight, 16.0, tokens.t2)
        ]
        .spacing(8)
        .align_y(Alignment::Center);
        let link = button(container(line).height(56).align_y(Alignment::Center).padding([0, 16]))
            .padding(0)
            .width(Length::Fill)
            .on_press(crate::Message::Settings(Message::Hit(hit.section, hit.group)))
            .style(move |_, status| match status {
                button::Status::Hovered | button::Status::Pressed => kit::rounded_button(tokens.fill, 0.0),
                _ => button::Style::default(),
            });
        card = card.push(link);
    }
    let card = container(card).width(Length::Fill).clip(true).style(move |_| container::Style {
        background: Some(tokens.card.into()),
        border: Border { color: tokens.line, width: 1.0, radius: 12.0.into() },
        ..container::Style::default()
    });
    column![space().height(kit::FIRST_GROUP_GAP), card].into()
}

fn nav_item<'a>(
    i18n: &'a I18n,
    tokens: Tokens,
    section: Section,
    active: Option<Section>,
    wide: bool,
) -> Element<'a, crate::Message> {
    let current = active == Some(section);
    let color = if current { tokens.t1 } else { tokens.t2 };
    let label = text(i18n.t(section.label_key())).size(crate::typeface::size::CONTROL).color(color);
    let content: Element<'a, crate::Message> = if wide {
        let mark = if current { color } else { Color { a: color.a * 0.7, ..color } };
        row![icon(section.icon(), 16.0, mark), label].spacing(10).align_y(Alignment::Center).into()
    } else {
        label.into()
    };
    let item = button(container(content).height(32).align_y(Alignment::Center).padding([0, 10]))
        .padding(0)
        .on_press(crate::Message::Settings(Message::Section(section)))
        .style(move |_, status| {
            if current {
                kit::rounded_button(tokens.fill2, 8.0)
            } else if matches!(status, button::Status::Hovered | button::Status::Pressed) {
                kit::rounded_button(tokens.fill, 8.0)
            } else {
                button::Style::default()
            }
        });
    if wide { item.width(Length::Fill).into() } else { item.into() }
}

fn language_choice(i18n: &I18n, preference: LanguagePreference) -> Choice<LanguagePreference> {
    Choice { value: preference, label: i18n.t(language_key(preference)).to_owned() }
}

fn theme_key(mode: Option<ThemeMode>) -> &'static str {
    match mode {
        Some(ThemeMode::Light) => "options.daylightModeLight",
        Some(ThemeMode::Dark) => "options.daylightModeDark",
        None => "options.daylightModeSystem",
    }
}

fn language_key(preference: LanguagePreference) -> &'static str {
    match preference {
        LanguagePreference::System => "options.appLanguageSystem",
        LanguagePreference::Fixed(Language::En) => "options.appLanguageEnUS",
        LanguagePreference::Fixed(Language::ZhCn) => "options.appLanguageZhCN",
        LanguagePreference::Fixed(Language::ZhTw) => "options.appLanguageZhTW",
        LanguagePreference::Fixed(Language::In) => "options.appLanguageInID",
        LanguagePreference::Fixed(Language::Vi) => "options.appLanguageViVN",
    }
}

fn seconds(since: Instant, now: Instant) -> f64 {
    if crate::motion::reduced(folia_store::MotionSurface::UiMicroMotion) {
        return 1.0;
    }
    now.saturating_duration_since(since).as_secs_f64()
}
