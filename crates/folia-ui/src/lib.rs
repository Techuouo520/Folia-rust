// Top-level UI: pages, navigation and the iced update/view/subscription functions that drive them.

mod album_wall;
mod background;
mod block_reflows;
mod chrome;
mod cover_flight;
mod design_size;
mod drag_capture;
mod focus_card;
mod home;
mod icons;
mod library;
mod login;
mod lots;
mod mode_picker;
mod motion;
mod now_playing;
mod online;
mod overlay;
mod page_fade;
mod panel;
mod platform_highlight;
mod platform_icon;
mod platform_rows;
mod playback;
mod playback_navigation;
mod player_bar;
mod positioner;
mod poster;
mod poster_motion;
mod quality_select;
mod queue_island;
mod search;
mod settings;
mod settings_kit;
mod splash;
mod status_toast;
mod track_list;
mod typeface;
mod wall;
mod wall_pan;
mod wall_shift;
mod workbench;

pub use background::{error_chain, format_time};
pub use focus_card::{Direction as CardDirection, Key as CardKey};
pub use home::title::seam_title_activates;
pub use home::{HELP_LIST, HelpRow, WallKey, default_level_is_spine, seam_safe_bottom, wall_key_of};
pub use overlay::{
    CAPSULE_COLLAPSE_MS, CAPSULE_EXPAND_MS, CAPSULE_FADE_MS, CAPSULE_LAYOUT_SPRING, CAPSULE_SETTLE_MS, CHROME_HOLD_MS,
    CHROME_LEAVE_MS, CapsuleHover, ChromeClock, ChromeSpring, GLOW_BLUR_PX, GlowEllipse, SUBTITLE_CLEARANCE,
    SUBTITLE_PRESENCE_SPRING, SpringParams, Subtitle, capsule_open, capsule_row, capsule_width, glow_coverage,
    glow_ellipse, row_gap, subtitle_at, subtitle_bottom, subtitle_line_width, translation_px, upcoming_look,
    upcoming_px,
};
pub use platform_highlight::ProviderHighlight;
pub use platform_rows::{ProviderRowStatus, order_provider_rows};
pub use playback_navigation::PlaybackNavigation;
/// The floating capsule. `shown` is its 0.26s ease-out hide, shared by opacity, offset and scale.
pub use player_bar::{CapsuleBox, PlayerBar, capsule_box};
pub use settings_kit::{Faded, Tokens};
pub use typeface::install as install_fonts;
pub use wall::{SEAM_ANCHOR, Seam, content_opacity, plan_opening, seam_geometry, seam_on_axis, wider_span};
pub use wall_pan::{Glide as WallGlide, Inertia as WallInertia, Release as WallRelease};
pub use wall_shift::{Kind as ShiftKind, Pose as ShiftPose, Shift as WallShift};

use std::path::PathBuf;

use folia_core::{AudioQuality, Language, LanguagePreference, Lyrics, Theme, Track};
use folia_i18n::I18n;
use folia_omni::Omni;
use folia_store::{
    AppearanceSettings, FrameRate, GeneralSettings, PlaybackSettings, Store, ThemeMode, VisualizerSettings,
};
use folia_visualizer::{Scene, Style, Values};
use iced::widget::{container, mouse_area, space, stack};
use iced::{Alignment, Color, Element, Length, Size, Subscription, Task, mouse};
use lucide_static_svg::Icon;

use crate::background::save;
use crate::library::LibraryPage;
use crate::playback::Playback;
use crate::search::SearchPage;

/// The window folia-app opens; the shell lays out before the first resize event arrives.
pub const INITIAL_WINDOW: Size = Size::new(1200.0, 800.0);
/// The smallest window folia-app allows; anything reported below it is a minimized window.
pub const MIN_WINDOW_SIZE: Size = Size::new(350.0, 100.0);
/// Shown when there is no online platform, or one that was never built in.
const STANDARD_ONLY: &[AudioQuality] = &[AudioQuality::Standard];

/// Everything the app reads before the window opens.
#[derive(Clone)]
pub struct Init {
    pub store: Store,
    pub general: GeneralSettings,
    pub appearance: AppearanceSettings,
    pub playback: PlaybackSettings,
    pub player: folia_store::PlayerSettings,
    pub visualizer: VisualizerSettings,
    pub i18n: I18n,
    pub system_locale: Option<String>,
    pub library_db: PathBuf,
    /// Where downloaded online audio is cached.
    pub cache_dir: PathBuf,
    /// The system keyring service that keeps platform sessions.
    pub keyring_service: String,
    /// Audio files given on the command line, played as a queue once the window opens.
    pub open_files: Vec<PathBuf>,
}

/// The home tab and the open collection, whose change turns the seam's content over; the last flag is the search panel.
type SeamKey = (folia_store::HomeTab, Option<String>, bool, bool);
/// The seam content before a turn: its tab, layer, search query and layout size, plus the key to compare.
type SeamBefore = (folia_store::HomeTab, Option<home::LayerSeam>, Option<String>, f32, SeamKey);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Page {
    Library,
    Search,
    NowPlaying,
}

#[derive(Debug, Clone)]
pub enum Message {
    Navigate(Page),
    OpenSettings,
    Settings(settings::Message),
    Workbench(workbench::Message),
    LanguagePicked(LanguagePreference),
    Theme(ThemeMode),
    FrameRate(FrameRate),
    /// A fixed lyric style, or `None` for a random style per song.
    Visualizer(Option<Style>),
    Library(library::Message),
    Search(search::Message),
    Playback(playback::Message),
    OpenFiles(Vec<Track>),
    Chrome(chrome::Message),
    /// The pointer moved in a window, or left it (`None`).
    Pointer(iced::window::Id, Option<iced::Point>),
    WindowReady(Option<iced::window::Id>),
    /// The page being left, captured to fade out over the one replacing it.
    PageShot(iced::window::Screenshot),
    /// The capture is over; the page changes now if no picture of it came.
    PageShotDone,
    WindowResized(Size),
    Maximized(bool),
    ChromeTick,
    PointerHeld(bool),
    /// The window lost focus; any press outside it will not report its release here.
    WindowUnfocused,
    RevealChrome,
    Bar(player_bar::Message),
    /// The bottom offset from settings, while its slider moves.
    BarOffset(f32),
    BarOffsetDone,
    BarOffsetReset,
    StartPositioning,
    Positioner(positioner::Message),
    PositionerKey(bool),
    ToggleFullscreen,
    SplashFrame,
    Home(home::Message),
    Panel(panel::Message),
    Art(i64, Option<folia_library::Cover>),
    Login(login::Message),
    /// Escape on the player page: leave fullscreen, close the panel, or go back to the wall.
    PlayerEscape,
    /// An input method is composing on the player page; Escape belongs to it.
    PlayerComposing(bool),
}

/// Settings id of the random lyric style choice, as in the original app.
const RANDOM_STYLE: &str = "random";

/// Pixel budget of the panel's decoded covers: eight 360×360 RGBA covers (about 0.5 MB each), so stepping
/// back and forth between neighbouring songs does not decode again.
const ART_BYTES: usize = 4 * 1024 * 1024;
/// The panel cover is decoded at 360px, the panel's width.
const ART_PX: u32 = 360;

pub struct Shell {
    store: Store,
    /// The page being left for or from the player fades out over the new one.
    page_fade: Option<page_fade::PageFade>,
    /// The page to show once the one being left has been captured.
    next_page: Option<Page>,
    seam_flip: Option<home::flip::SeamFlip>,
    general: GeneralSettings,
    appearance: AppearanceSettings,
    i18n: I18n,
    system_locale: Option<String>,
    theme: Theme,
    iced_theme: iced::Theme,
    page: Page,
    settings: Option<settings::Settings>,
    workbench: Option<workbench::Workbench>,
    visualizer: VisualizerSettings,
    library: LibraryPage,
    search: SearchPage,
    omni: Option<Omni>,
    playback: Playback,
    play_navigation: PlaybackNavigation,
    empty_lyrics: Lyrics,
    window: Option<iced::window::Id>,
    maximized: bool,
    fullscreen: bool,
    /// Origin for [`ChromeClock`] milliseconds.
    started: std::time::Instant,
    clock: ChromeClock,
    /// `controlBarPresence`: the subtitle's room for the capsule, sprung as the chrome shows and hides.
    subtitle_presence: ChromeSpring,
    bar: player_bar::PlayerBar,
    window_size: Size,
    pointer_held: bool,
    /// Press on the drag strip, and the pointer position of the first move after it.
    drag_armed: bool,
    drag_from: Option<(f32, f32)>,
    splash: splash::Splash,
    home: home::HomeChrome,
    panel: panel::PlayerPanel,
    art: folia_library::CoverCache<i64, iced::widget::image::Handle>,
    art_pending: Option<i64>,
    titlebar: chrome::Titlebar,
    player: folia_store::PlayerSettings,
    positioning: Option<positioner::Positioning>,
    login: login::Login,
    /// An input method is composing in a field on the player page.
    composing: bool,
    /// Song id string the original visualizer uses as its geometry seed.
    visualizer_seed: String,
}

impl Shell {
    pub fn new(init: Init) -> (Self, Task<Message>) {
        let theme = init.appearance.theme.theme();
        let omni = Omni::new(init.cache_dir, init.keyring_service)
            .inspect_err(|err| tracing::error!(error = err as &dyn std::error::Error, "online music unavailable"))
            .ok();
        let (library, load) = LibraryPage::new(
            init.library_db,
            init.general.home_tab,
            omni.clone(),
            init.general.online_provider.as_deref(),
        );
        let mut home = home::HomeChrome::new(INITIAL_WINDOW.width);
        home.restore_queue_island(init.general.wall_tools_queue_island_open);
        let shell = Self {
            store: init.store,
            general: init.general,
            iced_theme: iced_theme(&theme),
            theme,
            appearance: init.appearance,
            i18n: init.i18n,
            system_locale: init.system_locale,
            page: Page::Library,
            page_fade: None,
            next_page: None,
            seam_flip: None,
            settings: None,
            workbench: None,
            visualizer: init.visualizer,
            library,
            search: SearchPage::default(),
            playback: Playback::new(init.playback, omni.clone()),
            play_navigation: PlaybackNavigation::default(),
            omni,
            empty_lyrics: Lyrics::default(),
            window: None,
            maximized: false,
            fullscreen: false,
            started: std::time::Instant::now(),
            clock: ChromeClock::shown(0),
            subtitle_presence: ChromeSpring::settled(1.0, 0, SUBTITLE_PRESENCE_SPRING),
            bar: player_bar::PlayerBar::new(std::time::Instant::now()),
            window_size: INITIAL_WINDOW,
            pointer_held: false,
            drag_armed: false,
            drag_from: None,
            splash: splash::Splash::new(),
            home,
            panel: panel::PlayerPanel::new(std::time::Instant::now()),
            art: folia_library::CoverCache::new(ART_BYTES),
            art_pending: None,
            titlebar: chrome::Titlebar::new(std::time::Instant::now()),
            player: init.player,
            positioning: None,
            login: login::Login::new(std::time::Instant::now()),
            composing: false,
            visualizer_seed: "lumiere".to_owned(),
        };
        let open = if init.open_files.is_empty() { Task::none() } else { open_files(init.open_files) };
        let window = iced::window::oldest().map(Message::WindowReady);
        (shell, Task::batch([load.map(Message::Library), open, window]))
    }

    fn refresh_visualizer_seed(&mut self) {
        let next = match self.playback.current() {
            Some(track) => match &track.online {
                Some(online) => online.id.clone(),
                None => track.id.to_string(),
            },
            None => "lumiere".to_owned(),
        };
        if self.visualizer_seed != next {
            self.visualizer_seed = next;
        }
    }

    pub fn title(&self) -> String {
        match self.playback.current() {
            Some(track) => format!("{} - {}", track.display_name(), self.i18n.t("app.name")),
            None => self.i18n.t("app.name").to_owned(),
        }
    }

    pub fn theme(&self) -> iced::Theme {
        self.iced_theme.clone()
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        let before = self.page;
        let settles_page = matches!(message, Message::PageShot(_) | Message::PageShotDone);
        let seam_before = self.seam_before(&message);
        let opens_search = matches!(&message, Message::Home(home::Message::Search));
        let focus_title = matches!(message, Message::Home(home::Message::TitleToggle { keyboard: true, .. }));
        let task = self.handle(message);
        self.refresh_visualizer_seed();
        if self.page != before || self.playback.current().is_none() {
            self.play_navigation.cancel();
        }
        if self.page != before {
            self.composing = false;
        }
        let now = std::time::Instant::now();
        if let Some((tab, layer, search, layout, key)) = seam_before
            && key != self.seam_key()
        {
            let outgoing = home::flip::Outgoing { tab, layer, spine: key.2, search, layout };
            self.seam_flip = Some(home::flip::SeamFlip::new(outgoing, opens_search, focus_title, now));
        }
        let mut focus = Task::none();
        let mut focus_search = false;
        if let Some(flip) = self.seam_flip.as_mut() {
            focus_search = flip.take_search_focus(now);
            if !flip.running(now) {
                if flip.focus_title {
                    focus = iced::widget::operation::focus(home::title::id());
                }
                self.seam_flip = None;
            }
        }
        if focus_search {
            focus = Task::batch([focus, iced::widget::operation::focus(home::search_id())]);
        }
        if self.page_fade.as_ref().is_some_and(|fade| !fade.running(now)) {
            self.page_fade = None;
        }
        let fade = if settles_page { Task::none() } else { self.fade_page(before) };
        let task = Task::batch([task, fade, focus]);
        if self.page != Page::NowPlaying {
            self.positioning = None;
        }
        self.bar.set_home(self.page == Page::Library);
        self.home.sync(self.window_size.width, self.window_size.height, self.playback.current().is_some(), now);
        self.home.sync_queue(
            self.playback.queue.items(),
            self.playback.queue.current_index(),
            self.playback.is_fm(),
            now,
        );
        if let Some(horizontal) = self.home.take_flip(now) {
            self.library.set_horizontal(horizontal);
            self.library.replan(self.home.goal());
        }
        self.library.set_horizontal(self.home.is_horizontal());
        self.library.set_seam(self.home.width(now));
        self.library.plan_if_needed();
        let seam = self.library.seam();
        self.home.sync_tab(self.home.is_hidden() || seam.collapsed, now);
        self.login.sync(&self.library.online);
        self.library.set_tab_chrome(&mut self.home, now);
        self.library.sync_hint(&self.i18n, now);
        self.bar.set_forced(self.positioning.is_some());
        self.bar.sync(&self.playback, self.window_size.width, now);
        self.bar.set_shown(self.chrome_visible(), now);
        let presence = if self.chrome_visible() { 1.0 } else { 0.0 };
        self.subtitle_presence = self.subtitle_presence.retarget(self.now_ms(), presence);
        let (quality_levels, quality) = self.audio_quality();
        let room = panel::Room {
            width: self.window_size.width,
            height: self.window_size.height,
            bottom: self.bar_offset(),
            queue: self.playback.queue.items(),
            index: self.playback.queue.current_index(),
            quality_levels,
            quality,
        };
        self.panel.follow(room);
        // The focus card stays above the player bar's safe area (`getBottomInset`) and knows what is playing.
        self.library.bottom_inset = if self.playback.current().is_some() {
            home::seam_safe_bottom(self.bar_offset(), self.home.presence(now))
        } else {
            focus_card::REVEAL_PAD
        };
        self.library.playing_id = self.playback.current().map(|track| track.id);
        task
    }

    fn handle(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Navigate(page) => {
                self.play_navigation.cancel();
                self.page = page;
                if page == Page::NowPlaying {
                    self.reveal_chrome();
                }
                Task::none()
            }
            Message::OpenSettings => {
                self.settings = Some(settings::Settings::open());
                Task::none()
            }
            Message::Workbench(message) => self.workbench(message),
            Message::Settings(message) => {
                let Some(window) = self.settings.as_mut() else { return Task::none() };
                let task = window.update(message).map(Message::Settings);
                if window.finished() {
                    self.settings = None;
                }
                task
            }
            Message::LanguagePicked(preference) => self.set_language(preference),
            Message::Theme(mode) => {
                let before = self.appearance.clone();
                self.appearance.theme = mode;
                self.theme = mode.theme();
                self.iced_theme = iced_theme(&self.theme);
                self.appearance_changed(&before)
            }
            Message::FrameRate(rate) => {
                let before = self.appearance.clone();
                self.appearance.frame_rate = rate;
                self.appearance_changed(&before)
            }
            Message::Visualizer(style) => {
                self.appearance.visualizer = style.map_or(RANDOM_STYLE, Style::id).to_owned();
                save(&self.store, &self.appearance)
            }
            Message::Library(message) => {
                if matches!(message, library::Message::Loaded(_)) {
                    self.splash.library_ready();
                }
                let refresh_rows = matches!(
                    &message,
                    library::Message::Online(online::Message::Restored(_) | online::Message::Account(..))
                );
                let frames = matches!(message, library::Message::Frame);
                let queue_task = if frames {
                    let queue = self.playback.queue.items();
                    let list_h = queue_island::list_height(queue.len());
                    let now = std::time::Instant::now();
                    self.home
                        .queue_frame(queue, self.playback.queue.current_index(), list_h, now)
                        .map(|message| Message::Home(home::Message::ToolsQueue(message)))
                } else {
                    Task::none()
                };
                let (task, effect) = self.library.update(message, &self.i18n);
                if refresh_rows {
                    self.login.refresh_rows(&self.library.online);
                }
                let task = task.map(Message::Library);
                Task::batch([queue_task, task, self.library_effect(effect)])
            }
            Message::Search(message) => {
                if matches!(message, search::Message::Close) {
                    self.play_navigation.cancel();
                    self.search.close();
                }
                let now = std::time::Instant::now();
                if matches!(message, search::Message::Frame) && self.page == Page::Search && self.search.gone(now) {
                    self.page = Page::Library;
                }
                let provider = self.library.online.provider();
                let (task, play) = self.search.update(message, self.omni.as_ref(), provider);
                let task = task.map(Message::Search);
                match play {
                    Some(index) => {
                        let name = self.search.query();
                        let name = if name.is_empty() { self.i18n.t("search.sourceNetease") } else { name };
                        let origin = folia_core::Origin { name: name.to_owned() };
                        let start =
                            self.playback.play(self.search.results.clone(), index, Some(origin)).map(Message::Playback);
                        self.await_player();
                        Task::batch([task, start, self.ensure_art()])
                    }
                    None => task,
                }
            }
            Message::Playback(message) => {
                if let playback::Message::AudioReady(request, _, Err(_)) = &message {
                    self.play_navigation.failed(*request);
                }
                let started = match &message {
                    playback::Message::Started(request) => Some(*request),
                    _ => None,
                };
                let task = self.playback.update(message, &self.store).map(Message::Playback);
                if started.is_some_and(|request| {
                    request == self.playback.request_id() && self.play_navigation.ready(request, self.page)
                }) {
                    self.page = Page::NowPlaying;
                    self.reveal_chrome();
                }
                Task::batch([task, self.ensure_art()])
            }
            Message::OpenFiles(tracks) if !tracks.is_empty() => {
                let start = self.playback.play(tracks, 0, None).map(Message::Playback);
                self.await_player();
                Task::batch([start, self.ensure_art()])
            }
            Message::OpenFiles(_) => Task::none(),
            Message::WindowReady(id) => {
                self.window = id.or(self.window);
                Task::none()
            }
            Message::PageShot(shot) => {
                if let Some(page) = self.next_page.take() {
                    self.page = page;
                    self.page_fade = Some(page_fade::PageFade::new(shot, std::time::Instant::now()));
                }
                Task::none()
            }
            Message::PageShotDone => {
                if let Some(page) = self.next_page.take() {
                    self.page = page;
                }
                Task::none()
            }
            Message::WindowResized(size) => {
                // A minimized window reports a size below the minimum (Windows: 160×28); keep the last real viewport.
                if size.width < MIN_WINDOW_SIZE.width || size.height < MIN_WINDOW_SIZE.height {
                    return Task::none();
                }
                self.window_size = size;
                match self.window {
                    Some(id) => iced::window::is_maximized(id).map(Message::Maximized),
                    None => Task::none(),
                }
            }
            Message::Maximized(maximized) => {
                self.maximized = maximized;
                Task::none()
            }
            Message::ChromeTick => {
                self.clock = self.clock.tick(self.now_ms(), self.pointer_held);
                self.panel.set_chrome(self.chrome_visible(), std::time::Instant::now());
                Task::none()
            }
            Message::PointerHeld(held) => {
                self.pointer_held = held;
                if !held {
                    self.drag_armed = false;
                    if let Some(positioning) = self.positioning.as_mut() {
                        positioning.release();
                    }
                    self.drag_from = None;
                }
                if self.page == Page::NowPlaying {
                    self.reveal_chrome();
                }
                Task::none()
            }
            Message::WindowUnfocused => {
                self.drag_armed = false;
                self.drag_from = None;
                Task::none()
            }
            Message::RevealChrome => {
                if self.page == Page::NowPlaying {
                    self.reveal_chrome();
                }
                Task::none()
            }
            Message::BarOffset(offset) => {
                self.player.bottom_bar_offset = folia_store::clamp_bottom_bar_offset(offset, self.window_size.height);
                Task::none()
            }
            Message::BarOffsetDone => save(&self.store, &self.player),
            Message::BarOffsetReset => {
                self.player.bottom_bar_offset = folia_store::BOTTOM_BAR_BASE;
                save(&self.store, &self.player)
            }
            Message::StartPositioning => {
                if self.playback.current().is_none() {
                    return Task::none();
                }
                // `usePlayerBottomBarPositioningEntry`: close settings, go to the player, enter the mode.
                self.settings = None;
                self.page = Page::NowPlaying;
                self.reveal_chrome();
                self.positioning = Some(positioner::Positioning::new(self.bar_offset()));
                Task::none()
            }
            Message::Positioner(message) => self.positioner(message),
            Message::PositionerKey(commit) => {
                self.positioner(if commit { positioner::Message::Commit } else { positioner::Message::Cancel })
            }
            Message::Bar(player_bar::Message::Open) => {
                if self.page == Page::Library {
                    self.page = Page::NowPlaying;
                    self.reveal_chrome();
                }
                Task::none()
            }
            Message::Bar(message) => match self.bar.update(message, &self.playback, std::time::Instant::now()) {
                Some(message) => {
                    let task = self.playback.update(message, &self.store).map(Message::Playback);
                    Task::batch([task, self.ensure_art()])
                }
                None => Task::none(),
            },
            Message::ToggleFullscreen => self.toggle_fullscreen(),
            Message::SplashFrame => {
                self.splash.on_frame(std::time::Instant::now());
                Task::none()
            }
            Message::Home(message) => self.home(message),
            Message::Panel(message) => {
                let task = self.panel(message);
                Task::batch([task, self.ensure_art()])
            }
            Message::Art(id, cover) => {
                if self.art_pending == Some(id) {
                    self.art_pending = None;
                }
                let bytes = cover.as_ref().map_or(0, |cover| cover.rgba.len());
                tracing::debug!(bytes, held = self.art.used_bytes() + bytes, "panel cover decoded");
                let handle =
                    cover.map(|cover| iced::widget::image::Handle::from_rgba(cover.width, cover.height, cover.rgba));
                self.art.insert(id, handle, bytes);
                Task::none()
            }
            Message::Chrome(message) => self.chrome(message),
            Message::Pointer(window, at) if self.window == Some(window) => match at {
                Some(at) => self.chrome(chrome::Message::Activity { x: at.x, y: at.y }),
                None => self.chrome(chrome::Message::Left),
            },
            Message::Pointer(..) => Task::none(),
            // `resolvePlayerEscapeAction`: fullscreen leaves the key to the system, then the panel closes,
            // and only with neither does the player go back.
            Message::PlayerEscape if self.composing => Task::none(),
            Message::PlayerEscape => {
                if self.fullscreen {
                    self.toggle_fullscreen()
                } else if self.panel.is_open() {
                    self.panel(panel::Message::Close)
                } else {
                    self.handle(Message::Navigate(Page::Library))
                }
            }
            Message::PlayerComposing(composing) => {
                self.composing = composing;
                Task::none()
            }
            Message::Login(message) => {
                let (task, effect) = self.login.update(
                    message,
                    self.omni.as_ref(),
                    &self.library.online,
                    !self.playback.queue.items().is_empty(),
                );
                let task = task.map(Message::Login);
                let follow = match effect {
                    Some(login::Effect::Confirmed(provider)) => {
                        let switching = provider != self.library.online.provider();
                        let clear = if switching { self.leave_provider() } else { Task::none() };
                        let read = self.library.signed_in(provider, &self.i18n).map(Message::Library);
                        Task::batch([clear, read, self.keep_provider(provider)])
                    }
                    Some(login::Effect::Switch(provider)) => {
                        let clear = self.leave_provider();
                        let read = self.library.switch_provider(provider, &self.i18n).map(Message::Library);
                        Task::batch([clear, read, self.keep_provider(provider)])
                    }
                    Some(login::Effect::KeepSession(provider)) => self
                        .library
                        .online
                        .keep_session(provider)
                        .map(|message| Message::Library(library::Message::Online(message))),
                    Some(login::Effect::SignOut(provider)) => {
                        // Signing out of the active platform empties the player, as a switch does.
                        if provider == self.library.online.provider() {
                            self.playback.clear();
                            if self.page == Page::NowPlaying {
                                self.page = Page::Library;
                            }
                        }
                        self.library.sign_out(provider, &self.i18n).map(Message::Library)
                    }
                    None => Task::none(),
                };
                Task::batch([task, follow])
            }
        }
    }

    /// Opens the shared sign-in window: on the platform in use when someone is signed in, unchosen for a guest.
    fn open_login(&mut self) -> Task<Message> {
        let online = &self.library.online;
        let provider = online
            .platforms()
            .into_iter()
            .find(|platform| platform.available)
            .filter(|platform| matches!(online.account_for(platform.id), online::AccountState::SignedIn(_)))
            .map(|platform| platform.id);
        self.open_login_on(provider)
    }

    /// The sign-in window on `provider`, from the home page when the player's account tab asks for it.
    fn open_login_on(&mut self, provider: Option<&'static str>) -> Task<Message> {
        self.page = Page::Library;
        self.login.open(provider, &self.library.online, self.omni.as_ref()).map(Message::Login)
    }

    /// The platform the playing song belongs to, else the one in use.
    fn playing_provider(&self) -> &'static str {
        let Some(id) =
            self.playback.current().and_then(|track| track.online.as_ref()).map(|online| online.provider.as_str())
        else {
            return self.library.online.provider();
        };
        self.library
            .online
            .platforms()
            .into_iter()
            .find(|platform| platform.id == id)
            .map_or_else(|| self.library.online.provider(), |platform| platform.id)
    }

    /// The playing platform's levels and the stored preference resolved for it; `standard` alone without a platform.
    fn audio_quality(&self) -> (&'static [AudioQuality], AudioQuality) {
        let provider = self.playing_provider();
        let Some(omni) = &self.omni else { return (STANDARD_ONLY, AudioQuality::Standard) };
        let levels = omni.audio_quality_levels(provider);
        if levels.is_empty() {
            return (STANDARD_ONLY, AudioQuality::Standard);
        }
        (levels, omni.resolve_audio_quality(provider, self.playback.settings.audio_quality))
    }

    /// Another platform takes over: the player empties, as a sign-out does, since its songs belong to the old one.
    fn leave_provider(&mut self) -> Task<Message> {
        self.playback.clear();
        if self.page == Page::NowPlaying {
            self.page = Page::Library;
        }
        Task::none()
    }

    /// Remembers the platform in use for the next start (`activeProviderId`).
    fn keep_provider(&mut self, provider: &'static str) -> Task<Message> {
        if self.general.online_provider.as_deref() == Some(provider) {
            return Task::none();
        }
        self.general.online_provider = Some(provider.to_owned());
        save(&self.store, &self.general)
    }

    fn await_player(&mut self) {
        if self.playback.current().is_some() {
            self.play_navigation.begin(self.playback.request_id(), self.page);
        } else {
            self.play_navigation.cancel();
        }
    }

    /// Plays what the home wall chose, or keeps the tab it switched to.
    fn library_effect(&mut self, effect: Option<library::Effect>) -> Task<Message> {
        match effect {
            Some(library::Effect::Play { queue, index, origin }) => {
                let name = origin.unwrap_or_else(|| self.i18n.t("localMusic.allSongs").to_owned());
                let start = self.playback.play(queue, index, Some(folia_core::Origin { name })).map(Message::Playback);
                self.await_player();
                Task::batch([start, self.ensure_art()])
            }
            Some(library::Effect::PlayFm { queue, fm, origin }) => {
                let start =
                    self.playback.play_fm(queue, fm, folia_core::Origin { name: origin }).map(Message::Playback);
                self.await_player();
                Task::batch([start, self.ensure_art()])
            }
            Some(library::Effect::Enqueue(tracks)) => {
                let added = self.playback.enqueue(tracks);
                tracing::debug!(added, "songs added to the queue");
                Task::none()
            }
            Some(library::Effect::Toggle) => {
                self.playback.update(playback::Message::Toggle, &self.store).map(Message::Playback)
            }
            Some(library::Effect::EnterPlayer) => {
                self.page = Page::NowPlaying;
                self.reveal_chrome();
                Task::none()
            }
            Some(library::Effect::EnqueueOne(track)) => {
                // `applyQueueAddBehavior('append')`: a queued song moves to the end.
                match self.playback.queue.items().iter().position(|queued| queued.id == track.id) {
                    Some(index) => {
                        self.playback.update(playback::Message::MoveEnd(index), &self.store).map(Message::Playback)
                    }
                    None => {
                        self.playback.enqueue(vec![track]);
                        Task::none()
                    }
                }
            }
            Some(library::Effect::SaveTab(tab)) => {
                self.general.home_tab = tab;
                save(&self.store, &self.general)
            }
            None => Task::none(),
        }
    }

    /// Starts a window drag once the pointer has moved 4px from where the press was first seen.
    fn drag_move(&mut self, id: iced::window::Id, x: f32, y: f32) -> Task<Message> {
        if !self.drag_armed {
            return Task::none();
        }
        if !self.pointer_held {
            self.drag_armed = false;
            self.drag_from = None;
            return Task::none();
        }
        let Some((origin_x, origin_y)) = self.drag_from else {
            self.drag_from = Some((x, y));
            return Task::none();
        };
        let dx = x - origin_x;
        let dy = y - origin_y;
        if dx * dx + dy * dy < 16.0 {
            return Task::none();
        }
        self.drag_armed = false;
        self.drag_from = None;
        iced::window::drag(id)
    }

    fn now_ms(&self) -> u64 {
        u64::try_from(self.started.elapsed().as_millis()).unwrap_or(u64::MAX)
    }

    fn reveal_chrome(&mut self) {
        self.clock = self.clock.reveal(self.now_ms());
    }

    fn toggle_fullscreen(&mut self) -> Task<Message> {
        let Some(id) = self.window else {
            return iced::window::oldest().map(Message::WindowReady);
        };
        self.fullscreen = !self.fullscreen;
        let mode = if self.fullscreen { iced::window::Mode::Fullscreen } else { iced::window::Mode::Windowed };
        iced::window::set_mode(id, mode)
    }

    fn chrome(&mut self, message: chrome::Message) -> Task<Message> {
        let Some(id) = self.window else {
            return iced::window::oldest().map(Message::WindowReady);
        };
        // The title bar sits above the panel, so its presses never reach the panel's outside-click layer.
        if matches!(
            &message,
            chrome::Message::DragArm
                | chrome::Message::Resize(_)
                | chrome::Message::Minimize
                | chrome::Message::ToggleMaximize
                | chrome::Message::ToggleFullscreen
                | chrome::Message::Close
        ) {
            let _ = self.panel(panel::Message::QualitySelect(quality_select::Message::Close));
        }
        match message {
            chrome::Message::Drag => iced::window::drag(id),
            chrome::Message::DragArm => {
                self.drag_armed = true;
                self.drag_from = None;
                Task::none()
            }
            chrome::Message::DragMove { x, y } => self.drag_move(id, x, y),
            chrome::Message::DragEnd => {
                self.drag_armed = false;
                self.drag_from = None;
                Task::none()
            }
            chrome::Message::Resize(direction) => iced::window::drag_resize(id, direction),
            chrome::Message::Minimize => iced::window::minimize(id, true),
            chrome::Message::ToggleMaximize => {
                self.maximized = !self.maximized;
                iced::window::toggle_maximize(id)
            }
            chrome::Message::ToggleFullscreen => self.toggle_fullscreen(),
            chrome::Message::Close => iced::window::close(id),
            chrome::Message::Activity { x, y } => {
                self.clock = self.clock.pointer(self.now_ms(), x, y);
                let height = self.window_size.height;
                if let Some(positioning) = self.positioning.as_mut() {
                    positioning.drag(y, height);
                }
                self.sync_home_back();
                let now = std::time::Instant::now();
                self.panel.set_chrome(self.chrome_visible(), now);
                self.titlebar.pointer(Some(y), now);
                Task::none()
            }
            chrome::Message::Left => {
                self.clock = self.clock.leave(self.now_ms(), self.pointer_held);
                let now = std::time::Instant::now();
                self.panel.set_chrome(self.chrome_visible(), now);
                self.titlebar.pointer(None, now);
                Task::none()
            }
            chrome::Message::Hover(button) => {
                self.titlebar.hover(button, std::time::Instant::now());
                Task::none()
            }
            chrome::Message::Frame => Task::none(),
        }
    }

    pub fn view(&self) -> Element<'_, Message> {
        let (text_color, muted) =
            (folia_visualizer::color(self.theme.primary, 1.0), folia_visualizer::color(self.theme.secondary, 1.0));
        let daylight = self.appearance.theme == ThemeMode::Light;
        let page: Element<'_, Message> = match self.page {
            Page::Library | Page::Search => {
                let playing = self.playback.current().map(|track| track.id);
                let queued = self
                    .library
                    .expanded_track()
                    .is_some_and(|track| self.playback.queue.items().iter().any(|queued| queued.id == track.id));
                let look = library::CardLook {
                    queued,
                    playing: self.playback.is_playing(),
                    window_width: self.window_size.width,
                };
                self.library.view(&self.i18n, &self.theme, daylight, muted, playing, look).map(Message::Library)
            }
            Page::NowPlaying => {
                let lyrics = self.playback.lyrics.as_ref().unwrap_or(&self.empty_lyrics);
                let tuning = self.tuning_of(self.style());
                let scene = Scene {
                    style: self.style(),
                    lyrics,
                    time: self.playback.lyric_time(),
                    theme: &self.theme,
                    font_scale: self.appearance.lyrics_font_scale,
                    seed: self.visualizer_seed.as_str(),
                    stage: None,
                    tuning,
                    backdrop: true,
                };
                let subtitle = self.subtitle();
                let bottom = subtitle_bottom(self.bar_offset(), self.subtitle_presence.value(self.now_ms()));
                let background = folia_visualizer::color(self.theme.background, 1.0);
                now_playing::view(scene, &self.i18n, muted, background, self.window_size, subtitle, bottom)
            }
        };
        let playing = self.page == Page::NowPlaying;
        let now = std::time::Instant::now();
        let mut frame: iced::widget::Stack<'_, Message, iced::Theme, iced::Renderer> = if playing {
            stack![page]
        } else if self.page == Page::Search {
            let provider = self.library.online.provider();
            let search_source = self
                .omni
                .as_ref()
                .and_then(|omni| omni.platforms().into_iter().find(|platform| platform.id == provider))
                .map_or(provider, |platform| platform.short_name);
            let search = self
                .search
                .view(
                    search::View {
                        i18n: &self.i18n,
                        text: text_color,
                        background: folia_visualizer::color(self.theme.background, 1.0),
                        accent: folia_visualizer::color(self.theme.accent, 1.0),
                        daylight,
                        width: self.window_size.width,
                        playing: self.playback.current().map(|track| track.id),
                        source: search_source,
                    },
                    now,
                )
                .map(Message::Search);
            let search = chrome::drag_area(search, Message::Chrome);
            stack![page, search]
        } else {
            let shell = home::overlay(&self.home, self.home_host(), now).map(Message::Home);
            let mut layers = stack![page, shell];
            // The cover flying between the wall and the poster is drawn over both.
            if let (Page::Library, Some(flight)) = (self.page, self.library.flight_view(now)) {
                layers = layers.push(flight.map(Message::Library));
            }
            layers
        };
        if playing && self.back_hotspot() {
            let back = chrome::round_button(
                Icon::ChevronLeft,
                20.0,
                40.0,
                Color { a: 0.2, ..Color::BLACK },
                Color { r: 1.0, g: 1.0, b: 1.0, a: 0.6 },
                self.i18n.t("ui.back"),
                Message::Navigate(Page::Library),
            );
            frame = frame.push(pin(
                back,
                Alignment::Start,
                Alignment::Start,
                iced::Padding { top: 24.0, right: 0.0, bottom: 0.0, left: 24.0 },
            ));
        }
        // The original mounts the bar on home too, collapsed until hovered (`buildAppOverlaysModel.ts:255`).
        if (playing || self.page == Page::Library) && self.playback.current().is_some() {
            let look = player_bar::Look {
                i18n: &self.i18n,
                primary: text_color,
                secondary: muted,
                background: folia_visualizer::color(self.theme.background, 1.0),
                daylight,
                window: self.window_size,
                bottom: self.bar_offset(),
            };
            frame = frame.push(self.bar.view(&self.playback, look, now).map(Message::Bar));
        }
        if let (true, Some(_)) = (playing, self.positioning) {
            let row = overlay::capsule_row(self.window_size.width, false);
            let height = if self.window_size.width < 640.0 { 127.0 } else { 72.0 };
            let bar = iced::Rectangle {
                x: (self.window_size.width - row) / 2.0,
                y: self.window_size.height - self.bar_offset() - height,
                width: row,
                height,
            };
            let look = positioner::Look {
                i18n: &self.i18n,
                accent: folia_visualizer::color(self.theme.accent, 1.0),
                daylight,
                window: self.window_size,
                bar,
            };
            frame = frame.push(positioner::view(look).map(Message::Positioner));
        }
        if playing {
            let provider = self.playing_provider();
            let platform = self.library.online.platforms().into_iter().find(|platform| platform.id == provider);
            let account = match self.library.online.account_for(provider) {
                online::AccountState::SignedIn(account) => Some(account),
                _ => None,
            };
            let (quality_levels, quality) = self.audio_quality();
            let panel_view = panel::View {
                i18n: &self.i18n,
                theme: &self.theme,
                daylight,
                width: self.window_size.width,
                height: self.window_size.height,
                track: self.playback.current(),
                queue: self.playback.queue.items(),
                index: self.playback.queue.current_index(),
                volume: self.playback.settings.volume,
                muted: self.playback.muted,
                lyric_offset: self.playback.lyric_offset,
                bottom: self.bar_offset(),
                repeat: self.playback.settings.repeat,
                style_id: &self.appearance.visualizer,
                has_lyrics: self.playback.lyrics.as_ref().is_some_and(|lyrics| !lyrics.lines.is_empty()),
                font_scale: self.appearance.lyrics_font_scale,
                drawn_style: self.style(),
                origin: self.playback.queue.origin().map(|origin| origin.name.as_str()),
                art: self.playback.current().and_then(|track| self.art.peek(&track.id)).and_then(Option::as_ref),
                pill_hover: self.panel.pill_hover(),
                platform,
                account,
                account_avatar: self.library.online.avatar_for(provider),
                syncing: self.library.online.syncing(provider),
                quality_levels,
                quality,
            };
            frame = frame.push(self.panel.view(panel_view, std::time::Instant::now()).map(Message::Panel));
        }
        if let Some(window) = &self.workbench {
            let style = self.style();
            let live = self
                .playback
                .lyrics
                .as_ref()
                .filter(|lyrics| !lyrics.lines.is_empty())
                .map(|lyrics| (lyrics, self.playback.lyric_time()));
            frame = frame.push(window.view(workbench::Look {
                i18n: &self.i18n,
                theme: &self.theme,
                daylight,
                window: self.window_size,
                style,
                font_scale: self.appearance.lyrics_font_scale,
                live,
            }));
        }
        if let Some(window) = &self.settings {
            let values = settings::Values {
                language: self.general.language,
                resolved: self.i18n.language(),
                appearance: &self.appearance,
                style: self.chosen_style(),
                bar_offset: self.player.bottom_bar_offset,
                bar_max: folia_store::bottom_bar_max_offset(self.window_size.height),
                has_track: self.playback.current().is_some(),
            };
            frame = frame.push(window.view(&self.i18n, &self.theme, daylight, self.window_size, values));
        }
        let look = login::Look {
            i18n: &self.i18n,
            tokens: settings_kit::Tokens::new(&self.theme, daylight),
            daylight,
            window: self.window_size,
            platforms: self.library.online.platforms(),
            active: self.library.online.provider(),
            online: &self.library.online,
            account: self
                .library
                .online
                .account_for(self.login.chosen_provider().unwrap_or(self.library.online.provider())),
            avatar: self
                .library
                .online
                .avatar_for(self.login.chosen_provider().unwrap_or(self.library.online.provider())),
            has_track: !self.playback.queue.items().is_empty(),
        };
        if let Some(window) = self.login.view(look, now) {
            frame = frame.push(window.map(Message::Login));
        }
        if let Some(fade) = &self.page_fade {
            frame = frame.push(fade.view(now));
        }
        if let Some((provider, opacity, shift)) = self.playback.subscription_notice(now)
            && let Some(platform) = self
                .omni
                .as_ref()
                .and_then(|omni| omni.platforms().into_iter().find(|platform| platform.id == provider))
        {
            frame =
                frame.push(status_toast::view(&self.i18n, platform, daylight, self.window_size.width, opacity, shift));
        }
        if let Some((provider, ok, signed_out, opacity, shift)) = self.library.online.sync_notice(now)
            && let Some(platform) = self
                .omni
                .as_ref()
                .and_then(|omni| omni.platforms().into_iter().find(|platform| platform.id == provider))
        {
            frame = frame.push(status_toast::sync_view(
                &self.i18n,
                platform,
                status_toast::SyncOutcome { ok, signed_out },
                daylight,
                self.window_size.width,
                opacity,
                shift,
            ));
        }
        // `z-[9999]`: the title bar sits above every page and dialog.
        let look = chrome::TitlebarLook {
            i18n: &self.i18n,
            player: playing,
            daylight,
            ink: text_color,
            maximized: self.maximized,
            fullscreen: self.fullscreen,
        };
        frame = frame.push(self.titlebar.view(look, now).map(Message::Chrome));
        frame = frame.push(chrome::resize_frame().map(Message::Chrome));
        let now = std::time::Instant::now();
        if self.splash.showing(now) {
            // A canvas for the plain cover keeps a GPU allocation after the splash is gone.
            if self.splash.word(now) > 0.004 {
                frame =
                    frame.push(iced::widget::canvas(self.splash.program(now)).width(Length::Fill).height(Length::Fill));
            } else {
                let color = self.splash.cover_color(now);
                let shade: Element<'_, Message> = container(space())
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .style(move |_| iced::widget::container::Style {
                        background: Some(color.into()),
                        ..iced::widget::container::Style::default()
                    })
                    .into();
                let layer = if self.splash.blocks_pointer() {
                    mouse_area(shade).interaction(mouse::Interaction::Idle).into()
                } else {
                    shade
                };
                frame = frame.push(layer);
            }
        }
        frame.width(Length::Fill).height(Length::Fill).into()
    }

    fn chrome_visible(&self) -> bool {
        self.page != Page::NowPlaying || self.clock.visible(self.now_ms(), self.pointer_held)
    }

    /// Surfaces that take the keyboard the way an open sheet does: settings, sign-in, the workbench, search.
    fn blocks_space(&self) -> bool {
        self.settings.is_some() || self.workbench.is_some() || self.login.is_open() || self.page == Page::Search
    }

    /// The original back button only appears inside the top-left 120px square.
    fn back_hotspot(&self) -> bool {
        self.clock.pointer.is_some_and(|(x, y)| x <= HOTSPOT && y <= HOTSPOT)
    }

    fn subtitle(&self) -> overlay::Subtitle<'_> {
        if self.style() == Style::Still {
            return overlay::Subtitle { translation: None, upcoming: None };
        }
        let Some(lyrics) = self.playback.lyrics.as_ref() else {
            return overlay::Subtitle { translation: None, upcoming: None };
        };
        overlay::subtitle_at(&lyrics.lines, self.playback.lyric_time())
    }

    /// Ticks only while music plays: every frame on the visualizer page, a few times a second elsewhere.
    pub fn subscription(&self) -> Subscription<Message> {
        let mut parts = vec![
            iced::window::resize_events().map(|(_id, size)| Message::WindowResized(size)),
            iced::event::listen_with(fullscreen_key),
            iced::event::listen_with(window_pointer),
            // The release must reach every page, or a press that ends off the drag strip leaves the drag armed.
            iced::event::listen_with(chrome_pointer),
        ];
        if self.playback.current().is_some() && !self.blocks_space() {
            parts.push(iced::event::listen_with(space_key));
        }
        if self.page == Page::NowPlaying && self.clock.until_ms.is_some() {
            parts.push(iced::time::every(std::time::Duration::from_millis(200)).map(|_| Message::ChromeTick));
        }
        let player_keys = self.page == Page::NowPlaying
            && self.settings.is_none()
            && self.workbench.is_none()
            && !self.login.is_open()
            && self.positioning.is_none();
        if player_keys {
            parts.push(iced::event::listen_with(player_key));
            parts.push(iced::event::listen_with(player_ime));
        }
        if let Some(window) = &self.settings {
            parts.push(iced::event::listen_with(settings_key));
            if window.animating() {
                parts.push(iced::window::frames().map(|_| Message::Settings(settings::Message::Frame)));
            }
        }
        if let Some(window) = &self.workbench {
            parts.push(iced::event::listen_with(workbench_key));
            if window.ticking() {
                parts.push(iced::window::frames().map(|_| Message::Workbench(workbench::Message::Frame)));
            }
        }
        if self.positioning.is_some() {
            parts.push(iced::event::listen_with(positioner_key));
        }
        let sheets = self.settings.is_none() && self.workbench.is_none() && !self.login.is_open();
        // The tools panel leaves the wall its keys; only its Escape is the panel's (`WallToolsSurface`).
        let beside_tools = self.home.tools_open() && !self.home.more_open() && !self.library.menu_open();
        if self.page == Page::Library && sheets && (!self.home_menu_open() || beside_tools) {
            if self.home.search_query().is_some() {
                parts.push(iced::event::listen_with(seam_search_key));
                parts.push(iced::event::listen_with(seam_search_ime));
            } else {
                if beside_tools {
                    parts.push(iced::event::listen_with(wall_key_beside_tools));
                } else {
                    parts.push(iced::event::listen_with(wall_key));
                }
                if self.library.filter_editing() {
                    parts.push(iced::event::listen_with(filter_escape));
                }
            }
        }
        if self.login.is_open() {
            parts.push(iced::event::listen_with(login_key));
        }
        if self.login.animating(std::time::Instant::now()) {
            parts.push(iced::window::frames().map(|_| Message::Login(login::Message::Frame)));
        }
        if self.splash.showing(std::time::Instant::now()) {
            parts.push(iced::window::frames().map(|_| Message::SplashFrame));
        }
        let now = std::time::Instant::now();
        if self.playback.notice_animating(now) {
            parts.push(iced::window::frames().map(|_| Message::Playback(playback::Message::NoticeFrame)));
        }
        if self.library.online.sync_notice_animating(now) {
            parts.push(
                iced::window::frames().map(|_| Message::Library(library::Message::Online(online::Message::SyncFrame))),
            );
        }
        if self.page != Page::Search && (self.bar.animating(now) || self.subtitle_presence.running(self.now_ms())) {
            parts.push(iced::window::frames().map(|_| Message::Bar(player_bar::Message::Frame)));
        }
        if self.page_fade.as_ref().is_some_and(|fade| fade.running(now)) {
            parts.push(iced::window::frames().map(|_| Message::Chrome(chrome::Message::Frame)));
        }
        if self.titlebar.animating(now) {
            parts.push(iced::window::frames().map(|_| Message::Chrome(chrome::Message::Frame)));
        }
        if self.page == Page::Library
            && (self.home.animating(now)
                || self.library.coasting()
                || self.library.hint_busy(now)
                || self.seam_flip.is_some())
        {
            parts.push(iced::window::frames().map(|_| Message::Library(library::Message::Frame)));
        }
        if self.page == Page::NowPlaying && self.panel.animating(now) {
            parts.push(iced::window::frames().map(|_| Message::Panel(panel::Message::Frame)));
        }
        if self.page == Page::Library && self.home_menu_open() {
            parts.push(iced::event::listen_with(home_dismiss));
        }
        if self.page == Page::Search {
            if self.settings.is_none() {
                parts.push(iced::event::listen_with(search_key));
            }
            if self.search.animating(now) {
                parts.push(iced::window::frames().map(|_| Message::Search(search::Message::Frame)));
            }
        }
        if self.playback.is_playing() {
            let tick = |_| Message::Playback(playback::Message::Tick);
            let playback = if self.page != Page::NowPlaying {
                iced::time::every(std::time::Duration::from_millis(250)).map(tick)
            } else {
                let fps = match (self.appearance.frame_rate.fps(), self.style().max_fps()) {
                    (Some(setting), Some(cap)) => Some(setting.min(cap)),
                    (setting, cap) => setting.or(cap),
                };
                match fps {
                    Some(fps) => iced::time::every(std::time::Duration::from_secs_f64(1.0 / f64::from(fps))).map(tick),
                    None => iced::window::frames().map(tick),
                }
            };
            parts.push(playback);
        }
        Subscription::batch(parts)
    }

    fn workbench(&mut self, message: workbench::Message) -> Task<Message> {
        let style = self.style();
        let manifest = style.manifest();
        let Some(window) = self.workbench.as_mut() else { return Task::none() };
        let effect = window.update(message, manifest);
        let values = window.values.clone();
        match effect {
            workbench::Effect::None => Task::none(),
            workbench::Effect::Close => {
                self.workbench = None;
                Task::none()
            }
            workbench::Effect::Draft => {
                self.hold_tuning(style, values);
                Task::none()
            }
            workbench::Effect::Save => self.store_tuning(style, values),
            workbench::Effect::Style(next) => {
                self.appearance.visualizer = next.id().to_owned();
                if let Some(window) = self.workbench.as_mut() {
                    window.values = next.load(&self.visualizer.tables);
                }
                save(&self.store, &self.appearance)
            }
            workbench::Effect::Font(scale) => {
                let before = self.appearance.clone();
                self.appearance.lyrics_font_scale = scale;
                self.appearance_changed(&before)
            }
        }
    }

    fn hold_tuning(&mut self, style: Style, values: Values) {
        let manifest = style.manifest();
        self.visualizer.insert_style(manifest.storage_key, values.to_table(manifest), manifest.older_keys);
    }

    fn store_tuning(&mut self, style: Style, values: Values) -> Task<Message> {
        self.hold_tuning(style, values);
        save(&self.store, &self.visualizer)
    }

    fn tuning_of(&self, style: Style) -> Values {
        style.load(&self.visualizer.tables)
    }

    fn style(&self) -> Style {
        match self.chosen_style() {
            Some(style) => style,
            None => Style::random_for(self.playback.current().map_or(0, |track| track.id)),
        }
    }

    /// The style picked in settings; `None` means a random style per song.
    fn chosen_style(&self) -> Option<Style> {
        if self.appearance.visualizer == RANDOM_STYLE {
            return None;
        }
        Some(Style::from_id(&self.appearance.visualizer).unwrap_or_default())
    }

    fn home_host(&self) -> home::Host<'_> {
        let now = std::time::Instant::now();
        let leaving = self.seam_flip.as_ref().filter(|flip| flip.leaving(now));
        let search = match leaving {
            Some(flip) => flip.search.as_deref(),
            None => self.home.search_query(),
        };
        let mut host = home::Host {
            i18n: &self.i18n,
            text: folia_visualizer::color(self.theme.primary, 1.0),
            background: folia_visualizer::color(self.theme.background, 1.0),
            accent: folia_visualizer::color(self.theme.accent, 1.0),
            daylight: self.appearance.theme == ThemeMode::Light,
            has_track: self.playback.current().is_some(),
            busy: self.library.busy,
            explore_refresh: self.library.explore_refresh(),
            volume: self.playback.settings.volume,
            muted: self.playback.muted,
            tokens: settings_kit::Tokens::new(&self.theme, self.appearance.theme == ThemeMode::Light),
            queue: self.playback.queue.items(),
            queue_index: self.playback.queue.current_index(),
            fm: self.playback.is_fm(),
            window_width: self.window_size.width,
            window_height: self.window_size.height,
            seam: self.library.seam(),
            spine: leaving.map_or(self.home.is_spine(), |flip| flip.spine),
            horizontal: self.home.is_horizontal(),
            opacity: 1.0,
            bottom: if self.home.is_horizontal() {
                0.0
            } else {
                home::seam_safe_bottom(self.bar_offset(), self.home.presence(std::time::Instant::now()))
            },
            status: self.library.status(&self.i18n),
            show_tab: self.home.is_hidden() || self.library.seam().collapsed,
            tab_hidden: self.home.is_hidden(),
            tab: self.home.tab(std::time::Instant::now()),
            tabs: library::TABS.iter().map(|&tab| (tab, self.i18n.t(library::tab_label(tab)))).collect(),
            current: leaving.map_or(self.library.tab, |flip| flip.tab),
            account: self.account_slot(),
            layer: match leaving {
                Some(flip) => flip.layer.clone(),
                None => self.library.layer_seam(
                    &self.i18n,
                    self.playback.current().map(|track| track.id),
                    (self.window_size.height / home::LIST_ROW) as usize,
                    self.window_size,
                ),
            },
            poster_ground: self.library.poster_ground(),
            poster_tilted: self.library.poster_tilted(),
            list_hover: self.library.layer.as_ref().and_then(|layer| layer.list_hover),
            search,
        };
        // The flip turns at the outgoing content's layout width until it swaps, so the old seam does not dim out.
        let layout = leaving.map_or_else(
            || {
                home::content_layout(
                    host.horizontal,
                    host.spine,
                    host.search.is_some(),
                    host.layer.as_ref(),
                    host.window_width,
                )
            },
            |flip| flip.layout,
        );
        host.opacity = content_opacity(host.seam.width, layout.max(1.0))
            * self.seam_flip.as_ref().map_or(1.0, |flip| flip.opacity(now));
        host
    }

    /// What the seam shows, to tell when its content changes layer or tab and turns over.
    fn seam_key(&self) -> SeamKey {
        (
            self.library.tab,
            self.library.layer.as_ref().map(|layer| layer.collection.id.clone()),
            self.home.is_spine(),
            self.home.search_query().is_some(),
        )
    }

    /// The seam content before a message that can change the layer, tab or search panel, for it to turn away.
    fn seam_before(&self, message: &Message) -> Option<SeamBefore> {
        let turns = matches!(
            message,
            Message::Library(
                library::Message::Tab(_)
                    | library::Message::Back
                    | library::Message::ReloadCollection
                    | library::Message::Wall(wall::Message::Press(Some(_), _))
            ) | Message::Home(
                home::Message::SelectTab(_)
                    | home::Message::Back
                    | home::Message::ReloadCollection
                    | home::Message::TitleToggle { .. }
                    | home::Message::Expand
                    | home::Message::Search
                    | home::Message::CloseSearch
                    | home::Message::EscapeSearch
            )
        );
        if !turns || self.page != Page::Library {
            return None;
        }
        let rows = (self.window_size.height / home::LIST_ROW) as usize;
        let layer =
            self.library.layer_seam(&self.i18n, self.playback.current().map(|track| track.id), rows, self.window_size);
        let search = self.home.search_query().map(str::to_owned);
        let layout = home::content_layout(
            self.home.is_horizontal(),
            self.home.is_spine(),
            search.is_some(),
            layer.as_ref(),
            self.window_size.width,
        );
        Some((self.library.tab, layer, search, layout, self.seam_key()))
    }

    /// A change of page to or from the player is held until the page being left is captured, then cross-faded.
    fn fade_page(&mut self, before: Page) -> Task<Message> {
        let to = self.page;
        let crosses = before != to && (before == Page::NowPlaying || to == Page::NowPlaying);
        let Some(window) = self.window.filter(|_| crosses) else { return Task::none() };
        self.page = before;
        let capturing = self.next_page.replace(to).is_some();
        if capturing {
            return Task::none();
        }
        iced::window::screenshot(window).map(Message::PageShot).chain(Task::done(Message::PageShotDone))
    }

    /// The seam's account entry on online tabs (`useBravaisHomeAccount`).
    fn account_slot(&self) -> Option<home::AccountSlot<'_>> {
        if !self.library.tab.is_online() || self.library.layer.is_some() {
            return None;
        }
        let platforms = self.library.online.platforms();
        let platform = platforms.iter().find(|platform| platform.available).or_else(|| platforms.first())?;
        let mark = folia_visualizer::color(folia_core::Rgb::hex(platform.color), 1.0);
        let label = platform.short_name;
        Some(match self.library.online.signed_in() {
            Some(account) => home::AccountSlot {
                guest: false,
                name: account.nickname.clone(),
                avatar: self.library.online.avatar.as_ref(),
                icon: platform_icon::platform_icon(platform.id),
                letter: platform.letter,
                mark,
                tip: format!("{} · {label} · {}", self.i18n.t("home.switchOnlineProvider"), account.nickname),
            },
            None => home::AccountSlot {
                guest: true,
                name: self.i18n.t("libraryBravaisHome.connect").to_owned(),
                avatar: None,
                icon: platform_icon::platform_icon(platform.id),
                letter: platform.letter,
                mark,
                tip: format!(
                    "{} · {label} · {}",
                    self.i18n.t("libraryBravaisHome.connect"),
                    self.i18n.t("libraryBravaisAccount.notSignedIn")
                ),
            },
        })
    }

    /// Decodes the playing song's cover for the panel in the background, unless it is cached or on its way.
    fn ensure_art(&mut self) -> Task<Message> {
        let Some(track) = self.playback.current() else { return Task::none() };
        let id = track.id;
        if self.art_pending == Some(id) {
            return Task::none();
        }
        if self.art.peek(&id).is_some() {
            self.art.touch(&id);
            return Task::none();
        }
        if let Some(url) = track.cover_url.clone() {
            let Some(omni) = self.omni.clone() else {
                self.art.insert(id, None, 0);
                return Task::none();
            };
            self.art_pending = Some(id);
            return Task::perform(crate::background::online_cover(omni, url, ART_PX), move |cover| {
                Message::Art(id, cover)
            });
        }
        if track.path.as_os_str().is_empty() {
            self.art.insert(id, None, 0);
            return Task::none();
        }
        self.art_pending = Some(id);
        let path = track.path.clone();
        Task::perform(
            crate::background::blocking(move || folia_library::cover_for(&path, ART_PX, ART_PX)),
            move |cover| Message::Art(id, cover.flatten()),
        )
    }

    fn positioner(&mut self, message: positioner::Message) -> Task<Message> {
        let Some(positioning) = self.positioning.as_mut() else { return Task::none() };
        match message {
            positioner::Message::Grab => positioning.grab(self.clock.pointer.map(|(_, y)| y)),
            positioner::Message::Reset => positioning.reset(),
            positioner::Message::Cancel => self.positioning = None,
            positioner::Message::Commit => {
                self.player.bottom_bar_offset = positioning.live;
                self.positioning = None;
                return save(&self.store, &self.player);
            }
        }
        Task::none()
    }

    /// `bottom` of the bar and everything on its baseline: the one being dragged, else the saved one.
    fn bar_offset(&self) -> f32 {
        let offset = self.positioning.map_or(self.player.bottom_bar_offset, |positioning| positioning.live);
        folia_store::clamp_bottom_bar_offset(offset, self.window_size.height)
    }

    fn panel(&mut self, message: panel::Message) -> Task<Message> {
        let style = self.appearance.visualizer.clone();
        let (quality_levels, quality) = self.audio_quality();
        let room = panel::Room {
            width: self.window_size.width,
            height: self.window_size.height,
            bottom: self.bar_offset(),
            queue: self.playback.queue.items(),
            index: self.playback.queue.current_index(),
            quality_levels,
            quality,
        };
        let (effect, scroll) = self.panel.update(&message, &style, room);
        let scroll = scroll.map(Message::Panel);
        let task = match effect {
            panel::Effect::None => Task::none(),
            panel::Effect::Home => {
                self.page = Page::Library;
                Task::none()
            }
            panel::Effect::Locate => {
                self.page = Page::Library;
                match self.playback.current().map(|track| track.id) {
                    Some(track) => self.library_message(library::Message::Locate(track)),
                    None => Task::none(),
                }
            }
            panel::Effect::Settings => {
                self.settings = Some(settings::Settings::open());
                Task::none()
            }
            panel::Effect::Workbench => {
                let mut window = workbench::Workbench::open();
                window.values = self.tuning_of(self.style());
                self.workbench = Some(window);
                Task::none()
            }
            panel::Effect::Intensity => {
                self.theme.intensity = match self.theme.intensity {
                    folia_core::AnimationIntensity::Calm => folia_core::AnimationIntensity::Normal,
                    folia_core::AnimationIntensity::Normal => folia_core::AnimationIntensity::Chaotic,
                    folia_core::AnimationIntensity::Chaotic => folia_core::AnimationIntensity::Calm,
                };
                Task::none()
            }
            panel::Effect::Playback(message) => self.playback.update(message, &self.store).map(Message::Playback),
            panel::Effect::Theme(mode) => {
                let before = self.appearance.clone();
                let intensity = self.theme.intensity;
                self.appearance.theme = mode;
                self.theme = mode.theme();
                self.theme.intensity = intensity;
                self.iced_theme = iced_theme(&self.theme);
                self.appearance_changed(&before)
            }
            panel::Effect::Visualizer(style) => {
                self.appearance.visualizer = style.map_or(RANDOM_STYLE, Style::id).to_owned();
                save(&self.store, &self.appearance)
            }
            panel::Effect::Login => {
                let provider = self.playing_provider();
                self.open_login_on(Some(provider))
            }
            panel::Effect::Sync => {
                let provider = self.playing_provider();
                self.library.online.sync(provider).map(|message| Message::Library(library::Message::Online(message)))
            }
            panel::Effect::SignOut => {
                let provider = self.playing_provider();
                let clear =
                    if provider == self.library.online.provider() { self.leave_provider() } else { Task::none() };
                let sign_out = self.library.sign_out(provider, &self.i18n).map(Message::Library);
                Task::batch([clear, sign_out])
            }
            panel::Effect::Quality(level) if level == quality => Task::none(),
            panel::Effect::Quality(level) => {
                self.playback.update(playback::Message::Quality(level), &self.store).map(Message::Playback)
            }
        };
        Task::batch([task, scroll])
    }

    fn home(&mut self, message: home::Message) -> Task<Message> {
        if !matches!(message, home::Message::CollectionMenu) {
            self.library.dismiss_menu();
        }
        let reopen = matches!(message, home::Message::Tab) && !self.home.is_hidden();
        let query = self.home.search_query().map(str::to_owned);
        let queue_open = self.home.queue_island_open();
        self.home.update(&message);
        let now = std::time::Instant::now();
        let mut extra = Task::none();
        if self.home.queue_island_open() != queue_open {
            self.general.wall_tools_queue_island_open = self.home.queue_island_open();
            extra = save(&self.store, &self.general);
        }
        let mut replan = reopen;
        let task = match message {
            home::Message::TitleFocus { backwards } => {
                if backwards {
                    iced::widget::operation::focus_previous()
                } else {
                    iced::widget::operation::focus_next()
                }
            }
            home::Message::Search => {
                if self.library.layer.is_some() {
                    Task::none()
                } else {
                    self.home.open_search(now);
                    replan = true;
                    Task::none()
                }
            }
            home::Message::SubmitSearch => {
                let query = query.map(|query| query.trim().to_owned()).filter(|query| !query.is_empty());
                let Some(query) = query else { return Task::none() };
                self.home.close_search(now);
                replan = true;
                self.search.begin(query);
                self.search.reveal();
                self.page = Page::Search;
                let provider = self.library.online.provider();
                let (task, _) = self.search.update(search::Message::Submit, self.omni.as_ref(), provider);
                task.map(Message::Search)
            }
            home::Message::SearchQuery(_) => Task::none(),
            home::Message::EscapeSearch => {
                if self.home.search_query().is_none() {
                    replan = true;
                }
                Task::none()
            }
            home::Message::Composing(_) => Task::none(),
            home::Message::CloseSearch => {
                replan = true;
                Task::none()
            }
            home::Message::ToolsQueue(message) => {
                let queue = self.playback.queue.items();
                let list_h = queue_island::list_height(queue.len());
                let (task, change) = self.home.queue_update(message, queue, list_h);
                let effect = match change {
                    Some(queue_island::Change::Jump(index)) => {
                        self.playback.update(playback::Message::Jump(index), &self.store).map(Message::Playback)
                    }
                    Some(queue_island::Change::MoveNext(index)) => {
                        self.playback.update(playback::Message::MoveNext(index), &self.store).map(Message::Playback)
                    }
                    Some(queue_island::Change::MoveEnd(index)) => {
                        self.playback.update(playback::Message::MoveEnd(index), &self.store).map(Message::Playback)
                    }
                    Some(queue_island::Change::Remove(index)) => {
                        self.playback.update(playback::Message::Remove(index), &self.store).map(Message::Playback)
                    }
                    Some(queue_island::Change::Reorder(from, to)) => {
                        self.playback.update(playback::Message::Reorder(from, to), &self.store).map(Message::Playback)
                    }
                    None => Task::none(),
                };
                Task::batch([task.map(|message| Message::Home(home::Message::ToolsQueue(message))), effect])
            }
            home::Message::ToolsQueueClear => {
                let (clear, task) = self.home.press_queue_clear(now);
                if clear {
                    self.playback.clear_queue();
                }
                task.map(Message::Home)
            }
            home::Message::Player => {
                if self.playback.current().is_some() {
                    self.page = Page::NowPlaying;
                    self.reveal_chrome();
                }
                Task::none()
            }
            home::Message::Settings => {
                self.settings = Some(settings::Settings::open());
                Task::none()
            }
            home::Message::Import => self.library_message(library::Message::Import),
            home::Message::Reload => self.library_message(library::Message::Reload),
            home::Message::RefreshExplore => self.library_message(library::Message::RefreshExplore),
            home::Message::SelectTab(tab) => self.library_message(library::Message::Tab(tab)),
            home::Message::Back => self.library_message(library::Message::Back),
            home::Message::PlayAll => self.library_message(library::Message::PlayAll),
            home::Message::EnqueueAll => self.library_message(library::Message::EnqueueAll),
            home::Message::Query(query) => self.library_message(library::Message::Query(query)),
            home::Message::OpenList => self.library_message(library::Message::OpenList),
            home::Message::OpenFilter => self.library_message(library::Message::OpenFilter),
            home::Message::PosterCover(rect) => self.library_message(library::Message::PosterCoverAt(rect)),
            home::Message::FilterSubmit => self.library_message(library::Message::FilterSubmit),
            home::Message::ListScrolled(offset) => self.library_message(library::Message::ListScrolled(offset)),
            home::Message::LocateRow(row) => self.library_message(library::Message::LocateRow(row)),
            home::Message::PlayRow(row) => self.library_message(library::Message::PlayRow(row)),
            home::Message::ToggleSubscribe => self.library_message(library::Message::ToggleSubscribe),
            home::Message::CollectionMenu => self.library_message(library::Message::CollectionMenu),
            home::Message::ReloadCollection => self.library_message(library::Message::ReloadCollection),
            home::Message::RowHover(row) => self.library_message(library::Message::RowHover(row)),
            home::Message::Connect => self.open_login(),
            home::Message::Volume(volume) => {
                self.playback.update(playback::Message::Volume(volume), &self.store).map(Message::Playback)
            }
            home::Message::VolumeDone => {
                self.playback.update(playback::Message::VolumeDone, &self.store).map(Message::Playback)
            }
            home::Message::Mute => {
                self.playback.update(playback::Message::ToggleMute, &self.store).map(Message::Playback)
            }
            home::Message::Dismiss
            | home::Message::Escape
            | home::Message::Fold
            | home::Message::Expand
            | home::Message::TitleToggle { .. }
            | home::Message::Tab
            | home::Message::More
            | home::Message::Tools
            | home::Message::ToolsQueueToggle
            | home::Message::ToolsQueueClearTimeout(_)
            | home::Message::ToolsHelp
            | home::Message::BackHover(_)
            | home::Message::Frame => Task::none(),
        };
        if replan {
            self.library.replan(self.home.goal());
        }
        Task::batch([task, extra])
    }

    fn library_message(&mut self, message: library::Message) -> Task<Message> {
        let (task, effect) = self.library.update(message, &self.i18n);
        Task::batch([task.map(Message::Library), self.library_effect(effect)])
    }

    fn sync_home_back(&mut self) {
        let inside = self.clock.pointer.is_some_and(|(x, y)| x <= HOTSPOT && y <= HOTSPOT);
        let shown = self.page == Page::Library && self.playback.current().is_some() && inside;
        self.home.set_back(shown, std::time::Instant::now());
    }

    fn home_menu_open(&self) -> bool {
        self.home.menu_open() || self.library.menu_open()
    }

    /// Saves the appearance and lets an open settings window glide the marks the change moved.
    fn appearance_changed(&mut self, before: &AppearanceSettings) -> Task<Message> {
        if let Some(window) = self.settings.as_mut() {
            window.changed(before, &self.appearance);
        }
        save(&self.store, &self.appearance)
    }

    fn set_language(&mut self, preference: LanguagePreference) -> Task<Message> {
        let language: Language = preference.resolve(self.system_locale.as_deref());
        if let Err(err) = self.i18n.set_language(language) {
            tracing::error!(error = &err as &dyn std::error::Error, "cannot switch language");
            return Task::none();
        }
        self.general.language = preference;
        tracing::info!(language = language.code(), ?preference, "language switched");
        save(&self.store, &self.general)
    }
}

/// Reads the files' tags in the background and queues the playable ones.
fn open_files(paths: Vec<PathBuf>) -> Task<Message> {
    let work = background::blocking(move || {
        paths.iter().filter(|path| folia_library::is_audio(path)).map(|path| folia_library::read_track(path)).collect()
    });
    Task::perform(work, |tracks| Message::OpenFiles(tracks.unwrap_or_default()))
}

/// Top-left square that reveals the back button, from `PLAYER_CHROME_HOTSPOT_SIZE`.
const HOTSPOT: f32 = 120.0;

fn pin<'a>(button: Element<'a, Message>, x: Alignment, y: Alignment, pad: iced::Padding) -> Element<'a, Message> {
    container(button).width(Length::Fill).height(Length::Fill).align_x(x).align_y(y).padding(pad).into()
}

/// Space toggles play. Subscribed only when a track is loaded and no sheet is open.
fn space_key(event: iced::Event, status: iced::event::Status, _id: iced::window::Id) -> Option<Message> {
    space_toggle(event, status)
}

/// Maps Space to the player's play/pause toggle; a captured event (a text field, the focused seam title) and a
/// held key's repeats are ignored, so holding Space does not toggle on every repeat as the original does.
pub fn space_toggle(event: iced::Event, status: iced::event::Status) -> Option<Message> {
    if status == iced::event::Status::Captured {
        return None;
    }
    match event {
        iced::Event::Keyboard(iced::keyboard::Event::KeyPressed {
            key: iced::keyboard::Key::Named(iced::keyboard::key::Named::Space),
            repeat: false,
            ..
        }) => Some(Message::Playback(playback::Message::Toggle)),
        _ => None,
    }
}

/// F11 toggles fullscreen, the shortcut listed in the original help.
fn fullscreen_key(event: iced::Event, _status: iced::event::Status, _id: iced::window::Id) -> Option<Message> {
    match event {
        iced::Event::Keyboard(iced::keyboard::Event::KeyPressed {
            key: iced::keyboard::Key::Named(iced::keyboard::key::Named::F11),
            repeat: false,
            ..
        }) => Some(Message::ToggleFullscreen),
        _ => None,
    }
}

/// Escape closes the search page. An open settings window takes the key first.
fn search_key(event: iced::Event, status: iced::event::Status, _id: iced::window::Id) -> Option<Message> {
    if status == iced::event::Status::Captured {
        return None;
    }
    match event {
        iced::Event::Keyboard(iced::keyboard::Event::KeyPressed {
            key: iced::keyboard::Key::Named(iced::keyboard::key::Named::Escape),
            ..
        }) => Some(Message::Search(search::Message::Close)),
        _ => None,
    }
}

/// Escape clears the settings search, or closes the window when there is none; an open list takes it first.
fn settings_key(event: iced::Event, status: iced::event::Status, _id: iced::window::Id) -> Option<Message> {
    if status == iced::event::Status::Captured {
        return None;
    }
    match event {
        iced::Event::Keyboard(iced::keyboard::Event::KeyPressed {
            key: iced::keyboard::Key::Named(iced::keyboard::key::Named::Escape),
            ..
        }) => Some(Message::Settings(settings::Message::Escape)),
        _ => None,
    }
}

fn workbench_key(event: iced::Event, status: iced::event::Status, _id: iced::window::Id) -> Option<Message> {
    if status == iced::event::Status::Captured {
        return None;
    }
    match event {
        iced::Event::Keyboard(iced::keyboard::Event::KeyPressed {
            key: iced::keyboard::Key::Named(iced::keyboard::key::Named::Escape),
            ..
        }) => Some(Message::Workbench(workbench::Message::Close)),
        _ => None,
    }
}

/// Presses keep the chrome up; a scroll wheel counts as activity. Neither captures the event.
fn home_dismiss(event: iced::Event, status: iced::event::Status, _id: iced::window::Id) -> Option<Message> {
    if status == iced::event::Status::Captured {
        return None;
    }
    match event {
        iced::Event::Keyboard(iced::keyboard::Event::KeyPressed {
            key: iced::keyboard::Key::Named(iced::keyboard::key::Named::Escape),
            ..
        }) => Some(Message::Home(home::Message::Escape)),
        iced::Event::Mouse(iced::mouse::Event::ButtonPressed(_)) => Some(Message::Home(home::Message::Dismiss)),
        _ => None,
    }
}

// Window-level, like the original's capturing `mousemove` and the area's `mouseleave`: hovering a control on top
// is not leaving, so a control appearing under a resting pointer cannot hide the chrome again.
fn window_pointer(event: iced::Event, _status: iced::event::Status, window: iced::window::Id) -> Option<Message> {
    match event {
        iced::Event::Mouse(iced::mouse::Event::CursorMoved { position }) => {
            Some(Message::Pointer(window, Some(position)))
        }
        iced::Event::Mouse(iced::mouse::Event::CursorLeft) => Some(Message::Pointer(window, None)),
        iced::Event::Window(iced::window::Event::Unfocused) => Some(Message::WindowUnfocused),
        _ => None,
    }
}

fn chrome_pointer(event: iced::Event, _status: iced::event::Status, _id: iced::window::Id) -> Option<Message> {
    match event {
        iced::Event::Mouse(iced::mouse::Event::ButtonPressed(_)) => Some(Message::PointerHeld(true)),
        iced::Event::Mouse(iced::mouse::Event::ButtonReleased(_)) => Some(Message::PointerHeld(false)),
        iced::Event::Mouse(iced::mouse::Event::WheelScrolled { .. }) => Some(Message::RevealChrome),
        _ => None,
    }
}

fn iced_theme(theme: &Theme) -> iced::Theme {
    let palette = iced::theme::Palette {
        background: folia_visualizer::color(theme.background, 1.0),
        text: folia_visualizer::color(theme.primary, 1.0),
        primary: folia_visualizer::color(theme.accent, 1.0),
        ..iced::theme::Palette::DARK
    };
    iced::Theme::custom(theme.name.clone(), palette)
}

/// Escape on the player page, after the dialogs and the settings window have had their own listeners.
fn player_key(event: iced::Event, status: iced::event::Status, _id: iced::window::Id) -> Option<Message> {
    if status == iced::event::Status::Captured {
        return None;
    }
    match event {
        iced::Event::Keyboard(iced::keyboard::Event::KeyPressed {
            key: iced::keyboard::Key::Named(iced::keyboard::key::Named::Escape),
            repeat: false,
            ..
        }) => Some(Message::PlayerEscape),
        _ => None,
    }
}

/// Tracks the player page's input-method composition (`event.nativeEvent.isComposing`): only a preedit counts.
fn player_ime(event: iced::Event, _status: iced::event::Status, _id: iced::window::Id) -> Option<Message> {
    let iced::Event::InputMethod(event) = event else { return None };
    let composing = match event {
        iced::advanced::input_method::Event::Preedit(text, _) => !text.is_empty(),
        iced::advanced::input_method::Event::Opened
        | iced::advanced::input_method::Event::Commit(_)
        | iced::advanced::input_method::Event::Closed => false,
    };
    Some(Message::PlayerComposing(composing))
}

/// Escape closes the sign-in window (Sheet's own Escape).
/// The wall's keys (`resolveBravaisKey`), unless a control such as the filter took them.
fn wall_key(event: iced::Event, status: iced::event::Status, _id: iced::window::Id) -> Option<Message> {
    if status == iced::event::Status::Captured {
        return None;
    }
    let iced::Event::Keyboard(iced::keyboard::Event::KeyPressed { key, modifiers, repeat, .. }) = event else {
        return None;
    };
    Some(match home::wall_key_of(&key, modifiers, repeat)? {
        home::WallKey::Focus { backwards } => Message::Home(home::Message::TitleFocus { backwards }),
        home::WallKey::Search => Message::Home(home::Message::Search),
        home::WallKey::Card(key) => Message::Library(library::Message::Key(key)),
    })
}

/// The wall's keys under the open tools panel, whose Escape `home_dismiss` spends on closing it.
fn wall_key_beside_tools(event: iced::Event, status: iced::event::Status, id: iced::window::Id) -> Option<Message> {
    wall_key(event, status, id)
        .filter(|message| !matches!(message, Message::Library(library::Message::Key(focus_card::Key::Escape))))
}

/// Escape shuts the seam's search panel unless the input method is composing; the field keeps the key first.
fn seam_search_key(event: iced::Event, _status: iced::event::Status, _id: iced::window::Id) -> Option<Message> {
    match event {
        iced::Event::Keyboard(iced::keyboard::Event::KeyPressed {
            key: iced::keyboard::Key::Named(iced::keyboard::key::Named::Escape),
            repeat: false,
            ..
        }) => Some(Message::Home(home::Message::EscapeSearch)),
        _ => None,
    }
}

/// Tracks an input-method composition so Escape can leave it to the field (`event.nativeEvent.isComposing`).
fn seam_search_ime(event: iced::Event, _status: iced::event::Status, _id: iced::window::Id) -> Option<Message> {
    let iced::Event::InputMethod(event) = event else { return None };
    let composing = match event {
        iced::advanced::input_method::Event::Opened => true,
        iced::advanced::input_method::Event::Preedit(text, _) => !text.is_empty(),
        iced::advanced::input_method::Event::Commit(_) | iced::advanced::input_method::Event::Closed => false,
    };
    Some(Message::Home(home::Message::Composing(composing)))
}

/// Esc in the filter, which its input would otherwise keep for losing focus.
fn filter_escape(event: iced::Event, status: iced::event::Status, _id: iced::window::Id) -> Option<Message> {
    match event {
        iced::Event::Keyboard(iced::keyboard::Event::KeyPressed {
            key: iced::keyboard::Key::Named(iced::keyboard::key::Named::Escape),
            repeat: false,
            ..
        }) if status == iced::event::Status::Captured => {
            Some(Message::Library(library::Message::Key(focus_card::Key::Escape)))
        }
        _ => None,
    }
}

fn login_key(event: iced::Event, _status: iced::event::Status, _id: iced::window::Id) -> Option<Message> {
    match event {
        iced::Event::Keyboard(iced::keyboard::Event::KeyPressed {
            key: iced::keyboard::Key::Named(iced::keyboard::key::Named::Escape),
            ..
        }) => Some(Message::Login(login::Message::Close)),
        _ => None,
    }
}

/// Positioning mode's keyboard exits: Escape rolls back, Enter keeps the new height.
fn positioner_key(event: iced::Event, _status: iced::event::Status, _id: iced::window::Id) -> Option<Message> {
    let iced::Event::Keyboard(iced::keyboard::Event::KeyPressed { key: iced::keyboard::Key::Named(named), .. }) = event
    else {
        return None;
    };
    match named {
        iced::keyboard::key::Named::Escape => Some(Message::PositionerKey(false)),
        iced::keyboard::key::Named::Enter => Some(Message::PositionerKey(true)),
        _ => None,
    }
}
