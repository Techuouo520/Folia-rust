// Top-level UI: pages, navigation and the iced update/view/subscription functions that drive them.

mod ai_settings;
mod album_wall;
mod background;
mod background_image;
mod block_reflows;
pub mod catalog;
mod chrome;
mod color_picker;
mod cover_flight;
mod credits;
mod design_size;
pub mod developer;
mod drag_capture;
mod equalizer_dialog;
mod float_layer;
mod focus_card;
mod fonts;
pub mod graphics;
mod harmony;
mod home;
mod icons;
mod lattice;
mod lattice_controls;
mod library;
mod login;
mod lots;
mod measured;
#[cfg(target_os = "linux")]
mod media_session;
mod mode_picker;
mod motion;
mod now_playing;
mod online;
mod overlay;
mod page_fade;
mod panel;
mod panel_sheets;
mod platform_highlight;
mod platform_icon;
mod platform_rows;
mod playback;
mod playback_navigation;
mod playback_session;
mod player_bar;
mod playlist_dialog;
mod positioner;
mod poster;
mod poster_motion;
mod preview_art;
mod preview_stage;
mod quality_select;
mod queue_island;
pub mod radio;
mod radio_art;
mod radio_backdrop;
mod reorder;
mod scene_settings;
mod scene_sound;
mod scrollbar;
mod search;
pub mod search_results;
mod settings;
mod settings_kit;
pub mod smart_beat;
mod smooth_scroll;
pub mod song_theme;
mod sources;
mod splash;
mod storage;
pub use storage::Message as StorageMessage;
mod status_toast;
mod subtitle_text;
pub mod theme_park;
mod timeline;
mod typeface;
mod wall;
mod wall_canvas;
mod wall_hover;
mod wall_pan;
mod wall_shift;
mod workbench;

pub use background::{error_chain, format_time};
pub use chrome::resize_viewport;
pub use developer::Developer;
pub use equalizer_dialog::{Look as EqualizerLook, view as equalizer_view};
pub use focus_card::{Direction as CardDirection, Key as CardKey};
pub use home::title::seam_title_activates;
pub use home::{HELP_LIST, HelpRow, WallKey, default_level_is_spine, seam_safe_bottom, wall_key_of};
pub use home::{HomeChrome, Host as HomeHost, Message as HomeMessage, overlay as home_overlay};
pub use lattice::{Lattice, Message as LatticeMessage};
pub use library::Message as LibraryMessage;
pub use login::{Login, Look as LoginLook, Message as LoginMessage};
pub use online::Message as OnlineMessage;
pub use online::{AccountState, Online};
pub use overlay::{
    CAPSULE_COLLAPSE_MS, CAPSULE_EXPAND_MS, CAPSULE_FADE_MS, CAPSULE_LAYOUT_SPRING, CAPSULE_SETTLE_MS, CHROME_HOLD_MS,
    CHROME_LEAVE_MS, CapsuleHover, ChromeClock, ChromeSpring, GLOW_BLUR_PX, GlowEllipse, SUBTITLE_CLEARANCE,
    SUBTITLE_PRESENCE_SPRING, SpringParams, Subtitle, SubtitleRow, SubtitleRows, capsule_open, capsule_row,
    capsule_width, glow_coverage, glow_ellipse, row_gap, subtitle_at, subtitle_bottom, subtitle_line_width,
    subtitle_rows_at, translation_px, upcoming_look, upcoming_px,
};
pub use panel::Tab as PanelTab;
pub use panel::{LikeLook, Message as PanelMessage, PlayerPanel, Room as PanelRoom, View as PanelView, like_look};
pub use panel_sheets::{Look as SheetsLook, Sheets, view as sheets_view};
pub use platform_highlight::ProviderHighlight;
pub use platform_rows::{ProviderRowStatus, order_provider_rows};
pub use playback::{AudioFailure, LoadPhase, Loading, Message as PlaybackMessage, Playback};
pub use playback_navigation::PlaybackNavigation;
/// The floating capsule. `shown` is its 0.26s ease-out hide, shared by opacity, offset and scale.
pub use player_bar::{CapsuleBox, Look as PlayerBarLook, PlayerBar, capsule_box};
pub use playlist_dialog::{Dialog as PlaylistDialog, Look as PlaylistLook, Message as PlaylistMessage, PlaylistChoice};
pub use reorder::drag_to_reorder;
pub use scene_settings::{
    Look as SceneSettingsLook, Message as SceneSettingsMessage, State as SceneSettingsWindow, Tab as SceneSettingsTab,
};
pub use scene_sound::Message as SceneSoundMessage;
pub use search::{Kind as SearchKind, Message as SearchMessage, SearchPage, View as SearchView};
pub use search_results::format_duration;
pub use settings::{Message as SettingsMessage, Section as SettingsSection, Settings, Values as SettingsValues};
pub use settings_kit::{Faded, Tokens, panel_glass, quick_tile_fill, tools_glass};
pub use smooth_scroll::{LINE_PX, PROGRAM_EASE, PROGRAM_MS, Scroll, WHEEL_EASE, WHEEL_MS};
pub use sources::Sources;
pub use timeline::{Follow, Message as TimelineMessage, Timeline, USER_SCROLL_MS, active_line, seek_time};
pub use typeface::SYSTEM as INTERFACE_FONT;
pub use typeface::install as install_fonts;
pub use typeface::interface_font_for;
pub use typeface::text as interface_text;
pub use typeface::{TextStyle, UI_TEXT_STYLES};
pub use wall::{
    CoverLayers as WallCoverLayers, CoverSource as WallCover, Message as WallMessage, Shape as WallShape,
    Tile as WallTile, Wall,
};
pub use wall::{SEAM_ANCHOR, Seam, content_opacity, plan_opening, seam_geometry, seam_on_axis, wider_span};
pub use wall_pan::{Glide as WallGlide, Inertia as WallInertia, Release as WallRelease};
pub use wall_shift::{Kind as ShiftKind, Pose as ShiftPose, Shift as WallShift};
pub use workbench::{
    Effect as WorkbenchEffect, Hotspot, Live as WorkbenchLive, Look as WorkbenchLook, Message as WorkbenchMessage,
    Sample as WorkbenchSample, Source as WorkbenchSource, Tab as WorkbenchTab, Workbench, hotspot_at, scheduled_saves,
};

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

use crate::background::{blocking, save};
use crate::library::LibraryPage;

/// The window folia-app opens; the shell lays out before the first resize event arrives.
pub const INITIAL_WINDOW: Size = Size::new(1200.0, 800.0);
/// The smallest window folia-app allows; anything reported below it is a minimized window.
pub const MIN_WINDOW_SIZE: Size = Size::new(350.0, 100.0);
/// Shown when there is no online platform, or one that was never built in.
const STANDARD_ONLY: &[AudioQuality] = &[AudioQuality::Standard];

/// Everything the app reads before the window opens.
#[derive(Clone)]
pub struct Init {
    pub storage: folia_store::StorageSettings,
    pub store: Store,
    pub general: GeneralSettings,
    pub appearance: AppearanceSettings,
    pub graphics: folia_store::GraphicsSettings,
    pub scene: folia_store::SceneSettings,
    pub system_reduced_motion: bool,
    pub analysis_models: PathBuf,
    pub theme: folia_store::ThemeSettings,
    pub audio: folia_store::AudioSettings,
    pub playback: PlaybackSettings,
    pub player: folia_store::PlayerSettings,
    pub visualizer: VisualizerSettings,
    pub developer: folia_store::DeveloperSettings,
    pub runtime_log: folia_store::RuntimeLog,
    pub i18n: I18n,
    pub system_locale: Option<String>,
    pub library_db: PathBuf,
    /// Where downloaded online audio is cached.
    pub cache_dir: PathBuf,
    /// Pictures the listener uploads, such as a Monet portrait.
    pub data_dir: PathBuf,
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
    Lattice,
    Search,
    NowPlaying,
    Catalog,
}

enum NavigationEntry {
    Page(Page),
    Search(Box<SearchPage>),
    Catalog(Box<catalog::CatalogPage>),
}

#[derive(Debug, Clone)]
pub enum Message {
    Storage(storage::Message),
    #[cfg(target_os = "linux")]
    Media(folia_media::Command),
    BackgroundPainted(u64, Option<iced::widget::image::Handle>),
    BackgroundPicked(Option<std::path::PathBuf>),
    BackgroundLoaded(u64, std::path::PathBuf, Result<std::sync::Arc<image::RgbaImage>, String>),
    Navigate(Page),
    OpenSettings,
    OpenWorkbench,
    OpenThemePark,
    ThemePark(theme_park::Message),
    OpenScene,
    Settings(settings::Message),
    SceneSettings(scene_settings::Message),
    SceneSound(scene_sound::Message),
    SceneFailed(folia_visualizer::cinema::Failure),
    Sources(sources::Message),
    SongTheme(song_theme::Message),
    /// The developer page, its log panel and the two overlays.
    Developer(developer::Message),
    Workbench(workbench::Message),
    LanguagePicked(LanguagePreference),
    Theme(ThemeMode),
    FollowSystemTheme,
    SystemTheme(iced::theme::Mode),
    FrameRate(FrameRate),
    Graphics(graphics::Message),
    SmartBeat(smart_beat::Message),
    /// A fixed lyric style, or `None` for a random style per song.
    Visualizer(Option<Style>),
    Library(library::Message),
    Search(search::Message),
    Catalog(catalog::Message),
    Back,
    Playback(playback::Message),
    Session(playback_session::Message),
    VisualFrame,
    UiFrame,
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
    WindowScale(f32),
    Maximized(bool),
    ChromeTick,
    PointerHeld(bool),
    /// The window lost focus; any press outside it will not report its release here.
    WindowUnfocused,
    WindowClosing,
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
    Lattice(u64, lattice::Message),
    Panel(panel::Message),
    /// The playlist picker over the window.
    Playlists(playlist_dialog::Message),
    /// One of the sheets the panel opens over the window.
    Sheets(panel_sheets::Message),
    /// The local playlists after a write, or the write's failure.
    LocalPlaylists(Result<Vec<folia_library::Playlist>, String>),
    /// A short status line under the title bar: whether it is good news, and its words.
    Status(bool, String),
    StatusLeaving(u64),
    StatusGone(u64),
    StatusFrame,
    /// The first lyric file was read and parsed for the playing song, or its failure.
    ImportedLyrics(Result<folia_core::Lyrics, String>),
    /// The platform's lyrics were asked for again for the playing song.
    MatchedLyrics(u64, i64, Result<Option<folia_core::Lyrics>, String>),
    /// The playing song's lyrics were written to this file, or its failure.
    ExportedLyrics(Result<std::path::PathBuf, String>),
    Art(i64, u64, crate::background::PanelArt),
    PortraitPicked(Option<std::path::PathBuf>),
    PortraitDecoded(String, Option<folia_library::Cover>),
    PortraitStored(std::path::PathBuf, folia_library::Cover),
    /// The Monet portrait was dragged to this horizontal offset.
    MonetOffset(f32),
    /// A lyric line was clicked on the playing page: play from this position in seconds.
    SeekTo(f64),
    /// The installed font families finished loading for the font picker.
    FontsLoaded(Vec<String>),
    Login(login::Message),
    /// Escape on the player page: leave fullscreen, close the panel, or go back to the wall.
    PlayerEscape,
    /// An input method is composing on the player page; Escape belongs to it.
    PlayerComposing(bool),
    Timeline(timeline::Message),
}

impl From<folia_visualizer::styles::monet::Event> for Message {
    fn from(event: folia_visualizer::styles::monet::Event) -> Self {
        match event {
            folia_visualizer::styles::monet::Event::Portrait(moved) => Message::MonetOffset(moved.0),
            folia_visualizer::styles::monet::Event::Seek(at) => Message::SeekTo(at),
        }
    }
}

/// Settings id of the random lyric style choice, as in the original app.
const RANDOM_STYLE: &str = "random";

/// Pixel budget of the panel's decoded covers: eight 360×360 RGBA covers (about 0.5 MB each), so stepping
/// back and forth between neighbouring songs does not decode again.
const ART_BYTES: usize = 4 * 1024 * 1024;
/// The panel cover is decoded at 360px, the panel's width.
const ART_PX: u32 = 360;

pub struct Shell {
    storage: storage::Storage,
    #[cfg(target_os = "linux")]
    media_session: media_session::Bridge,
    store: Store,
    /// The page being left for or from the player fades out over the new one.
    page_fade: Option<page_fade::PageFade>,
    /// The page to show once the one being left has been captured.
    next_page: Option<Page>,
    seam_flip: Option<home::flip::SeamFlip>,
    general: GeneralSettings,
    appearance: AppearanceSettings,
    theme_settings: folia_store::ThemeSettings,
    song_themes: song_theme::SongThemes,
    i18n: I18n,
    system_locale: Option<String>,
    theme: Theme,
    iced_theme: iced::Theme,
    page: Page,
    settings: Option<settings::Settings>,
    sources: sources::Sources,
    workbench: Option<workbench::Workbench>,
    theme_park: Option<theme_park::ThemePark>,
    visualizer: VisualizerSettings,
    developer: developer::Developer,
    developer_settings: folia_store::DeveloperSettings,
    graphics: folia_store::GraphicsSettings,
    scene: folia_store::SceneSettings,
    scene_window: Option<scene_settings::State>,
    scene_sound: scene_sound::SceneSound,
    scene_generation: u64,
    scene_failed: bool,
    closing: bool,
    smart_beat: smart_beat::State,
    applied_gpu: folia_store::GpuPreference,
    system_reduced_motion: bool,
    system_motion_error: Option<String>,
    visualizer_hold: Option<f64>,
    native_blur_applied: bool,
    native_blur_failed: bool,
    runtime_log: folia_store::RuntimeLog,
    memory_monitor: folia_store::MemoryMonitor,
    library: LibraryPage,
    search: SearchPage,
    catalog: Option<catalog::CatalogPage>,
    history: Vec<NavigationEntry>,
    playback_source: Option<NavigationEntry>,
    pending_playback_source: Option<(u64, NavigationEntry)>,
    omni: Option<Omni>,
    playback: Playback,
    playback_session: playback_session::Session,
    play_navigation: PlaybackNavigation,
    /// The local playlists as last read or written, the liked-songs list first.
    playlists: Vec<folia_library::Playlist>,
    /// The database the playlists live in; writes open it off the UI thread.
    db_path: PathBuf,
    sheets: panel_sheets::Sheets,
    add_to_playlist: playlist_dialog::Dialog,
    /// The panel's data as the last update left it: the picker rows, the style's quick controls and the reasons a
    /// heart or star is out of reach. `view` borrows these, so they must outlive the frame.
    panel_choices: Vec<panel::PlaylistChoice>,
    panel_quick: Vec<folia_visualizer::QuickControl>,
    panel_like_reason: Option<String>,
    panel_add_reason: Option<String>,
    /// `StatusToast`: whether it is good news, its words and when it started.
    status: Option<(bool, String, std::time::Instant)>,
    status_serial: u64,
    empty_lyrics: Lyrics,
    window: Option<iced::window::Id>,
    maximized: bool,
    fullscreen: bool,
    /// Origin for [`ChromeClock`] milliseconds.
    started: std::time::Instant,
    clock: ChromeClock,
    /// The playing-page back hotspot as it was when no modal covered it.
    back_latched: bool,
    /// `controlBarPresence`: the subtitle's room for the capsule, sprung as the chrome shows and hides.
    subtitle_presence: ChromeSpring,
    bar: player_bar::PlayerBar,
    window_size: Size,
    window_scale: f32,
    pointer_held: bool,
    /// Press on the drag strip, and the pointer position of the first move after it.
    drag_armed: bool,
    drag_from: Option<(f32, f32)>,
    splash: splash::Splash,
    home: home::HomeChrome,
    lattice: Option<lattice::Lattice>,
    lattice_generation: u64,
    panel: panel::PlayerPanel,
    art: folia_library::CoverCache<i64, iced::widget::image::Handle>,
    art_pending: Option<(i64, u64)>,
    art_task: Option<iced::task::Handle>,
    art_request_full: bool,
    art_generation: u64,
    /// Cover colours keyed by track id, taken when the cover is decoded.
    cover_palettes: folia_visualizer::PaletteStore,
    background_cover: Option<(i64, std::sync::Arc<image::RgbaImage>)>,
    background_poster: background_image::Poster,
    background_upload: Option<std::sync::Arc<image::RgbaImage>>,
    background_upload_generation: u64,
    titlebar: chrome::Titlebar,
    player: folia_store::PlayerSettings,
    positioning: Option<positioner::Positioning>,
    login: login::Login,
    /// The lyrics timeline window opened from the capsule.
    timeline: timeline::Timeline,
    /// An input method is composing in a field on the player page.
    composing: bool,
    /// Song id string the original visualizer uses as its geometry seed.
    visualizer_seed: String,
    /// Latest spectrum for the playing page and the style preview. Other pages leave it alone.
    audio: folia_core::AudioFrame,
    /// The equalizer and ReplayGain choices.
    audio_settings: folia_store::AudioSettings,
    /// A random style drawn again from the pool for one song (`randomVisualizerReroll`).
    random_choice: Option<(i64, Style)>,
    data_dir: std::path::PathBuf,
    /// Uploaded Monet portrait, keyed by its saved path.
    monet_portrait: Option<(String, iced::widget::image::Handle)>,
    /// Families installed on this machine, read off the text system in the background.
    system_fonts: Vec<String>,
}

struct UiFrames {
    settings: bool,
    login: bool,
    timeline: bool,
    splash: bool,
    notice: bool,
    online: bool,
    bar: bool,
    chrome: bool,
    home: bool,
    library: bool,
    panel: bool,
    status: bool,
    catalog: bool,
    search: bool,
}

impl UiFrames {
    fn active(&self) -> bool {
        [
            self.settings,
            self.login,
            self.timeline,
            self.splash,
            self.notice,
            self.online,
            self.bar,
            self.chrome,
            self.home,
            self.library,
            self.panel,
            self.status,
            self.catalog,
            self.search,
        ]
        .into_iter()
        .any(|active| active)
    }
}

impl Shell {
    fn ui_frames(&self, now: std::time::Instant) -> UiFrames {
        UiFrames {
            settings: self.settings.as_ref().is_some_and(settings::Settings::animating),
            login: self.login.animating(now),
            timeline: self.timeline.is_open() && self.timeline.animating(now),
            splash: self.splash.showing(now),
            notice: self.playback.notice_animating(now),
            online: self.library.online.sync_notice_animating(now),
            bar: self.page != Page::Search
                && (self.bar.animating(now) || self.subtitle_presence.running(self.now_ms())),
            chrome: self.page_fade.as_ref().is_some_and(|fade| fade.running(now)) || self.titlebar.animating(now),
            home: self.page == Page::Lattice && self.home.animating(now)
                || matches!(self.page, Page::Library | Page::Lattice)
                    && (self.home.scrolling(now) || self.home.tools_open() && self.song_themes.generating),
            library: self.page == Page::Library
                && (self.home.animating(now)
                    || self.library.coasting()
                    || self.library.hint_busy(now)
                    || self.seam_flip.is_some()),
            panel: self.page == Page::NowPlaying && self.panel.controls_animating(now),
            status: self.status.is_some(),
            catalog: self.page == Page::Catalog && self.catalog.as_ref().is_some_and(|page| page.animating(now)),
            search: self.page == Page::Search && (self.search.animating(now) || self.search.spinning()),
        }
    }

    fn advance_ui_frames(&mut self) -> Task<Message> {
        let frames = self.ui_frames(std::time::Instant::now());
        if !frames.active() {
            return Task::none();
        }
        let mut run = |active, message| if active { self.handle(message) } else { Task::none() };
        Task::batch([
            run(frames.settings, Message::Settings(settings::Message::Frame)),
            run(frames.login, Message::Login(login::Message::Frame)),
            run(frames.timeline, Message::Timeline(timeline::Message::Frame)),
            run(frames.splash, Message::SplashFrame),
            run(frames.notice, Message::Playback(playback::Message::NoticeFrame)),
            run(frames.online, Message::Library(library::Message::Online(online::Message::SyncFrame))),
            run(frames.bar, Message::Bar(player_bar::Message::Frame)),
            run(frames.chrome, Message::Chrome(chrome::Message::Frame)),
            run(frames.home, Message::Home(home::Message::Frame)),
            run(frames.library, Message::Library(library::Message::Frame)),
            run(frames.panel, Message::Panel(panel::Message::Frame)),
            run(frames.status, Message::StatusFrame),
            run(frames.catalog, Message::Catalog(catalog::Message::Frame)),
            run(frames.search, Message::Search(search::Message::Frame)),
        ])
    }

    pub fn new(init: Init) -> (Self, Task<Message>) {
        let storage = storage::Storage::new(init.storage, init.cache_dir);
        #[cfg(target_os = "linux")]
        let media_session = media_session::Bridge::new(storage.active.clone());
        typeface::language(init.i18n.language());
        motion::configure(&init.graphics.motion, init.system_reduced_motion);
        let smart_beat =
            smart_beat::State::new(&init.visualizer, init.analysis_models.clone(), init.data_dir.join("analysis.db"));
        let (scene_sound, scene_sound_task) = scene_sound::SceneSound::new(storage.active.clone());
        let theme = theme_for(&init.theme, init.appearance.theme, init.appearance.animation_intensity, None);
        let (song_themes, theme_load) =
            song_theme::SongThemes::new(init.store.clone(), init.data_dir.clone(), init.keyring_service.clone());
        let omni = Omni::new(storage.active.clone(), init.keyring_service)
            .inspect_err(|err| tracing::error!(error = err as &dyn std::error::Error, "online music unavailable"))
            .ok();
        let db_path = init.library_db.clone();
        let (library, load) = LibraryPage::new(
            init.library_db,
            init.general.home_tab,
            omni.clone(),
            init.general.online_provider.as_deref(),
            init.general.wall_randomize_on_entry,
        );
        let mut home = home::HomeChrome::new(INITIAL_WINDOW.width);
        home.restore_queue_island(init.general.wall_tools_queue_island_open);
        let equalizer = folia_player::Parameters::of(&init.audio.equalizer);
        let replay_gain = init.audio.replay_gain_mode;
        let mut developer = developer::Developer::default();
        developer.refresh_log(&init.runtime_log);
        let memory_monitor =
            folia_store::MemoryMonitor::new(init.data_dir.join("logs").join("memory").join("folia.log"));
        memory_monitor.set_log(
            init.developer.memory_monitor_enabled && init.developer.memory_log_enabled,
            init.developer.memory_log_mode,
        );
        if init.developer.memory_monitor_enabled {
            memory_monitor.start(init.developer.interval_ms());
            developer.refresh_memory(memory_monitor.history());
        }
        let playback = Playback::new(init.playback, omni.clone());
        let playback_session = playback_session::Session::new(&playback, init.open_files.is_empty());
        let mut shell = Self {
            #[cfg(target_os = "linux")]
            media_session,
            store: init.store,
            general: init.general,
            lattice: None,
            lattice_generation: 0,
            iced_theme: iced_theme(&theme),
            theme,
            appearance: init.appearance,
            theme_settings: init.theme,
            song_themes,
            i18n: init.i18n,
            system_locale: init.system_locale,
            page: Page::Library,
            page_fade: None,
            next_page: None,
            seam_flip: None,
            settings: None,
            sources: sources::Sources::default(),
            workbench: None,
            theme_park: None,
            visualizer: init.visualizer,
            developer,
            developer_settings: init.developer,
            applied_gpu: init.graphics.gpu_preference,
            graphics: init.graphics,
            scene: init.scene,
            scene_window: None,
            scene_sound,
            scene_generation: 0,
            scene_failed: false,
            closing: false,
            smart_beat,
            system_reduced_motion: init.system_reduced_motion,
            system_motion_error: None,
            visualizer_hold: None,
            native_blur_applied: false,
            native_blur_failed: false,
            runtime_log: init.runtime_log,
            memory_monitor,
            library,
            search: SearchPage::default(),
            catalog: None,
            history: Vec::new(),
            playback_source: None,
            storage,
            pending_playback_source: None,
            playback,
            playback_session,
            play_navigation: PlaybackNavigation::default(),
            playlists: Vec::new(),
            db_path: db_path.clone(),
            sheets: panel_sheets::Sheets::default(),
            add_to_playlist: playlist_dialog::Dialog::default(),
            panel_choices: Vec::new(),
            panel_quick: Vec::new(),
            panel_like_reason: None,
            panel_add_reason: None,
            status: None,
            status_serial: 0,
            omni,
            empty_lyrics: Lyrics::default(),
            window: None,
            maximized: false,
            fullscreen: false,
            started: std::time::Instant::now(),
            clock: ChromeClock::shown(0),
            back_latched: false,
            subtitle_presence: ChromeSpring::settled(1.0, 0, SUBTITLE_PRESENCE_SPRING),
            bar: player_bar::PlayerBar::new(std::time::Instant::now()),
            window_size: INITIAL_WINDOW,
            window_scale: 1.0,
            pointer_held: false,
            drag_armed: false,
            drag_from: None,
            splash: splash::Splash::new(),
            home,
            panel: panel::PlayerPanel::new(std::time::Instant::now()),
            art: folia_library::CoverCache::new(ART_BYTES),
            art_pending: None,
            art_task: None,
            art_request_full: false,
            art_generation: 0,
            cover_palettes: folia_visualizer::PaletteStore::new(),
            background_cover: None,
            background_poster: background_image::Poster::default(),
            background_upload: None,
            background_upload_generation: 0,
            titlebar: chrome::Titlebar::new(std::time::Instant::now()),
            player: init.player,
            positioning: None,
            login: login::Login::new(std::time::Instant::now()),
            timeline: timeline::Timeline::default(),
            composing: false,
            visualizer_seed: "lumiere".to_owned(),
            audio: folia_core::AudioFrame::SILENT,
            audio_settings: init.audio,
            random_choice: None,
            data_dir: init.data_dir,
            monet_portrait: None,
            system_fonts: Vec::new(),
        };
        shell.playback.set_equalizer(equalizer);
        shell.playback.set_replay_gain(replay_gain, None);
        let playlists = {
            let path = db_path;
            Task::perform(blocking(move || open_playlists(&path)), |result| {
                Message::LocalPlaylists(result.unwrap_or_else(|| Err("playlist worker stopped".to_owned())))
            })
        };
        let open = if init.open_files.is_empty() { Task::none() } else { open_files(init.open_files) };
        let window = iced::window::oldest().map(Message::WindowReady);
        let portrait = shell.load_portrait();
        let background = shell.load_background();
        let fonts = Task::perform(crate::background::blocking(fonts::installed_families), |fonts| {
            Message::FontsLoaded(fonts.unwrap_or_default())
        });
        let sources = shell.sources.restore(shell.omni.clone(), shell.store.clone(), shell.data_dir.clone());
        let motion = if shell.graphics.motion.follow_system { graphics::poll() } else { Task::none() };
        let restore = playback_session::load(shell.store.clone());
        (
            shell,
            Task::batch([
                load.map(Message::Library),
                open,
                window,
                portrait,
                background,
                playlists,
                fonts,
                sources,
                motion,
                theme_load,
                iced::system::theme().map(Message::SystemTheme),
                scene_sound_task,
                restore,
            ]),
        )
    }

    fn open_workbench(&mut self) -> Task<Message> {
        let mut window = workbench::Workbench::open();
        window.values = self.tuning_of(self.style());
        let task = window.load_art().map(Message::Workbench);
        self.workbench = Some(window);
        task
    }

    fn workbench_look(&self) -> workbench::Look<'_> {
        let style = self.style();
        let daylight = self.theme_is_light();
        let smart_beat = self.smart_beat.view(self.playback.heard_position());
        let live = self.playback.current().map(|track| workbench::Live {
            smart_beat: smart_beat.sample,
            lyrics: self.playback.lyrics.as_ref().unwrap_or(&self.empty_lyrics),
            time: self.playback.lyric_time(),
            track: Some(track),
            seed: &self.visualizer_seed,
            audio: &self.audio,
            portrait: self.art.peek(&track.id).and_then(Option::as_ref),
            colors: self.cover_palettes.get(track.id).unwrap_or(&[]),
            source: self.background_cover.as_ref().filter(|(id, _)| *id == track.id).map(|(_, image)| image),
        });
        workbench::Look {
            smart_beat,
            portrait: self.portrait_handle(self.playback.current().map(|track| track.id)),
            i18n: &self.i18n,
            theme: &self.theme,
            daylight,
            window: self.window_size,
            style,
            font_scale: self.appearance.lyrics_font_scale,
            live,
            audio: &self.audio,
            appearance: &self.appearance,
            graphics: &self.graphics,
            fonts: &self.system_fonts,
            dpi: self.window_scale,
            latent: folia_visualizer::LatentTuning::read(self.visualizer.tables.get("latent_background_tuning")),
            sora: folia_visualizer::SoraTuning::read(self.visualizer.tables.get("sora_background_tuning")),
            monet: folia_visualizer::monet_background::Tuning::read(
                self.visualizer.tables.get("monet_background_tuning"),
            ),
            picture: self.background_poster.image(),
            nomand: folia_visualizer::nomand::Tuning::read(self.visualizer.tables.get("nomand_background_tuning")),
            cover: self.playback.current().and_then(|track| self.cover_palettes.get(track.id)).unwrap_or(&[]),
            has_background_upload: self.background_upload.is_some(),
        }
    }

    fn refresh_theme(&mut self) {
        self.theme = theme_for(
            &self.theme_settings,
            self.appearance.theme,
            self.appearance.animation_intensity,
            self.song_themes.applied.as_ref(),
        );
        self.iced_theme = iced_theme(&self.theme);
    }

    fn scene_settings(&mut self, message: scene_settings::Message) -> Task<Message> {
        use scene_settings::Message as Change;
        if let Some(window) = &mut self.scene_window {
            window.update(&message);
        }
        match message {
            Change::Close => {
                self.scene_window = None;
                return Task::none();
            }
            Change::Tab(_) => return Task::none(),
            Change::Select(scene) => {
                if self.scene.scene_id != scene || self.scene_failed {
                    self.scene_generation = self.scene_generation.wrapping_add(1);
                    self.scene_failed = false;
                    self.scene.scene_id = scene;
                }
            }
            Change::Quality(quality) => self.scene.scene_options.cinema.quality = quality,
            Change::Seat(seat) => self.scene.scene_options.cinema.viewpoint = seat,
            Change::HouseLights(on) => self.scene.scene_options.cinema.house_lights = on,
            Change::Curtains(on) => self.scene.scene_options.cinema.curtains_on_pause = on,
            Change::Sound(on) => self.scene.scene_room_sound = on,
            Change::Output(output) => self.scene.scene_room_output = output,
            Change::Reset => {
                self.scene.scene_options.cinema = Default::default();
                self.scene_generation = self.scene_generation.wrapping_add(1);
                self.scene_failed = false;
            }
        }
        save(&self.store, &self.scene)
    }

    fn portrait_handle(&self, id: Option<i64>) -> Option<&iced::widget::image::Handle> {
        let values = Style::Monet.load(&self.visualizer.tables);
        if values.choice(Style::Monet.manifest(), "portraitSource") == "none" {
            return None;
        }
        if values.choice(Style::Monet.manifest(), "portraitSource") == "custom"
            && let Some((_, handle)) = &self.monet_portrait
        {
            return Some(handle);
        }
        id.and_then(|id| self.art.peek(&id)).and_then(Option::as_ref)
    }

    fn store_monet_offset(&mut self, offset: f32) -> Task<Message> {
        let manifest = Style::Monet.manifest();
        let mut values = Style::Monet.load(&self.visualizer.tables);
        values.set_number(manifest, "portraitOffsetX", offset);
        if self.style() == Style::Monet
            && let Some(window) = self.workbench.as_mut()
        {
            window.values = values.clone();
        }
        self.store_tuning(Style::Monet, values)
    }

    fn load_portrait(&self) -> Task<Message> {
        let values = Style::Monet.load(&self.visualizer.tables);
        let Some(path) = values.path("portraitPath").map(str::to_owned) else { return Task::none() };
        decode_portrait(path)
    }

    fn pick_portrait(&self) -> Task<Message> {
        Task::perform(
            crate::background::blocking(|| {
                rfd::FileDialog::new().add_filter("Image", &["png", "jpg", "jpeg", "webp"]).pick_file()
            }),
            |picked| Message::PortraitPicked(picked.flatten()),
        )
    }

    fn store_portrait(&mut self, source: std::path::PathBuf) -> Task<Message> {
        let Some(ext) = source.extension().and_then(|ext| ext.to_str()).map(|ext| ext.to_ascii_lowercase()) else {
            return Task::none();
        };
        if !matches!(ext.as_str(), "png" | "jpg" | "jpeg" | "webp") {
            return Task::none();
        }
        let stamp =
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |time| time.as_millis());
        let dest = self.data_dir.join("portraits").join(format!("monet-{stamp}.{ext}"));
        Task::perform(crate::background::blocking(move || copy_portrait(source, dest)), |copied| {
            match copied.flatten() {
                Some((path, cover)) => Message::PortraitStored(path, cover),
                None => Message::PortraitPicked(None),
            }
        })
    }

    fn remember_portrait(&mut self, path: std::path::PathBuf, cover: folia_library::Cover) -> Task<Message> {
        let text = path.display().to_string();
        let handle = iced::widget::image::Handle::from_rgba(cover.width, cover.height, cover.rgba);
        self.monet_portrait = Some((text.clone(), handle));
        let manifest = Style::Monet.manifest();
        let mut values = Style::Monet.load(&self.visualizer.tables);
        values.set_path(manifest, "portraitPath", Some(&text));
        values.set_choice(manifest, "portraitSource", "custom");
        if self.style() == Style::Monet
            && let Some(window) = self.workbench.as_mut()
        {
            window.values = values.clone();
            window.note("notifications.monetPortraitUpdated");
        }
        tracing::info!(path = %text, "monet portrait saved");
        self.store_tuning(Style::Monet, values)
    }

    fn clear_portrait(&mut self) -> Task<Message> {
        let manifest = Style::Monet.manifest();
        let mut values = Style::Monet.load(&self.visualizer.tables);
        if let Some(path) = values.path("portraitPath").map(str::to_owned) {
            let _ = std::thread::spawn(move || {
                let _ = std::fs::remove_file(path);
            });
        }
        values.set_path(manifest, "portraitPath", None);
        if values.choice(manifest, "portraitSource") == "custom" {
            values.set_choice(manifest, "portraitSource", "cover");
        }
        self.monet_portrait = None;
        if self.style() == Style::Monet
            && let Some(window) = self.workbench.as_mut()
        {
            window.values = values.clone();
            window.note("notifications.monetPortraitCleared");
        }
        tracing::info!("monet portrait cleared");
        self.store_tuning(Style::Monet, values)
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

    pub fn window_style(&self, _theme: &iced::Theme) -> iced::theme::Style {
        iced::theme::Style {
            background_color: if self.player_transparent() {
                Color::TRANSPARENT
            } else {
                folia_visualizer::color(self.theme.background, 1.0)
            },
            text_color: folia_visualizer::color(self.theme.primary, 1.0),
        }
    }

    fn player_transparent(&self) -> bool {
        self.page == Page::NowPlaying && (self.theme_settings.transparent_player_background || self.native_blur_applied)
    }

    pub(crate) fn scene_active(&self) -> bool {
        self.page == Page::NowPlaying && self.scene.scene_id.is_some() && !self.scene_failed && !self.closing
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        if self.closing {
            return match message {
                Message::Session(message) => self.playback_session.update(message, &mut self.playback, &self.store),
                _ => Task::none(),
            };
        }
        let seek_finished = matches!(message, Message::Playback(playback::Message::SeekDone));
        motion::configure(&self.graphics.motion, self.system_reduced_motion);
        if self.graphics.background_throttling && folia_render::backend::window_hidden() {
            self.visualizer_hold.get_or_insert_with(|| self.playback.lyric_time());
        } else {
            self.visualizer_hold = None;
        }
        self.refresh_panel_data();
        let before = self.page;
        let wall_seed = self.library.order_seed();
        let settles_page = matches!(message, Message::PageShot(_) | Message::PageShotDone);
        let seam_before = self.seam_before(&message);
        let opens_search = matches!(&message, Message::Home(home::Message::Search));
        let focus_title = matches!(message, Message::Home(home::Message::TitleToggle { keyboard: true, .. }));
        // The original analyser is read once per animation frame; its 0.6 smoothing assumes that rate.
        let frame_tick = matches!(
            message,
            Message::VisualFrame
                | Message::Playback(playback::Message::Tick)
                | Message::Workbench(workbench::Message::Frame)
        );
        let lyrics_settled = matches!(&message, Message::Playback(playback::Message::LyricsLoaded(request, ..)) if *request == self.playback.request_id());
        let task = self.handle(message);
        #[cfg(target_os = "linux")]
        self.media_session.sync(
            &self.playback,
            self.playback.current().and_then(|track| self.art.peek(&track.id)).and_then(Option::as_ref),
        );
        if self.closing {
            return task;
        }
        let entered_wall =
            if before != Page::Library && self.page == Page::Library && self.library.order_seed() == wall_seed {
                self.library.enter_wall(&self.i18n).map(Message::Library)
            } else {
                Task::none()
            };
        let task = Task::batch([task, entered_wall]);
        let wall = self.library.wall_appearance(self.appearance.wall, &self.i18n).map(Message::Library);
        let lattice_task = if self.page == Page::Lattice {
            let handoff = (before == Page::Library).then(|| self.library.take_handoff());
            let generation = self.lattice_generation;
            let lattice = self.lattice.get_or_insert_with(Default::default);
            if let Some(handoff) = handoff {
                lattice.enter(handoff);
            }
            lattice.set_randomize_on_entry(self.general.wall_randomize_on_entry);
            lattice
                .sync(&self.playback, self.appearance.wall, &self.i18n, self.omni.as_ref())
                .map(move |message| Message::Lattice(generation, message))
        } else {
            if before == Page::Lattice
                && self.page == Page::Library
                && let Some(lattice) = &mut self.lattice
            {
                self.library.accept_handoff(lattice.leave());
            }
            if self.lattice.take().is_some() {
                self.lattice_generation = self.lattice_generation.wrapping_add(1);
            }
            Task::none()
        };
        let covers = if self.page == Page::Lattice {
            self.library.suspend_wall(true)
        } else if matches!(self.page, Page::Library | Page::Search) {
            self.library.suspend_wall(false)
        } else {
            Task::none()
        };
        let task = Task::batch([task, wall, lattice_task, covers.map(Message::Library)]);
        let colors = self.playback.current().and_then(|track| self.cover_palettes.get(track.id)).unwrap_or(&[]);
        let cover_ready = self.playback.current().is_some_and(|track| self.art.peek(&track.id).is_some());
        let theme_task = self.song_themes.observe(&self.playback, colors, cover_ready, lyrics_settled);
        let mut remember_theme = Task::none();
        if self.song_themes.theme_changed {
            self.song_themes.theme_changed = false;
            if self.song_themes.applied.is_some() {
                self.theme_settings.source = folia_store::ThemeSource::Ai;
            }
            self.refresh_theme();
            remember_theme = Task::batch([self.song_themes.remember(), save(&self.store, &self.theme_settings)]);
        }
        let task = Task::batch([task, theme_task, remember_theme]);
        if self.page != before {
            self.scene_generation = self.scene_generation.wrapping_add(1);
            self.scene_failed = false;
        }
        let scene_active = self.scene_active();
        let room = self.scene_sound.sync(&self.scene, scene_active, &mut self.playback);
        let task = Task::batch([task, room]);
        let task = Task::batch([task, self.prepare_background()]);
        let task = Task::batch([task, self.prepare_preview_background()]);
        let task = Task::batch([task, self.playback.refresh_prefetch().map(Message::Playback)]);
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
        self.bar.set_home(matches!(self.page, Page::Library | Page::Lattice));
        self.home.sync(self.window_size.width, self.window_size.height, self.playback.current().is_some(), now);
        self.home.filter_queue(self.playback.queue.items(), self.playback.queue.revision());
        let list_h = self.home.list_height(
            self.window_size,
            now,
            self.playback.queue.items().len(),
            self.page == Page::Lattice,
            self.playback.is_fm() || self.appearance.wall.radio,
        );
        self.home.sync_queue_height(
            self.playback.queue.items(),
            self.playback.queue.current_index(),
            self.playback.is_fm(),
            now,
            list_h,
        );
        if let Some(horizontal) = self.home.take_flip(now) {
            self.library.set_horizontal(horizontal);
            self.library.replan(self.home.goal());
        }
        self.library.set_horizontal(self.home.is_horizontal());
        self.library.set_seam(self.home.width(now));
        self.library.plan_if_needed();
        let wall_pointer = self.clock.pointer.map(|(x, y)| iced::Point::new(x, y)).filter(|at| {
            matches!(self.page, Page::Library | Page::Lattice)
                && !self.modal_open()
                && !self.library.menu_open()
                && !self.bar.hover().inside
                && !self.home.covers_wall(
                    *at,
                    self.window_size,
                    self.playback.queue.items().len(),
                    (self.page == Page::Lattice, self.playback.is_fm() || self.appearance.wall.radio),
                    self.playback.current().is_some(),
                    now,
                )
        });
        self.library.wall_pointer(wall_pointer.filter(|_| self.page == Page::Library), now);
        if let Some(lattice) = &mut self.lattice {
            lattice.pointer(wall_pointer, now);
        }
        let seam = self.library.seam();
        self.home.sync_tab(self.home.is_hidden() || seam.collapsed, now);
        self.login.sync(&self.library.online);
        let liked = self.song_liked();
        self.panel.sync_like(liked, now);
        self.library.set_tab_chrome(&mut self.home, now);
        self.library.sync_hint(&self.i18n, now);
        let radio_capsule =
            self.lattice.as_ref().filter(|_| self.page == Page::Lattice).and_then(lattice::Lattice::radio_capsule);
        self.bar.set_forced(self.positioning.is_some() || radio_capsule == Some(true));
        self.bar.sync(&self.playback, self.window_size.width, now);
        self.timeline.sync(
            timeline::Sync {
                song: self.playback.current().map(|track| track.id),
                lyrics: self.playback.lyrics.as_ref(),
                duration: self.playback.duration(),
                time: self.playback.lyric_time(),
                window: self.window_size,
                disabled: self.positioning.is_some(),
            },
            now,
        );
        self.bar.set_modal(self.modal_open());
        self.panel.set_modal(self.modal_open());
        if !self.modal_open() {
            self.bar.set_shown(self.chrome_visible() && radio_capsule != Some(false), now);
            let presence = if self.chrome_visible() { 1.0 } else { 0.0 };
            self.subtitle_presence = self.subtitle_presence.retarget(self.now_ms(), presence);
        }
        let (quality_levels, quality) = self.audio_quality();
        let room = panel::Room {
            background: folia_visualizer::Background::resolve(self.appearance.stored_background_mode()),
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
        let reads_audio = self.page == Page::NowPlaying
            || radio_capsule.is_some()
            || self.wall_backdrop_visible()
            || self.workbench.is_some()
            || self.theme_park.is_some();
        if reads_audio && self.visualizer_hold.is_none() && (frame_tick || !self.playback.is_playing()) {
            self.audio = self.playback.audio_frame(wall_ms());
        }
        if self.page == Page::Lattice
            && frame_tick
            && let Some(lattice) = &mut self.lattice
        {
            lattice.sample_audio(&self.audio, now);
        }
        self.sync_native_blur();
        let analysis = self.sync_smart_beat(frame_tick);
        let session = self.playback_session.observe(&self.playback, &self.store, seek_finished);
        let task = if analysis.units() == 0 { task } else { Task::batch([task, analysis]) };
        if session.units() == 0 { task } else { Task::batch([task, session]) }
    }

    fn load_background(&self) -> Task<Message> {
        let Some(path) = self
            .visualizer
            .tables
            .get("custom_background_image")
            .and_then(|value| value.get("path"))
            .and_then(toml::Value::as_str)
            .map(std::path::PathBuf::from)
        else {
            return Task::none();
        };
        let request = path.clone();
        let generation = self.background_upload_generation;
        Task::perform(crate::background::blocking(move || background_image::load(&request)), move |result| {
            Message::BackgroundLoaded(
                generation,
                path.clone(),
                result.unwrap_or_else(|| Err("image worker stopped".to_owned())),
            )
        })
    }

    fn import_background(&mut self, source: std::path::PathBuf) -> Task<Message> {
        self.background_upload_generation = self.background_upload_generation.wrapping_add(1);
        let generation = self.background_upload_generation;
        let stamp =
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |time| time.as_nanos());
        let path = self.data_dir.join("backgrounds").join(format!("image-{stamp}.png"));
        let destination = path.clone();
        Task::perform(
            crate::background::blocking(move || background_image::import(&source, &destination)),
            move |result| {
                Message::BackgroundLoaded(
                    generation,
                    path.clone(),
                    result.unwrap_or_else(|| Err("image worker stopped".to_owned())),
                )
            },
        )
    }

    fn prepare_background(&mut self) -> Task<Message> {
        let mode = folia_visualizer::Background::resolve(self.appearance.stored_background_mode());
        let common = mode == folia_visualizer::Background::Common && self.appearance.use_cover_color_bg;
        if !common && !matches!(mode, folia_visualizer::Background::Monet | folia_visualizer::Background::Nomand) {
            self.background_poster.clear();
            if self.scene_active() {
                return self.ensure_art();
            }
            self.background_cover = None;
            return Task::none();
        }
        let art = self.ensure_art();
        let cover = self
            .background_cover
            .as_ref()
            .filter(|(id, _)| self.playback.current().is_some_and(|track| track.id == *id))
            .map(|(_, image)| image);
        let tuning =
            folia_visualizer::monet_background::Tuning::read(self.visualizer.tables.get("monet_background_tuning"));
        let raw = mode == folia_visualizer::Background::Nomand;
        let uploaded = if raw {
            folia_visualizer::nomand::Tuning::read(self.visualizer.tables.get("nomand_background_tuning")).uploaded
        } else {
            tuning.uploaded
        };
        let source = if common {
            cover
        } else if uploaded {
            self.background_upload.as_ref().or(cover)
        } else {
            cover.or(self.background_upload.as_ref())
        };
        let Some(job) =
            self.background_poster.request(source, &self.theme, tuning, raw, common.then_some(self.window_size))
        else {
            return art;
        };
        let generation = job.generation;
        let painting = Task::perform(crate::background::blocking(move || job.paint()), move |image| {
            Message::BackgroundPainted(generation, image)
        });
        Task::batch([art, painting])
    }

    fn prepare_preview_background(&mut self) -> Task<Message> {
        let cover = self
            .background_cover
            .as_ref()
            .filter(|(id, _)| self.playback.current().is_some_and(|track| track.id == *id))
            .map(|(_, image)| image);
        let tuning =
            folia_visualizer::monet_background::Tuning::read(self.visualizer.tables.get("monet_background_tuning"));
        let nomand = folia_visualizer::nomand::Tuning::read(self.visualizer.tables.get("nomand_background_tuning"));
        let (preview, theme) = if let Some(park) = &mut self.theme_park {
            (&mut park.preview, &park.preview_theme)
        } else if let Some(window) = &mut self.workbench {
            (window, &self.theme)
        } else {
            return Task::none();
        };
        preview
            .prepare_background(
                theme,
                &self.appearance,
                tuning,
                nomand,
                (cover, self.background_upload.as_ref()),
                self.window_size,
            )
            .map(Message::Workbench)
    }

    fn page_snapshot(&self) -> NavigationEntry {
        match self.page {
            Page::Search => NavigationEntry::Search(Box::new(self.search.snapshot())),
            Page::Catalog => self.catalog.as_ref().map_or(NavigationEntry::Page(Page::Library), |page| {
                NavigationEntry::Catalog(Box::new(page.snapshot()))
            }),
            page => NavigationEntry::Page(page),
        }
    }

    fn remember_page(&mut self) {
        let entry = match self.page {
            Page::Catalog => match self.catalog.take() {
                Some(mut page) => {
                    page.suspend();
                    NavigationEntry::Catalog(Box::new(page))
                }
                None => return,
            },
            Page::Search => {
                let mut page = std::mem::take(&mut self.search);
                page.suspend();
                NavigationEntry::Search(Box::new(page))
            }
            page => NavigationEntry::Page(page),
        };
        self.history.push(entry);
        if self.history.len() > 32 {
            self.history.remove(0);
        }
    }

    fn restore_page(&mut self, entry: NavigationEntry) -> Task<Message> {
        match entry {
            NavigationEntry::Catalog(page) => {
                self.catalog = Some(*page);
                self.page = Page::Catalog;
                self.catalog
                    .as_mut()
                    .map_or_else(Task::none, |page| page.activate(self.omni.as_ref()).map(Message::Catalog))
            }
            NavigationEntry::Search(page) => {
                self.search = *page;
                self.page = Page::Search;
                self.search.restore(self.omni.as_ref()).map(Message::Search)
            }
            NavigationEntry::Page(page) => {
                self.page = page;
                Task::none()
            }
        }
    }

    fn navigate_back(&mut self) -> Task<Message> {
        if self.page == Page::Catalog && self.catalog.as_mut().is_some_and(|page| page.close_biography()) {
            return Task::none();
        }
        self.play_navigation.cancel();
        if let Some(page) = &mut self.catalog {
            page.suspend();
        }
        self.catalog = None;
        self.search.suspend();
        let entry = self.history.pop().unwrap_or(NavigationEntry::Page(Page::Library));
        self.restore_page(entry)
    }

    fn return_home(&mut self) {
        self.play_navigation.cancel();
        self.catalog = None;
        self.search.suspend();
        self.history.clear();
        self.page = Page::Library;
    }

    fn locate_playback_source(&mut self) -> Task<Message> {
        let entry = match &self.playback_source {
            Some(NavigationEntry::Catalog(page)) => NavigationEntry::Catalog(Box::new(page.snapshot())),
            Some(NavigationEntry::Search(page)) => NavigationEntry::Search(Box::new(page.snapshot())),
            _ => {
                let entered = if self.page != Page::Library {
                    self.library.enter_wall(&self.i18n).map(Message::Library)
                } else {
                    Task::none()
                };
                self.page = Page::Library;
                let located = match self.playback.current().map(|track| track.id) {
                    Some(track) => self.library_message(library::Message::Locate(track)),
                    None => Task::none(),
                };
                return Task::batch([entered, located]);
            }
        };
        self.play_navigation.cancel();
        self.remember_page();
        self.restore_page(entry)
    }

    fn catalog_message(&mut self, message: catalog::Message) -> Task<Message> {
        if matches!(message, catalog::Message::Back) {
            return self.navigate_back();
        }
        if matches!(
            message,
            catalog::Message::TrackArtists(_)
                | catalog::Message::TrackAlbum(_)
                | catalog::Message::OpenArtist(_)
                | catalog::Message::OpenAlbum(_)
        ) {
            let Some((page, task)) = catalog::CatalogPage::open(message, self.omni.as_ref()) else {
                return Task::none();
            };
            self.play_navigation.cancel();
            self.remember_page();
            self.catalog = Some(page);
            self.page = Page::Catalog;
            return task.map(Message::Catalog);
        }
        if let catalog::Message::AllReady(_, _, enqueue, ref result) = message {
            let active =
                self.page == Page::Catalog && self.catalog.as_ref().is_some_and(|page| page.accepts_all(&message));
            if !active {
                return Task::none();
            }
            let succeeded = result.is_ok();
            let task = self
                .catalog
                .as_mut()
                .map_or_else(Task::none, |page| page.update(message.clone(), self.omni.as_ref()).map(Message::Catalog));
            if succeeded {
                return Task::batch([
                    task,
                    self.catalog_message(if enqueue {
                        catalog::Message::EnqueueAll
                    } else {
                        catalog::Message::PlayAll
                    }),
                ]);
            }
            return task;
        }
        if matches!(message, catalog::Message::PlayAll | catalog::Message::EnqueueAll) {
            let enqueue = matches!(message, catalog::Message::EnqueueAll);
            if let Some(task) = self.catalog.as_mut().and_then(|page| page.prepare_all(enqueue, self.omni.as_ref())) {
                return task.map(Message::Catalog);
            }
        }
        if matches!(message, catalog::Message::Data(..) | catalog::Message::Cover(..) | catalog::Message::Backdrop(..))
        {
            if let Some(page) = self.catalog.as_mut().filter(|page| page.accepts(&message)) {
                return page.update(message, self.omni.as_ref()).map(Message::Catalog);
            }
            for entry in &mut self.history {
                if let NavigationEntry::Catalog(page) = entry
                    && page.accepts(&message)
                {
                    return page.update(message, self.omni.as_ref()).map(Message::Catalog);
                }
            }
            return Task::none();
        }
        let Some(page) = self.catalog.as_ref() else { return Task::none() };
        match message {
            catalog::Message::Play(_) | catalog::Message::PlayAll => {
                let index = if let catalog::Message::Play(index) = message { index } else { 0 };
                if page.tracks.is_empty() || index >= page.tracks.len() {
                    return Task::none();
                }
                let origin = folia_core::Origin { name: page.title().to_owned() };
                let start = self.playback.play(page.tracks.clone(), index, Some(origin)).map(Message::Playback);
                self.await_player();
                Task::batch([start, self.ensure_art()])
            }
            catalog::Message::Enqueue(index) => {
                if let Some(track) = page.tracks.get(index).cloned() {
                    self.playback.enqueue(vec![track]);
                }
                Task::none()
            }
            catalog::Message::EnqueueAll => {
                self.playback.enqueue(page.tracks.clone());
                Task::none()
            }
            message => self
                .catalog
                .as_mut()
                .map_or_else(Task::none, |page| page.update(message, self.omni.as_ref()).map(Message::Catalog)),
        }
    }

    fn handle(&mut self, message: Message) -> Task<Message> {
        match message {
            #[cfg(target_os = "linux")]
            Message::Media(command) => self.media_command(command),
            Message::UiFrame => {
                let playback = if self.playback.is_playing() {
                    self.handle(Message::Playback(playback::Message::Tick))
                } else {
                    Task::none()
                };
                Task::batch([playback, self.advance_ui_frames()])
            }
            Message::VisualFrame => {
                let playback = self.handle(Message::Playback(playback::Message::Tick));
                let ui = self.advance_ui_frames();
                let lattice = if self.page == Page::Lattice {
                    self.handle(Message::Lattice(self.lattice_generation, lattice::Message::Frame))
                } else {
                    Task::none()
                };
                let preview = if self.workbench.as_ref().is_some_and(workbench::Workbench::ticking)
                    || self.theme_park.as_ref().is_some_and(|park| park.preview.ticking())
                {
                    self.handle(Message::Workbench(workbench::Message::Frame))
                } else {
                    Task::none()
                };
                let panel = if self.page == Page::NowPlaying && self.panel.visualizing() {
                    self.panel(panel::Message::Frame)
                } else {
                    Task::none()
                };
                if preview.units() == 0 && panel.units() == 0 && lattice.units() == 0 && ui.units() == 0 {
                    playback
                } else {
                    Task::batch([playback, preview, panel, lattice, ui])
                }
            }
            Message::SceneSound(message) => self.scene_sound.update(message, &mut self.playback),
            Message::SceneFailed(failure) => {
                if failure.generation == self.scene_generation && self.scene_active() {
                    tracing::error!(error = &failure.error as &dyn std::error::Error, "scene renderer failed");
                    self.scene_failed = true;
                }
                Task::none()
            }
            Message::Back => self.navigate_back(),
            Message::Catalog(message) => self.catalog_message(message),
            Message::Navigate(page) => {
                self.play_navigation.cancel();
                if page == Page::Library {
                    self.return_home();
                    return Task::none();
                }
                if page == Page::NowPlaying && self.page != Page::NowPlaying {
                    self.remember_page();
                }
                self.page = page;
                if page == Page::NowPlaying {
                    self.reveal_chrome();
                }
                Task::none()
            }
            Message::OpenWorkbench => self.open_workbench(),
            Message::OpenThemePark => {
                let lyrics = self
                    .playback
                    .lyrics
                    .as_ref()
                    .map(|lyrics| lyrics.lines.iter().map(|line| line.text.as_str()).collect::<Vec<_>>().join("\n"))
                    .unwrap_or_default();
                let title = self.playback.current().map_or("", |track| track.title.as_str());
                let mut park = theme_park::ThemePark::open(
                    self.song_themes.current.as_ref().or(self.song_themes.applied.as_ref()),
                    self.theme_settings.custom_dual_theme.as_ref(),
                    self.theme_settings.source != folia_store::ThemeSource::Ai,
                    self.theme_is_light(),
                    self.playback.current().map(folia_ai::song_key),
                    folia_ai::theme_prompt(title, &lyrics),
                );
                park.preview.values = self.tuning_of(self.style());
                park.preview_theme.intensity = self.appearance.animation_intensity;
                let task = park.preview.load_art().map(Message::Workbench);
                self.theme_park = Some(park);
                task
            }
            Message::ThemePark(message) => {
                let Some(park) = self.theme_park.as_mut() else {
                    return Task::none();
                };
                match park.update(message) {
                    theme_park::Event::None => Task::none(),
                    theme_park::Event::Close => {
                        self.theme_park = None;
                        Task::none()
                    }
                    theme_park::Event::Copy(text) => iced::clipboard::write(text),
                    theme_park::Event::Save { custom, theme, song } => {
                        let theme = *theme;
                        self.theme_park = None;
                        if custom {
                            self.sheet_event(panel_sheets::Event::SaveTheme(theme))
                        } else if let Some(song) = song {
                            self.song_themes.save_manual(song, theme)
                        } else {
                            Task::none()
                        }
                    }
                }
            }
            Message::OpenScene => {
                self.scene_window = Some(scene_settings::State::open());
                Task::none()
            }
            Message::SceneSettings(message) => self.scene_settings(message),
            Message::OpenSettings => {
                self.settings = Some(settings::Settings::open());
                Task::none()
            }
            Message::Workbench(message) => {
                let style = self.style();
                if matches!(message, workbench::Message::Pause | workbench::Message::Source(workbench::Source::Live)) {
                    let live = self.playback.current().map(|track| workbench::Live {
                        smart_beat: self.smart_beat.frame(self.playback.heard_position()),
                        lyrics: self.playback.lyrics.as_ref().unwrap_or(&self.empty_lyrics),
                        time: self.playback.lyric_time(),
                        track: Some(track),
                        seed: &self.visualizer_seed,
                        audio: &self.audio,
                        portrait: self.art.peek(&track.id).and_then(Option::as_ref),
                        colors: self.cover_palettes.get(track.id).unwrap_or(&[]),
                        source: self
                            .background_cover
                            .as_ref()
                            .filter(|(id, _)| *id == track.id)
                            .map(|(_, image)| image),
                    });
                    let choosing_live = matches!(message, workbench::Message::Source(workbench::Source::Live));
                    if let Some(park) = &mut self.theme_park {
                        park.preview.capture_live(live, choosing_live);
                    } else if let Some(window) = &mut self.workbench {
                        window.capture_live(live, choosing_live);
                    }
                }
                if let Some(park) = self.theme_park.as_mut() {
                    let latent =
                        folia_visualizer::LatentTuning::read(self.visualizer.tables.get("latent_background_tuning"));
                    let _ = park.preview.update(message, style.manifest(), &self.appearance, latent);
                    Task::none()
                } else {
                    self.workbench(message)
                }
            }
            Message::Sources(message) => {
                let before = self.sources.custom_only();
                let task = self.sources.update(message, self.omni.clone());
                if before == self.sources.custom_only() {
                    return task;
                }
                if self.sources.custom_only() {
                    self.login.close();
                }
                self.play_navigation.cancel();
                Task::batch([task, self.playback.source_mode_changed().map(Message::Playback)])
            }
            Message::SongTheme(message) => {
                let prefer_custom = matches!(message, song_theme::Message::PreferCustom(true));
                let loaded = matches!(message, song_theme::Message::Loaded(..));
                let failure = self.song_themes.active_error(&message);
                let colors = self.playback.current().and_then(|track| self.cover_palettes.get(track.id)).unwrap_or(&[]);
                let task = self.song_themes.update(message, &self.store, &self.playback, colors);
                if loaded {
                    self.refresh_theme();
                }
                if prefer_custom && self.theme_settings.custom_dual_theme.is_some() {
                    self.theme_settings.source = folia_store::ThemeSource::Custom;
                    self.refresh_theme();
                }
                if failure {
                    let notice = self.status(false, self.i18n.t("status.themeGenerationFailed").to_owned());
                    return Task::batch([task, notice]);
                }
                task
            }
            Message::Settings(message) => {
                if self.sheets.any_open() && matches!(message, settings::Message::Escape) {
                    self.sheets.close_all();
                    return Task::none();
                }
                if self.sources.has_alert() && matches!(message, settings::Message::Escape) {
                    return self.sources.update(sources::Message::DismissUpdates, self.omni.clone());
                }
                let Some(window) = self.settings.as_mut() else { return Task::none() };
                let task = window.update(message).map(Message::Settings);
                if window.finished() {
                    self.settings = None;
                }
                task
            }
            Message::Developer(message) => self.developer(message),
            Message::Storage(message) => self.storage_message(message),
            Message::LanguagePicked(preference) => self.set_language(preference),
            Message::Theme(mode) => {
                let before = self.appearance.clone();
                self.appearance.follow_system_theme = false;
                self.appearance.theme = mode;
                self.theme = theme_for(
                    &self.theme_settings,
                    mode,
                    self.appearance.animation_intensity,
                    self.song_themes.applied.as_ref(),
                );
                self.iced_theme = iced_theme(&self.theme);
                self.appearance_changed(&before)
            }
            Message::FollowSystemTheme => {
                let before = self.appearance.clone();
                self.appearance.follow_system_theme = true;
                Task::batch([self.appearance_changed(&before), iced::system::theme().map(Message::SystemTheme)])
            }
            Message::SystemTheme(mode) => {
                if !self.appearance.follow_system_theme {
                    return Task::none();
                }
                let mode = match mode {
                    iced::theme::Mode::Light => ThemeMode::Light,
                    iced::theme::Mode::Dark => ThemeMode::Dark,
                    iced::theme::Mode::None => return Task::none(),
                };
                if self.appearance.theme == mode {
                    return Task::none();
                }
                let before = self.appearance.clone();
                self.appearance.theme = mode;
                self.theme = theme_for(
                    &self.theme_settings,
                    mode,
                    self.appearance.animation_intensity,
                    self.song_themes.applied.as_ref(),
                );
                self.iced_theme = iced_theme(&self.theme);
                self.appearance_changed(&before)
            }
            Message::FrameRate(rate) => {
                let before = self.appearance.clone();
                self.appearance.frame_rate = rate;
                self.appearance_changed(&before)
            }
            Message::Graphics(message) => self.graphics_message(message),
            Message::SmartBeat(message) => {
                self.smart_beat.update(message);
                Task::none()
            }
            Message::Visualizer(style) => {
                self.appearance.visualizer = style.map_or(RANDOM_STYLE, Style::id).to_owned();
                save(&self.store, &self.appearance)
            }
            Message::Library(library::Message::Key(key)) if self.page == Page::Lattice => {
                self.handle(Message::Lattice(self.lattice_generation, lattice::Message::Key(key)))
            }
            Message::Library(message) => {
                let like_notice = match &message {
                    library::Message::Online(online::Message::LikeSettled(_, _, liked, result)) => {
                        let key = if result.is_ok() {
                            if *liked { "status.liked" } else { "status.unliked" }
                        } else {
                            "status.likeFailed"
                        };
                        Some((result.is_ok(), self.i18n.t(key).to_owned()))
                    }
                    _ => None,
                };
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
                    let list_h = self.home.list_height(
                        self.window_size,
                        std::time::Instant::now(),
                        queue.len(),
                        self.page == Page::Lattice,
                        self.playback.is_fm() || self.appearance.wall.radio,
                    );
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
                let notice = match like_notice {
                    Some((ok, words)) => self.status(ok, words),
                    None => Task::none(),
                };
                Task::batch([queue_task, task, self.library_effect(effect), notice])
            }
            Message::Search(message) => {
                if matches!(
                    message,
                    search::Message::Results(..) | search::Message::More(..) | search::Message::Cover(..)
                ) && !self.search.accepts_message(&message)
                {
                    let provider = self.library.online.provider();
                    for entry in &mut self.history {
                        if let NavigationEntry::Search(page) = entry
                            && page.accepts_message(&message)
                        {
                            return page.update(message, self.omni.as_ref(), provider).0.map(Message::Search);
                        }
                    }
                    return Task::none();
                }
                if matches!(message, search::Message::Close) {
                    self.play_navigation.cancel();
                    return self.navigate_back();
                }
                let now = std::time::Instant::now();
                if matches!(message, search::Message::Frame) && self.page == Page::Search && self.search.gone(now) {
                    self.page = Page::Library;
                }
                let provider = self.library.online.provider();
                let (task, outcome) = self.search.update(message, self.omni.as_ref(), provider);
                let task = task.map(Message::Search);
                match outcome {
                    search::Outcome::Play(index) => {
                        let name = self.search.query();
                        let name = if name.is_empty() { self.i18n.t("search.sourceNetease") } else { name };
                        let origin = folia_core::Origin { name: name.to_owned() };
                        let start =
                            self.playback.play(self.search.results.clone(), index, Some(origin)).map(Message::Playback);
                        self.await_player();
                        Task::batch([task, start, self.ensure_art()])
                    }
                    search::Outcome::Enqueue(index) => {
                        if let Some(track) = self.search.results.get(index).cloned() {
                            let added = self.playback.enqueue(vec![track]);
                            tracing::debug!(added, "search result added to the queue");
                        }
                        task
                    }
                    search::Outcome::Catalog(message) => Task::batch([task, self.catalog_message(message)]),
                    search::Outcome::None => task,
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
                    self.pending_playback_source.as_ref().is_some_and(|(pending, _)| *pending == request)
                }) {
                    self.playback_source = self.pending_playback_source.take().map(|(_, source)| source);
                }
                if started.is_some_and(|request| {
                    request == self.playback.request_id() && self.play_navigation.ready(request, self.page)
                }) {
                    if self.page != Page::NowPlaying {
                        self.remember_page();
                    }
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
                self.window.map_or_else(Task::none, |id| iced::window::scale_factor(id).map(Message::WindowScale))
            }
            Message::WindowScale(scale) => {
                self.window_scale = scale.max(0.5);
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
                let size = chrome::resize_viewport(self.window_size, size, self.fullscreen);
                if size == self.window_size {
                    return Task::none();
                }
                self.window_size = size;
                match self.window {
                    Some(id) => Task::batch([
                        iced::window::is_maximized(id).map(Message::Maximized),
                        iced::window::scale_factor(id).map(Message::WindowScale),
                    ]),
                    None => Task::none(),
                }
            }
            Message::Maximized(maximized) => {
                self.maximized = maximized;
                Task::none()
            }
            Message::ChromeTick => {
                self.clock = self.clock.tick(self.now_ms(), self.pointer_held);
                if !self.modal_open() {
                    self.panel.set_chrome(self.chrome_visible(), std::time::Instant::now());
                }
                Task::none()
            }
            Message::PointerHeld(held) => {
                self.pointer_held = held;
                if !held {
                    self.developer.release();
                    self.drag_armed = false;
                    if let Some(positioning) = self.positioning.as_mut() {
                        positioning.release();
                    }
                    self.drag_from = None;
                }
                if self.page == Page::NowPlaying {
                    self.reveal_chrome();
                }
                if held {
                    Task::none()
                } else {
                    let radio = self.lattice.as_mut().map_or_else(Task::none, |lattice| {
                        lattice
                            .release_radio(self.clock.pointer.map(|(x, y)| iced::Point::new(x, y)), self.omni.as_ref())
                    });
                    let generation = self.lattice_generation;
                    Task::batch([self.finish_slider(), radio.map(move |message| Message::Lattice(generation, message))])
                }
            }
            Message::WindowUnfocused => {
                self.pointer_held = false;
                self.drag_armed = false;
                self.drag_from = None;
                self.finish_slider()
            }
            Message::WindowClosing => self.request_close(),
            Message::Session(message) => {
                let task = self.playback_session.update(message, &mut self.playback, &self.store);
                Task::batch([task, self.ensure_art()])
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
                if self.page != Page::NowPlaying {
                    self.remember_page();
                }
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
                if self.page != Page::NowPlaying {
                    self.remember_page();
                    self.page = Page::NowPlaying;
                    self.reveal_chrome();
                }
                Task::none()
            }
            Message::Bar(player_bar::Message::Timeline) => {
                if self.playback.lyrics.is_some() {
                    let now = std::time::Instant::now();
                    self.timeline.open(
                        self.playback.current().map(|track| track.id),
                        self.playback.lyrics.as_ref(),
                        self.playback.duration(),
                        self.playback.lyric_time(),
                        self.window_size,
                        now,
                    );
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
            Message::Lattice(generation, message)
                if self.page == Page::Lattice && generation == self.lattice_generation =>
            {
                let Some(page) = self.lattice.as_mut() else {
                    return Task::none();
                };
                let (task, effect) = page.update(message, self.omni.as_ref());
                Task::batch([
                    task.map(move |message| Message::Lattice(generation, message)),
                    effect.map_or_else(Task::none, |message| self.handle(message)),
                ])
            }
            Message::Lattice(..) => Task::none(),
            Message::Panel(message) => {
                let task = self.panel(message);
                Task::batch([task, self.ensure_art()])
            }
            Message::Playlists(message) => {
                let prompt = matches!(&message, playlist_dialog::Message::Create);
                let task = match self.add_to_playlist.update(message) {
                    playlist_dialog::Event::None => Task::none(),
                    playlist_dialog::Event::Pick(index) => self.add_to_playlist(index),
                    playlist_dialog::Event::Create(name) => self.create_playlist(name),
                };
                if prompt {
                    Task::batch([
                        task,
                        iced::widget::operation::focus(iced::widget::Id::new(playlist_dialog::NAME_INPUT)),
                    ])
                } else {
                    task
                }
            }
            Message::Sheets(message) => {
                let event = self.sheets.update(message, &self.audio_settings.equalizer);
                self.sheet_event(event)
            }
            Message::LocalPlaylists(result) => match result {
                Ok(playlists) => {
                    self.playlists = playlists;
                    Task::none()
                }
                Err(err) => {
                    tracing::warn!(error = %err, "local playlists unavailable");
                    self.status(false, err)
                }
            },
            Message::Status(ok, text) => {
                if text.is_empty() {
                    Task::none()
                } else {
                    self.status(ok, text)
                }
            }
            Message::StatusLeaving(serial) if serial == self.status_serial => {
                Task::perform(blocking(|| std::thread::sleep(std::time::Duration::from_millis(220))), move |_| {
                    Message::StatusGone(serial)
                })
            }
            Message::StatusGone(serial) if serial == self.status_serial => {
                self.status = None;
                Task::none()
            }
            Message::StatusLeaving(_) | Message::StatusGone(_) | Message::StatusFrame => Task::none(),
            Message::ImportedLyrics(result) => match result {
                Ok(lyrics) => {
                    self.playback.set_lyrics(Some(lyrics));
                    let words = self.i18n.t("status.lyricsImported").to_owned();
                    self.status(true, words)
                }
                Err(err) => self.status(false, err),
            },
            Message::MatchedLyrics(request, id, result) => {
                if request != self.playback.request_id() || self.playback.current().is_none_or(|track| track.id != id) {
                    return Task::none();
                }
                match result {
                    Ok(lyrics) => {
                        let found = lyrics.is_some();
                        self.playback.set_lyrics(lyrics);
                        let key = if found { "status.lyricsMatched" } else { "lyricExport.noLyrics" };
                        let words = self.i18n.t(key).to_owned();
                        self.status(found, words)
                    }
                    Err(err) => self.status(false, err),
                }
            }
            Message::ExportedLyrics(result) => match result {
                Ok(path) => {
                    tracing::info!(path = %path.display(), "lyrics exported");
                    let words = self.i18n.t("lyricExport.exported").to_owned();
                    self.status(true, words)
                }
                Err(err) if err == "cancelled" => Task::none(),
                Err(err) => {
                    tracing::warn!(error = %err, "lyrics not exported");
                    self.status(false, err)
                }
            },
            Message::Art(id, generation, art) => {
                if !folia_visualizer::accept_art(self.art_pending, id, generation) {
                    tracing::debug!(id, generation, "dropped a stale cover");
                    return Task::none();
                }
                self.art_pending = None;
                self.art_task = None;
                if self.playback.current().is_some_and(|track| track.id == id) {
                    self.background_cover = art.background.map(|image| (id, image));
                }
                let keep = self.playback.current().map(|track| track.id);
                match art.image {
                    Some(cover) => {
                        let bytes = cover.rgba.len();
                        tracing::debug!(bytes, held = self.art.used_bytes() + bytes, "panel cover decoded");
                        self.cover_palettes.insert(id, art.colors, keep);
                        let handle = iced::widget::image::Handle::from_rgba(cover.width, cover.height, cover.rgba);
                        self.art.insert(id, Some(handle), bytes);
                    }
                    None => {
                        tracing::debug!(id, "panel cover missing");
                        self.cover_palettes.remove(id);
                        self.art.insert(id, None, 0);
                    }
                }
                Task::none()
            }
            Message::BackgroundPainted(generation, image) => {
                self.background_poster.finish(generation, image);
                Task::none()
            }
            Message::BackgroundPicked(Some(path)) => self.import_background(path),
            Message::BackgroundPicked(None) => Task::none(),
            Message::BackgroundLoaded(generation, path, result) => {
                if generation != self.background_upload_generation {
                    return Task::none();
                }
                match result {
                    Ok(image) => {
                        self.background_upload = Some(image);
                        let mut table = toml::map::Map::new();
                        table.insert("path".to_owned(), toml::Value::String(path.to_string_lossy().into_owned()));
                        self.visualizer.insert_style("custom_background_image", table, &[]);
                        if generation > 0 {
                            match folia_visualizer::Background::resolve(self.appearance.stored_background_mode()) {
                                folia_visualizer::Background::Monet => {
                                    let mut tuning = folia_visualizer::monet_background::Tuning::read(
                                        self.visualizer.tables.get("monet_background_tuning"),
                                    );
                                    tuning.uploaded = true;
                                    self.visualizer.insert_style("monet_background_tuning", tuning.table(), &[]);
                                }
                                folia_visualizer::Background::Nomand => {
                                    let mut tuning = folia_visualizer::nomand::Tuning::read(
                                        self.visualizer.tables.get("nomand_background_tuning"),
                                    );
                                    tuning.uploaded = true;
                                    self.visualizer.insert_style("nomand_background_tuning", tuning.table(), &[]);
                                }
                                _ => {}
                            }
                        }
                        save(&self.store, &self.visualizer)
                    }
                    Err(err) => {
                        tracing::warn!(error = %err, "custom background unavailable");
                        let text = self.i18n.t("notifications.unsupportedImageFormat").to_owned();
                        self.status(false, text)
                    }
                }
            }
            Message::MonetOffset(offset) => self.store_monet_offset(offset),
            Message::SeekTo(position) => {
                let _ = self.playback.update(playback::Message::Seeking(position), &self.store);
                self.playback.update(playback::Message::SeekDone, &self.store).map(Message::Playback)
            }
            Message::FontsLoaded(fonts) => {
                tracing::info!(count = fonts.len(), "system fonts read");
                self.system_fonts = fonts;
                Task::none()
            }
            Message::PortraitPicked(Some(path)) => self.store_portrait(path),
            Message::PortraitPicked(None) => Task::none(),
            Message::PortraitDecoded(path, Some(cover)) => {
                let handle = iced::widget::image::Handle::from_rgba(cover.width, cover.height, cover.rgba);
                self.monet_portrait = Some((path, handle));
                Task::none()
            }
            Message::PortraitDecoded(_, None) => Task::none(),
            Message::PortraitStored(path, cover) => self.remember_portrait(path, cover),
            Message::Chrome(message) => self.chrome(message),
            Message::Pointer(window, at) if self.window == Some(window) => match at {
                Some(at) => {
                    self.developer.pointer(at, self.window_size);
                    let activity = self.chrome(chrome::Message::Activity { x: at.x, y: at.y });
                    let drag = self.drag_move(window, at.x, at.y);
                    Task::batch([activity, drag])
                }
                None => self.chrome(chrome::Message::Left),
            },
            Message::Pointer(..) => Task::none(),
            // `resolvePlayerEscapeAction`: fullscreen leaves the key to the system, then the panel closes,
            // and only with neither does the player go back.
            Message::PlayerEscape if self.composing => Task::none(),
            Message::PlayerEscape => {
                if self.sheets.any_open() {
                    self.sheets.close_all();
                    Task::none()
                } else if self.add_to_playlist.is_open() {
                    self.add_to_playlist.close();
                    Task::none()
                } else if self.fullscreen {
                    self.toggle_fullscreen()
                } else if self.panel.is_open() {
                    self.panel(panel::Message::Close)
                } else {
                    self.navigate_back()
                }
            }
            Message::PlayerComposing(composing) => {
                self.composing = composing;
                Task::none()
            }
            Message::Timeline(message) => self.timeline(message),
            Message::Login(_) if self.sources.custom_only() => Task::none(),
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
        if self.sources.custom_only() {
            return Task::none();
        }
        self.return_home();
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
        self.history.clear();
        self.catalog = None;
        self.search = SearchPage::default();
        self.playback_source = None;
        self.pending_playback_source = None;
        self.play_navigation.cancel();
        self.playback.clear();
        self.page = Page::Library;
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
            self.pending_playback_source = Some((self.playback.request_id(), self.page_snapshot()));
            self.play_navigation.begin(self.playback.request_id(), self.page);
        } else {
            self.play_navigation.cancel();
        }
    }

    /// Plays what the home wall chose, or keeps the tab it switched to.
    fn library_effect(&mut self, effect: Option<library::Effect>) -> Task<Message> {
        match effect {
            Some(library::Effect::Catalog(message)) => self.catalog_message(message),
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
                if self.page != Page::NowPlaying {
                    self.remember_page();
                }
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
        self.drag_armed = false;
        self.drag_from = None;
        folia_render::backend::fullscreen_frame(self.fullscreen);
        let mode = if self.fullscreen { iced::window::Mode::Fullscreen } else { iced::window::Mode::Windowed };
        iced::window::set_mode(id, mode)
    }

    fn request_close(&mut self) -> Task<Message> {
        if self.storage.defer_close() {
            return Task::none();
        }
        if self.closing {
            return Task::none();
        }
        let task = self.playback_session.close(&self.playback, &self.store);
        self.closing = true;
        self.scene_generation = self.scene_generation.wrapping_add(1);
        self.playback.shutdown();
        task
    }

    fn storage_message(&mut self, message: storage::Message) -> Task<Message> {
        #[cfg(target_os = "linux")]
        let before = self.storage.active.clone();
        let task =
            self.storage.update(message, &self.store, self.omni.clone(), &mut self.playback, &mut self.scene_sound);
        #[cfg(target_os = "linux")]
        if before != self.storage.active {
            self.media_session = media_session::Bridge::new(self.storage.active.clone());
        }
        if self.storage.take_close() { Task::batch([task, self.request_close()]) } else { task }
    }

    fn chrome(&mut self, message: chrome::Message) -> Task<Message> {
        if matches!(message, chrome::Message::Close) {
            return self.request_close();
        }
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
            chrome::Message::Drag if !self.fullscreen => iced::window::drag(id),
            chrome::Message::Drag => Task::none(),
            chrome::Message::DragArm => {
                if self.fullscreen {
                    return Task::none();
                }
                // The press is on the strip; the subscription's `PointerHeld(true)` may come after the first move.
                self.pointer_held = true;
                self.drag_armed = true;
                self.drag_from = self.clock.pointer;
                Task::none()
            }
            chrome::Message::DragEnd => {
                self.drag_armed = false;
                self.drag_from = None;
                Task::none()
            }
            chrome::Message::Resize(direction) if !self.fullscreen => iced::window::drag_resize(id, direction),
            chrome::Message::Resize(_) => Task::none(),
            chrome::Message::Minimize => iced::window::minimize(id, true),
            chrome::Message::ToggleMaximize => {
                if self.fullscreen {
                    return Task::none();
                }
                self.maximized = !self.maximized;
                iced::window::toggle_maximize(id)
            }
            chrome::Message::ToggleFullscreen => self.toggle_fullscreen(),
            chrome::Message::Close => self.request_close(),
            chrome::Message::Activity { x, y } => {
                self.clock = self.clock.pointer(self.now_ms(), x, y);
                let height = self.window_size.height;
                if let Some(positioning) = self.positioning.as_mut() {
                    positioning.drag(y, height);
                }
                self.latch_back();
                self.sync_home_back();
                let now = std::time::Instant::now();
                if !self.modal_open() {
                    self.panel.set_chrome(self.chrome_visible(), now);
                }
                self.titlebar.pointer(Some(y), now);
                Task::none()
            }
            chrome::Message::Left => {
                self.clock = self.clock.leave(self.now_ms(), self.pointer_held);
                self.latch_back();
                let now = std::time::Instant::now();
                if !self.modal_open() {
                    self.panel.set_chrome(self.chrome_visible(), now);
                }
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

    fn wall_backdrop_visible(&self) -> bool {
        self.page == Page::Library
            && (self.appearance.wall.look != folia_store::WallLook::Solid
                || self.appearance.wall.seam_clear && self.library.seam().width > 0.0 && !self.library.poster_tilted())
    }

    fn song_surface(&self, wall: bool) -> Element<'_, Message> {
        let muted = folia_visualizer::color(self.theme.secondary, 1.0);
        let daylight = self.appearance.theme == ThemeMode::Light;
        let (title, artist, album, track_id) = match self.playback.current() {
            Some(track) => (
                track.title.clone(),
                track.artist.clone().unwrap_or_default(),
                track.album.clone().unwrap_or_default(),
                Some(track.id),
            ),
            None => (String::new(), String::new(), String::new(), None),
        };
        let portrait = self.portrait_handle(track_id).cloned();
        let tuning = self.tuning_of(self.style());
        let lyrics = if wall && !self.appearance.wall.backdrop_lyrics {
            &self.empty_lyrics
        } else {
            self.playback.lyrics.as_ref().unwrap_or(&self.empty_lyrics)
        };
        let scene = Scene {
            smart_beat: self.smart_beat.frame(self.playback.heard_position()),
            render_quality: tuning.render_quality(self.graphics.lyric_render_quality),
            static_mode: self.graphics.static_mode,
            paused: !self.playback.is_playing(),
            daylight,
            style: self.style(),
            lyrics,
            time: self.visualizer_hold.unwrap_or_else(|| self.playback.lyric_time()),
            theme: &self.theme,
            font_scale: self.appearance.lyrics_font_scale,
            seed: self.visualizer_seed.as_str(),
            stage: None,
            tuning,
            backdrop: true,
            title,
            artist,
            album,
            portrait,
            typography: fonts::typography(&self.appearance),
            preview: false,
            audio: &self.audio,
        };
        let subtitle = self.subtitle();
        let bottom = subtitle_bottom(self.bar_offset(), self.subtitle_presence.value(self.now_ms()));
        let background = folia_visualizer::color(self.theme.background, 1.0);
        let vignette = if self.appearance.disable_visualizer_vignette { 0.0 } else { 0.28 };
        let now = std::time::Instant::now();
        let foot = now_playing::Foot {
            bottom,
            glass: self.bar.shown(now) > 0.0
                || self.panel.glass(now) > 0.0
                || self.panel.handle_amount(now) > 0.0
                || self.back_hotspot()
                || self.notice_visible(now),
        };
        let surface = if self.scene_active() {
            now_playing::Surface::Cinema {
                config: folia_visualizer::cinema::Config {
                    generation: self.scene_generation,
                    options: &self.scene.scene_options.cinema,
                    render_quality: scene.render_quality,
                    theme: &self.theme,
                    audio: &self.audio,
                    song_id: track_id,
                    cover: self
                        .background_cover
                        .as_ref()
                        .filter(|(id, _)| Some(*id) == track_id)
                        .map(|(_, image)| image),
                    cover_pending: self.art_pending.is_some_and(|(id, _)| Some(id) == track_id),
                    paused: !self.playback.is_playing(),
                    static_mode: self.graphics.static_mode,
                    reduced_motion: motion::reduced(folia_store::MotionSurface::UiMicroMotion),
                    active: self.visualizer_hold.is_none(),
                    queue_ended: self.playback.queue_ended(),
                    publish_blur: true,
                },
                scene,
            }
        } else {
            scene.into()
        };
        let screen = now_playing::Screen {
            transparent: self.player_transparent(),
            window: self.window_size,
            vignette,
            subtitle_opacity: self.appearance.subtitle.opacity,
            subtitle_background: self.appearance.subtitle.background,
            subtitle_blur: self.appearance.subtitle.upcoming_blur,
            harmony: &self.appearance.subtitle,
            dpi: self.window_scale,
            opacity: self.appearance.visualizer_opacity,
            subtitle_scale: self.appearance.subtitle.font_scale,
            subtitle_font: fonts::resolve(if self.appearance.subtitle.font_inherits_lyrics {
                &self.appearance.lyrics_font
            } else {
                &self.appearance.subtitle.font
            }),
            shared: now_playing::Backdrop {
                static_mode: self.graphics.static_mode,
                mode: folia_visualizer::Background::resolve(self.appearance.stored_background_mode()),
                shapes: !self.appearance.disable_visualizer_geometric_background,
                vignette: !self.appearance.disable_visualizer_vignette,
                paused: !self.playback.is_playing() || self.visualizer_hold.is_some(),
            },
            cover: self.playback.current().and_then(|track| self.cover_palettes.get(track.id)).unwrap_or(&[]),
            cover_overlay: self.appearance.use_cover_color_bg.then_some(self.appearance.cover_background_opacity),
            latent: folia_visualizer::LatentTuning::read(self.visualizer.tables.get("latent_background_tuning")),
            daylight,
            sora: folia_visualizer::SoraTuning::read(self.visualizer.tables.get("sora_background_tuning")),
            monet: self.monet_background_tuning(),
            picture: self.background_poster.image(),
            nomand: folia_visualizer::nomand::Tuning::read(self.visualizer.tables.get("nomand_background_tuning")),
        };
        if wall {
            now_playing::wall(surface, screen, self.appearance.wall)
        } else {
            now_playing::projected(surface, &self.i18n, muted, background, screen, subtitle, foot)
        }
    }

    pub fn view(&self) -> Element<'_, Message> {
        motion::configure(&self.graphics.motion, self.system_reduced_motion);
        let (text_color, muted) =
            (folia_visualizer::color(self.theme.primary, 1.0), folia_visualizer::color(self.theme.secondary, 1.0));
        let daylight = self.appearance.theme == ThemeMode::Light;
        let page: Element<'_, Message> = match self.page {
            Page::Lattice => {
                let generation = self.lattice_generation;
                self.lattice.as_ref().map_or_else(
                    || iced::widget::space().into(),
                    |page| {
                        page.view(&self.i18n, &self.theme, &self.appearance, &self.playback, self.window_size)
                            .map(move |message| Message::Lattice(generation, message))
                    },
                )
            }
            Page::Library | Page::Search | Page::Catalog => {
                let playing = self.playback.current().map(|track| track.id);
                let queued = self
                    .library
                    .expanded_track()
                    .is_some_and(|track| self.playback.queue.items().iter().any(|queued| queued.id == track.id));
                let now = std::time::Instant::now();
                let look = library::CardLook {
                    queued,
                    playing: self.playback.is_playing(),
                    window_width: self.window_size.width,
                    // The tools button is always glass. Search, the title bar and a toast claim a source too.
                    glass: self.page == Page::Library
                        || self.search.fade(now) > 0.0
                        || self.titlebar.frost(now) > 0.0
                        || self.notice_visible(now)
                        || self.timeline.is_open(),
                    modal: self.modal_open(),
                };
                let wall =
                    self.library.view(&self.i18n, &self.theme, daylight, muted, playing, look).map(Message::Library);
                let backdrop = self.wall_backdrop_visible();
                let background = if backdrop {
                    self.song_surface(true)
                } else {
                    space().width(Length::Fill).height(Length::Fill).into()
                };
                // Keep the wall in the same child slot when panning moves the transparent seam offscreen.
                let surface = stack![background, wall].into();
                let capture =
                    (backdrop && self.home.tools(now) > 0.0).then_some(folia_render::quality::WallLayer::GlassSource);
                folia_render::quality::wall_layer(
                    surface,
                    capture,
                    Some(Box::new(move |images| {
                        self.library.wall_images(playing, images);
                    })),
                )
            }
            Page::NowPlaying => self.song_surface(false),
        };
        let playing = self.page == Page::NowPlaying;
        let now = std::time::Instant::now();
        let mut frame: iced::widget::Stack<'_, Message, iced::Theme, iced::Renderer> = if playing {
            stack![page]
        } else if self.page == Page::Catalog {
            match &self.catalog {
                Some(catalog) => stack![
                    catalog
                        .view(catalog::View {
                            i18n: &self.i18n,
                            ink: text_color,
                            muted,
                            accent: folia_visualizer::color(self.theme.accent, 1.0),
                            background: folia_visualizer::color(self.theme.background, 1.0),
                            daylight,
                            width: self.window_size.width,
                            height: self.window_size.height
                        })
                        .map(Message::Catalog)
                ],
                None => stack![page],
            }
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
                        secondary: muted,
                        background: folia_visualizer::color(self.theme.background, 1.0),
                        accent: folia_visualizer::color(self.theme.accent, 1.0),
                        daylight,
                        width: self.window_size.width,
                        source: search_source,
                        catalog: self.omni.as_ref().map(|omni| omni.capabilities(provider)).unwrap_or_default(),
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
            // `VisualizerShell` back: `backdrop-blur-md` (12px) under `bg-black/20`.
            let backdrop = if self.player_transparent() { 0.0 } else { 12.0 };
            let back = iced::widget::stack![
                chrome::glass_disc(40.0, [0.0; 4], backdrop, 1.0),
                chrome::round_button(
                    Icon::ChevronLeft,
                    20.0,
                    40.0,
                    Color { a: 0.2, ..Color::BLACK },
                    Color { r: 1.0, g: 1.0, b: 1.0, a: 0.6 },
                    self.i18n.t("ui.back"),
                    Message::Back,
                )
            ]
            .into();
            frame = frame.push(pin(
                back,
                Alignment::Start,
                Alignment::Start,
                iced::Padding { top: 24.0, right: 0.0, bottom: 0.0, left: 24.0 },
            ));
        }
        // The original mounts the bar on home too, collapsed until hovered (`buildAppOverlaysModel.ts:255`).
        if (playing || matches!(self.page, Page::Library | Page::Lattice)) && self.playback.current().is_some() {
            let look = player_bar::Look {
                i18n: &self.i18n,
                primary: text_color,
                secondary: muted,
                background: folia_visualizer::color(self.theme.background, 1.0),
                daylight,
                transparent_background: self.player_transparent(),
                window: self.window_size,
                bottom: self.bar_offset(),
                modal: self.modal_open(),
                controls_disabled: self.positioning.is_some(),
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
            let style = self.style();
            let quick = &self.panel_quick;
            let like_reason = self.panel_like_reason.as_deref();
            let add_reason = self.panel_add_reason.as_deref();
            let can_match = self.omni.is_some() && self.playback.current().is_some_and(|track| track.online.is_some());
            let panel_view = panel::View {
                catalog: self
                    .playback
                    .current()
                    .and_then(|track| track.online.as_ref())
                    .and_then(|id| self.omni.as_ref().map(|omni| omni.capabilities(&id.provider)))
                    .unwrap_or_default(),
                i18n: &self.i18n,
                theme: &self.theme,
                daylight,
                width: self.window_size.width,
                height: self.window_size.height,
                track: self.playback.current(),
                queue: self.playback.queue.items(),
                index: self.playback.queue.current_index(),
                volume: self.playback.settings.volume,
                muted: self.playback.settings.muted,
                lyric_offset: self.playback.lyric_offset,
                bottom: self.bar_offset(),
                repeat: self.playback.settings.repeat,
                style_id: &self.appearance.visualizer,
                scene: self.scene.scene_id,
                has_lyrics: self.playback.lyrics.as_ref().is_some_and(|lyrics| !lyrics.lines.is_empty()),
                font_scale: self.appearance.lyrics_font_scale,
                drawn_style: style,
                origin: self.playback.queue.origin().map(|origin| origin.name.as_str()),
                art: self.playback.current().and_then(|track| self.art.peek(&track.id)).and_then(Option::as_ref),
                pill_hover: self.panel.pill_hover(),
                platform,
                account,
                account_controls: !self.sources.custom_only(),
                account_avatar: self.library.online.avatar_for(provider),
                syncing: self.library.online.syncing(provider),
                quality_levels,
                quality,
                modal: self.modal_open(),
                audio: &self.audio,
                liked: self.song_liked(),
                like_disabled: like_reason,
                can_add: self.can_add_song(),
                add_disabled: add_reason,
                quick,
                theme_source: self.theme_settings.source,
                transparent_background: self.theme_settings.transparent_player_background,
                transparent_surface: self.player_transparent(),
                has_custom_theme: self.theme_settings.custom_dual_theme.is_some(),
                can_generate_theme: self.song_themes.can_generate(&self.playback),
                generating_theme: self.song_themes.generating,
                random_pool: self.theme_settings.random_pool.as_deref(),
                all_styles: &Style::ALL,
                background: folia_visualizer::Background::resolve(self.appearance.stored_background_mode()),
                equalizer_on: self.audio_settings.equalizer.enabled,
                replay_gain: self.audio_settings.replay_gain_mode,
                gain: self.playback_gain(),
                can_match,
                can_import_lyrics: self.playback.current().is_some(),
                can_export_lyrics: self.playback.lyrics.as_ref().is_some_and(|lyrics| !lyrics.lines.is_empty()),
                lyric_provider: self
                    .playback
                    .current()
                    .and_then(|track| track.online.as_ref())
                    .and_then(|online| {
                        self.library.online.platforms().into_iter().find(|platform| platform.id == online.provider)
                    })
                    .map_or("", |platform| platform.short_name),
                can_shuffle: !self.playback.is_fm() && self.playback.queue.items().len() >= 2,
                can_save_queue: self.playback.can_save_queue(),
            };
            frame = frame.push(self.panel.view(panel_view, std::time::Instant::now()).map(Message::Panel));
        }
        let can_create = match self.playback.current() {
            Some(track) if track.online.is_none() => track.id > 0,
            Some(track) => track.online.as_ref().is_some_and(|online| self.can_create_playlist(&online.provider)),
            None => false,
        };
        if let Some(dialog) = self.add_to_playlist.view(playlist_dialog::Look {
            i18n: &self.i18n,
            tokens: settings_kit::Tokens::new(&self.theme, daylight),
            daylight,
            window: self.window_size,
            playlists: &self.panel_choices,
            can_create,
        }) {
            frame = frame.push(dialog.map(Message::Playlists));
        }
        if let Some(window) = &self.settings {
            let values = settings::Values {
                storage: &self.storage,
                scrobble_enabled: self.playback.settings.netease_scrobble_enabled,
                scrobble_ready: self.omni.as_ref().is_some_and(|omni| omni.playback_reporting_ready("netease")),
                sources: &self.sources,
                language: self.general.language,
                resolved: self.i18n.language(),
                appearance: &self.appearance,
                theme_settings: &self.theme_settings,
                graphics: &self.graphics,
                applied_gpu: self.applied_gpu,
                system_reduced_motion: self.system_reduced_motion,
                song_themes: &self.song_themes,
                style: self.chosen_style(),
                scene: self.scene.scene_id,
                bar_offset: self.player.bottom_bar_offset,
                bar_max: folia_store::bottom_bar_max_offset(self.window_size.height),
                has_track: self.playback.current().is_some(),
                developer: &self.developer,
                developer_settings: &self.developer_settings,
                runtime_log: &self.runtime_log,
                memory_log_path: self.memory_monitor.log_path(),
            };
            frame = frame.push(window.view(&self.i18n, &self.theme, daylight, self.window_size, values));
        }
        if let Some(window) = &self.workbench {
            frame = frame.push(window.view(self.workbench_look()));
        }
        if let Some(park) = &self.theme_park {
            frame = frame.push(park.view(self.workbench_look(), &self.theme, daylight));
        }
        if let Some(window) = &self.scene_window {
            frame = frame.push(
                window
                    .view(
                        &self.scene,
                        scene_settings::Look {
                            i18n: &self.i18n,
                            theme: &self.theme,
                            daylight,
                            window: self.window_size,
                            sound_available: self.scene_sound.available,
                            sound_loading: self.scene_sound.loading,
                            sound_error: self.scene_sound.error.as_deref(),
                        },
                    )
                    .map(Message::SceneSettings),
            );
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
        if !self.sources.custom_only()
            && let Some(window) = self.login.view(look, now)
        {
            frame = frame.push(window.map(Message::Login));
        }
        if let Some(layer) =
            self.timeline.view(&self.i18n, &self.theme, daylight, self.window_size, self.playback.lyrics.as_ref(), now)
        {
            frame = frame.push(layer.map(Message::Timeline));
        }
        if let Some(sheet) = panel_sheets::view(
            &self.sheets,
            panel_sheets::Look {
                i18n: &self.i18n,
                daylight,
                window: self.window_size,
                has_lyrics: self.playback.lyrics.as_ref().is_some_and(|lyrics| !lyrics.lines.is_empty()),
                tokens: settings_kit::Tokens::new(&self.theme, daylight),
            },
        ) {
            frame = frame.push(sheet.map(Message::Sheets));
        }
        if let Some((ok, words, opacity, shift)) = self.status_notice(now) {
            frame = frame.push(status_toast::message_view(ok, words, daylight, self.window_size.width, opacity, shift));
        }
        if let Some(fade) = &self.page_fade {
            frame = frame.push(fade.view(now));
        }
        if let Some((opacity, shift)) = self.playback.loading_notice(now) {
            frame = frame.push(status_toast::loading_view(
                &self.i18n,
                daylight,
                self.window_size.width,
                opacity,
                shift,
                self.playback.loading_angle(now),
            ));
        }
        if let Some((opacity, shift)) = self.playback.custom_source_notice(now) {
            frame = frame.push(status_toast::custom_source_view(
                &self.i18n,
                daylight,
                self.window_size.width,
                opacity,
                shift,
            ));
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
        // `DraggableDebugWindow` is z-300: above the dialogs, below the title bar.
        let dev_tokens = settings_kit::Tokens::new(&self.theme, daylight);
        if let Some(alert) = self.sources.alert(&self.i18n, dev_tokens) {
            frame = frame.push(alert);
        }
        if let Some(layer) =
            developer::log_overlay(&self.i18n, dev_tokens, &self.developer, &self.developer_settings, self.window_size)
        {
            frame = frame.push(layer);
        }
        if let Some(layer) = developer::memory_overlay(
            &self.i18n,
            dev_tokens,
            &self.developer,
            &self.developer_settings,
            self.memory_monitor.log_path(),
            self.window_size,
        ) {
            frame = frame.push(layer);
        }
        if self.storage.moving {
            frame = frame.push(self.storage.overlay(&self.i18n, settings_kit::Tokens::new(&self.theme, daylight)));
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
        if !self.fullscreen {
            frame = frame.push(chrome::resize_frame().map(Message::Chrome));
        }
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

    /// Settings, the lyric workbench, the sign-in window or the lyrics timeline.
    fn modal_open(&self) -> bool {
        if self.storage.moving {
            return true;
        }
        if self.theme_park.is_some() {
            return true;
        }
        self.sources.has_alert()
            || self.settings.is_some()
            || self.workbench.is_some()
            || self.scene_window.is_some()
            || self.login.is_open()
            || self.sheets.any_open()
            || self.add_to_playlist.is_open()
            || self.timeline.is_open()
    }

    /// Surfaces that take the keyboard the way an open sheet does: settings, sign-in, the workbench, search.
    fn blocks_space(&self) -> bool {
        self.modal_open() || matches!(self.page, Page::Search | Page::Catalog)
    }

    /// The original back button only appears inside the top-left 120px square.
    /// A modal keeps whatever it was when the window opened.
    fn back_hotspot(&self) -> bool {
        if self.modal_open() { self.back_latched } else { self.live_back_hotspot() }
    }

    fn live_back_hotspot(&self) -> bool {
        self.clock.pointer.is_some_and(|(x, y)| x <= HOTSPOT && y <= HOTSPOT)
    }

    fn latch_back(&mut self) {
        if !self.modal_open() {
            self.back_latched = self.live_back_hotspot();
        }
    }

    /// A status toast is on screen, so the page under it has to feed the blur.
    fn notice_visible(&self, now: std::time::Instant) -> bool {
        self.playback.subscription_notice(now).is_some_and(|(_, opacity, _)| opacity > 0.0)
            || self.playback.custom_source_notice(now).is_some_and(|(opacity, _)| opacity > 0.0)
            || self.playback.loading_notice(now).is_some_and(|(opacity, _)| opacity > 0.0)
            || self.library.online.sync_notice(now).is_some_and(|(_, _, _, opacity, _)| opacity > 0.0)
    }

    fn subtitle(&self) -> overlay::SubtitleRows<'_> {
        if !self.style().shared_subtitle() {
            return overlay::SubtitleRows::default();
        }
        let Some(lyrics) = self.playback.lyrics.as_ref() else {
            return overlay::SubtitleRows::default();
        };
        overlay::subtitle_rows_at(
            &lyrics.lines,
            self.visualizer_hold.unwrap_or_else(|| self.playback.lyric_time()),
            &self.appearance.subtitle,
        )
    }

    /// Ticks only while music plays: every frame on the visualizer page, a few times a second elsewhere.
    pub fn subscription(&self) -> Subscription<Message> {
        let mut parts = vec![
            iced::system::theme_changes().map(Message::SystemTheme),
            iced::window::resize_events().map(|(_id, size)| Message::WindowResized(size)),
            iced::event::listen_with(fullscreen_key),
            iced::event::listen_with(window_pointer),
            // The release must reach every page, or a press that ends off the drag strip leaves the drag armed.
            iced::event::listen_with(chrome_pointer),
            iced::event::listen_with(developer_key),
        ];
        #[cfg(target_os = "linux")]
        if !self.closing {
            parts.push(self.media_session.subscription());
        }
        if self.graphics.motion.follow_system {
            parts.push(
                iced::time::every(std::time::Duration::from_secs(3))
                    .map(|_| Message::Graphics(graphics::Message::Poll)),
            );
        }
        if self.graphics.background_throttling && folia_render::backend::window_hidden() {
            if self.playback.is_playing() {
                parts.push(
                    iced::time::every(std::time::Duration::from_millis(250))
                        .map(|_| Message::Playback(playback::Message::Tick)),
                );
            }
            return Subscription::batch(parts);
        }
        if self.sources.polls() {
            parts.push(
                iced::time::every(std::time::Duration::from_secs(1)).map(|_| Message::Sources(sources::Message::Poll)),
            );
        }
        if self.sources.has_alert() && self.settings.is_none() {
            parts.push(iced::event::listen_with(settings_key));
        }
        if self.developer.log_open || self.settings.as_ref().is_some_and(settings::Settings::shows_developer) {
            parts.push(
                iced::time::every(std::time::Duration::from_millis(400))
                    .map(|_| Message::Developer(developer::Message::Tick)),
            );
        }
        if self.developer.memory_open {
            parts.push(
                iced::time::every(std::time::Duration::from_millis(250))
                    .map(|_| Message::Developer(developer::Message::Tick)),
            );
        }
        if self.playback.current().is_some() && !self.blocks_space() {
            parts.push(iced::event::listen_with(if self.page == Page::Lattice {
                lattice_space_key
            } else {
                space_key
            }));
        }
        if self.page == Page::NowPlaying && self.clock.until_ms.is_some() {
            parts.push(iced::time::every(std::time::Duration::from_millis(200)).map(|_| Message::ChromeTick));
        }
        let player_keys = self.page == Page::NowPlaying
            && self.theme_park.is_none()
            && self.settings.is_none()
            && self.workbench.is_none()
            && self.scene_window.is_none()
            && !self.login.is_open()
            && !self.timeline.is_open()
            && self.positioning.is_none();
        if player_keys {
            parts.push(iced::event::listen_with(player_key));
            parts.push(iced::event::listen_with(player_ime));
        }
        if self.settings.is_some()
            && self.workbench.is_none()
            && self.theme_park.is_none()
            && self.scene_window.is_none()
        {
            parts.push(iced::event::listen_with(settings_key));
        }
        if self.workbench.is_some() {
            parts.push(iced::event::listen_with(workbench_key));
        }
        if self.theme_park.is_some() {
            parts.push(iced::event::listen_with(theme_park_key));
        }
        if self.scene_window.is_some() {
            parts.push(iced::event::listen_with(scene_settings_key));
        }
        if self.positioning.is_some() {
            parts.push(iced::event::listen_with(positioner_key));
        }
        let sheets =
            self.settings.is_none() && self.workbench.is_none() && self.theme_park.is_none() && !self.login.is_open();
        // The tools panel leaves the wall its keys; only its Escape is the panel's (`WallToolsSurface`).
        let beside_tools = self.home.tools_open() && !self.home.more_open() && !self.library.menu_open();
        if self.page == Page::Lattice && sheets && !self.home.more_open() && !self.home.queue_editing() {
            parts.push(iced::event::listen_with(if beside_tools { wall_key_beside_tools } else { wall_key }));
        }
        if self.page == Page::Library
            && sheets
            && self.home.command_query().is_none()
            && !self.home.queue_editing()
            && (!self.home_menu_open() || beside_tools)
        {
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
        if self.timeline.is_open() {
            parts.push(iced::event::listen_with(timeline_key));
        }
        let now = std::time::Instant::now();
        let loading_motion = self.playback.loading.active()
            || (self.playback.is_playing() && self.playback.is_buffering())
            || self.playback.loading_notice(now).is_some();
        let lattice_motion = self.page == Page::Lattice
            && self.lattice.as_ref().is_some_and(|page| {
                page.animating(now) || self.playback.is_playing() && page.lyrics_visible(&self.playback)
            });
        if matches!(self.page, Page::Library | Page::Lattice) && self.home_menu_open() {
            parts.push(iced::event::listen_with(home_dismiss));
        }
        if self.page == Page::Catalog && sheets {
            parts.push(iced::event::listen_with(catalog_key));
        }
        if self.page == Page::Search && self.settings.is_none() {
            parts.push(iced::event::listen_with(search_key));
        }
        let preview = self
            .workbench
            .as_ref()
            .is_some_and(|window| window.ticking() || (window.live_running() && self.playback.is_playing()))
            || self.theme_park.as_ref().is_some_and(|park| {
                park.preview.ticking() || (park.preview.live_running() && self.playback.is_playing())
            });
        let ui_motion = self.ui_frames(now).active();
        let visual = self.scene_active()
            || lattice_motion
            || preview
            || loading_motion
            || (self.wall_backdrop_visible() && self.playback.is_playing())
            || (self.page == Page::NowPlaying && (self.playback.is_playing() || self.panel.visualizing()));
        if visual || ui_motion {
            let cap = if ui_motion
                || self.scene_active()
                || preview
                || self.panel.visualizing()
                || loading_motion
                || lattice_motion
            {
                None
            } else {
                self.style().max_fps()
            };
            let fps = match (self.appearance.frame_rate.fps(), cap) {
                (Some(setting), Some(cap)) => Some(setting.min(cap)),
                (setting, cap) => setting.or(cap),
            };
            let frames = match fps {
                Some(fps) => folia_render::frames_at(fps),
                None => iced::window::frames(),
            };
            parts.push(if visual { frames.map(|_| Message::VisualFrame) } else { frames.map(|_| Message::UiFrame) });
        } else if self.playback.is_playing() {
            parts.push(
                iced::time::every(std::time::Duration::from_millis(250))
                    .map(|_| Message::Playback(playback::Message::Tick)),
            );
        }
        Subscription::batch(parts)
    }

    fn developer(&mut self, message: developer::Message) -> Task<Message> {
        use developer::Message;
        match message {
            // Not gated on the switch, as the original: with recording off the panel says so.
            Message::ChordLog => {
                self.developer.log_open = !self.developer.log_open;
                if self.developer.log_open {
                    self.developer.refresh_log(&self.runtime_log);
                }
                Task::none()
            }
            Message::ChordMemory => {
                if self.developer_settings.memory_monitor_enabled {
                    self.developer.memory_open = !self.developer.memory_open;
                    if self.developer.memory_open {
                        self.developer.refresh_memory(self.memory_monitor.history());
                    }
                }
                Task::none()
            }
            Message::DragStart(panel) => {
                self.developer.grab(panel);
                Task::none()
            }
            Message::CloseLog => {
                self.developer.log_open = false;
                Task::none()
            }
            Message::CloseMemory => {
                self.developer.memory_open = false;
                Task::none()
            }
            Message::Capture(on) => {
                self.developer_settings.console_capture_enabled = on;
                self.apply_log_file();
                self.developer.refresh_log(&self.runtime_log);
                save(&self.store, &self.developer_settings)
            }
            Message::File(on) => {
                self.developer_settings.runtime_log_enabled = on;
                self.apply_log_file();
                tracing::info!(on, path = %self.runtime_log.path().display(), "runtime log file");
                save(&self.store, &self.developer_settings)
            }
            Message::FileMode(mode) => {
                self.developer_settings.runtime_log_mode = mode;
                self.apply_log_file();
                save(&self.store, &self.developer_settings)
            }
            Message::Memory(on) => {
                self.developer_settings.memory_monitor_enabled = on;
                if on {
                    self.memory_monitor
                        .set_log(self.developer_settings.memory_log_enabled, self.developer_settings.memory_log_mode);
                    self.memory_monitor.start(self.developer_settings.interval_ms());
                    self.developer.refresh_memory(self.memory_monitor.history());
                } else {
                    self.memory_monitor.stop();
                    self.memory_monitor.set_log(false, self.developer_settings.memory_log_mode);
                    self.developer.memory_open = false;
                }
                save(&self.store, &self.developer_settings)
            }
            Message::MemoryFile(on) => {
                self.developer_settings.memory_log_enabled = on;
                self.memory_monitor.set_log(
                    on && self.developer_settings.memory_monitor_enabled,
                    self.developer_settings.memory_log_mode,
                );
                save(&self.store, &self.developer_settings)
            }
            Message::MemoryFileMode(mode) => {
                self.developer_settings.memory_log_mode = mode;
                self.memory_monitor.set_log(
                    self.developer_settings.memory_log_enabled && self.developer_settings.memory_monitor_enabled,
                    mode,
                );
                save(&self.store, &self.developer_settings)
            }
            Message::MemoryInterval(interval) => {
                self.developer_settings.memory_interval_ms = folia_store::clamp_interval_ms(interval);
                if self.developer_settings.memory_monitor_enabled {
                    self.memory_monitor.start(self.developer_settings.interval_ms());
                    self.developer.refresh_memory(self.memory_monitor.history());
                }
                save(&self.store, &self.developer_settings)
            }
            Message::ToggleTarget(target) => {
                if target.is_empty() {
                    self.developer.hidden_targets.clear();
                } else if target == "*" {
                    self.developer.hidden_targets =
                        self.developer.lines.iter().map(|line| line.target.clone()).collect();
                } else if !self.developer.hidden_targets.remove(&target) {
                    self.developer.hidden_targets.insert(target);
                }
                Task::none()
            }
            Message::ToggleSeries(series) => {
                if !self.developer.hidden_series.remove(&series) {
                    self.developer.hidden_series.insert(series);
                }
                Task::none()
            }
            Message::ClearLog => {
                self.runtime_log.clear();
                self.developer.refresh_log(&self.runtime_log);
                Task::none()
            }
            Message::Copy => {
                let text = self.developer.visible_text();
                self.developer.copied = Some(std::time::Instant::now());
                iced::clipboard::write(text).map(|()| crate::Message::Developer(Message::Copied))
            }
            Message::Copied => {
                self.developer.copied = Some(std::time::Instant::now());
                Task::none()
            }
            Message::OpenRuntimeFolder => {
                let folder = self.runtime_log.path().parent().map(std::path::Path::to_owned);
                folder.map_or_else(Task::none, open_folder)
            }
            Message::OpenMemoryFolder => {
                let folder = self.memory_monitor.log_path().parent().map(std::path::Path::to_owned);
                folder.map_or_else(Task::none, open_folder)
            }
            Message::Tick => {
                if self.developer.log_open || self.settings.as_ref().is_some_and(settings::Settings::shows_developer) {
                    self.developer.refresh_log(&self.runtime_log);
                }
                if self.developer.memory_open {
                    self.developer.refresh_memory(self.memory_monitor.history());
                }
                Task::none()
            }
        }
    }

    /// Hands the three log switches to the runtime log; the file work itself stays on its own thread.
    fn apply_log_file(&self) {
        self.runtime_log.configure(
            self.developer_settings.console_capture_enabled,
            self.developer_settings.runtime_log_enabled,
            self.developer_settings.runtime_log_mode,
        );
    }

    fn timeline(&mut self, message: timeline::Message) -> Task<Message> {
        let now = std::time::Instant::now();
        let (task, effect) = self.timeline.update(
            message,
            self.playback.lyrics.as_ref(),
            self.playback.lyric_offset,
            self.playback.duration(),
            now,
        );
        let seek = match effect {
            timeline::Effect::None => Task::none(),
            timeline::Effect::Seek(time) => {
                let seeking = self.playback.update(playback::Message::Seeking(time), &self.store);
                let done = self.playback.update(playback::Message::SeekDone, &self.store);
                Task::batch([seeking, done]).map(Message::Playback)
            }
        };
        Task::batch([task.map(Message::Timeline), seek])
    }

    fn workbench(&mut self, message: workbench::Message) -> Task<Message> {
        let style = self.style();
        let manifest = style.manifest();
        let Some(window) = self.workbench.as_mut() else { return Task::none() };
        let latent = folia_visualizer::LatentTuning::read(self.visualizer.tables.get("latent_background_tuning"));
        let update = window.update(message, manifest, &self.appearance, latent);
        let scroll = window.scroll_frame(std::time::Instant::now()).map(Message::Workbench);
        let values = window.values.clone();
        let appearance_changed = update.appearance.is_some();
        if let Some(appearance) = update.appearance {
            self.appearance = appearance;
        }
        if let Some(tuning) = update.latent {
            self.visualizer.insert_style("latent_background_tuning", tuning.table(), &[]);
        }
        if let Some(tuning) = update.monet {
            self.visualizer.insert_style("monet_background_tuning", tuning.table(), &[]);
        }
        if let Some(tuning) = update.sora {
            self.visualizer.insert_style("sora_background_tuning", tuning.table(), &[]);
        }
        if let Some(tuning) = update.nomand {
            self.visualizer.insert_style("nomand_background_tuning", tuning.table(), &[]);
        }
        let (write_visualizer, write_appearance) =
            workbench::scheduled_saves(&update.effect, appearance_changed, update.flush_visualizer);
        let mut tasks = vec![scroll];
        if write_visualizer && matches!(update.effect, workbench::Effect::Style(_)) {
            tasks.push(self.store_tuning(style, values.clone()));
        }
        match update.effect {
            workbench::Effect::None
            | workbench::Effect::Save
            | workbench::Effect::SaveAppearance
            | workbench::Effect::Appearance => {}
            workbench::Effect::Close => self.workbench = None,
            workbench::Effect::Draft => self.hold_tuning(style, values.clone()),
            workbench::Effect::SmartBeat(change) => self.smart_beat.change(change, &mut self.visualizer),
            workbench::Effect::ScrollTop => tasks.push(self.snap_settings()),
            workbench::Effect::Style(next) => {
                self.appearance.visualizer = next.id().to_owned();
                if let Some(window) = self.workbench.as_mut() {
                    window.values = next.load(&self.visualizer.tables);
                }
            }
            workbench::Effect::Button("uploadPortrait") => tasks.push(self.pick_portrait()),
            workbench::Effect::Button("clearPortrait") => tasks.push(self.clear_portrait()),
            workbench::Effect::Button("uploadBackground") => tasks.push(Task::perform(
                crate::background::blocking(|| {
                    rfd::FileDialog::new().add_filter("Image", &["png", "jpg", "jpeg", "webp"]).pick_file()
                }),
                |path| Message::BackgroundPicked(path.flatten()),
            )),
            workbench::Effect::Button("clearBackground") => {
                self.background_upload_generation = self.background_upload_generation.wrapping_add(1);
                self.background_upload = None;
                self.visualizer.tables.remove("custom_background_image");
                let mut tuning = folia_visualizer::monet_background::Tuning::read(
                    self.visualizer.tables.get("monet_background_tuning"),
                );
                tuning.uploaded = false;
                self.visualizer.insert_style("monet_background_tuning", tuning.table(), &[]);
                let mut nomand =
                    folia_visualizer::nomand::Tuning::read(self.visualizer.tables.get("nomand_background_tuning"));
                nomand.uploaded = false;
                self.visualizer.insert_style("nomand_background_tuning", nomand.table(), &[]);
                tasks.push(save(&self.store, &self.visualizer));
            }
            workbench::Effect::Button(_) => {}
        }
        if update.scroll_top {
            tasks.push(self.snap_settings());
        }
        if update.close {
            self.workbench = None;
        }
        if write_visualizer && !matches!(update.effect, workbench::Effect::Style(_)) {
            tasks.push(self.store_tuning(style, values));
        }
        if write_appearance {
            tasks.push(save(&self.store, &self.appearance));
        }
        Task::batch(tasks)
    }

    fn snap_settings(&self) -> Task<Message> {
        iced::widget::operation::snap_to(
            workbench::settings_scroll_id(),
            iced::widget::operation::RelativeOffset { x: Some(0.0), y: Some(0.0) },
        )
    }

    /// Writes the file for a slider that is still down. A second call in the same gesture does nothing.
    fn finish_slider(&mut self) -> Task<Message> {
        let outcome = self.workbench.as_mut().map(|window| (window.release_drag(), window.values.clone()));
        let Some((effect, values)) = outcome else { return Task::none() };
        let style = self.style();
        match effect {
            workbench::Effect::Save => self.store_tuning(style, values),
            workbench::Effect::SaveAppearance => save(&self.store, &self.appearance),
            _ => Task::none(),
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
            None => {
                let song = self.playback.current().map_or(0, |track| track.id);
                if let Some((chosen_for, style)) = self.random_choice
                    && chosen_for == song
                {
                    return style;
                }
                Style::random_in(&self.selected_styles(), song)
            }
        }
    }

    /// The styles the random choice draws from, in registry order.
    fn selected_styles(&self) -> Vec<Style> {
        let Some(pool) = &self.theme_settings.random_pool else { return Style::ALL.to_vec() };
        let chosen: Vec<Style> =
            Style::ALL.into_iter().filter(|style| pool.iter().any(|entry| entry == style.id())).collect();
        if chosen.is_empty() { Style::ALL.to_vec() } else { chosen }
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
            scrobble_enabled: self.playback.settings.netease_scrobble_enabled,
            scrobble_ready: self.omni.as_ref().is_some_and(|omni| omni.playback_reporting_ready("netease")),
            wall: self.appearance.wall,
            lattice: self.page == Page::Lattice,
            can_save: self.home.island_shown(now) && self.playback.can_save_queue(),
            can_generate: self.song_themes.can_generate(&self.playback),
            generating: self.song_themes.generating,
            i18n: &self.i18n,
            text: folia_visualizer::color(self.theme.primary, 1.0),
            background: folia_visualizer::color(self.theme.background, 1.0),
            accent: folia_visualizer::color(self.theme.accent, 1.0),
            daylight: self.appearance.theme == ThemeMode::Light,
            has_track: self.playback.current().is_some(),
            busy: self.library.busy,
            explore_refresh: self.library.explore_refresh(),
            randomize_on_entry: self.general.wall_randomize_on_entry,
            volume: self.playback.settings.volume,
            muted: self.playback.settings.muted,
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
            modal: self.modal_open(),
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
        let seam = host.seam;
        let at_seam = self
            .clock
            .pointer
            .is_some_and(|(x, y)| ((if seam.horizontal { y } else { x }) - seam.center).abs() <= seam.width / 2.0);
        let active = at_seam || host.search.is_some() || self.library.filter_editing() || self.home.more_open();
        let seam_fade = if !active && !host.wall.lights_on { 0.4 } else { 1.0 };
        host.opacity = seam_fade
            * content_opacity(host.seam.width, layout.max(1.0))
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
        if self.sources.custom_only() || !self.library.tab.is_online() || self.library.layer.is_some() {
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
        let Some(track) = self.playback.current() else {
            self.art_task = None;
            self.art_pending = None;
            self.background_cover = None;
            return Task::none();
        };
        let id = track.id;
        if self.art_pending.is_some_and(|(pending, _)| pending != id) {
            self.art_task = None;
            self.art_pending = None;
        }
        let mode = folia_visualizer::Background::resolve(self.appearance.stored_background_mode());
        let full = self.scene_active()
            || matches!(mode, folia_visualizer::Background::Monet | folia_visualizer::Background::Nomand)
            || mode == folia_visualizer::Background::Common && self.appearance.use_cover_color_bg;
        if self.art_pending.is_some_and(|(pending, _)| pending == id) && (!full || self.art_request_full) {
            return Task::none();
        }
        if self.art.peek(&id).is_some_and(|cover| {
            cover.is_none() || !full || self.background_cover.as_ref().is_some_and(|(source, _)| *source == id)
        }) {
            self.art.touch(&id);
            return Task::none();
        }
        self.art_task = None;
        self.art_request_full = full;
        if let Some(url) = track.cover_url.clone() {
            let Some(omni) = self.omni.clone() else {
                self.cover_palettes.remove(id);
                self.art.insert(id, None, 0);
                return Task::none();
            };
            let generation = self.begin_art(id);
            let (task, handle) =
                Task::perform(crate::background::panel_art_online(omni, url, ART_PX, full), move |art| {
                    Message::Art(id, generation, art)
                })
                .abortable();
            self.art_task = Some(handle.abort_on_drop());
            return task;
        }
        if track.path.as_os_str().is_empty() {
            self.cover_palettes.remove(id);
            self.art.insert(id, None, 0);
            return Task::none();
        }
        let path = track.path.clone();
        let generation = self.begin_art(id);
        let (task, handle) =
            Task::perform(
                async move {
                    crate::background::blocking(move || crate::background::panel_art_file(path, ART_PX, full)).await
                },
                move |art| Message::Art(id, generation, art.unwrap_or_else(crate::background::PanelArt::missing)),
            )
            .abortable();
        self.art_task = Some(handle.abort_on_drop());
        task
    }

    fn begin_art(&mut self, id: i64) -> u64 {
        self.art_generation = self.art_generation.wrapping_add(1);
        let generation = self.art_generation;
        self.art_pending = Some((id, generation));
        generation
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
            background: folia_visualizer::Background::resolve(self.appearance.stored_background_mode()),
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
                self.return_home();
                Task::none()
            }
            panel::Effect::Locate => self.locate_playback_source(),
            panel::Effect::Settings => {
                self.settings = Some(settings::Settings::open());
                Task::none()
            }
            panel::Effect::Workbench => self.open_workbench(),
            panel::Effect::Scene => {
                self.scene_window = Some(scene_settings::State::open());
                Task::none()
            }
            panel::Effect::Intensity => {
                let before = self.appearance.clone();
                self.appearance.animation_intensity = match self.appearance.animation_intensity {
                    folia_core::AnimationIntensity::Calm => folia_core::AnimationIntensity::Normal,
                    folia_core::AnimationIntensity::Normal => folia_core::AnimationIntensity::Chaotic,
                    folia_core::AnimationIntensity::Chaotic => folia_core::AnimationIntensity::Calm,
                };
                self.theme = theme_for(
                    &self.theme_settings,
                    self.appearance.theme,
                    self.appearance.animation_intensity,
                    self.song_themes.applied.as_ref(),
                );
                self.iced_theme = iced_theme(&self.theme);
                self.appearance_changed(&before)
            }
            panel::Effect::Playback(message) => self.playback.update(message, &self.store).map(Message::Playback),
            panel::Effect::Theme(mode) => {
                let before = self.appearance.clone();
                self.appearance.follow_system_theme = false;
                self.appearance.theme = mode;
                self.theme = theme_for(
                    &self.theme_settings,
                    mode,
                    self.appearance.animation_intensity,
                    self.song_themes.applied.as_ref(),
                );
                self.iced_theme = iced_theme(&self.theme);
                self.appearance_changed(&before)
            }
            panel::Effect::Visualizer(style) => {
                self.appearance.visualizer = style.map_or(RANDOM_STYLE, Style::id).to_owned();
                save(&self.store, &self.appearance)
            }
            panel::Effect::Login | panel::Effect::Sync | panel::Effect::SignOut if self.sources.custom_only() => {
                Task::none()
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
            panel::Effect::Like => self.toggle_like(),
            panel::Effect::AddToPlaylist => {
                self.add_to_playlist.open();
                Task::none()
            }
            panel::Effect::CopySongInfo => self.copy_song_info(),
            panel::Effect::Quick(key) => self.apply_quick(key),
            panel::Effect::Reroll => self.reroll_style(),
            panel::Effect::PoolToggle(style) => self.toggle_pool(&style),
            panel::Effect::EqualizerOpen => {
                self.sheets.open_equalizer(&self.audio_settings.equalizer);
                Task::none()
            }
            panel::Effect::ThemeSource(source) => {
                if source == folia_store::ThemeSource::Custom && self.theme_settings.custom_dual_theme.is_none() {
                    return Task::none();
                }
                if source == folia_store::ThemeSource::Ai {
                    let theme = self
                        .song_themes
                        .current
                        .clone()
                        .or(self.song_themes.applied.clone())
                        .unwrap_or_else(folia_ai::default_theme);
                    self.song_themes.applied = Some(theme);
                }
                self.theme_settings.source = source;
                self.theme = theme_for(
                    &self.theme_settings,
                    self.appearance.theme,
                    self.appearance.animation_intensity,
                    self.song_themes.applied.as_ref(),
                );
                self.iced_theme = iced_theme(&self.theme);
                let theme_settings = self.theme_settings.clone();
                Task::batch([save(&self.store, &theme_settings), self.song_themes.remember()])
            }
            panel::Effect::GenerateTheme => {
                let colors = self.playback.current().and_then(|track| self.cover_palettes.get(track.id)).unwrap_or(&[]);
                self.song_themes.update(song_theme::Message::Generate, &self.store, &self.playback, colors)
            }
            panel::Effect::Catalog(message) => self.catalog_message(message),
            panel::Effect::TransparentBackground(enabled) => {
                self.theme_settings.transparent_player_background = enabled;
                tracing::info!(enabled, "player background transparency changed");
                save(&self.store, &self.theme_settings)
            }
            panel::Effect::Background(background) => {
                let before = self.appearance.clone();
                self.appearance.set_background_mode(Some(background.id()));
                self.appearance_changed(&before)
            }
            panel::Effect::QuickEditOpen => {
                if self.theme_settings.source == folia_store::ThemeSource::Ai {
                    return self.handle(Message::OpenThemePark);
                }
                self.sheets.open_editor(self.theme_settings.custom_dual_theme.as_ref(), self.theme_is_light());
                Task::none()
            }
            panel::Effect::MatchOnline => self.match_online_lyrics(),
            panel::Effect::LyricFileOpen => {
                self.sheets.open_lyric_file();
                Task::none()
            }
            panel::Effect::ExportLyrics => self.export_lyrics(),
            panel::Effect::ImportLyrics => self.import_lyrics(),
            panel::Effect::ReplayGain(mode) => {
                self.audio_settings.replay_gain_mode = mode;
                self.playback.set_replay_gain(mode, self.playback_gain());
                let audio = self.audio_settings.clone();
                save(&self.store, &audio)
            }
            panel::Effect::SaveQueue(name) => self.save_queue(name),
            panel::Effect::Shuffle => {
                if self.playback.shuffle(status_seed()) {
                    self.status(true, self.i18n.t("status.queueShuffled").to_owned())
                } else {
                    Task::none()
                }
            }
        };
        Task::batch([task, scroll])
    }

    /// Shows a `StatusToast` line for four seconds (`runStatusMessageTime`).
    fn status(&mut self, ok: bool, text: String) -> Task<Message> {
        self.status_serial = self.status_serial.wrapping_add(1);
        let serial = self.status_serial;
        self.status = Some((ok, text, std::time::Instant::now()));
        Task::perform(blocking(|| std::thread::sleep(std::time::Duration::from_millis(4000))), move |_| {
            Message::StatusLeaving(serial)
        })
    }

    fn theme_is_light(&self) -> bool {
        self.appearance.theme == ThemeMode::Light
    }

    /// Rebuilds what the panel's view borrows this frame.
    fn refresh_panel_data(&mut self) {
        let style = self.style();
        self.panel_quick = folia_visualizer::quick_controls(style, &self.tuning_of(style));
        self.panel_choices = self.playlist_choices();
        self.panel_like_reason = self.like_reason();
        self.panel_add_reason = self.add_reason();
    }

    /// `holdStatusMessage`: the toast's enter and leave.
    fn status_notice(&self, now: std::time::Instant) -> Option<(bool, String, f32, f32)> {
        let (ok, words, started) = self.status.as_ref()?;
        let ms = now.saturating_duration_since(*started).as_secs_f64() * 1000.0;
        let entered = folia_visualizer::Ease::new(0.2, 0.8, 0.2, 1.0).at((ms / 320.0).clamp(0.0, 1.0)) as f32;
        let left = folia_visualizer::Ease::new(0.4, 0.0, 1.0, 1.0).at(((ms - 4000.0) / 220.0).clamp(0.0, 1.0)) as f32;
        Some((*ok, words.clone(), entered * (1.0 - left), -18.0 * (1.0 - entered) - 10.0 * left))
    }

    fn playback_gain(&self) -> Option<folia_core::ReplayGain> {
        self.playback.current().and_then(|track| track.replay_gain)
    }

    fn local_track(&self) -> Option<i64> {
        self.playback.current().filter(|track| track.online.is_none() && track.id > 0).map(|track| track.id)
    }

    fn local_song_liked(&self, id: i64) -> bool {
        self.playlists.iter().find(|playlist| playlist.favorite).is_some_and(|playlist| playlist.song_ids.contains(&id))
    }

    fn song_liked(&self) -> bool {
        match self.playback.current() {
            Some(track) => match &track.online {
                Some(online) => self.library.online.is_liked(&online.provider, &online.id),
                None => self.local_track().is_some_and(|id| self.local_song_liked(id)),
            },
            None => false,
        }
    }

    /// `resolveLikeAvailability`'s reason, worded.
    fn like_reason(&self) -> Option<String> {
        let online = self.playback.current()?.online.as_ref()?;
        if self.library.online.can_like(&online.provider) {
            return None;
        }
        let platform = self.library.online.platforms().into_iter().find(|platform| platform.id == online.provider);
        let name = platform.map_or(online.provider.as_str(), |platform| platform.short_name);
        Some(self.i18n.format("status.providerLikeUnavailable", &[("provider", name)]))
    }

    /// The playlists the playing song can go into, in the order the picker shows them.
    fn playlist_choices(&self) -> Vec<panel::PlaylistChoice> {
        let online = self.playback.current().is_some_and(|track| track.online.is_some());
        if online {
            return self
                .online_playlists()
                .into_iter()
                .map(|collection| panel::PlaylistChoice {
                    local: None,
                    online: Some((collection.provider.clone(), collection.id.clone())),
                    name: collection.name.clone(),
                    detail: format!("{} {}", collection.track_count.unwrap_or(0), self.i18n.t("playlist.tracks")),
                })
                .collect();
        }
        self.playlists
            .iter()
            .map(|playlist| panel::PlaylistChoice {
                local: Some(playlist.id),
                online: None,
                name: playlist.name.clone(),
                detail: format!("{} {}", playlist.song_ids.len(), self.i18n.t("playlist.tracks")),
            })
            .collect()
    }

    /// The account's own playlists of the playing platform.
    fn online_playlists(&self) -> Vec<folia_omni::Collection> {
        let Some(provider) =
            self.playback.current().and_then(|track| track.online.as_ref()).map(|o| o.provider.clone())
        else {
            return Vec::new();
        };
        self.library
            .online
            .playlists
            .ready()
            .into_iter()
            .flatten()
            .filter(|collection| {
                collection.provider == provider
                    && collection.kind == folia_omni::CollectionKind::Playlist
                    && collection.owned
            })
            .cloned()
            .collect()
    }

    fn can_add_song(&self) -> bool {
        let Some(online) = self.playback.current().and_then(|track| track.online.as_ref()) else {
            return self.playback.current().is_some();
        };
        let capabilities = self.omni.as_ref().map_or(Default::default(), |omni| omni.capabilities(&online.provider));
        capabilities.playlist_track_mutations
            && (!self.online_playlists().is_empty() || self.can_create_playlist(&online.provider))
    }

    fn can_create_playlist(&self, provider: &str) -> bool {
        self.omni.as_ref().is_some_and(|omni| omni.capabilities(provider).create_playlist)
    }

    /// `status.providerPlaylistMutationUnavailable`, or why there is nowhere to put the song.
    fn add_reason(&self) -> Option<String> {
        let online = self.playback.current().and_then(|track| track.online.as_ref())?;
        let capabilities = self.omni.as_ref().map_or(Default::default(), |omni| omni.capabilities(&online.provider));
        if !capabilities.playlist_track_mutations {
            let name = self
                .library
                .online
                .platforms()
                .into_iter()
                .find(|platform| platform.id == online.provider)
                .map_or_else(|| online.provider.clone(), |platform| platform.short_name.to_owned());
            return Some(self.i18n.format("status.providerPlaylistMutationUnavailable", &[("provider", &name)]));
        }
        None
    }

    fn toggle_like(&mut self) -> Task<Message> {
        let Some(track) = self.playback.current().cloned() else { return Task::none() };
        if let Some(online) = track.online {
            if !self.library.online.can_like(&online.provider) {
                let reason = self.like_reason().unwrap_or_default();
                return self.status(false, reason);
            }
            let liked = !self.library.online.is_liked(&online.provider, &online.id);
            let load =
                self.library.online.ensure_liked().map(|message| Message::Library(library::Message::Online(message)));
            let fallback = self.library.online.provider();
            let provider = self
                .library
                .online
                .platforms()
                .into_iter()
                .find(|platform| platform.id == online.provider)
                .map_or(fallback, |platform| platform.id);
            let press = self
                .library
                .online
                .press_like(provider, online.id, liked)
                .map(|message| Message::Library(library::Message::Online(message)));
            return Task::batch([load, press]);
        }
        let Some(id) = self.local_track() else { return Task::none() };
        let favorite = !self.local_song_liked(id);
        let path = self.db_path.clone();
        let task = blocking(move || {
            let library = folia_library::Library::open(&path).map_err(|err| error_chain(&err))?;
            library.set_favorite(id, favorite).map_err(|err| error_chain(&err))?;
            library.playlists().map_err(|err| error_chain(&err))
        });
        let words = self.i18n.t(if favorite { "status.liked" } else { "status.unliked" }).to_owned();
        Task::batch([
            Task::perform(task, |result| {
                Message::LocalPlaylists(result.unwrap_or_else(|| Err("playlist worker stopped".to_owned())))
            }),
            self.status(true, words),
        ])
    }

    fn add_to_playlist(&mut self, index: usize) -> Task<Message> {
        let Some(choice) = self.playlist_choices().into_iter().nth(index) else { return Task::none() };
        if let Some(playlist) = choice.local {
            let Some(track) = self.local_track() else { return Task::none() };
            let path = self.db_path.clone();
            let task = blocking(move || {
                let library = folia_library::Library::open(&path).map_err(|err| error_chain(&err))?;
                library.add_to_playlist(playlist, &[track]).map_err(|err| error_chain(&err))?;
                library.playlists().map_err(|err| error_chain(&err))
            });
            return Task::perform(task, |result| {
                Message::LocalPlaylists(result.unwrap_or_else(|| Err("playlist worker stopped".to_owned())))
            });
        }
        let Some((provider, id)) = choice.online else { return Task::none() };
        let Some(song) = self.playback.current().and_then(|track| track.online.clone()) else { return Task::none() };
        let Some(omni) = self.omni.clone() else { return Task::none() };
        let collection = folia_omni::Collection {
            provider: provider.clone(),
            id,
            kind: folia_omni::CollectionKind::Playlist,
            name: choice.name,
            cover_url: None,
            track_count: None,
            creator: None,
            description: None,
            owned: true,
        };
        let words = self.i18n.t("status.playlistUpdated").to_owned();
        let add = Task::perform(
            async move { omni.update_playlist_tracks(&collection, &[song], true).await.map_err(|err| error_chain(&err)) },
            move |result| match result {
                Ok(()) => Message::Status(true, words.clone()),
                Err(err) => Message::Status(false, err),
            },
        );
        Task::batch([
            add,
            self.library.online.reload_playlists().map(|message| Message::Library(library::Message::Online(message))),
        ])
    }

    fn create_playlist(&mut self, name: String) -> Task<Message> {
        let Some(track) = self.playback.current().cloned() else { return Task::none() };
        if let Some(online) = track.online {
            let Some(omni) = self.omni.clone() else { return Task::none() };
            if !omni.capabilities(&online.provider).create_playlist {
                return Task::none();
            }
            let words = self.i18n.t("status.playlistUpdated").to_owned();
            let created = Task::perform(
                async move {
                    let playlist =
                        omni.create_playlist(&online.provider, &name).await.map_err(|err| error_chain(&err))?;
                    omni.update_playlist_tracks(&playlist, &[online], true).await.map_err(|err| error_chain(&err))?;
                    Ok::<(), String>(())
                },
                move |result| match result {
                    Ok(()) => Message::Status(true, words.clone()),
                    Err(err) => Message::Status(false, err),
                },
            );
            let reload = self
                .library
                .online
                .reload_playlists()
                .map(|message| Message::Library(library::Message::Online(message)));
            return Task::batch([created, reload]);
        }
        let Some(id) = self.local_track() else { return Task::none() };
        let path = self.db_path.clone();
        let task = blocking(move || {
            let library = folia_library::Library::open(&path).map_err(|err| error_chain(&err))?;
            let playlist = library.create_playlist(&name).map_err(|err| error_chain(&err))?;
            library.add_to_playlist(playlist.id, &[id]).map_err(|err| error_chain(&err))?;
            library.playlists().map_err(|err| error_chain(&err))
        });
        Task::perform(task, |result| {
            Message::LocalPlaylists(result.unwrap_or_else(|| Err("playlist worker stopped".to_owned())))
        })
    }

    fn copy_song_info(&mut self) -> Task<Message> {
        let Some(track) = self.playback.current() else { return Task::none() };
        let mut line = track.display_name();
        if let Some(album) = &track.album {
            line.push_str(" - ");
            line.push_str(album);
        }
        if let Some(url) = song_page_url(track) {
            line.push('\n');
            line.push_str(&url);
        }
        let words = self.i18n.t("status.copiedSongInfo").to_owned();
        let copied = words.clone();
        Task::batch([
            iced::clipboard::write(line).map(move |()| Message::Status(true, copied.clone())),
            self.status(true, words),
        ])
    }

    fn apply_quick(&mut self, key: &'static str) -> Task<Message> {
        let style = self.style();
        let mut values = self.tuning_of(style);
        if !folia_visualizer::apply_quick(style, &mut values, key) {
            return Task::none();
        }
        self.store_tuning(style, values)
    }

    fn reroll_style(&mut self) -> Task<Message> {
        if self.appearance.visualizer != RANDOM_STYLE {
            return Task::none();
        }
        let song = self.playback.current().map_or(0, |track| track.id);
        let pool = self.selected_styles();
        let current = self.style();
        if pool.len() < 2 {
            return Task::none();
        }
        let at = pool.iter().position(|style| *style == current).unwrap_or(0);
        let next = pool[(at + 1) % pool.len()];
        self.random_choice = Some((song, next));
        Task::none()
    }

    fn toggle_pool(&mut self, id: &str) -> Task<Message> {
        let mut selected: Vec<String> = self.selected_styles().into_iter().map(|style| style.id().to_owned()).collect();
        if let Some(at) = selected.iter().position(|entry| entry == id) {
            if selected.len() <= 1 {
                return Task::none();
            }
            selected.remove(at);
        } else {
            let order: Vec<&str> = Style::ALL.iter().map(|style| style.id()).collect();
            selected.push(id.to_owned());
            selected.sort_by_key(|entry| order.iter().position(|known| known == entry).unwrap_or(order.len()));
        }
        self.theme_settings.random_pool = Some(selected.clone());
        let theme_settings = self.theme_settings.clone();
        save(&self.store, &theme_settings)
    }

    fn match_online_lyrics(&mut self) -> Task<Message> {
        let (Some(online), Some(omni)) =
            (self.playback.current().and_then(|track| track.online.clone()), self.omni.clone())
        else {
            return Task::none();
        };
        let request = self.playback.request_id();
        let id = self.playback.current().map_or(0, |track| track.id);
        Task::perform(async move { omni.lyrics(&online).await.map_err(|err| error_chain(&err)) }, move |result| {
            Message::MatchedLyrics(request, id, result)
        })
    }

    fn import_lyrics(&mut self) -> Task<Message> {
        Task::perform(
            blocking(|| {
                let path = rfd::FileDialog::new()
                    .add_filter("Lyrics", &["lrc", "vtt", "ttml", "qrc", "yrc", "krc", "txt", "json"])
                    .pick_file()?;
                let bytes = std::fs::read(&path).ok()?;
                let text = folia_library::decode_text(&bytes);
                let parsed = folia_lyrics::parse(folia_lyrics::Format::detect(&text), &text, "", "");
                (!parsed.lines.is_empty()).then_some(parsed)
            }),
            |lyrics| Message::ImportedLyrics(lyrics.flatten().ok_or_else(|| "no lyrics in the file".to_owned())),
        )
    }

    fn export_lyrics(&mut self) -> Task<Message> {
        let Some(lyrics) = self.playback.lyrics.clone() else { return Task::none() };
        let Some(track) = self.playback.current().cloned() else { return Task::none() };
        let text = lyrics_to_lrc(&lyrics);
        Task::perform(
            blocking(move || {
                let name = format!("{} - {}.lrc", track.artist.clone().unwrap_or_default(), track.title);
                let Some(path) = rfd::FileDialog::new().set_file_name(name.trim_start_matches(" - ")).save_file()
                else {
                    return Err("cancelled".to_owned());
                };
                std::fs::write(&path, text).map_err(|err| err.to_string())?;
                Ok(path)
            }),
            |result| Message::ExportedLyrics(result.unwrap_or_else(|| Err("export worker stopped".to_owned()))),
        )
    }

    fn save_queue(&mut self, name: String) -> Task<Message> {
        if !self.playback.can_save_queue() {
            return Task::none();
        }
        let ids: Vec<i64> = self.playback.queue.items().iter().map(|track| track.id).collect();
        let path = self.db_path.clone();
        let task = blocking(move || {
            let mut library = folia_library::Library::open(&path).map_err(|err| error_chain(&err))?;
            library.save_queue(&name, &ids).map_err(|err| error_chain(&err))?;
            library.playlists().map_err(|err| error_chain(&err))
        });
        Task::perform(task, |result| {
            Message::LocalPlaylists(result.unwrap_or_else(|| Err("playlist worker stopped".to_owned())))
        })
    }

    fn sheet_event(&mut self, event: panel_sheets::Event) -> Task<Message> {
        match event {
            panel_sheets::Event::None => Task::none(),
            panel_sheets::Event::Equalizer(settings) => {
                self.audio_settings.equalizer = settings.clone();
                self.playback.set_equalizer(folia_player::Parameters::of(&settings));
                let audio = self.audio_settings.clone();
                save(&self.store, &audio)
            }
            panel_sheets::Event::SaveTheme(theme) => {
                self.theme_settings.custom_dual_theme = Some(theme);
                self.theme_settings.source = folia_store::ThemeSource::Custom;
                self.theme = theme_for(
                    &self.theme_settings,
                    self.appearance.theme,
                    self.appearance.animation_intensity,
                    self.song_themes.applied.as_ref(),
                );
                self.iced_theme = iced_theme(&self.theme);
                let theme_settings = self.theme_settings.clone();
                save(&self.store, &theme_settings)
            }
            panel_sheets::Event::ImportLyrics => self.import_lyrics(),
            panel_sheets::Event::ExportLyrics => self.export_lyrics(),
        }
    }

    fn home(&mut self, message: home::Message) -> Task<Message> {
        if !matches!(message, home::Message::CollectionMenu) {
            self.library.dismiss_menu();
        }
        let reopen = matches!(message, home::Message::Tab);
        let query = self.home.search_query().map(str::to_owned);
        let queue_open = self.home.queue_island_open();
        let playlist_name = matches!(message, home::Message::CommitQueue)
            .then(|| self.home.queue_name().unwrap_or_default().trim().to_owned());
        if matches!(message, home::Message::LatticeCommands) && self.page != Page::Lattice {
            return Task::none();
        }
        let dock_delta = self.home.dock_delta();
        self.home.update(&message);
        let now = std::time::Instant::now();
        let mut extra = Task::none();
        if self.home.queue_island_open() != queue_open {
            self.general.wall_tools_queue_island_open = self.home.queue_island_open();
            extra = save(&self.store, &self.general);
        }
        let mut replan = reopen;
        let task = match message {
            home::Message::RandomizeOnEntry => {
                self.general.wall_randomize_on_entry = !self.general.wall_randomize_on_entry;
                Task::batch([
                    self.library
                        .set_randomize_on_entry(self.general.wall_randomize_on_entry, &self.i18n)
                        .map(Message::Library),
                    save(&self.store, &self.general),
                ])
            }
            home::Message::TitleFocus { backwards } => {
                if backwards {
                    iced::widget::operation::focus_previous()
                } else {
                    iced::widget::operation::focus_next()
                }
            }
            home::Message::Search if self.page == Page::Lattice => Task::none(),
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
                self.remember_page();
                self.page = Page::Search;
                let provider = self.library.online.provider();
                let (task, _) = self.search.update(search::Message::Submit, self.omni.as_ref(), provider);
                task.map(Message::Search)
            }
            home::Message::Catalog(message) => self.catalog_message(message),
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
            home::Message::ToolsQueue(_) | home::Message::ToolsQueueClear if self.playback.is_fm() => Task::none(),
            home::Message::ToolsQueue(message) => {
                let queue = self.playback.queue.items();
                let list_h = self.home.list_height(
                    self.window_size,
                    std::time::Instant::now(),
                    queue.len(),
                    self.page == Page::Lattice,
                    self.playback.is_fm() || self.appearance.wall.radio,
                );
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
            home::Message::ToggleLattice if self.page == Page::Lattice => self.navigate_back(),
            home::Message::Destination | home::Message::ToggleLattice => {
                self.play_navigation.cancel();
                if self.page == Page::Lattice {
                    self.return_home();
                    Task::none()
                } else {
                    self.remember_page();
                    self.page = Page::Lattice;
                    Task::none()
                }
            }
            home::Message::Back if self.page == Page::Lattice => self.navigate_back(),
            home::Message::PosterDetails => {
                self.handle(Message::Lattice(self.lattice_generation, lattice::Message::Details))
            }
            home::Message::AutoFocus(on) => {
                self.appearance.wall.auto_focus = on;
                save(&self.store, &self.appearance)
            }
            home::Message::Vignette(on) => {
                self.appearance.wall.vignette = on;
                save(&self.store, &self.appearance)
            }
            home::Message::BadgeCurrentOnly(on) => {
                self.appearance.wall.badge_current_only = on;
                save(&self.store, &self.appearance)
            }
            home::Message::LocatePlaying if self.page == Page::Lattice => {
                self.handle(Message::Lattice(self.lattice_generation, lattice::Message::Locate))
            }
            home::Message::LocatePlaying => self.locate_playback_source(),
            home::Message::GenerateTheme => self.panel(panel::Message::GenerateTheme),
            home::Message::Shuffle => self.panel(panel::Message::Shuffle),
            home::Message::DockRelease => self.home(if dock_delta.unwrap_or_default() >= 36.0 {
                home::Message::Commands
            } else {
                home::Message::Tools
            }),
            home::Message::DockPress(_)
            | home::Message::DockMove(_)
            | home::Message::DockCancel
            | home::Message::QueueName(_) => Task::none(),
            home::Message::Chord | home::Message::LatticeCommands | home::Message::Commands => {
                iced::widget::operation::focus("wall-commands")
            }
            home::Message::CommandQuery(query) if self.home.executing() => {
                let action = match query.to_ascii_lowercase().as_str() {
                    "r" => Some(home::Message::Shuffle),
                    "c" => Some(home::Message::LocatePlaying),
                    "p" if self.page == Page::Library => {
                        Some(home::Message::WallLook(self.appearance.wall.look.next()))
                    }
                    _ => None,
                };
                action.map_or_else(Task::none, |action| self.home(home::Message::RunCommand(Box::new(action))))
            }
            home::Message::RunCommand(action) => self.home(*action),
            home::Message::CacheDirectory => self.storage_message(storage::Message::Choose),
            home::Message::Scrobble => self
                .playback
                .update(playback::Message::Scrobble(!self.playback.settings.netease_scrobble_enabled), &self.store)
                .map(Message::Playback),
            home::Message::CommandQuery(_) => Task::none(),
            home::Message::CycleTab(backwards) => {
                if self.page != Page::Library || self.library.layer.is_some() {
                    return Task::none();
                }
                let tabs = library::TABS;
                let index = tabs.iter().position(|&tab| tab == self.library.tab).unwrap_or(0);
                self.home(home::Message::SelectTab(
                    tabs[(index + if backwards { tabs.len() - 1 } else { 1 }) % tabs.len()],
                ))
            }
            home::Message::QueueSearch(Some(query)) if query.is_empty() => {
                iced::widget::operation::focus("wall-queue-search")
            }
            home::Message::QueueSearch(_) => Task::none(),
            home::Message::SaveQueue => iced::widget::operation::focus("wall-playlist-name"),
            home::Message::CommitQueue => {
                playlist_name.filter(|name| !name.is_empty()).map_or_else(Task::none, |name| self.save_queue(name))
            }
            home::Message::ExportAppearance => match self.appearance.wall.export() {
                Ok(text) => iced::clipboard::write(text),
                Err(err) => {
                    tracing::warn!(error = &err as &dyn std::error::Error, "wall preset export failed");
                    self.status(false, self.i18n.t("wallTools.invalidPreset").to_owned())
                }
            },
            home::Message::ImportAppearance => {
                iced::clipboard::read().map(|text| Message::Home(home::Message::PastedAppearance(text)))
            }
            home::Message::PastedAppearance(text) => {
                match folia_store::WallAppearance::import(
                    text.as_deref().filter(|text| text.len() <= 65536).unwrap_or_default(),
                ) {
                    Ok(wall) => {
                        self.appearance.wall = wall;
                        save(&self.store, &self.appearance)
                    }
                    Err(_) => {
                        tracing::debug!("invalid wall preset");
                        self.status(false, self.i18n.t("wallTools.invalidPreset").to_owned())
                    }
                }
            }
            home::Message::CommitAppearance => save(&self.store, &self.appearance),
            home::Message::WallLook(_)
            | home::Message::Windows(_)
            | home::Message::Tint(_)
            | home::Message::Lights(_)
            | home::Message::CustomTint(_)
            | home::Message::Intensity(_)
            | home::Message::TintColor(_)
            | home::Message::SeamClear(_)
            | home::Message::BackdropLyrics(_)
            | home::Message::BackdropBlur(_)
            | home::Message::Radio(_)
            | home::Message::HideUpcoming(_) => {
                let wall = &mut self.appearance.wall;
                match message {
                    home::Message::WallLook(look) => wall.look = look,
                    home::Message::Windows(count) if wall.look == folia_store::WallLook::Partial => {
                        wall.windows_per_block = count.clamp(1, 6)
                    }
                    home::Message::Radio(on) if self.settings.is_some() || !self.playback.is_fm() => wall.radio = on,
                    home::Message::HideUpcoming(on) => wall.hide_upcoming = on,
                    home::Message::Tint(on) => wall.tint = on,
                    home::Message::Lights(on) => wall.lights_on = on,
                    home::Message::CustomTint(on) => wall.custom_tint = on,
                    home::Message::Intensity(value) => {
                        wall.intensity = value.clamp(0.0, 1.0);
                        return Task::none();
                    }
                    home::Message::TintColor(color) => {
                        wall.tint_color = color;
                        return Task::none();
                    }
                    home::Message::SeamClear(on) => wall.seam_clear = on,
                    home::Message::BackdropLyrics(on) => wall.backdrop_lyrics = on,
                    home::Message::BackdropBlur(on) => wall.backdrop_blur = on,
                    _ => return Task::none(),
                }
                save(&self.store, &self.appearance)
            }
            home::Message::ToolsQueueClear => {
                let (clear, task) = self.home.press_queue_clear(now);
                let notice = if clear && self.playback.clear_queue() {
                    self.status(true, self.i18n.t("status.queueCleared").to_owned())
                } else {
                    Task::none()
                };
                Task::batch([task.map(Message::Home), notice])
            }
            home::Message::Player => {
                if self.playback.current().is_some() {
                    if self.page != Page::NowPlaying {
                        self.remember_page();
                    }
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
            home::Message::ListScrolled(viewport) => self.library_message(library::Message::ListScrolled(viewport)),
            home::Message::ListWheel(delta) => self.library_message(library::Message::ListWheel(delta)),
            home::Message::HelpWheel(delta) => self.home.help_wheel(delta).map(Message::Home),
            home::Message::HelpScrolled(viewport) => {
                self.home.help_scrolled(&viewport);
                Task::none()
            }
            home::Message::Frame => {
                let scroll = self.home.scroll_frame().map(Message::Home);
                if self.page == Page::Lattice {
                    let queue = self.playback.queue.items();
                    Task::batch([
                        scroll,
                        self.home
                            .queue_frame(
                                queue,
                                self.playback.queue.current_index(),
                                queue_island::list_height(queue.len()),
                                now,
                            )
                            .map(|message| Message::Home(home::Message::ToolsQueue(message))),
                    ])
                } else {
                    scroll
                }
            }
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
            | home::Message::BackHover(_) => Task::none(),
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
        if self.modal_open() {
            return;
        }
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
        typeface::language(language);
        tracing::info!(language = language.code(), ?preference, "language switched");
        save(&self.store, &self.general)
    }
}

fn decode_portrait(path: String) -> Task<Message> {
    let file = std::path::PathBuf::from(&path);
    Task::perform(
        crate::background::blocking(move || {
            let bytes = std::fs::read(&file).ok()?;
            folia_library::cover_rgba(&bytes, 512, 512).ok()
        }),
        move |cover| Message::PortraitDecoded(path, cover.flatten()),
    )
}

fn copy_portrait(
    source: std::path::PathBuf,
    dest: std::path::PathBuf,
) -> Option<(std::path::PathBuf, folia_library::Cover)> {
    std::fs::create_dir_all(dest.parent()?).ok()?;
    std::fs::copy(&source, &dest).ok()?;
    let bytes = std::fs::read(&dest).ok()?;
    let cover = folia_library::cover_rgba(&bytes, 512, 512).ok()?;
    Some((dest, cover))
}

/// Reads the files' tags in the background and queues the playable ones.
/// The platform's page for a song, for the copied title line's second line.
fn song_page_url(track: &Track) -> Option<String> {
    let online = track.online.as_ref()?;
    match online.provider.as_str() {
        "netease" => Some(format!("https://music.163.com/#/song?id={}", online.id)),
        "qq" => online.id.split('-').next().map(|mid| format!("https://y.qq.com/n/ryqq/songDetail/{mid}")),
        "kugou" => online
            .id
            .split('-')
            .next()
            .filter(|hash| !hash.is_empty())
            .map(|hash| format!("https://www.kugou.com/song/#hash={hash}")),
        "spotify" => Some(format!("https://open.spotify.com/track/{}", online.id)),
        "ytm" => Some(format!("https://music.youtube.com/watch?v={}", online.id)),
        _ => None,
    }
}

/// Plain enhanced-LRC: the line's time and text, then its translation at the same time.
fn lyrics_to_lrc(lyrics: &Lyrics) -> String {
    let mut out = String::new();
    for line in &lyrics.lines {
        let seconds = line.start.max(0.0);
        let minutes = (seconds / 60.0).floor() as u64;
        let stamp = format!("[{:02}:{:05.2}]", minutes, seconds - minutes as f64 * 60.0);
        out.push_str(&stamp);
        out.push_str(&line.text);
        out.push('\n');
        if let Some(translation) = &line.translation {
            out.push_str(&stamp);
            out.push_str(translation);
            out.push('\n');
        }
    }
    out
}

/// A fresh seed for a shuffle, from the clock.
fn status_seed() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0x9e37_79b9, |since| since.as_nanos() as u64)
}

/// The interface theme for a mode: the custom pair when it is chosen, else the built-in preset, with the stored
/// animation intensity.
fn theme_for(
    settings: &folia_store::ThemeSettings,
    mode: ThemeMode,
    intensity: folia_core::AnimationIntensity,
    song: Option<&folia_store::DualTheme>,
) -> Theme {
    if settings.source == folia_store::ThemeSource::Ai && song.is_none() {
        let fallback = folia_ai::default_theme();
        return if mode == ThemeMode::Light { fallback.light.theme(intensity) } else { fallback.dark.theme(intensity) };
    }
    let selected = match settings.source {
        folia_store::ThemeSource::Default => None,
        folia_store::ThemeSource::Custom => settings.custom_dual_theme.as_ref(),
        folia_store::ThemeSource::Ai => song,
    };
    match selected {
        Some(dual) => {
            let side = if mode == ThemeMode::Light { &dual.light } else { &dual.dark };
            side.theme(intensity)
        }
        _ => {
            let mut theme = mode.theme();
            theme.intensity = intensity;
            theme
        }
    }
}

/// Reads the local playlists for the cached list; runs off the UI thread.
fn open_playlists(path: &std::path::Path) -> Result<Vec<folia_library::Playlist>, String> {
    folia_library::Library::open(path).and_then(|library| library.playlists()).map_err(|err| error_chain(&err))
}

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

/// Opens a log folder from a background task; the file manager is not the interface thread's work.
fn open_folder(folder: std::path::PathBuf) -> Task<Message> {
    Task::perform(
        blocking(move || {
            let _ = std::fs::create_dir_all(&folder);
            let _ = open::that(folder);
        }),
        |_| Message::Developer(developer::Message::Tick),
    )
}

/// Lattice consumes the same uncaptured Space event for its focused poster's controls.
fn lattice_space_key(event: iced::Event, status: iced::event::Status, id: iced::window::Id) -> Option<Message> {
    space_key(event, status, id).map(|_| Message::Home(home::Message::PosterDetails))
}

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

/// Alt+Shift+D and Alt+Shift+M, the two shortcuts printed on the developer page.
fn developer_key(event: iced::Event, _status: iced::event::Status, _id: iced::window::Id) -> Option<Message> {
    let iced::Event::Keyboard(iced::keyboard::Event::KeyPressed { key, modifiers, repeat: false, .. }) = event else {
        return None;
    };
    if !modifiers.alt() || !modifiers.shift() {
        return None;
    }
    let iced::keyboard::Key::Character(text) = key else { return None };
    match text.to_lowercase().as_str() {
        "d" => Some(Message::Developer(developer::Message::ChordLog)),
        "m" => Some(Message::Developer(developer::Message::ChordMemory)),
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

fn scene_settings_key(event: iced::Event, status: iced::event::Status, _id: iced::window::Id) -> Option<Message> {
    match event {
        iced::Event::Keyboard(iced::keyboard::Event::KeyPressed {
            key: iced::keyboard::Key::Named(iced::keyboard::key::Named::Escape),
            ..
        }) if status == iced::event::Status::Ignored => Some(Message::SceneSettings(scene_settings::Message::Close)),
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

fn theme_park_key(event: iced::Event, status: iced::event::Status, id: iced::window::Id) -> Option<Message> {
    workbench_key(event, status, id).map(|_| Message::ThemePark(theme_park::Message::Close))
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
        iced::Event::Window(iced::window::Event::CloseRequested) => Some(Message::WindowClosing),
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
        home::WallKey::Destination => Message::Home(home::Message::ToggleLattice),
        home::WallKey::Chord => Message::Home(home::Message::Chord),
        home::WallKey::Commands => Message::Home(home::Message::Commands),
        home::WallKey::LatticeCommands => Message::Home(home::Message::LatticeCommands),
        home::WallKey::CycleTab(backwards) => Message::Home(home::Message::CycleTab(backwards)),
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

/// Escape closes the lyrics timeline; the command palette's own Escape is not open here.
fn timeline_key(event: iced::Event, status: iced::event::Status, _id: iced::window::Id) -> Option<Message> {
    if status == iced::event::Status::Captured {
        return None;
    }
    match event {
        iced::Event::Keyboard(iced::keyboard::Event::KeyPressed {
            key: iced::keyboard::Key::Named(iced::keyboard::key::Named::Escape),
            ..
        }) => Some(Message::Timeline(timeline::Message::Close)),
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

/// Wall-clock milliseconds, the `Date.now()` the idle breath reads.
fn wall_ms() -> f64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|elapsed| elapsed.as_secs_f64() * 1000.0)
        .unwrap_or(0.0)
}

fn catalog_key(event: iced::Event, status: iced::event::Status, _window: iced::window::Id) -> Option<Message> {
    if status == iced::event::Status::Ignored
        && matches!(
            event,
            iced::Event::Keyboard(iced::keyboard::Event::KeyPressed {
                key: iced::keyboard::Key::Named(iced::keyboard::key::Named::Escape),
                ..
            })
        )
    {
        Some(Message::Back)
    } else {
        None
    }
}
