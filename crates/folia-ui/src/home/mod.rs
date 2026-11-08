// Home shell: the seam, the corner back button and the tools panel.

pub mod flip;
mod seam;
pub mod title;
mod tools;

use std::time::{Duration, Instant};

use folia_core::Track;
use folia_i18n::I18n;
use folia_visualizer::Ease;

use folia_store::HomeTab;

use crate::motion::Tween;
use crate::queue_island::{self, QueueList};
use crate::wall::Seam;
use iced::widget::{mouse_area, space};
use iced::{Color, Element, Length, Task};

/// Seam width tween from `useBravaisSeam`: 0.36s, `cubic-bezier(0.65, 0, 0.35, 1)`.
const SEAM_EASE: Ease = Ease::new(0.65, 0.0, 0.35, 1.0);
const SEAM_MS: f64 = 360.0;
/// Panel open from `WallToolsButton`: 0.2s ease-out.
const PANEL_EASE: Ease = Ease::new(0.0, 0.0, 0.2, 1.0);
const PANEL_MS: f64 = 200.0;
/// Queue island from `WallToolsQueueIsland`: 0.18s ease-out, trailing the panel by 0.09s when both open.
const ISLAND_EASE: Ease = Ease::new(0.0, 0.0, 0.2, 1.0);
const ISLAND_MS: f64 = 180.0;
const ISLAND_TRAIL_MS: u64 = 90;
/// The help list from `WallToolsSurface`: 0.18s ease-out.
const HELP_MS: f64 = 180.0;
/// `CLEAR_CONFIRM_MS` and `CLEAR_CONFIRM_GUARD_MS` on the queue header's clear button.
const CLEAR_CONFIRM_MS: u64 = 3000;
const CLEAR_GUARD_MS: u64 = 400;
/// The help list's rows: their height and the list's padding (`li` 30px, `ul` 7px 12px).
pub const HELP_ROW_H: f32 = 30.0;
pub const HELP_PAD: f32 = 14.0;
/// The help rows this build actually has keys for, in the original's order.
pub const HELP_ROWS: usize = 6;

/// One line of the tools panel's help list (`BravaisHelpRow`): the label key and the key cap.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HelpRow {
    pub label: &'static str,
    pub kbd: &'static str,
}

/// `BRAVAIS_HELP_ROWS` less the rows whose keys this build has no action for (typing to filter, Tab between the
/// wall and the seam, F6, `: + C`, `: + R`, Ctrl/Cmd + B and K).
pub const HELP_LIST: [HelpRow; HELP_ROWS] = [
    HelpRow { label: "libraryBravais.helpSearch", kbd: "/" },
    HelpRow { label: "libraryBravais.helpMove", kbd: "↑ ↓ ← →" },
    HelpRow { label: "libraryBravais.helpOpen", kbd: "Enter" },
    HelpRow { label: "libraryBravais.helpPlay", kbd: "Enter" },
    HelpRow { label: "libraryBravais.helpEnqueue", kbd: "Shift + Enter" },
    HelpRow { label: "libraryBravais.helpBack", kbd: "ESC" },
];

/// What a key does on the home wall.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WallKey {
    /// `/`: the home seam's online search.
    Search,
    /// Tab: the next or previous focusable control.
    Focus {
        backwards: bool,
    },
    Card(crate::focus_card::Key),
}

/// `resolveBravaisKey(event) ?? resolveBravaisHomeKey(event)`, for the keys this build has.
pub fn wall_key_of(key: &iced::keyboard::Key, modifiers: iced::keyboard::Modifiers, repeat: bool) -> Option<WallKey> {
    use iced::keyboard::{Key, key::Named};
    let chord = modifiers.control() || modifiers.logo() || modifiers.alt();
    if *key == Key::Named(Named::Tab) && !chord {
        return Some(WallKey::Focus { backwards: modifiers.shift() });
    }
    if let Some(card) = crate::focus_card::key_of(key, modifiers, repeat) {
        return Some(WallKey::Card(card));
    }
    let slash = matches!(key, Key::Character(character) if character.as_str() == "/");
    (slash && !repeat && !chord).then_some(WallKey::Search)
}
/// Concealed back button: 200ms CSS `ease`.
const BACK_EASE: Ease = Ease::new(0.25, 0.1, 0.25, 1.0);
const BACK_MS: f64 = 200.0;
/// Edge tab slide: 240ms `cubic-bezier(0.2, 0.7, 0.25, 1)`.
const TAB_EASE: Ease = Ease::new(0.2, 0.7, 0.25, 1.0);
const TAB_MS: f64 = 240.0;
/// Menu pop from `bravaisPopMotion`: 0.18s in, 0.12s out.
const POP_IN: Ease = Ease::new(0.2, 0.7, 0.25, 1.0);
const POP_OUT: Ease = Ease::new(0.42, 0.0, 1.0, 1.0);
const POP_IN_MS: f64 = 180.0;
const POP_OUT_MS: f64 = 120.0;
/// `BRAVAIS_NARROW_VIEWPORT_PX`.
const NARROW_VW: f32 = 900.0;
const NARROW: f32 = 120.0;
const SPINE: f32 = 64.0;
const ROW_NARROW: f32 = 72.0;
const ROW_SPINE: f32 = 52.0;
/// `BRAVAIS_SEAM_FULL_WIDTH` and `BRAVAIS_SEAM_ROW_FULL_HEIGHT`: a collection's full info strip.
const FULL: f32 = 300.0;
/// `BRAVAIS_PANEL_MAX_WIDTH`.
pub const PANEL: f32 = 420.0;
/// `BRAVAIS_LIST_ROW_HEIGHT`.
pub const LIST_ROW: f32 = 44.0;
const ROW_FULL: f32 = 212.0;
/// Presence spring of the player safe area: stiffness 280, damping 28.
const PRESENCE_MS: f64 = 600.0;

/// `full` is the home narrow seam, `spine` the book spine, `hidden` the edge tab.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Level {
    Full,
    Spine,
    Hidden,
}

#[derive(Debug, Clone)]
pub enum Message {
    Fold,
    Expand,
    TitleToggle {
        spine: bool,
        keyboard: bool,
    },
    TitleFocus {
        backwards: bool,
    },
    Tab,
    More,
    Tools,
    /// The panel's queue cell: shows or hides the queue island above it.
    ToolsQueueToggle,
    /// A message from the queue island's list (drag, scroll, row actions).
    ToolsQueue(queue_island::Message),
    /// The queue header's clear button: the first press arms, the second clears.
    ToolsQueueClear,
    /// The armed clear button timed out after `CLEAR_CONFIRM_MS`.
    ToolsQueueClearTimeout(u64),
    /// The help button under the panel's appearance rows.
    ToolsHelp,
    /// Escape: cancels the armed clear first, then closes the panel.
    Escape,
    /// The magnifier or `/`: opens the seam's online search panel.
    Search,
    /// The panel's query field changed.
    SearchQuery(String),
    /// Enter or 「搜索」: the panel hands `SearchQuery` to the search page.
    SubmitSearch,
    /// 「关闭搜索」, × or Escape: only the panel shuts.
    CloseSearch,
    /// Escape while the search field has focus; an IME composition keeps it.
    EscapeSearch,
    /// The search field's input method opened or closed a composition.
    Composing(bool),
    Settings,
    Player,
    Import,
    Reload,
    /// 「换一批」 on the explore tab.
    RefreshExplore,
    Dismiss,
    Frame,
    Volume(f32),
    VolumeDone,
    Mute,
    BackHover(bool),
    SelectTab(HomeTab),
    /// The account entry: opens the shared sign-in window.
    Connect,
    /// The collection seam's back: returns to the home layer.
    Back,
    PlayAll,
    EnqueueAll,
    Query(String),
    OpenList,
    /// 「过滤当前页」 on the album poster.
    OpenFilter,
    /// Where the poster's cover canvas is on screen.
    PosterCover(iced::Rectangle),
    /// Enter in the collection filter.
    FilterSubmit,
    ListScrolled(f32),
    LocateRow(usize),
    PlayRow(usize),
    ToggleSubscribe,
    CollectionMenu,
    ReloadCollection,
    RowHover(Option<usize>),
}

pub struct HomeChrome {
    level: Level,
    /// The open width of the full level, vertical and sideways: the home seam, a collection's info strip,
    /// or its list panel.
    full: (f32, f32),
    restore: Level,
    horizontal: bool,
    /// The axis to open on once the current one has shut.
    pending_horizontal: Option<bool>,
    seen_width: f32,
    more: bool,
    tools_goal: f32,
    tools_from: f32,
    tools_at: Instant,
    /// `wallToolsQueueIslandOpen`: remembered across panel openings (`wall_tools_queue_island_open`).
    queue_open: bool,
    island_from: f32,
    island_goal: f32,
    island_at: Instant,
    /// Personal FM hides the island but keeps the preference, as the original does.
    island_fm: bool,
    queue: QueueList,
    /// The clear button is armed; `None` after the 3s window or a successful clear.
    queue_clear: Option<Instant>,
    clear_serial: u64,
    help_from: f32,
    help_goal: f32,
    help_at: Instant,
    seam_goal: f32,
    seam_from: f32,
    seam_at: Instant,
    back_goal: f32,
    back_from: f32,
    back_at: Instant,
    /// `transition: transform 200ms ease` on hover.
    back_hover: Tween,
    menu_goal: f32,
    menu_from: f32,
    menu_at: Instant,
    menu_ms: f64,
    presence_from: f32,
    presence_goal: f32,
    presence_at: Instant,
    tab_from: f32,
    tab_goal: f32,
    tab_at: Instant,
    /// `useBravaisHomeUiStore().searchOpen`: the home seam's online search panel.
    search_open: bool,
    search_query: String,
    /// The level to go back to when the panel closes.
    search_level: Option<Level>,
    /// The search field is in an input-method composition; Escape belongs to it.
    composing: bool,
}

impl HomeChrome {
    /// Seeds the level from the window folia-app opens, so the first resize after a restore is a real change.
    pub fn new(width: f32) -> Self {
        let now = Instant::now();
        let level = default_level(width);
        let open = open_width(level, false, (NARROW, ROW_NARROW));
        Self {
            level,
            full: (NARROW, ROW_NARROW),
            restore: level,
            horizontal: false,
            pending_horizontal: None,
            seen_width: width,
            more: false,
            tools_goal: 0.0,
            tools_from: 0.0,
            tools_at: now,
            queue_open: false,
            island_from: 0.0,
            island_goal: 0.0,
            island_at: now,
            island_fm: false,
            queue: QueueList::default(),
            queue_clear: None,
            clear_serial: 0,
            help_from: 0.0,
            help_goal: 0.0,
            help_at: now,
            seam_goal: open,
            seam_from: open,
            seam_at: now,
            back_goal: 0.0,
            back_from: 0.0,
            back_at: now,
            back_hover: Tween::new(0.0, BACK_MS, BACK_EASE, now),
            menu_goal: 0.0,
            menu_from: 0.0,
            menu_at: now,
            menu_ms: POP_IN_MS,
            presence_from: 0.0,
            presence_goal: 0.0,
            presence_at: now,
            tab_from: 0.0,
            tab_goal: 0.0,
            tab_at: now,
            search_open: false,
            search_query: String::new(),
            search_level: None,
            composing: false,
        }
    }

    /// `wallToolsQueueIslandOpen`, read from the settings file at startup.
    pub fn restore_queue_island(&mut self, open: bool) {
        self.queue_open = open;
    }

    pub fn queue_island_open(&self) -> bool {
        self.queue_open
    }

    /// The playing song and Personal FM state, from the shell on every update: the list follows the song, and
    /// FM hides the island without touching the preference.
    pub fn sync_queue(&mut self, queue: &[Track], current: Option<usize>, fm: bool, now: Instant) {
        if fm != self.island_fm {
            self.island_fm = fm;
            self.set_island(!fm && self.queue_open && self.tools_goal > 0.5, now, None);
        }
        if queue.is_empty() {
            self.queue_clear = None;
        }
        if self.island_shown(now) {
            let list_h = queue_island::list_height(queue.len());
            self.queue.follow(queue, current, list_h, now);
        }
    }

    pub fn queue_update(
        &mut self,
        message: queue_island::Message,
        queue: &[Track],
        list_h: f32,
    ) -> (Task<queue_island::Message>, Option<queue_island::Change>) {
        self.queue.update(message, queue, list_h)
    }

    pub fn queue_frame(
        &mut self,
        queue: &[Track],
        current: Option<usize>,
        list_h: f32,
        now: Instant,
    ) -> Task<queue_island::Message> {
        if self.island_shown(now) { self.queue.frame(queue, current, list_h, now) } else { Task::none() }
    }

    /// The clear button: arms for three seconds, then clears on the second press (outside the 400ms guard).
    pub fn press_queue_clear(&mut self, now: Instant) -> (bool, Task<Message>) {
        if let Some(at) = self.queue_clear {
            if now.saturating_duration_since(at).as_millis() < u128::from(CLEAR_GUARD_MS) {
                return (false, Task::none());
            }
            self.queue_clear = None;
            return (true, Task::none());
        }
        self.queue_clear = Some(now);
        self.clear_serial = self.clear_serial.wrapping_add(1);
        let serial = self.clear_serial;
        let wait = crate::background::blocking(|| std::thread::sleep(Duration::from_millis(CLEAR_CONFIRM_MS)));
        (false, Task::perform(wait, move |_| Message::ToolsQueueClearTimeout(serial)))
    }

    pub fn queue_clear_armed(&self) -> bool {
        self.queue_clear.is_some()
    }

    fn set_island(&mut self, open: bool, now: Instant, trail_ms: Option<u64>) {
        let current = self.island_goal > 0.5;
        if open == current && trail_ms.is_none() {
            return;
        }
        self.island_from = self.island(now);
        self.island_goal = if open { 1.0 } else { 0.0 };
        self.island_at = now + Duration::from_millis(trail_ms.unwrap_or(0));
        if open && !current {
            self.queue.opened();
        }
        if !open {
            self.queue_clear = None;
        }
    }

    fn set_help(&mut self, open: bool, now: Instant) {
        if open == (self.help_goal > 0.5) {
            return;
        }
        self.help_from = self.help(now);
        self.help_goal = if open { 1.0 } else { 0.0 };
        self.help_at = now;
    }

    pub fn update(&mut self, message: &Message) {
        let now = Instant::now();
        match message {
            Message::Fold => self.fold(now),
            Message::Expand => self.set_level(Level::Full, now),
            Message::TitleToggle { spine, .. } => {
                let level = if *spine { Level::Spine } else { Level::Full };
                if level != self.level {
                    self.set_level(level, now);
                }
            }
            Message::Tab => self.restore_or_reopen(now),
            Message::More => {
                if self.level == Level::Spine {
                    self.set_level(Level::Full, now);
                    self.set_menu(now, true);
                } else {
                    self.set_menu(now, !self.more);
                }
            }
            Message::Tools => {
                let opening = self.tools_goal <= 0.5;
                self.retarget_tools(now, if opening { 1.0 } else { 0.0 });
                if opening {
                    let trail = if self.queue_open { Some(ISLAND_TRAIL_MS) } else { None };
                    self.set_island(self.queue_open && !self.island_fm, now, trail);
                }
            }
            Message::ToolsQueueToggle => {
                self.queue_open = !self.queue_open;
                let open = self.queue_open && self.tools_goal > 0.5 && !self.island_fm;
                self.set_island(open, now, None);
            }
            Message::ToolsHelp => self.set_help(self.help_goal <= 0.5, now),
            Message::ToolsQueueClearTimeout(serial) => {
                if *serial == self.clear_serial {
                    self.queue_clear = None;
                }
            }
            Message::ToolsQueueClear | Message::ToolsQueue(_) => {}
            Message::Escape => {
                if self.queue_clear.take().is_none() {
                    self.set_menu(now, false);
                    if self.tools_goal > 0.5 {
                        self.retarget_tools(now, 0.0);
                    }
                }
            }
            Message::Dismiss => {
                self.set_menu(now, false);
                if self.tools_goal > 0.5 {
                    self.retarget_tools(now, 0.0);
                }
            }
            Message::Settings
            | Message::Player
            | Message::Search
            | Message::Import
            | Message::Reload
            | Message::RefreshExplore
            | Message::SelectTab(_)
            | Message::Connect
            | Message::Back
            | Message::PlayAll
            | Message::EnqueueAll
            | Message::OpenList
            | Message::OpenFilter
            | Message::ToggleSubscribe
            | Message::ReloadCollection => {
                self.set_menu(now, false);
                if self.tools_goal > 0.5 {
                    self.retarget_tools(now, 0.0);
                }
            }
            Message::BackHover(hovered) => self.back_hover.set(if *hovered { 1.0 } else { 0.0 }, now),
            Message::SearchQuery(query) => self.search_query = query.clone(),
            Message::CloseSearch => self.close_search(now),
            Message::EscapeSearch => {
                if !self.composing {
                    self.close_search(now);
                }
            }
            Message::Composing(composing) => self.composing = *composing,
            Message::Frame
            | Message::Volume(_)
            | Message::VolumeDone
            | Message::Mute
            | Message::Query(_)
            | Message::ListScrolled(_)
            | Message::LocateRow(_)
            | Message::PlayRow(_)
            | Message::CollectionMenu
            | Message::RowHover(_)
            | Message::PosterCover(_)
            | Message::SubmitSearch
            | Message::FilterSubmit => {}
            Message::TitleFocus { .. } => {}
        }
    }

    /// Viewport width picks the default level; a portrait window (`height ≥ width × 1.1`) turns the seam sideways.
    pub fn sync(&mut self, width: f32, height: f32, has_track: bool, now: Instant) {
        if width > 0.0 && height > 0.0 {
            self.sync_width(width, now);
            self.sync_axis(height >= width * 1.1, now);
        }
        let goal = if has_track { 1.0 } else { 0.0 };
        if (goal - self.presence_goal).abs() > 0.001 {
            self.presence_from = self.presence(now);
            self.presence_goal = goal;
            self.presence_at = now;
        }
    }

    /// Opens the full info strip for a collection layer (`BRAVAIS_SEAM_FULL_WIDTH`), its list panel
    /// (`min(BRAVAIS_PANEL_MAX_WIDTH, viewport − 52)`), or narrows back to the home seam.
    /// An album poster is `resolvePosterWidth` wide.
    pub fn set_collection(&mut self, collection: bool, list: bool, poster: bool, now: Instant) {
        let full = match (collection, list) {
            (false, _) => (NARROW, ROW_NARROW),
            (true, false) if poster => (crate::poster::width(self.seen_width), ROW_FULL),
            (true, false) => (FULL, ROW_FULL),
            (true, true) => (PANEL.min(self.seen_width - 52.0).max(FULL.min(self.seen_width)), ROW_FULL),
        };
        if full == self.full {
            return;
        }
        self.full = full;
        self.set_menu(now, false);
        if self.level != Level::Hidden {
            self.retarget_seam(now);
        }
    }

    /// Slides the edge tab in when it becomes visible.
    pub fn sync_tab(&mut self, visible: bool, now: Instant) {
        let goal = if visible { 1.0 } else { 0.0 };
        if (goal - self.tab_goal).abs() < 0.001 {
            return;
        }
        self.tab_from = self.tab(now);
        self.tab_goal = goal;
        self.tab_at = now;
    }

    /// Once a direction change has shut the seam, opens it on the new axis. `Some` asks the wall to replan.
    pub fn take_flip(&mut self, now: Instant) -> Option<bool> {
        let next = self.pending_horizontal?;
        if self.width(now) > 0.5 {
            return None;
        }
        self.pending_horizontal = None;
        self.horizontal = next;
        self.seam_from = 0.0;
        self.seam_goal = self.goal_width();
        self.seam_at = now;
        Some(next)
    }

    pub fn is_hidden(&self) -> bool {
        self.level == Level::Hidden
    }

    pub fn is_horizontal(&self) -> bool {
        self.horizontal
    }

    pub fn is_spine(&self) -> bool {
        self.level == Level::Spine
    }

    /// The width this level opens to, not the width it has animated to.
    pub fn goal(&self) -> f32 {
        self.goal_width()
    }

    /// Open/close the home seam's online search panel; the level itself comes back after closing.
    pub fn open_search(&mut self, now: Instant) {
        if self.search_open {
            return;
        }
        if self.level == Level::Hidden {
            self.set_level(self.restore, now);
        }
        self.search_level = Some(self.level);
        self.search_open = true;
        self.search_query.clear();
        self.composing = false;
        self.set_level(Level::Full, now);
        if self.tools_goal > 0.5 {
            self.retarget_tools(now, 0.0);
        }
    }

    pub fn close_search(&mut self, now: Instant) {
        if !self.search_open {
            return;
        }
        self.search_open = false;
        self.search_query.clear();
        self.composing = false;
        if let Some(level) = self.search_level.take() {
            self.set_level(level, now);
        }
    }

    /// The query while the panel is open; `None` means the home seam shows its tabs and tools.
    pub fn search_query(&self) -> Option<&str> {
        self.search_open.then_some(self.search_query.as_str())
    }

    pub fn set_back(&mut self, shown: bool, now: Instant) {
        let goal = if shown { 1.0 } else { 0.0 };
        if (goal - self.back_goal).abs() < 0.001 {
            return;
        }
        self.back_from = self.back(now);
        self.back_goal = goal;
        self.back_at = now;
    }

    pub fn menu_open(&self) -> bool {
        self.more || self.tools_open()
    }

    /// The tools panel is open. It leaves the wall its keys except Escape, as `WallToolsSurface` does.
    pub fn tools_open(&self) -> bool {
        self.tools_goal > 0.5
    }

    /// The seam's more menu is open.
    pub fn more_open(&self) -> bool {
        self.more
    }

    pub fn animating(&self, now: Instant) -> bool {
        self.busy(self.tools_from, self.tools_goal, self.tools_at, now, PANEL_MS)
            || self.busy(self.island_from, self.island_goal, self.island_at, now, ISLAND_MS)
            || self.busy(self.help_from, self.help_goal, self.help_at, now, HELP_MS)
            || self.queue.animating(now)
            || self.busy(self.seam_from, self.seam_goal, self.seam_at, now, SEAM_MS)
            || self.busy(self.back_from, self.back_goal, self.back_at, now, BACK_MS)
            || self.busy(self.menu_from, self.menu_goal, self.menu_at, now, self.menu_ms)
            || self.busy(self.tab_from, self.tab_goal, self.tab_at, now, TAB_MS)
            || self.back_hover.running(now)
            || self.presence_running(now)
            || self.pending_horizontal.is_some()
    }

    pub fn width(&self, now: Instant) -> f32 {
        mix(self.seam_from, self.seam_goal, self.seam_at, now, SEAM_MS, SEAM_EASE)
    }

    pub fn tools(&self, now: Instant) -> f32 {
        mix(self.tools_from, self.tools_goal, self.tools_at, now, PANEL_MS, PANEL_EASE)
    }

    pub fn island(&self, now: Instant) -> f32 {
        mix(self.island_from, self.island_goal, self.island_at, now, ISLAND_MS, ISLAND_EASE)
    }

    /// Kept in the layout while it fades out, as `AnimatePresence` does.
    pub fn island_shown(&self, now: Instant) -> bool {
        self.island_goal > 0.5 || self.island(now) > 0.005
    }

    pub fn help(&self, now: Instant) -> f32 {
        mix(self.help_from, self.help_goal, self.help_at, now, HELP_MS, ISLAND_EASE)
    }

    pub fn help_shown(&self, now: Instant) -> bool {
        self.help_goal > 0.5 || self.help(now) > 0.005
    }

    pub fn back(&self, now: Instant) -> f32 {
        mix(self.back_from, self.back_goal, self.back_at, now, BACK_MS, BACK_EASE)
    }

    pub fn menu(&self, now: Instant) -> f32 {
        let ease = if self.menu_goal > self.menu_from { POP_IN } else { POP_OUT };
        mix(self.menu_from, self.menu_goal, self.menu_at, now, self.menu_ms, ease)
    }

    pub fn tab(&self, now: Instant) -> f32 {
        mix(self.tab_from, self.tab_goal, self.tab_at, now, TAB_MS, TAB_EASE)
    }

    pub fn presence(&self, now: Instant) -> f32 {
        let elapsed = now.saturating_duration_since(self.presence_at).as_secs_f64();
        if elapsed * 1000.0 >= PRESENCE_MS {
            return self.presence_goal;
        }
        folia_visualizer::spring(f64::from(self.presence_from), f64::from(self.presence_goal), elapsed, 280.0, 28.0)
            as f32
    }

    fn sync_width(&mut self, width: f32, now: Instant) {
        let crossed = (self.seen_width < NARROW_VW) != (width < NARROW_VW);
        self.seen_width = width;
        if !crossed {
            return;
        }
        let fallback = default_level(width);
        if self.search_open || self.level == Level::Hidden {
            self.restore = fallback;
        } else {
            self.set_level(fallback, now);
        }
    }

    fn sync_axis(&mut self, portrait: bool, now: Instant) {
        if portrait == self.horizontal || self.pending_horizontal == Some(portrait) {
            return;
        }
        self.pending_horizontal = Some(portrait);
        self.seam_from = self.width(now);
        self.seam_goal = 0.0;
        self.seam_at = now;
        self.set_menu(now, false);
    }

    fn fold(&mut self, now: Instant) {
        if self.level == Level::Hidden {
            return;
        }
        self.restore = self.level;
        self.set_level(Level::Hidden, now);
    }

    fn restore_or_reopen(&mut self, now: Instant) {
        if self.level == Level::Hidden {
            self.set_level(self.restore, now);
            return;
        }
        self.seam_from = 0.0;
        self.seam_goal = self.goal_width();
        self.seam_at = now;
    }

    fn set_level(&mut self, level: Level, now: Instant) {
        self.level = level;
        if level != Level::Hidden {
            self.restore = level;
        }
        if level != Level::Full {
            self.set_menu(now, false);
        }
        self.retarget_seam(now);
    }

    fn set_menu(&mut self, now: Instant, open: bool) {
        self.more = open;
        self.menu_from = self.menu(now);
        self.menu_goal = if open { 1.0 } else { 0.0 };
        self.menu_at = now;
        self.menu_ms = if open { POP_IN_MS } else { POP_OUT_MS };
    }

    fn retarget_seam(&mut self, now: Instant) {
        self.seam_from = self.width(now);
        self.seam_goal = self.goal_width();
        self.seam_at = now;
    }

    /// A search panel takes the full info-strip width even from the narrow seam or the spine.
    fn goal_width(&self) -> f32 {
        if self.search_open && self.level != Level::Hidden {
            if self.horizontal { ROW_FULL } else { FULL }
        } else {
            open_width(self.level, self.horizontal, self.full)
        }
    }

    fn retarget_tools(&mut self, now: Instant, goal: f32) {
        self.tools_from = self.tools(now);
        self.tools_goal = goal;
        self.tools_at = now;
        if goal > 0.5 {
            self.set_menu(now, false);
        } else {
            self.set_island(false, now, None);
            self.set_help(false, now);
        }
    }

    fn presence_running(&self, now: Instant) -> bool {
        (self.presence_from - self.presence_goal).abs() > 0.001
            && now.saturating_duration_since(self.presence_at).as_secs_f64() * 1000.0 < PRESENCE_MS
    }

    fn busy(&self, from: f32, goal: f32, at: Instant, now: Instant, ms: f64) -> bool {
        (from - goal).abs() > 0.001 && now.saturating_duration_since(at).as_secs_f64() * 1000.0 < ms
    }
}

/// `resolveSeamLevel` default: under 900px the seam starts as a spine.
pub fn default_level_is_spine(width: f32) -> bool {
    width < NARROW_VW
}

fn default_level(width: f32) -> Level {
    if default_level_is_spine(width) { Level::Spine } else { Level::Full }
}

fn open_width(level: Level, horizontal: bool, full: (f32, f32)) -> f32 {
    match (level, horizontal) {
        (Level::Hidden, _) => 0.0,
        (Level::Spine, false) => SPINE,
        (Level::Spine, true) => ROW_SPINE,
        (Level::Full, false) => full.0,
        (Level::Full, true) => full.1,
    }
}

/// Seam content bottom inset. No song keeps 20px; a song springs to `offset + 80 + 8`.
pub fn seam_safe_bottom(offset: f32, presence: f32) -> f32 {
    let safe = offset + 80.0 + 8.0;
    20.0 + presence * (safe - 20.0)
}

pub struct Host<'a> {
    pub i18n: &'a I18n,
    pub text: Color,
    pub background: Color,
    pub accent: Color,
    pub daylight: bool,
    pub has_track: bool,
    pub busy: bool,
    /// The explore tab's 「换一批」 (`refresh-explore`): `Some(enabled)` while the tab shows the page.
    pub explore_refresh: Option<bool>,
    pub volume: f32,
    pub muted: bool,
    /// The queue island's colours (settings tokens faded by the island's own animation).
    pub tokens: crate::settings_kit::Tokens,
    pub queue: &'a [Track],
    pub queue_index: Option<usize>,
    pub fm: bool,
    pub window_width: f32,
    pub window_height: f32,
    pub seam: Seam,
    pub spine: bool,
    pub horizontal: bool,
    pub opacity: f32,
    pub bottom: f32,
    pub status: Option<String>,
    pub show_tab: bool,
    pub tab_hidden: bool,
    pub tab: f32,
    /// The first-level tabs with their labels, and the one open.
    pub tabs: Vec<(HomeTab, &'a str)>,
    pub current: HomeTab,
    /// The account entry above the tools, on online tabs only.
    pub account: Option<AccountSlot<'a>>,
    /// The open collection's title and count line; `None` on the home layer.
    pub layer: Option<LayerSeam>,
    /// The album poster's ground, drawn once for a size and palette.
    pub poster_ground: Option<&'a iced::widget::canvas::Cache>,
    /// The layer is turned with the poster: its ground is drawn by the wall, turned with it.
    pub poster_tilted: bool,
    /// The list row under the pointer.
    pub list_hover: Option<usize>,
    /// The query while the home seam's search panel is open.
    pub search: Option<&'a str>,
    /// Settings, the workbench or sign-in is open.
    pub modal: bool,
}

/// The open collection as its seam shows it (`useBravaisCollectionSeam`).
#[derive(Debug, Clone)]
pub struct LayerSeam {
    pub title: String,
    /// The count, or `matches / total` while filtering.
    pub meta: String,
    pub query: String,
    pub filtering: bool,
    pub match_label: String,
    /// Saved, saving in flight, and the star's title; `None` where it cannot be saved.
    pub star: Option<(bool, bool, String)>,
    pub list: bool,
    /// The list rows in view start at row `first` of `count`.
    pub first: usize,
    pub count: usize,
    pub rows: Vec<ListRow>,
    pub menu: bool,
    pub playable: bool,
    /// An album at the full level shows as a poster (`BravaisSeamAlbumPoster`).
    pub album: Option<AlbumSeam>,
}

/// The collection's filter input, focused when the poster's 「过滤当前页」 opens it (`openBravaisFilter`).
pub fn filter_id() -> iced::widget::Id {
    iced::widget::Id::new("bravais-collection-filter")
}

/// The home seam's search field, focused as soon as the panel opens.
pub fn search_id() -> iced::widget::Id {
    iced::widget::Id::new("bravais-seam-search")
}

/// The layout width (vertical) or height (sideways) of the content on screen; the seam flip freezes it at the old value.
pub(crate) fn content_layout(
    horizontal: bool,
    spine: bool,
    search: bool,
    layer: Option<&LayerSeam>,
    window_width: f32,
) -> f32 {
    seam::layout_of(horizontal, spine, search, layer, window_width)
}

/// What the album poster shows (`projectBravaisSeamAlbum`).
#[derive(Debug, Clone)]
pub struct AlbumSeam {
    pub palette: crate::poster::Palette,
    pub composition: crate::poster::Composition,
    pub cover: Option<iced::widget::image::Handle>,
    /// 「专辑 · 年份」.
    pub kicker: String,
    pub artists: String,
    pub facts: Vec<String>,
    pub description: Option<String>,
    /// The watermark: the hero word's last character, else the title's.
    pub ghost: String,
    /// The title wrapped into the poster's column.
    pub title: std::rc::Rc<crate::poster_motion::TitleLayout>,
    pub entry: crate::poster_motion::Entry,
    /// The cover is flying in from the wall; the poster's own stays hidden until it lands.
    pub flying: bool,
    /// The cover came by flight, so it does not fall in as well (`.bravais-album-cover[data-flight]`).
    pub flown: bool,
    /// How far the content stays in from both sides while the layer turns (the safe area).
    pub inset: f32,
}

/// One row of the song list (`BravaisListPanel`): number, title over artist, length.
#[derive(Debug, Clone)]
pub struct ListRow {
    pub badge: String,
    pub title: String,
    pub subtitle: String,
    pub time: String,
    pub current: bool,
}

/// The seam's account entry (`BravaisSeamAccountSwitcher`).
pub struct AccountSlot<'a> {
    /// Nobody signed in: a plug and the connect label.
    pub guest: bool,
    pub name: String,
    pub avatar: Option<&'a iced::widget::image::Handle>,
    /// The platform's app icon, drawn in place of its letter.
    pub icon: Option<iced::widget::image::Handle>,
    pub letter: char,
    pub mark: Color,
    pub tip: String,
}

/// Back button, then the seam, then a dismiss layer, then the tools. The seam paints over the back.
pub fn overlay<'a>(home: &'a HomeChrome, host: Host<'a>, now: Instant) -> Element<'a, Message> {
    let back = tools::back(&host, home.back(now), home.back_hover.value(now));
    let mut layers = iced::widget::stack![back, seam::bar(home, &host, now)];
    if home.menu_open() {
        let scrim = mouse_area(space().width(Length::Fill).height(Length::Fill)).on_press(Message::Dismiss);
        layers = layers.push(scrim);
    }
    if let Some(menu) = seam::popup(home, &host, now) {
        layers = layers.push(menu);
    }
    layers.push(tools::dock(home, &host, now)).width(Length::Fill).height(Length::Fill).into()
}

fn tr<'a>(host: &Host<'a>, key: &'static str) -> &'a str {
    host.i18n.t(key)
}

fn mix(from: f32, goal: f32, at: Instant, now: Instant, ms: f64, ease: Ease) -> f32 {
    let t = (now.saturating_duration_since(at).as_secs_f64() * 1000.0 / ms).clamp(0.0, 1.0);
    from + (goal - from) * ease.at(t) as f32
}
