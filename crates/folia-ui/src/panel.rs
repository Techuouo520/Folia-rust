// The now-playing player panel: frame from `PanelShell`, handle from `PanelHandle`, tabs from `UnifiedPanel`.

use crate::typeface::text;
use crate::typeface::text_input;
use std::time::Instant;

use folia_core::{AudioQuality, Repeat, Theme, Track};
use folia_i18n::I18n;
use folia_omni::{Account, Platform};
use folia_render::Plate;
use folia_store::{ReplayGainMode, ThemeMode, ThemeSource};
use folia_visualizer::{Ease, QuickControl, QuickKind, Style};
use iced::widget::text::LineHeight;
use iced::widget::{button, column, container, image, mouse_area, row, shader, slider, space, stack, tooltip};
use iced::{Alignment, Background, Border, Color, Element, Length, Size, Task, Theme as IcedTheme};
use lucide_static_svg::Icon;

use crate::icons::{Variant, icon, variant};
use crate::mode_picker;
use crate::motion::{CSS_TRANSITION, Tween};
use crate::platform_icon::mark_faded;
use crate::playback;
use crate::quality_select;
use crate::queue_island::{self, QueueList};
use crate::settings_kit::{self, Faded, Tokens};

pub use crate::playlist_dialog::PlaylistChoice;

const OPEN_MS: f64 = 300.0;
const CLOSE_MS: f64 = 220.0;
/// The kit's `fast`.
const FAST_MS: f64 = 120.0;
/// `FOLDED_PX`: the folded header and its cover.
const FOLDED: f32 = 48.0;
const HANDLE_MS: f64 = 240.0;
const OPEN_EASE: Ease = Ease::new(0.22, 1.0, 0.36, 1.0);
/// Handle colours and hover, `transition-all duration-300`.
const HANDLE_TONE_MS: f64 = 300.0;
/// `HANDLE_ROOM_PX`: the 48px handle and the 12px gap above it.
const HANDLE_ROOM: f32 = 60.0;
/// `BARE_GAP_PX`: under the surface when no handle is drawn.
const BARE_GAP: f32 = 8.0;
const TOP_MARGIN: f32 = 56.0;
const SHADOW_BLUR: f32 = 80.0;
const SHADOW_Y: f32 = 30.0;
/// Three sigmas of the `80px` blur.
const SHADOW_REACH: f32 = 120.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Cover,
    Source,
    Controls,
    Queue,
    Quality,
}

#[derive(Debug, Clone)]
pub enum Message {
    Toggle,
    /// Escape: the open panel closes and stays on the player page.
    Close,
    Choose(Tab),
    Frame,
    /// A tab body's measured height.
    Measured(Tab, f32),
    /// A click on the panel's face away from its controls.
    FacePress,
    HandleHover(bool),
    PillHover(bool),
    /// Sets the lyric timeline offset, in milliseconds.
    Offset(i64),
    Queue(queue_island::Message),
    Volume(f32),
    VolumeDone,
    Mute,
    Repeat,
    Home,
    Locate,
    Settings,
    Theme(ThemeMode),
    Picker(mode_picker::Message),
    Intensity,
    /// The sign-in shortcut on the guest card: opens the sign-in window on the track's platform.
    Login,
    /// The sync button.
    Sync,
    /// The account card's sign-out.
    SignOut,
    QualitySelect(quality_select::Message),
    /// The heart on the song header.
    Like,
    /// The star: the playlist picker opens.
    AddToPlaylist,
    /// The song title's press: copies the song's line.
    CopySongInfo,
    /// The lyric style's own quick control with this key.
    Quick(&'static str),
    /// With a random style chosen: draw another style for this song.
    Reroll,
    PoolOpen,
    PoolClose,
    /// Turns one style on or off in the random pool.
    PoolToggle(String),
    /// The audio effects window.
    EqualizerOpen,
    /// The theme source's segments and the quick editor.
    ThemeSource(folia_store::ThemeSource),
    QuickEditOpen,
    /// The lyric tab's buttons.
    MatchOnline,
    LyricFileOpen,
    ExportLyrics,
    ImportLyrics,
    ReplayGain(folia_store::ReplayGainMode),
    /// The save-as-playlist prompt: the queue's name field opens, then saves.
    SaveQueue,
    SaveQueueName(String),
    SaveQueueConfirm,
    SaveQueueCancel,
    Shuffle,
}

pub enum Effect {
    None,
    Home,
    Locate,
    Settings,
    Workbench,
    Playback(playback::Message),
    Intensity,
    Theme(ThemeMode),
    Visualizer(Option<Style>),
    Login,
    Sync,
    SignOut,
    Quality(AudioQuality),
    Like,
    AddToPlaylist,
    CopySongInfo,
    Quick(&'static str),
    Reroll,
    PoolToggle(String),
    EqualizerOpen,
    ThemeSource(folia_store::ThemeSource),
    QuickEditOpen,
    MatchOnline,
    LyricFileOpen,
    ExportLyrics,
    ImportLyrics,
    ReplayGain(folia_store::ReplayGainMode),
    SaveQueue(String),
    Shuffle,
}

pub struct PlayerPanel {
    open: bool,
    tab: Tab,
    /// Playing-page chrome is up. The handle stays with it (`hideToggleButton`).
    chrome: bool,
    pill_hover: bool,
    queue: QueueList,
    /// Surface opacity and scale: 0 is `{ opacity: 0, scale: 0.92 }`, 1 is open.
    surface: Tween,
    /// Handle presence: `{ opacity: 0, x: 20, y: 12, scale: 0.92 }` at 0.
    handle: Tween,
    /// Handle colours, closed 0 and open 1 (`transition-all duration-300`).
    handle_tone: Tween,
    handle_hover: Tween,
    /// `IconSwap` on the heart: 0 just changed, 1 settled.
    like_swap: Tween,
    /// The liked state the heart last showed; `None` before the first frame.
    liked_seen: Option<bool>,
    /// The song header folding from the cover tab (0) to the others (1): shape, folded row, big block.
    fold: [Tween; 3],
    /// The last tab other than the cover, whose body the panel grows to.
    body_tab: Tab,
    /// The body tab before it: the height eases from its body to `body_tab`'s (`st-t-unfold`, 300ms).
    body_from: Tab,
    grow: Tween,
    /// The tab before the current one: the dock's mark slides from it and the new page comes in
    /// (`st-page-in`, 220ms, from 6px below).
    tab_from: Tab,
    page: Tween,
    pickers: mode_picker::Pickers,
    quality: quality_select::Select,
    /// The random pool's popover, and the queue's save-as-playlist name field.
    pool_open: bool,
    save_queue: Option<String>,
    /// A modal covers the window: handle hover does not retarget, and floats stay in the normal stack.
    modal: bool,
    /// Measured body heights of the source, controls and quality tabs (`useMeasuredSize`); 0 until laid out.
    bodies: Bodies,
}

/// The body heights `measured` reported, so a tab's panel fits its content instead of an estimate.
#[derive(Debug, Clone, Copy, Default)]
struct Bodies {
    source: f32,
    controls: f32,
    quality: f32,
}

impl Bodies {
    fn get(self, tab: Tab) -> f32 {
        match tab {
            Tab::Source => self.source,
            Tab::Controls => self.controls,
            Tab::Quality => self.quality,
            Tab::Cover | Tab::Queue => 0.0,
        }
    }

    fn set(&mut self, tab: Tab, height: f32) {
        match tab {
            Tab::Source => self.source = height,
            Tab::Controls => self.controls = height,
            Tab::Quality => self.quality = height,
            Tab::Cover | Tab::Queue => {}
        }
    }
}

impl PlayerPanel {
    /// The panel's own popovers: the random pool and the queue's name field.
    fn overlay<'a>(
        &self,
        view: View<'a>,
        tokens: Faded,
        size: Size,
        top: f32,
        origin: (f32, f32),
    ) -> Option<Element<'a, Message>> {
        let body = if self.pool_open {
            let mut rows = column![].spacing(2);
            let pool = view.random_pool;
            for style in view.all_styles {
                let id = style.id();
                let on = pool.is_none_or(|pool| pool.iter().any(|entry| entry == id));
                rows = rows.push(
                    button(
                        row![
                            container(if on { icon(Icon::Check, 13.0, tokens.accent) } else { space().into() })
                                .width(Length::Fixed(16.0)),
                            text(view.i18n.t(style.label_key())).size(crate::typeface::size::CONTROL).color(tokens.t1),
                        ]
                        .spacing(8)
                        .align_y(Alignment::Center),
                    )
                    .width(Length::Fill)
                    .padding(pad(7.0, 8.0, 7.0, 8.0))
                    .on_press(Message::PoolToggle(id.to_owned()))
                    .style(button::text),
                );
            }
            let head = row![
                text(view.i18n.t("options.randomPoolChoose"))
                    .size(crate::typeface::size::CONTROL)
                    .font(crate::typeface::adapt(settings_kit::SEMIBOLD))
                    .color(tokens.t1),
                space().width(Length::Fill),
                text_btn(view.i18n.t("ui.close"), tokens.t3, Some(Message::PoolClose)),
            ]
            .align_y(Alignment::Center);
            column![head, hairline(tokens), rows].spacing(8)
        } else if let Some(name) = &self.save_queue {
            let field = text_input(view.i18n.t("localMusic.enterPlaylistName"), name)
                .on_input(Message::SaveQueueName)
                .on_submit(Message::SaveQueueConfirm)
                .size(crate::typeface::size::CONTROL)
                .padding(8)
                .style(move |_theme: &IcedTheme, _status| iced::widget::text_input::Style {
                    background: Background::Color(tokens.win),
                    border: Border { radius: 8.0.into(), width: 1.0, color: tokens.line },
                    icon: tokens.t3,
                    placeholder: tokens.t3,
                    value: tokens.t1,
                    selection: tokens.fill2,
                });
            let buttons = row![
                text_btn(view.i18n.t("ui.cancel"), tokens.t2, Some(Message::SaveQueueCancel)),
                text_btn(view.i18n.t("options.save"), tokens.t1, Some(Message::SaveQueueConfirm)),
            ]
            .spacing(6)
            .align_y(Alignment::Center);
            column![
                text(view.i18n.t("localMusic.saveQueueAsPlaylist"))
                    .size(crate::typeface::size::CONTROL)
                    .font(crate::typeface::adapt(settings_kit::SEMIBOLD))
                    .color(tokens.t1),
                field,
                container(buttons).width(Length::Fill).align_x(Alignment::End),
            ]
            .spacing(8)
        } else {
            return None;
        };
        let width = (size.width - 24.0).clamp(160.0, 300.0);
        let card =
            container(body).width(Length::Fixed(width)).padding(12).style(move |_theme: &IcedTheme| container::Style {
                background: Some(tokens.win.into()),
                border: Border { radius: 12.0.into(), width: 1.0, color: tokens.line },
                shadow: iced::Shadow {
                    color: Color { a: 0.35 * tokens.alpha(), ..Color::BLACK },
                    offset: iced::Vector::new(0.0, 14.0),
                    blur_radius: 36.0,
                },
                ..container::Style::default()
            });
        let left = 8.0;
        let top = (top + 60.0).min(size.height - 60.0);
        Some(
            container(card)
                .width(Length::Fill)
                .height(Length::Fill)
                .align_x(Alignment::Start)
                .align_y(Alignment::Start)
                .padding(pad(top + origin.1, 0.0, 0.0, left + origin.0))
                .into(),
        )
    }
    pub fn new(now: Instant) -> Self {
        Self {
            open: false,
            tab: Tab::Cover,
            chrome: true,
            pill_hover: false,
            queue: QueueList::default(),
            surface: Tween::new(0.0, OPEN_MS, OPEN_EASE, now),
            handle: Tween::new(1.0, HANDLE_MS, Ease::OUT, now),
            handle_tone: Tween::new(0.0, HANDLE_TONE_MS, CSS_TRANSITION, now),
            handle_hover: Tween::new(0.0, HANDLE_TONE_MS, CSS_TRANSITION, now),
            like_swap: Tween::new(1.0, 220.0, Ease::OUT, now),
            liked_seen: None,
            fold: [
                Tween::new(0.0, OPEN_MS, OPEN_EASE, now),
                Tween::new(0.0, CLOSE_MS, OPEN_EASE, now),
                Tween::new(1.0, CLOSE_MS, OPEN_EASE, now),
            ],
            body_tab: Tab::Controls,
            body_from: Tab::Controls,
            grow: Tween::new(1.0, OPEN_MS, OPEN_EASE, now),
            tab_from: Tab::Cover,
            page: Tween::new(1.0, CLOSE_MS, OPEN_EASE, now),
            pickers: mode_picker::Pickers::new(now),
            quality: quality_select::Select::new(now),
            pool_open: false,
            bodies: Bodies::default(),
            save_queue: None,
            modal: false,
        }
    }

    pub fn pill_hover(&self) -> bool {
        self.pill_hover
    }

    pub fn is_open(&self) -> bool {
        self.open
    }

    /// While a modal is open, handle hover does not change its goal.
    pub fn set_modal(&mut self, modal: bool) {
        self.modal = modal;
    }

    /// The handle-hover goal: 0 at rest, 1 under the pointer.
    pub fn handle_hover_goal(&self) -> f32 {
        self.handle_hover.goal()
    }

    /// Records the pointer on the handle. A modal leaves the goal unchanged.
    pub fn on_handle_hover(&mut self, hovered: bool, now: Instant) {
        if self.modal {
            return;
        }
        self.handle_hover.set(if hovered { 1.0 } else { 0.0 }, now);
    }

    /// Shows or hides the handle with the playing-page chrome.
    pub fn set_chrome(&mut self, visible: bool, now: Instant) {
        self.chrome = visible;
        self.handle.set(if visible { 1.0 } else { 0.0 }, now);
    }

    /// The heart that was just filled or emptied comes in a little small (`IconSwap`'s `move`, 220ms).
    pub fn sync_like(&mut self, liked: bool, now: Instant) {
        if self.liked_seen == Some(liked) {
            return;
        }
        if self.liked_seen.is_some() {
            self.like_swap.restart(0.0, now);
        }
        self.liked_seen = Some(liked);
    }

    pub fn animating(&self, now: Instant) -> bool {
        [self.surface, self.handle, self.handle_tone, self.handle_hover, self.grow, self.page, self.like_swap]
            .iter()
            .chain(&self.fold)
            .any(|tween| tween.running(now))
            || (self.open && self.pickers.animating(now))
            || self.quality.animating(now)
            || (self.open && self.tab == Tab::Queue && self.queue.animating(now))
    }

    /// The playing song changed: the open queue list scrolls to it (`align: 'smart'`).
    pub fn follow(&mut self, room: Room<'_>) {
        let list_h = queue_list_h(room.width, room.height, room.bottom, self.chrome, room.queue.len());
        self.queue.follow(room.queue, room.index, list_h, Instant::now());
    }

    pub fn update(&mut self, message: &Message, style_id: &str, room: Room<'_>) -> (Effect, Task<Message>) {
        let now = Instant::now();
        let list_h = queue_list_h(room.width, room.height, room.bottom, self.chrome, room.queue.len());
        let effect = match message {
            Message::Toggle => {
                self.set_open(!self.open, now);
                if self.open && self.tab == Tab::Queue {
                    self.queue.opened();
                }
                Effect::None
            }
            Message::Close => {
                if self.quality.is_open() {
                    self.quality.update(quality_select::Message::Close, room.quality_levels, room.quality, now);
                } else if self.pool_open || self.save_queue.is_some() {
                    self.pool_open = false;
                    self.save_queue = None;
                } else {
                    self.set_open(false, now);
                }
                Effect::None
            }
            Message::Choose(tab) => {
                if *tab == Tab::Queue && self.tab != Tab::Queue {
                    self.queue.opened();
                }
                self.tab_from = self.tab;
                self.tab = *tab;
                self.page.restart(0.0, now);
                self.fold_to(*tab, now);
                self.pickers.set_open(None, now);
                self.quality.dismiss(now);
                Effect::None
            }
            Message::Queue(message) => {
                let (task, change) = self.queue.update(message.clone(), room.queue, list_h);
                let effect = match change {
                    Some(queue_island::Change::Jump(index)) => Effect::Playback(playback::Message::Jump(index)),
                    Some(queue_island::Change::MoveNext(index)) => Effect::Playback(playback::Message::MoveNext(index)),
                    Some(queue_island::Change::MoveEnd(index)) => Effect::Playback(playback::Message::MoveEnd(index)),
                    Some(queue_island::Change::Remove(index)) => Effect::Playback(playback::Message::Remove(index)),
                    Some(queue_island::Change::Reorder(from, to)) => {
                        Effect::Playback(playback::Message::Reorder(from, to))
                    }
                    None => Effect::None,
                };
                return (effect, task.map(Message::Queue));
            }
            Message::PillHover(hovered) => {
                self.pill_hover = *hovered;
                Effect::None
            }
            Message::HandleHover(hovered) => {
                self.on_handle_hover(*hovered, now);
                Effect::None
            }
            Message::Volume(value) => Effect::Playback(playback::Message::Volume(*value)),
            Message::VolumeDone => Effect::Playback(playback::Message::VolumeDone),
            Message::Mute => Effect::Playback(playback::Message::ToggleMute),
            Message::Offset(ms) => Effect::Playback(playback::Message::LyricOffset(*ms as f64 / 1000.0)),
            Message::Repeat => Effect::Playback(playback::Message::CycleRepeat),
            Message::Home => {
                self.set_open(false, now);
                Effect::Home
            }
            Message::Locate => {
                self.set_open(false, now);
                Effect::Locate
            }
            Message::Settings => {
                self.set_open(false, now);
                Effect::Settings
            }
            Message::Theme(mode) => Effect::Theme(*mode),
            Message::Picker(message) => match self.pickers.update(message.clone(), style_id, now) {
                mode_picker::Outcome::None => Effect::None,
                mode_picker::Outcome::Style(style) => Effect::Visualizer(style),
                mode_picker::Outcome::Settings => {
                    self.set_open(false, now);
                    Effect::Workbench
                }
            },
            Message::Intensity => Effect::Intensity,
            Message::Login => {
                self.set_open(false, now);
                Effect::Login
            }
            Message::Sync => Effect::Sync,
            Message::SignOut => {
                self.set_open(false, now);
                Effect::SignOut
            }
            Message::QualitySelect(message) => {
                match self.quality.update(*message, room.quality_levels, room.quality, now) {
                    Some(level) => Effect::Quality(level),
                    None => Effect::None,
                }
            }
            Message::Like => Effect::Like,
            Message::AddToPlaylist => Effect::AddToPlaylist,
            Message::CopySongInfo => Effect::CopySongInfo,
            Message::Quick(key) => Effect::Quick(key),
            Message::Reroll => Effect::Reroll,
            Message::PoolOpen => {
                self.pool_open = true;
                Effect::None
            }
            Message::PoolClose => {
                self.pool_open = false;
                Effect::None
            }
            Message::PoolToggle(style) => Effect::PoolToggle(style.clone()),
            Message::EqualizerOpen => Effect::EqualizerOpen,
            Message::ThemeSource(source) => Effect::ThemeSource(*source),
            Message::QuickEditOpen => Effect::QuickEditOpen,
            Message::MatchOnline => Effect::MatchOnline,
            Message::LyricFileOpen => Effect::LyricFileOpen,
            Message::ExportLyrics => Effect::ExportLyrics,
            Message::ImportLyrics => Effect::ImportLyrics,
            Message::ReplayGain(mode) => Effect::ReplayGain(*mode),
            Message::SaveQueue => {
                self.save_queue = Some(String::new());
                Effect::None
            }
            Message::SaveQueueName(name) => {
                if let Some(draft) = &mut self.save_queue {
                    *draft = name.clone();
                }
                Effect::None
            }
            Message::SaveQueueConfirm => {
                let Some(name) = self.save_queue.take() else { return (Effect::None, Task::none()) };
                let name = name.trim().to_owned();
                if name.is_empty() {
                    return (Effect::None, Task::none());
                }
                Effect::SaveQueue(name)
            }
            Message::SaveQueueCancel => {
                self.save_queue = None;
                Effect::None
            }
            Message::Shuffle => Effect::Shuffle,
            Message::Measured(tab, height) => {
                self.bodies.set(*tab, *height);
                Effect::None
            }
            Message::Frame => {
                if self.open && self.tab == Tab::Queue {
                    let task = self.queue.frame(room.queue, room.index, list_h, now);
                    return (Effect::None, task.map(Message::Queue));
                }
                Effect::None
            }
            Message::FacePress => {
                if self.quality.is_open() {
                    self.quality.update(quality_select::Message::Close, room.quality_levels, room.quality, now);
                }
                self.pool_open = false;
                self.save_queue = None;
                Effect::None
            }
        };
        (effect, Task::none())
    }

    /// How open the surface is, including the close fade. Zero once it has finished closing.
    pub fn glass(&self, now: Instant) -> f32 {
        self.surface.value(now)
    }

    /// How visible the handle is. The playing page keeps a blur source while this is above zero.
    pub fn handle_amount(&self, now: Instant) -> f32 {
        self.handle.value(now)
    }

    pub fn view<'a>(&'a self, view: View<'a>, now: Instant) -> Element<'a, Message> {
        let open = self.surface.value(now);
        let shown = self.handle.value(now);
        let surface = (open > 0.0).then(|| surface(self, view, open, now));
        let look = HandleLook {
            open: self.open,
            shown,
            tone: self.handle_tone.value(now),
            hover: self.handle_hover.value(now),
        };
        let handle = (shown > 0.0).then(|| handle_button(view, look));
        // Always the same two layers: the handle coming and going must not rebuild the surface's widget state
        // (the queue list's scroll position).
        match surface {
            Some(surface) => {
                let mut layers = stack![];
                if self.quality.is_open() {
                    // A click outside the panel closes the quality menu, as the original's document listener.
                    // It closes on release without capturing, so the same click still reaches the window buttons
                    // and the page tabs under it.
                    let catcher = mouse_area(container(space()).width(Length::Fill).height(Length::Fill))
                        .on_release(Message::QualitySelect(quality_select::Message::Close));
                    layers = layers.push(catcher);
                }
                layers = layers.push(surface).push(handle.unwrap_or_else(|| space().into()));
                layers.into()
            }
            None => handle.unwrap_or_else(|| space().into()),
        }
    }

    /// `st-t-unfold` for the shape, `st-t-move` for the folded row; the big block leaves with `st-t-fast`.
    fn fold_to(&mut self, tab: Tab, now: Instant) {
        let folded = if tab == Tab::Cover { 0.0 } else { 1.0 };
        if tab != Tab::Cover && tab != self.body_tab {
            self.body_from = self.body_tab;
            self.body_tab = tab;
            self.grow.restart(0.0, now);
        }
        self.fold[0].set(folded, now);
        self.fold[1].set(folded, now);
        if folded > 0.5 {
            self.fold[2].set_with(0.0, FAST_MS, CSS_TRANSITION, now);
        } else {
            self.fold[2].set_with(1.0, CLOSE_MS, OPEN_EASE, now);
        }
    }

    /// Opens with the kit's `unfold` (300ms) and closes with `move` (220ms), both `KIT_EASE.out`.
    fn set_open(&mut self, open: bool, now: Instant) {
        self.open = open;
        let (goal, ms) = if open { (1.0, OPEN_MS) } else { (0.0, CLOSE_MS) };
        self.surface.set_with(goal, ms, OPEN_EASE, now);
        self.handle_tone.set(goal, now);
        if !open {
            self.pickers.dismiss(now);
            self.quality.dismiss(now);
        }
    }
}

/// What the panel's update needs of the window and the queue.
#[derive(Clone, Copy)]
pub struct Room<'a> {
    pub width: f32,
    pub height: f32,
    pub bottom: f32,
    pub queue: &'a [Track],
    pub index: Option<usize>,
    /// The playing platform's levels, lowest first, and the stored preference resolved for it.
    pub quality_levels: &'static [AudioQuality],
    pub quality: AudioQuality,
}

#[derive(Clone, Copy)]
pub struct View<'a> {
    pub i18n: &'a I18n,
    pub theme: &'a Theme,
    pub daylight: bool,
    pub width: f32,
    pub height: f32,
    pub track: Option<&'a Track>,
    pub queue: &'a [Track],
    pub index: Option<usize>,
    pub volume: f32,
    pub muted: bool,
    /// Lyric timeline offset of the playing song, seconds.
    pub lyric_offset: f64,
    /// The bottom baseline the panel and handle stand on, with the user's offset.
    pub bottom: f32,
    pub repeat: Repeat,
    pub style_id: &'a str,
    pub has_lyrics: bool,
    /// `lyricsFontScale`, for the style pictures.
    pub font_scale: f32,
    /// The style on the page; with random chosen, the one this song drew.
    pub drawn_style: Style,
    pub origin: Option<&'a str>,
    pub art: Option<&'a iced::widget::image::Handle>,
    pub pill_hover: bool,
    /// The track's platform, and that platform's cached account.
    pub platform: Option<Platform>,
    pub account: Option<&'a Account>,
    pub account_avatar: Option<&'a iced::widget::image::Handle>,
    pub syncing: bool,
    /// The playing platform's levels, lowest first, and the stored preference resolved for it.
    pub quality_levels: &'static [AudioQuality],
    pub quality: AudioQuality,
    /// Settings, the workbench or sign-in is open.
    pub modal: bool,
    pub audio: &'a folia_core::AudioFrame,
    /// The playing song is in the account's liked list (or the local liked list).
    pub liked: bool,
    /// Why the heart is out of reach, already worded; `None` when it can be pressed.
    pub like_disabled: Option<&'a str>,
    pub can_add: bool,
    pub add_disabled: Option<&'a str>,
    /// The chosen style's own quick controls.
    pub quick: &'a [QuickControl],
    pub theme_source: ThemeSource,
    pub has_custom_theme: bool,
    /// The random pool (style ids), or every style when `None`.
    pub random_pool: Option<&'a [String]>,
    pub all_styles: &'a [Style],
    pub equalizer_on: bool,
    pub replay_gain: ReplayGainMode,
    pub gain: Option<folia_core::ReplayGain>,
    /// The lyric tab's reach: a platform to match on, an import, an export.
    pub can_match: bool,
    pub can_import_lyrics: bool,
    pub can_export_lyrics: bool,
    /// The platform's name for the lyric source row.
    pub lyric_provider: &'a str,
    /// The queue can be shuffled and saved as a local playlist.
    pub can_shuffle: bool,
    pub can_save_queue: bool,
}

fn surface<'a>(panel: &'a PlayerPanel, view: View<'a>, open: f32, now: Instant) -> Element<'a, Message> {
    // iced cannot fade a subtree, so every paint of the surface takes the open state's opacity.
    let raw = tokens_of(&view);
    let tokens = Faded::new(raw, raw.t1, raw.accent, view.daylight, open);
    let online = view.track.is_some_and(|track| track.online.is_some());
    let local = view.track.is_some_and(|track| track.online.is_none());
    let tab = shown_tab(panel.tab, local, online);
    let edge = if view.width >= 768.0 { 32.0 } else { 16.0 };
    // `HANDLE_ROOM_PX` while the handle is drawn under the surface, `BARE_GAP_PX` once it hides.
    let under = if panel.chrome { HANDLE_ROOM } else { BARE_GAP };
    let panel_w = (view.width - 32.0).clamp(200.0, 360.0);
    let inner = (panel_w - 24.0).max(96.0);
    let max_h = (view.height - under - view.bottom - TOP_MARGIN).max(220.0);
    // The header and body heights move with the fold, between the cover tab and the last other tab.
    let (big_h, _, cover_px) = measure(Tab::Cover, view.queue.len(), view.account.is_some(), inner, max_h, 0.0);
    let body_of = |tab| measure(tab, view.queue.len(), view.account.is_some(), inner, max_h, panel.bodies.get(tab)).1;
    let (from_body, to_body) = (body_of(panel.body_from), body_of(panel.body_tab));
    let full_body = from_body + (to_body - from_body) * panel.grow.value(now);
    let fold =
        Fold { shape: panel.fold[0].value(now), folded: panel.fold[1].value(now), under: panel.fold[2].value(now) };
    let header_h = big_h + (FOLDED - big_h) * fold.shape;
    let body_h = full_body * fold.shape;
    let visual_h = (24.0 + header_h + body_h + 12.0 + 36.0).min(max_h);
    let visual_right = view.width - edge;
    let visual_bottom = view.height - view.bottom - under;
    let visual_left = visual_right - panel_w;
    let visual_top = visual_bottom - visual_h;
    let glass = settings_kit::panel_glass(view.daylight);
    // `--pn-shadow`: `0 30px 80px` and a 1px ring. `open` fades the blur and the fill together.
    let (shadow, ring) = if view.daylight {
        ([0.0, 0.0, 0.0, 0.24], [0.0, 0.0, 0.0, 0.06])
    } else {
        ([0.0, 0.0, 0.0, 0.55], [1.0, 1.0, 1.0, 0.09])
    };
    let x0 = (visual_left - SHADOW_REACH).max(0.0);
    let y0 = (visual_top - (SHADOW_REACH - SHADOW_Y)).max(0.0);
    let x1 = (visual_right + SHADOW_REACH).min(view.width).max(x0 + 1.0);
    let y1 = (visual_bottom + SHADOW_REACH + SHADOW_Y).min(view.height).max(y0 + 1.0);
    let plate = shader(Plate {
        radius: 20.0,
        fill_origin: [visual_left - x0, visual_top - y0],
        fill_size: [panel_w, visual_h],
        fill: glass,
        shadow,
        shadow_blur: SHADOW_BLUR,
        shadow_y: SHADOW_Y,
        shadow_spread: 0.0,
        ring,
        backdrop: 40.0,
        fade: open,
        saturate: 1.3,
    })
    .width(Length::Fixed(x1 - x0))
    .height(Length::Fixed(y1 - y0));
    let switch = Switch { from: shown_tab(panel.tab_from, local, online), page: panel.page.value(now) };
    let column = Column { tab, width: panel_w, header_h, body_h, cover_px, open, fold, switch };
    let swap = panel.like_swap.value(now);
    let face = mouse_area(face_column(view, tokens, column, swap, (panel, now)))
        .on_press(Message::FacePress)
        .interaction(iced::mouse::Interaction::Idle);
    let face = container(face).width(Length::Fixed(panel_w)).height(Length::Fixed(visual_h));
    let mut picker = None;
    if tab == Tab::Controls {
        let look = mode_picker::Look {
            i18n: view.i18n,
            tokens: tokens.tokens(),
            theme: view.theme,
            font_scale: view.font_scale,
            style_id: view.style_id,
            open,
            audio: view.audio,
        };
        let mut place = picker_place(&view, panel.pickers.drawn(), Size::new(panel_w, visual_h), header_h);
        place.origin = (visual_left - x0, visual_top - y0);
        picker = panel.pickers.picker(look, place, now, view.modal);
    }
    let mut quality = None;
    if tab == Tab::Quality {
        // The account block (56 signed in, 44 for a guest row), the hairline and the 44px quality row, whose
        // trigger is 6px inside it. The menu is drawn over the face, as the mode pickers are.
        let account_h = if view.account.is_some() { 56.0 } else { 44.0 };
        let trigger_top = header_h + account_h + 56.0;
        let place = quality_select::Place {
            window_height: view.height,
            window_top: visual_top,
            trigger_top,
            trigger_bottom: trigger_top + 32.0,
            trigger_right: panel_w - 16.0,
            origin: (visual_left - x0, visual_top - y0),
        };
        let look = quality_select::Look {
            i18n: view.i18n,
            tokens: tokens.tokens(),
            daylight: view.daylight,
            levels: view.quality_levels,
            value: view.quality,
            now,
        };
        quality = panel.quality.menu(look, place, view.modal);
    }
    let face = container(face).padding(pad(visual_top - y0, 0.0, 0.0, visual_left - x0));
    let mut layers = stack![plate, face];
    if let Some(picker) = picker {
        layers = layers.push(picker);
    }
    if let Some(quality) = quality {
        layers = layers.push(quality);
    }
    if let Some(extra) =
        panel.overlay(view, tokens, Size::new(panel_w, visual_h), visual_top, (visual_left - x0, visual_top - y0))
    {
        layers = layers.push(extra);
    }
    let patch: Element<'a, Message> = layers.width(Length::Fixed(x1 - x0)).height(Length::Fixed(y1 - y0)).into();
    // `transformOrigin`: the handle's centre, 24px in from the surface's right and `under - 24` below it.
    let scale = 0.92 + 0.08 * open;
    let origin = iced::Vector::new(visual_right - 24.0, visual_bottom + under - 24.0);
    let centre = iced::Vector::new((x0 + x1) / 2.0, (y0 + y1) / 2.0);
    let shift = (origin - centre) * (1.0 - scale);
    let patch: Element<'a, Message> = crate::float_layer::place(patch, view.modal, shift, scale);
    // The patch stops at the window edge, which clips the shadow there as the window would.
    container(patch).width(Length::Fill).height(Length::Fill).padding(pad(y0, 0.0, 0.0, x0)).into()
}

/// The mode rows' pickers and the frame time, for the controls tab.
type Rows<'a> = (&'a PlayerPanel, Instant);

struct Column {
    tab: Tab,
    width: f32,
    header_h: f32,
    body_h: f32,
    cover_px: f32,
    open: f32,
    fold: Fold,
    switch: Switch,
}

/// A tab change in flight: the tab it left and the new page's progress (0 just switched, 1 settled).
#[derive(Clone, Copy)]
struct Switch {
    from: Tab,
    page: f32,
}

fn face_column<'a>(view: View<'a>, tokens: Faded, column: Column, swap: f32, rows: Rows<'a>) -> Element<'a, Message> {
    let Column { tab, width, header_h, body_h, cover_px, open, fold, switch } = column;
    let ink = Color { a: open, ..tokens.t1 };
    let header = header(view, tokens, cover_px, header_h, fold, swap);
    // The body keeps its hairline while it closes back into the cover tab; it is cut to its moving height.
    let content: Element<'a, Message> = if tab == Tab::Cover {
        space().into()
    } else {
        let page_ink = Color { a: ink.a * switch.page, ..ink };
        let page = tab_body(view, tokens.faded(switch.page), tab, page_ink, rows);
        let page = if matches!(tab, Tab::Source | Tab::Controls | Tab::Quality) {
            crate::measured::measured(page, rows.0.bodies.get(tab), move |height| Message::Measured(tab, height))
        } else {
            page
        };
        container(page).padding(pad(6.0 * (1.0 - switch.page), 0.0, 0.0, 0.0)).into()
    };
    let body: Element<'a, Message> = if body_h < 0.5 {
        space().into()
    } else {
        column![
            container(space()).height(1.0).width(Length::Fill).style(move |_theme: &IcedTheme| {
                container::Style { background: Some(tokens.line.into()), ..container::Style::default() }
            }),
            space().height(8.0),
            content,
        ]
        .padding(pad(12.0, 0.0, 0.0, 0.0))
        .height(Length::Fixed(body_h))
        .clip(true)
        .into()
    };
    column![header, body, space().height(12.0), dock(view, tokens, (tab, switch), width - 28.0, ink)]
        .spacing(0)
        .padding(12)
        .width(Length::Fixed(width))
        .into()
}

/// Where the song header is between the cover tab's big cover (0) and the other tabs' folded row (1).
#[derive(Clone, Copy)]
struct Fold {
    /// Cover size and header height, `st-t-unfold`.
    shape: f32,
    /// The folded title and heart, `st-t-move`.
    folded: f32,
    /// The big title and the actions under the cover.
    under: f32,
}

/// `SongHeader`: one cover that shrinks from the panel's width to 48px (radius 10 to 8) while the header folds
/// to 48px; the folded title and heart come in from 4px above, the big title and actions leave 8px down.
fn header<'a>(
    view: View<'a>,
    tokens: Faded,
    cover_px: f32,
    header_h: f32,
    fold: Fold,
    swap: f32,
) -> Element<'a, Message> {
    let (title, album) = title_album(view);
    let open = tokens.t1.a;
    let size = cover_px + (FOLDED - cover_px) * fold.shape;
    let cover = cover_box(tokens, view.art, size, 10.0 - 2.0 * fold.shape, 40.0 - 20.0 * fold.shape);
    let cover = button(cover)
        .padding(0)
        .on_press_maybe((fold.shape > 0.5).then_some(Message::Choose(Tab::Cover)))
        .style(|_theme: &IcedTheme, _status| button::Style::default());

    let small = tokens.faded(fold.folded);
    let small_ink = Color { a: open * fold.folded, ..tokens.t1 };
    let lift = -4.0 * (1.0 - fold.folded);
    let lines = column![
        text(title)
            .size(crate::typeface::size::BODY)
            .line_height(LineHeight::Absolute(iced::Pixels(20.0)))
            .font(crate::typeface::adapt(settings_kit::SEMIBOLD))
            .color(small_ink)
            .wrapping(iced::widget::text::Wrapping::None),
        artists(view, small, 12.0, 18.0),
    ];
    let lines = container(lines).width(Length::Fill).clip(true).padding(pad(5.0 + lift, 44.0, 0.0, 60.0));
    let heart = container(heart(view, small, small_ink, false, swap))
        .width(Length::Fill)
        .align_x(Alignment::End)
        .padding(pad(8.0 + lift, 0.0, 0.0, 0.0));
    // Hidden parts are left out, so their buttons take no clicks.
    let folded_row: Element<'a, Message> = if fold.folded > 0.0 { stack![lines, heart].into() } else { space().into() };

    let big = tokens.faded(fold.under);
    let big_ink = Color { a: open * fold.under, ..tokens.t1 };
    let drop = 8.0 * (1.0 - fold.under);
    let credits = column![
        button(
            text(title)
                .size(crate::typeface::size::SUBHEADING)
                .line_height(LineHeight::Absolute(iced::Pixels(26.0)))
                .font(crate::typeface::adapt(settings_kit::SEMIBOLD))
                .color(big_ink)
        )
        .padding(0)
        .on_press_maybe((view.track.is_some() && fold.under > 0.5).then_some(Message::CopySongInfo))
        .style(button::text),
        artists(view, big, 13.0, 20.0),
        text(album)
            .size(crate::typeface::size::HINT)
            .line_height(LineHeight::Absolute(iced::Pixels(18.0)))
            .color(big.t3),
    ];
    let under = column![credits, actions(view, big, big_ink, swap)].padding(pad(14.0, 4.0, 0.0, 4.0));

    // A box `drop` taller than the clipped header, bottom-aligned, moves the block down by `drop`.
    let under: Element<'a, Message> = if fold.under > 0.0 {
        container(under).width(Length::Fill).height(Length::Fixed(header_h + drop)).align_y(Alignment::End).into()
    } else {
        space().into()
    };

    stack![under, folded_row, cover].width(Length::Fill).height(Length::Fixed(header_h)).clip(true).into()
}

fn artists<'a>(view: View<'a>, tokens: Faded, size: f32, leading: f32) -> Element<'a, Message> {
    let raw = view.track.and_then(|track| track.artist.as_deref()).unwrap_or("");
    let mut row = iced::widget::Row::new().spacing(0);
    for (index, name) in raw.split(" / ").filter(|name| !name.is_empty()).enumerate() {
        if index > 0 {
            row = row
                .push(text(" / ").size(size).line_height(LineHeight::Absolute(iced::Pixels(leading))).color(tokens.t3));
        }
        row = row.push(text(name).size(size).line_height(LineHeight::Absolute(iced::Pixels(leading))).color(tokens.t2));
    }
    row.into()
}

fn actions<'a>(view: View<'a>, tokens: Faded, ink: Color, swap: f32) -> Element<'a, Message> {
    let dim = Color { a: 0.4 * ink.a, ..tokens.t2 };
    let mut items = vec![heart(view, tokens, ink, true, swap)];
    if view.track.is_some() {
        let (color, fill, tip, on_press) = match view.add_disabled {
            Some(reason) => (dim, Some(tokens.fill), reason.to_owned(), None),
            None if view.can_add => (
                ink,
                Some(tokens.fill),
                view.i18n.t("localMusic.addToPlaylist").to_owned(),
                Some(Message::AddToPlaylist),
            ),
            None => (dim, Some(tokens.fill), view.i18n.t("localMusic.addToPlaylist").to_owned(), None),
        };
        items.push(icon_btn(tokens, Icon::Star, (17.0, 32.0), color, fill, tip, on_press));
    }
    if let Some(name) = view.origin {
        items.push(source_pill(view, tokens, ink, name, view.pill_hover));
    }
    items.push(space().width(Length::Fill).into());
    items.push(icon_btn(
        tokens,
        Icon::House,
        (17.0, 32.0),
        ink,
        None,
        view.i18n.t("ui.backToHome").to_owned(),
        Some(Message::Home),
    ));
    items.push(icon_btn(
        tokens,
        Icon::Settings,
        (17.0, 32.0),
        ink,
        None,
        view.i18n.t("ui.options").to_owned(),
        Some(Message::Settings),
    ));
    row(items).spacing(6).padding(pad(14.0, 0.0, 0.0, 0.0)).into()
}

/// `SongActions` playing-from pill: 32px high, fully round, `pl-2.5 pr-3 gap-1.5`, Crosshair 14 and 12px text,
/// both centred on the row; hover lifts the fill to `--st-fill2` and the text to `--st-t1`.
fn source_pill<'a>(view: View<'a>, tokens: Faded, ink: Color, name: &'a str, hovered: bool) -> Element<'a, Message> {
    let open = ink.a;
    let tip = view.i18n.format("ui.playingFrom", &[("name", name)]);
    let tint = fade(if hovered { tokens.t1 } else { tokens.t2 }, open);
    let fill = fade(if hovered { tokens.fill2 } else { tokens.fill }, open);
    let mark = container(icon(Icon::Crosshair, 14.0, tint)).center(Length::Fixed(14.0));
    let label = text(name).size(crate::typeface::size::HINT).color(tint).wrapping(iced::widget::text::Wrapping::None);
    let body = row![mark, label].spacing(6).align_y(Alignment::Center);
    let body = container(body).center_y(Length::Fixed(32.0)).padding(pad(0.0, 12.0, 0.0, 10.0)).clip(true);
    let button =
        button(body).padding(0).on_press(Message::Locate).style(move |_theme: &IcedTheme, _status| button::Style {
            background: Some(Background::Color(fill)),
            border: Border { radius: 16.0.into(), ..Border::default() },
            ..button::Style::default()
        });
    let button = mouse_area(button).on_enter(Message::PillHover(true)).on_exit(Message::PillHover(false));
    tooltip(button, text(tip).size(crate::typeface::size::HINT), tooltip::Position::Top).style(tokens.tip()).into()
}

/// The heart's paint and shape: filled on `--pn-like` when liked, hollow on `--st-t2` otherwise, the whole control at
/// 40% out of reach, all multiplied by the panel's fade.
#[derive(Debug, Clone, Copy)]
pub struct LikeLook {
    pub filled: bool,
    pub ink: Color,
}

pub fn like_look(daylight: bool, liked: bool, disabled: bool, open: f32, text: Color) -> LikeLook {
    let mut ink = if liked {
        if daylight { Color::from_rgb8(0xe0, 0x35, 0x4f) } else { Color::from_rgb8(0xf2, 0x60, 0x7a) }
    } else {
        text
    };
    ink.a = (if liked { 1.0 } else { 0.62 }) * open * if disabled { 0.4 } else { 1.0 };
    LikeLook { filled: liked, ink }
}

/// `LikeButton`: a filled heart on `--pn-like` when liked, `--st-t2` and hollow otherwise; `IconSwap` brings the
/// heart that just changed in at 80% and full opacity over 220ms.
fn heart<'a>(view: View<'a>, tokens: Faded, ink: Color, filled: bool, swap: f32) -> Element<'a, Message> {
    let on = view.liked;
    let disabled = view.like_disabled.is_some();
    let look = like_look(view.daylight, on, disabled, ink.a, tokens.t1);
    let color = Color { a: look.ink.a * swap, ..look.ink };
    let glyph = 17.0 * (0.8 + 0.2 * swap);
    let mark: Element<'a, Message> =
        if look.filled { variant(Variant::FilledHeart, glyph, color) } else { icon(Icon::Heart, glyph, color) };
    let tip = match view.like_disabled {
        Some(reason) => reason.to_owned(),
        None => view.i18n.t(if on { "player.unlike" } else { "player.like" }).to_owned(),
    };
    let bg = (on || filled).then_some({
        let fill = tokens.fill;
        Color { a: fill.a * if disabled { 0.4 } else { 1.0 }, ..fill }
    });
    let hot_fill = if filled { tokens.fill2 } else { tokens.fill };
    let mark = container(mark)
        .width(Length::Fixed(32.0))
        .height(Length::Fixed(32.0))
        .align_x(Alignment::Center)
        .align_y(Alignment::Center);
    let button = button(mark).padding(0).on_press_maybe((!disabled).then_some(Message::Like)).style(
        move |_theme: &IcedTheme, status| {
            let hot = !disabled && matches!(status, button::Status::Hovered | button::Status::Pressed);
            button::Style {
                background: bg.or(hot.then_some(hot_fill)).map(Background::Color),
                border: Border { radius: 8.0.into(), ..Border::default() },
                ..button::Style::default()
            }
        },
    );
    tooltip(button, text(tip).size(crate::typeface::size::HINT), tooltip::Position::Top).style(tokens.tip()).into()
}

/// `TabDock`: one raised mark shared by the tabs, sliding from the old tab to the new one (`move`, 220ms).
fn dock<'a>(
    view: View<'a>,
    tokens: Faded,
    (tab, switch): (Tab, Switch),
    width: f32,
    ink: Color,
) -> Element<'a, Message> {
    let local = view.track.is_some_and(|track| track.online.is_none());
    let online = view.track.is_some_and(|track| track.online.is_some());
    let mut tabs = vec![
        (Tab::Cover, view.i18n.t("panel.cover")),
        (Tab::Controls, view.i18n.t("panel.controls")),
        (Tab::Queue, view.i18n.t("panel.playlist")),
        (Tab::Quality, view.i18n.t("panel.account")),
    ];
    if local {
        tabs.insert(1, (Tab::Source, view.i18n.t("localMusic.folder")));
    } else if online {
        tabs.insert(1, (Tab::Source, view.i18n.t("localMusic.lyrics")));
    }
    let index_of = |wanted: Tab| tabs.iter().position(|(id, _)| *id == wanted);
    let to = index_of(tab).unwrap_or(0) as f32;
    let from = index_of(switch.from).map_or(to, |index| index as f32);
    let cell = width / tabs.len() as f32;
    let at = (from + (to - from) * switch.page) * cell;
    let mark = container(space()).width(Length::Fixed(cell)).height(Length::Fill).style(move |_theme: &IcedTheme| {
        container::Style {
            background: Some(tokens.raise.into()),
            border: Border { radius: 8.0.into(), ..Border::default() },
            shadow: iced::Shadow {
                color: Color { a: 0.2 * ink.a, ..Color::BLACK },
                offset: iced::Vector::new(0.0, 1.0),
                blur_radius: 2.0,
            },
            ..container::Style::default()
        }
    });
    let buttons = tabs.into_iter().map(|(id, label)| {
        let on = id == tab;
        button(
            container(text(label).size(crate::typeface::size::CONTROL).color(if on {
                ink
            } else {
                Color { a: ink.a, ..tokens.t2 }
            }))
            .center_x(Length::Fill)
            .center_y(Length::Fill),
        )
        .padding(0)
        .width(Length::Fill)
        .height(Length::Fill)
        .on_press_maybe((!on).then_some(Message::Choose(id)))
        .style(move |_theme: &IcedTheme, _status| button::Style { text_color: ink, ..button::Style::default() })
        .into()
    });
    // A stack takes its first layer's size, so the mark's layer spans the dock and the buttons share its width.
    let mark = container(mark).padding(pad(0.0, 0.0, 0.0, at)).width(Length::Fill).height(Length::Fill);
    let marked = stack![mark, row(buttons).spacing(0).width(Length::Fill).height(Length::Fill)];
    container(marked)
        .padding(2)
        .width(Length::Fill)
        .height(Length::Fixed(36.0))
        .style(move |_theme: &IcedTheme| container::Style {
            background: Some(tokens.fill.into()),
            border: Border { radius: 10.0.into(), ..Border::default() },
            ..container::Style::default()
        })
        .into()
}

fn tab_body<'a>(view: View<'a>, tokens: Faded, tab: Tab, ink: Color, rows: Rows<'a>) -> Element<'a, Message> {
    match tab {
        Tab::Cover => space().into(),
        Tab::Controls => controls(view, tokens, ink, rows),
        Tab::Queue => queue_tab(rows.0, view, tokens, ink),
        Tab::Source => source_tab(view, tokens),
        Tab::Quality => quality_tab(view, tokens, ink, &rows.0.quality, rows.1),
    }
}

fn controls<'a>(view: View<'a>, tokens: Faded, ink: Color, rows: Rows<'a>) -> Element<'a, Message> {
    // `VolumeRow`: a mute shows 0 and keeps the level underneath.
    let shown = if view.muted { 0.0 } else { view.volume };
    let volume_icon = if shown <= 0.0 {
        Icon::VolumeX
    } else if shown < 0.5 {
        Icon::Volume1
    } else {
        Icon::Volume2
    };
    let volume = row![
        icon_btn(
            tokens,
            volume_icon,
            (17.0, 32.0),
            ink,
            None,
            view.i18n.t("ui.volume").to_owned(),
            Some(Message::Mute)
        ),
        slider(0.0..=1.0, shown, Message::Volume)
            .step(0.01_f32)
            .on_release(Message::VolumeDone)
            .width(Length::Fill)
            .style(move |_, _| settings_kit::slider_style(tokens.tokens())),
        container(
            text(format!("{}%", (shown * 100.0).round() as i32))
                .size(crate::typeface::size::HINT)
                .color(Color { a: ink.a, ..tokens.t2 })
        )
        .width(Length::Fixed(38.0))
        .align_x(Alignment::End),
    ]
    .spacing(8)
    .align_y(Alignment::Center)
    .height(Length::Fixed(40.0));
    let loop_label = match view.repeat {
        Repeat::Off => view.i18n.t("player.loopOff"),
        Repeat::All => view.i18n.t("player.loopAll"),
        Repeat::One => view.i18n.t("player.loopOne"),
    };
    let tiles = row![
        tile(tokens, Icon::Repeat, loop_label, view.repeat != Repeat::Off, Some(Message::Repeat)),
        tile(tokens, Icon::Blend, view.i18n.t("ui.automix"), false, None),
        tile(
            tokens,
            Icon::SlidersHorizontal,
            view.i18n.t("ui.equalizerTitle"),
            view.equalizer_on,
            Some(Message::EqualizerOpen)
        ),
    ]
    .spacing(8);
    let (panel, now) = rows;
    let pickers = &panel.pickers;
    let look = mode_picker::Look {
        i18n: view.i18n,
        tokens: tokens.tokens(),
        theme: view.theme,
        font_scale: view.font_scale,
        style_id: view.style_id,
        open: ink.a,
        audio: view.audio,
    };
    let intensity = view.i18n.t(intensity_key(view.theme.intensity));
    let intensity_tip = format!("{}: {}", view.i18n.t("ui.animationIntensity"), intensity);
    let aside = chip(tokens, ink.a, &intensity_tip, intensity, Some(Message::Intensity));
    let style_row = pickers.row(look, mode_picker::Row::Style, Some(aside), now);
    let quick = quick_row(view, tokens, ink);
    // Random: the style this song drew (reroll) and the pool.
    let random = (view.style_id == "random").then(|| {
        let drawn = view.i18n.t(view.drawn_style.label_key());
        let reroll = format!("{}: {}", view.i18n.t("options.randomVisualizerReroll"), drawn);
        let count = view.random_pool.map_or(view.all_styles.len(), <[String]>::len);
        let pool = view.i18n.t("options.randomPoolChoose");
        let pool_tip = format!(
            "{pool}: {}",
            view.i18n.format(
                "options.randomPoolCount",
                &[("selected", &count.to_string()), ("total", &view.all_styles.len().to_string())]
            )
        );
        container(
            row![
                chip(tokens, ink.a, &reroll, drawn, (count > 1).then_some(Message::Reroll)),
                chip(tokens, ink.a, &pool_tip, pool, Some(Message::PoolOpen)),
            ]
            .spacing(6),
        )
        .width(Length::Fill)
        .align_x(Alignment::End)
        .padding(pad(4.0, 0.0, 0.0, 0.0))
    });
    let background = pickers.row(look, mode_picker::Row::Background, None, now);
    let transparent = row![
        container(
            tooltip(
                text(view.i18n.t("options.transparentPlayerBackground"))
                    .size(crate::typeface::size::BODY)
                    .color(fade(tokens.t2, ink.a * 0.4)),
                text(view.i18n.t("options.transparentPlayerBackgroundUnavailable")).size(crate::typeface::size::HINT),
                tooltip::Position::Top,
            )
            .style(tokens.tip())
        )
        .width(Length::Fill),
        off_switch(tokens),
    ]
    .align_y(Alignment::Center)
    .height(Length::Fixed(44.0));
    let track = tokens.fill;
    let custom = view.theme_source == ThemeSource::Custom && view.has_custom_theme;
    let choices = container(
        row![
            source_seg_press(
                tokens,
                ink,
                view.i18n.t("ui.default"),
                view.theme_source == ThemeSource::Default,
                Some(Message::ThemeSource(ThemeSource::Default))
            ),
            source_seg(tokens, ink, view.i18n.t("ui.aiTheme"), false, Some(view.i18n.t("ui.aiThemeUnavailable")),),
            source_seg_press(
                tokens,
                ink,
                view.i18n.t("options.customTheme"),
                custom,
                view.has_custom_theme.then_some(Message::ThemeSource(ThemeSource::Custom))
            ),
        ]
        .spacing(0)
        .width(Length::Fill)
        .height(Length::Fill),
    )
    .width(Length::Fill)
    .height(Length::Fixed(32.0))
    .padding(2)
    .style(move |_theme: &IcedTheme| container::Style {
        background: Some(track.into()),
        border: Border { radius: 8.0.into(), ..Border::default() },
        ..container::Style::default()
    });
    let palette_tip = if custom {
        view.i18n.t("options.customThemeQuickEditTitle").to_owned()
    } else {
        view.i18n.t("options.themeQuickEditUnavailable").to_owned()
    };
    let sources = row![
        choices,
        icon_btn(
            tokens,
            Icon::Palette,
            (15.0, 28.0),
            if custom { fade(tokens.t2, ink.a) } else { fade(tokens.t2, ink.a * 0.4) },
            Some(tokens.fill),
            palette_tip,
            custom.then_some(Message::QuickEditOpen)
        ),
    ]
    .spacing(6)
    .align_y(Alignment::Center);
    let theme_name = if view.theme_source == ThemeSource::Custom {
        view.theme.name.to_string()
    } else if view.daylight {
        view.i18n.t("theme.daylightDefault").to_owned()
    } else {
        view.i18n.t("theme.midnightDefault").to_owned()
    };
    let name_row = row![
        text(theme_name).size(crate::typeface::size::CONTROL).color(fade(tokens.t2, ink.a)).width(Length::Fill),
        day_night(tokens, ink, view.daylight),
        text_btn(view.i18n.t("ui.generateAITheme"), fade(tokens.t2, ink.a * 0.4), None),
    ]
    .spacing(6)
    .align_y(Alignment::Center);
    let mut column = column![volume, tiles, hairline(tokens), style_row];
    if let Some(quick) = quick {
        column = column.push(quick);
    }
    if let Some(random) = random {
        column = column.push(random);
    }
    column.push(background).push(transparent).push(hairline(tokens)).push(sources).push(name_row).into()
}

/// `renderQuickControls`: the style's own chips and toggles under its row.
fn quick_row<'a>(view: View<'a>, tokens: Faded, ink: Color) -> Option<Element<'a, Message>> {
    if view.quick.is_empty() {
        return None;
    }
    let mut row = iced::widget::Row::new().spacing(6);
    for control in view.quick {
        let label = view.i18n.t(control.label_key);
        let tip = format!("{}: {}", view.i18n.t(control.tip_key), label);
        row = row.push(match control.kind {
            QuickKind::Toggle { on } => {
                quick_toggle(tokens, ink.a, control.glyph, on, &tip, Message::Quick(control.key))
            }
            QuickKind::Cycle => chip(tokens, ink.a, &tip, label, Some(Message::Quick(control.key))),
        });
    }
    Some(container(row).width(Length::Fill).align_x(Alignment::End).padding(pad(4.0, 0.0, 0.0, 0.0)).into())
}

fn quick_toggle<'a>(
    tokens: Faded,
    alpha: f32,
    glyph: &str,
    on: bool,
    tip: &str,
    message: Message,
) -> Element<'a, Message> {
    let lucide = match glyph {
        "rotate" => Icon::RotateCw,
        "spline" => Icon::Spline,
        "separator" => Icon::SeparatorVertical,
        "frame" => Icon::Frame,
        "eye-off" => Icon::EyeOff,
        "align-left" => Icon::TextAlignStart,
        "move" => Icon::GripVertical,
        "audio" => Icon::AudioWaveform,
        _ => Icon::Palette,
    };
    let ink = if on { tokens.t1 } else { tokens.t2 };
    let button = button(container(icon(lucide, 14.0, fade(ink, alpha))).center(Length::Fixed(28.0)))
        .width(Length::Fixed(28.0))
        .height(Length::Fixed(28.0))
        .padding(0)
        .on_press(message)
        .style(move |_theme: &IcedTheme, status| {
            let hot = matches!(status, button::Status::Hovered | button::Status::Pressed);
            let fill = if on { tokens.raise } else { tokens.fill };
            button::Style {
                background: Some(Background::Color(fade(if hot { tokens.fill2 } else { fill }, alpha))),
                border: Border { radius: 8.0.into(), ..Border::default() },
                ..button::Style::default()
            }
        });
    tooltip(button, text(tip.to_owned()).size(crate::typeface::size::HINT), tooltip::Position::Top)
        .style(tokens.tip())
        .into()
}

fn queue_tab<'a>(panel: &'a PlayerPanel, view: View<'a>, tokens: Faded, ink: Color) -> Element<'a, Message> {
    if view.queue.is_empty() {
        return text(view.i18n.t("queue.empty"))
            .size(crate::typeface::size::CONTROL)
            .color(Color { a: ink.a, ..tokens.t2 })
            .into();
    }
    let head = row![
        text(view.i18n.t("queue.title"))
            .size(crate::typeface::size::CONTROL)
            .font(crate::typeface::adapt(settings_kit::SEMIBOLD))
            .color(Color { a: ink.a, ..tokens.t2 }),
        text(view.queue.len().to_string()).size(crate::typeface::size::CONTROL).color(Color { a: ink.a, ..tokens.t3 }),
        space().width(Length::Fill),
        text_btn(
            view.i18n.t("localMusic.saveQueueAsPlaylist"),
            if view.can_save_queue { Color { a: ink.a, ..tokens.t2 } } else { Color { a: 0.4 * ink.a, ..tokens.t2 } },
            view.can_save_queue.then_some(Message::SaveQueue)
        ),
        icon_btn(
            tokens,
            Icon::Shuffle,
            (15.0, 28.0),
            if view.can_shuffle { ink } else { Color { a: 0.4 * ink.a, ..tokens.t2 } },
            None,
            if view.queue.len() < 2 {
                view.i18n.t("wallTools.shuffleUnavailable").to_owned()
            } else if !view.can_shuffle {
                view.i18n.t("wallTools.queueUnavailableInFm").to_owned()
            } else {
                view.i18n.t("queue.shuffle").to_owned()
            },
            view.can_shuffle.then_some(Message::Shuffle),
        ),
    ]
    .spacing(6)
    .align_y(Alignment::Center)
    .height(Length::Fixed(36.0));
    let list_h = queue_list_h(view.width, view.height, view.bottom, panel.chrome, view.queue.len());
    let look = queue_island::Look {
        i18n: view.i18n,
        tokens: tokens.tokens(),
        ink,
        daylight: view.daylight,
        actions_on_current: true,
        reorder: false,
        row: queue_island::PANEL_ROW,
    };
    let list = panel.queue.view(look, view.queue, view.index, list_h, Instant::now()).map(Message::Queue);
    column![head, list].spacing(0).into()
}

/// `SourceTab`: where the lyrics come from, the timeline offset, online matching and the lyric file, and the gain.
fn source_tab<'a>(view: View<'a>, tokens: Faded) -> Element<'a, Message> {
    let i18n = view.i18n;
    let online = view.track.is_some_and(|track| track.online.is_some());
    let source = if !view.has_lyrics {
        i18n.t("localMusic.statusNone")
    } else if online {
        view.lyric_provider
    } else {
        i18n.t("localMusic.statusLocal")
    };
    let source =
        text(source).size(crate::typeface::size::CONTROL).color(tokens.t2).wrapping(iced::widget::text::Wrapping::None);
    // The reset keeps its place at 0 so the number beside it does not move.
    let offset_ms = (view.lyric_offset * 1000.0).round() as i64;
    let reset = icon_btn(
        tokens,
        Icon::RotateCcw,
        (15.0, 28.0),
        fade(tokens.t2, if offset_ms == 0 { 0.0 } else { 1.0 }),
        None,
        i18n.t("localMusic.resetLyricTimelineOffset").to_owned(),
        (offset_ms != 0).then_some(Message::Offset(0)),
    );
    let offset = row![reset, stepper(tokens, offset_ms, i18n.t("localMusic.lyricTimelineOffset"))]
        .spacing(6)
        .align_y(Alignment::Center);
    let buttons = row![
        panel_button(
            tokens,
            Icon::Search,
            i18n.t("localMusic.matchOnline"),
            true,
            view.can_match.then_some(Message::MatchOnline)
        ),
        panel_button(tokens, Icon::ArrowUpDown, i18n.t("lyricFile.button"), false, Some(Message::LyricFileOpen)),
    ]
    .spacing(8);
    let gains = [
        ("localMusic.replayGainOff", ReplayGainMode::Off),
        ("localMusic.replayGainAuto", ReplayGainMode::Auto),
        ("localMusic.replayGainTrack", ReplayGainMode::Track),
        ("localMusic.replayGainAlbum", ReplayGainMode::Album),
    ];
    let mut segments = row![].spacing(0);
    for (key, mode) in gains {
        segments =
            segments.push(gain_segment(tokens, i18n.t(key), view.replay_gain == mode, Message::ReplayGain(mode)));
    }
    let segments =
        container(segments.height(Length::Fill)).width(Length::Fill).height(Length::Fixed(32.0)).padding(2).style(
            move |_theme: &IcedTheme| container::Style {
                background: Some(tokens.fill.into()),
                border: Border { radius: 8.0.into(), ..Border::default() },
                ..container::Style::default()
            },
        );
    let hint = match (view.replay_gain, view.gain) {
        (ReplayGainMode::Off, _) => None,
        (_, Some(gain)) => {
            let mut words = Vec::new();
            if let Some(db) = gain.track_db {
                words.push(format!("{} {db:+.1} dB", i18n.t("localMusic.replayGainTrack")));
            }
            if let Some(db) = gain.album_db {
                words.push(format!("{} {db:+.1} dB", i18n.t("localMusic.replayGainAlbum")));
            }
            (!words.is_empty()).then(|| words.join(" · "))
        }
        (_, None) => Some(i18n.t("localMusic.replayGainUnavailable").to_owned()),
    };
    let mut column = column![
        panel_row(tokens, i18n.t("localMusic.lyrics"), source.into()),
        panel_row(tokens, i18n.t("localMusic.lyricTimelineOffset"), offset.into()),
        container(buttons).padding(pad(8.0, 4.0, 8.0, 4.0)),
        container(hairline(tokens)).padding(pad(8.0, 4.0, 8.0, 4.0)),
        panel_row(tokens, i18n.t("localMusic.replayGainTitle"), segments.into()),
    ];
    if let Some(hint) = hint {
        column = column.push(
            container(text(hint).size(crate::typeface::size::HINT).color(tokens.t3))
                .width(Length::Fill)
                .align_x(Alignment::End)
                .padding(pad(0.0, 4.0, 4.0, 4.0)),
        );
    }
    column.into()
}

/// `Segments`' option on the source tab: chosen sits on `--st-raise`, the others take the press.
fn gain_segment<'a>(tokens: Faded, label: &'a str, chosen: bool, message: Message) -> Element<'a, Message> {
    let ink = if chosen { tokens.t1 } else { tokens.t2 };
    button(
        container(
            text(label).size(crate::typeface::size::CONTROL).color(ink).wrapping(iced::widget::text::Wrapping::None),
        )
        .center_x(Length::Fill)
        .center_y(Length::Fill),
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .padding(0)
    .on_press(message)
    .style(move |_theme: &IcedTheme, status| {
        let hot = matches!(status, button::Status::Hovered | button::Status::Pressed);
        button::Style {
            background: chosen
                .then_some(Background::Color(tokens.raise))
                .or_else(|| hot.then_some(Background::Color(tokens.fill2))),
            border: Border { radius: 6.0.into(), ..Border::default() },
            ..button::Style::default()
        }
    })
    .into()
}

/// `PanelRow`: at least 44px, `px-1 py-1.5`, a 14px name on the left and the controls to the right.
fn panel_row<'a>(tokens: Faded, label: &'a str, control: Element<'a, Message>) -> Element<'a, Message> {
    let name = text(label).size(crate::typeface::size::BODY).line_height(iced::Pixels(20.0)).color(tokens.t1);
    let name = container(name).width(Length::Fixed(72.0));
    let control = container(control).width(Length::Fill).align_x(Alignment::End);
    row![name, control]
        .spacing(12)
        .align_y(Alignment::Center)
        .padding(pad(6.0, 4.0, 6.0, 4.0))
        .height(Length::Fixed(44.0))
        .into()
}

/// `Stepper`: a 32px `--st-fill` pill with minus and plus of 250ms, the value in 13px mono and `ms` in `--st-t3`.
fn stepper<'a>(tokens: Faded, value_ms: i64, label: &'a str) -> Element<'a, Message> {
    let step = move |lucide: Icon, to: i64| -> Element<'a, Message> {
        button(container(icon(lucide, 14.0, tokens.t2)).center(Length::Fixed(28.0)))
            .padding(0)
            .on_press(Message::Offset(to))
            .style(move |_theme: &IcedTheme, status| button::Style {
                background: matches!(status, button::Status::Hovered | button::Status::Pressed)
                    .then_some(Background::Color(tokens.fill2)),
                border: Border { radius: 8.0.into(), ..Border::default() },
                ..button::Style::default()
            })
            .into()
    };
    let number = if value_ms > 0 { format!("+{value_ms}") } else { value_ms.to_string() };
    let mono = iced::Font::MONOSPACE;
    let value = text(number).size(crate::typeface::size::CONTROL).font(crate::typeface::adapt(mono)).color(tokens.t1);
    let value = container(value).width(Length::Fixed(48.0)).align_x(Alignment::End);
    let unit =
        container(text("ms").size(crate::typeface::size::HINT).font(crate::typeface::adapt(mono)).color(tokens.t3))
            .padding(pad(0.0, 4.0, 0.0, 3.0));
    let body = row![step(Icon::Minus, value_ms - 250), value, unit, step(Icon::Plus, value_ms + 250)]
        .align_y(Alignment::Center);
    let pill = container(body).center_y(Length::Fixed(32.0)).style(move |_theme: &IcedTheme| container::Style {
        background: Some(tokens.fill.into()),
        border: Border { radius: 8.0.into(), ..Border::default() },
        ..container::Style::default()
    });
    tooltip(pill, text(label).size(crate::typeface::size::HINT), tooltip::Position::Top).style(tokens.tip()).into()
}

/// `PanelButton`: 32px, `rounded-lg`, `--st-fill`, a 14px icon and 13px text; disabled it is at 40%.
fn panel_button<'a>(
    tokens: Faded,
    lucide: Icon,
    label: &'a str,
    grow: bool,
    on_press: Option<Message>,
) -> Element<'a, Message> {
    let enabled = on_press.is_some();
    let ink = if enabled { tokens.t1 } else { fade(tokens.t1, 0.4) };
    let label =
        text(label).size(crate::typeface::size::CONTROL).color(ink).wrapping(iced::widget::text::Wrapping::None);
    let body = row![icon(lucide, 14.0, ink), label].spacing(6).align_y(Alignment::Center);
    let button = button(container(body).center_y(Length::Fixed(32.0)).center_x(Length::Fill))
        .height(Length::Fixed(32.0))
        .padding(pad(0.0, 12.0, 0.0, 12.0))
        .on_press_maybe(on_press)
        .style(move |_theme: &IcedTheme, status| {
            let hot = enabled && matches!(status, button::Status::Hovered | button::Status::Pressed);
            button::Style {
                background: Some(
                    fade(if hot { tokens.fill2 } else { tokens.fill }, if enabled { 1.0 } else { 0.4 }).into(),
                ),
                border: Border { radius: 8.0.into(), ..Border::default() },
                ..button::Style::default()
            }
        });
    if grow { button.width(Length::Fill).into() } else { button.into() }
}

fn quality_tab<'a>(
    view: View<'a>,
    tokens: Faded,
    ink: Color,
    select: &'a quality_select::Select,
    now: Instant,
) -> Element<'a, Message> {
    let account = match (view.platform, view.account) {
        (Some(platform), Some(account)) => account_card(view, platform, account, tokens, ink),
        _ => labeled(
            ink,
            view.i18n.t("account.guestMode"),
            text_btn(view.i18n.t("account.loginOnHome"), ink, Some(Message::Login)),
        ),
    };
    let sync: Element<'a, Message> = if view.account.is_some() {
        let label = if view.syncing { view.i18n.t("account.syncing") } else { view.i18n.t("account.syncData") };
        let button = action_button(tokens, Icon::RefreshCw, label, (!view.syncing).then_some(Message::Sync));
        container(button).padding(pad(8.0, 4.0, 8.0, 4.0)).into()
    } else {
        space().height(0).into()
    };
    let line = container(hairline(tokens)).padding(pad(8.0, 4.0, 8.0, 4.0));
    column![
        account,
        line,
        labeled(
            ink,
            view.i18n.t("account.audioQuality"),
            select.trigger(view.i18n, tokens.tokens(), ink, view.quality, now)
        ),
        sync,
    ]
    .spacing(0)
    .into()
}

/// `AccountTab`'s signed-in card: a 40px face with the platform's mark on its corner, the name and the member crown,
/// `provider · ID: ...`, and the sign-out on the right.
fn account_card<'a>(
    view: View<'a>,
    platform: Platform,
    account: &'a Account,
    tokens: Faded,
    ink: Color,
) -> Element<'a, Message> {
    let face: Element<'a, Message> = match view.account_avatar {
        Some(handle) => image(handle.clone())
            .width(Length::Fixed(40.0))
            .height(Length::Fixed(40.0))
            .border_radius(20.0)
            .opacity(ink.a)
            .into(),
        None => container(icon(Icon::User, 18.0, Color { a: 0.42 * ink.a, ..tokens.t3 }))
            .center(Length::Fixed(40.0))
            .style(move |_theme: &IcedTheme| container::Style {
                background: Some(tokens.fill.into()),
                border: Border { radius: 20.0.into(), ..Border::default() },
                ..container::Style::default()
            })
            .into(),
    };
    let ring =
        container(mark_faded(platform, 16.0, ink.a)).padding(2.0).style(move |_theme: &IcedTheme| container::Style {
            background: Some(tokens.win.into()),
            border: Border { radius: 10.0.into(), ..Border::default() },
            ..container::Style::default()
        });
    let picture = stack![
        container(face)
            .width(Length::Fixed(46.0))
            .height(Length::Fixed(46.0))
            .align_x(Alignment::Start)
            .align_y(Alignment::Start),
        container(ring)
            .width(Length::Fixed(46.0))
            .height(Length::Fixed(46.0))
            .align_x(Alignment::End)
            .align_y(Alignment::End),
    ];
    let warn = if view.daylight { Color::from_rgb8(0xb9, 0x7a, 0x10) } else { Color::from_rgb8(0xe3, 0xa9, 0x49) };
    let mut name = row![
        text(account.nickname.clone())
            .size(crate::typeface::size::BODY)
            .font(crate::typeface::adapt(settings_kit::SEMIBOLD))
            .line_height(crate::typeface::leading::PX_20_0)
            .color(ink)
    ]
    .spacing(6)
    .align_y(Alignment::Center);
    if account.vip {
        name = name.push(icon(Icon::Crown, 14.0, Color { a: ink.a, ..warn }));
    }
    let words = column![
        name,
        text(format!("{} · ID: {}", platform.short_name, account.id))
            .size(crate::typeface::size::HINT)
            .line_height(crate::typeface::leading::PX_18_0)
            .color(tokens.t2),
    ];
    let logout = icon_btn(
        tokens,
        Icon::LogOut,
        (17.0, 32.0),
        ink,
        None,
        view.i18n.t("account.logout").to_owned(),
        Some(Message::SignOut),
    );
    container(row![picture, words, space().width(Length::Fill), logout].spacing(12).align_y(Alignment::Center))
        .height(Length::Fixed(56.0))
        .padding(pad(4.0, 0.0, 4.0, 4.0))
        .into()
}

/// `PanelButton`: 32px, `rounded-lg`, `--st-fill`, a 14px icon and 13px text.
fn action_button<'a>(tokens: Faded, lucide: Icon, label: &'a str, on_press: Option<Message>) -> Element<'a, Message> {
    let enabled = on_press.is_some();
    let ink = if enabled { tokens.t1 } else { fade(tokens.t1, 0.4) };
    let body = row![icon(lucide, 14.0, ink), text(label).size(crate::typeface::size::CONTROL).color(ink)]
        .spacing(6)
        .align_y(Alignment::Center);
    button(container(body).center_y(Length::Fixed(32.0)).center_x(Length::Fill))
        .width(Length::Fill)
        .height(Length::Fixed(32.0))
        .padding(pad(0.0, 12.0, 0.0, 12.0))
        .on_press_maybe(on_press)
        .style(move |_theme: &IcedTheme, status| {
            let hot = matches!(status, button::Status::Hovered | button::Status::Pressed);
            button::Style {
                background: Some((if hot { tokens.fill2 } else { tokens.fill }).into()),
                border: Border { radius: 8.0.into(), ..Border::default() },
                ..button::Style::default()
            }
        })
        .into()
}

/// Where the handle is in its transitions, each 0..1.
#[derive(Clone, Copy)]
struct HandleLook {
    open: bool,
    shown: f32,
    tone: f32,
    hover: f32,
}

/// `PanelHandle`: a 48px round button on the bottom baseline. Closed it shows Settings2 on black 40%
/// (daylight white 70%), open an X on white; the colours cross over in 300ms. Closed it grows to 105% under
/// the pointer from 768px up, and below that it sits half tucked away at 60% until hovered.
fn handle_button<'a>(view: View<'a>, look: HandleLook) -> Element<'a, Message> {
    let edge = if view.width >= 768.0 { 32.0 } else { 16.0 };
    let closed = 1.0 - look.tone;
    let (rest_fill, rest_ink) = if view.daylight {
        (Color { a: 0.7, ..Color::WHITE }, Color::from_rgb8(0x18, 0x18, 0x1b))
    } else {
        (Color { a: 0.4, ..Color::BLACK }, Color::WHITE)
    };
    let (hover_scale, tuck, opacity) = if view.width >= 768.0 {
        (1.0 + 0.05 * look.hover * closed, 0.0, 1.0)
    } else {
        (1.0, 24.0 * (1.0 - look.hover) * closed, 1.0 - 0.4 * (1.0 - look.hover) * closed)
    };
    let alpha = look.shown * opacity;
    let fill = mix_color(rest_fill, Color::WHITE, look.tone);
    let ink = mix_color(rest_ink, Color::BLACK, look.tone);
    // `backdrop-blur-md` is 12px in Tailwind 4. `alpha` fades the blur and the fill together.
    let glass = shader(Plate {
        radius: 24.0,
        fill_origin: [0.0, 0.0],
        fill_size: [48.0, 48.0],
        fill: [fill.r, fill.g, fill.b, fill.a],
        shadow: [0.0; 4],
        shadow_blur: 0.0,
        shadow_y: 0.0,
        shadow_spread: 0.0,
        ring: [0.0; 4],
        backdrop: 12.0,
        fade: alpha,
        saturate: 1.0,
    })
    .width(Length::Fixed(48.0))
    .height(Length::Fixed(48.0));
    let glyph = container(icon(if look.open { Icon::X } else { Icon::Settings2 }, 20.0, Color { a: alpha, ..ink }))
        .center(Length::Fixed(48.0));
    let mark = container(stack![glass, glyph]).width(Length::Fixed(48.0)).height(Length::Fixed(48.0)).style(
        move |_theme: &IcedTheme| container::Style {
            border: Border { radius: 24.0.into(), ..Border::default() },
            shadow: iced::Shadow {
                color: Color { a: 0.1 * alpha, ..Color::BLACK },
                offset: iced::Vector::new(0.0, 10.0),
                blur_radius: 15.0,
            },
            ..container::Style::default()
        },
    );
    let button = button(mark).padding(0).on_press(Message::Toggle).style(|_, _| button::Style::default());
    // `group w-20`: the hover reaches 32px left of the button.
    let group = container(button).width(Length::Fixed(80.0)).align_x(Alignment::End);
    let group = mouse_area(group).on_enter(Message::HandleHover(true)).on_exit(Message::HandleHover(false));
    let scale = (0.92 + 0.08 * look.shown) * hover_scale;
    let shift = iced::Vector::new(20.0 * (1.0 - look.shown) + tuck, 12.0 * (1.0 - look.shown));
    let group: Element<'a, Message> = crate::float_layer::place(group, view.modal, shift, scale);
    container(group)
        .width(Length::Fill)
        .height(Length::Fill)
        .align_x(Alignment::End)
        .align_y(Alignment::End)
        .padding(pad(0.0, edge, view.bottom, 0.0))
        .into()
}

fn mix_color(from: Color, to: Color, t: f32) -> Color {
    Color {
        r: from.r + (to.r - from.r) * t,
        g: from.g + (to.g - from.g) * t,
        b: from.b + (to.b - from.b) * t,
        a: from.a + (to.a - from.a) * t,
    }
}

fn cover_box<'a>(
    tokens: Faded,
    art: Option<&'a iced::widget::image::Handle>,
    px: f32,
    radius: f32,
    glyph: f32,
) -> Element<'a, Message> {
    let face: Element<'a, Message> = if let Some(art) = art {
        image(art.clone())
            .width(Length::Fixed(px))
            .height(Length::Fixed(px))
            .content_fit(iced::ContentFit::Cover)
            .border_radius(radius)
            // The tokens carry the surface's fade; `--st-t1` is opaque at rest.
            .opacity(tokens.t1.a)
            .into()
    } else {
        container(icon(Icon::Disc, glyph, tokens.t3))
            .width(Length::Fixed(px))
            .height(Length::Fixed(px))
            .align_x(Alignment::Center)
            .align_y(Alignment::Center)
            .into()
    };
    container(face)
        .width(Length::Fixed(px))
        .height(Length::Fixed(px))
        .style(move |_theme: &IcedTheme| container::Style {
            background: Some(tokens.fill.into()),
            border: Border { radius: radius.into(), width: 1.0, color: tokens.line },
            ..container::Style::default()
        })
        .into()
}

fn tile<'a>(tokens: Faded, lucide: Icon, label: &'a str, lit: bool, on_press: Option<Message>) -> Element<'a, Message> {
    let enabled = on_press.is_some();
    let (icon_color, label_color) = if lit { (tokens.accent, tokens.t1) } else { (tokens.t3, tokens.t2) };
    let icon_color = if enabled { icon_color } else { fade(icon_color, 0.4) };
    let label_color = if enabled { label_color } else { fade(label_color, 0.4) };
    let paint = tokens.tokens();
    button(
        column![
            container(icon(lucide, 18.0, icon_color)).width(Length::Fill).align_x(Alignment::Start),
            text(label).size(crate::typeface::size::HINT).color(label_color),
        ]
        .spacing(8)
        .padding(pad(10.0, 10.0, 9.0, 10.0)),
    )
    .width(Length::Fill)
    .height(Length::Fixed(64.0))
    .padding(0)
    .on_press_maybe(on_press)
    .style(move |_theme: &IcedTheme, status| {
        let hot = enabled && matches!(status, button::Status::Hovered | button::Status::Pressed);
        button::Style {
            background: Some(Background::Color(settings_kit::quick_tile_fill(paint, lit, enabled, hot))),
            border: Border { radius: 10.0.into(), ..Border::default() },
            ..button::Style::default()
        }
    })
    .into()
}

fn labeled<'a>(ink: Color, label: &'a str, control: Element<'a, Message>) -> Element<'a, Message> {
    row![text(label).size(crate::typeface::size::BODY).color(ink).width(Length::Fill), control]
        .spacing(12)
        .align_y(Alignment::Center)
        .padding(pad(6.0, 4.0, 6.0, 4.0))
        .height(Length::Fixed(44.0))
        .into()
}

fn text_btn<'a>(label: &'a str, color: Color, on_press: Option<Message>) -> Element<'a, Message> {
    button(text(label).size(crate::typeface::size::CONTROL).color(color))
        .padding(pad(8.0, 12.0, 8.0, 12.0))
        .on_press_maybe(on_press)
        .style(button::text)
        .into()
}

/// `ModeStepperRow`: arrows at the two ends, the mode in the middle, an optional chip before the right arrow.
/// `More settings` lives in the picture picker (`QuickEffectPicker`), which is not drawn here.
fn hairline<'a>(tokens: Faded) -> Element<'a, Message> {
    container(space())
        .width(Length::Fill)
        .height(1.0)
        .style(move |_theme: &IcedTheme| container::Style {
            background: Some(tokens.line.into()),
            ..container::Style::default()
        })
        .into()
}

fn icon_btn<'a>(
    tokens: Faded,
    lucide: Icon,
    (glyph, box_px): (f32, f32),
    color: Color,
    fill: Option<Color>,
    tip: String,
    on_press: Option<Message>,
) -> Element<'a, Message> {
    let mark = container(icon(lucide, glyph, color))
        .width(Length::Fixed(box_px))
        .height(Length::Fixed(box_px))
        .align_x(Alignment::Center)
        .align_y(Alignment::Center);
    let button = button(mark).padding(0).on_press_maybe(on_press).style(move |_theme: &IcedTheme, status| {
        let hot = matches!(status, button::Status::Hovered | button::Status::Pressed);
        button::Style {
            background: fill.or(hot.then_some(tokens.tint(color, 0.08))).map(Background::Color),
            border: Border { radius: 8.0.into(), ..Border::default() },
            ..button::Style::default()
        }
    });
    tooltip(button, text(tip).size(crate::typeface::size::HINT), tooltip::Position::Top).style(tokens.tip()).into()
}

fn intensity_key(intensity: folia_core::AnimationIntensity) -> &'static str {
    match intensity {
        folia_core::AnimationIntensity::Calm => "animation.calm",
        folia_core::AnimationIntensity::Normal => "animation.normal",
        folia_core::AnimationIntensity::Chaotic => "animation.chaotic",
    }
}

/// `QuickControlChip`: 28px high, `rounded-lg`, `px-2.5`, 12px; `--st-fill` with `--st-t2`, hover `--st-fill2`
/// with `--st-t1`, disabled at 40%.
fn chip<'a>(tokens: Faded, alpha: f32, tip: &str, label: &'a str, on_press: Option<Message>) -> Element<'a, Message> {
    let alpha = if on_press.is_some() { alpha } else { alpha * 0.4 };
    let enabled = on_press.is_some();
    let button = button(
        container(text(label).size(crate::typeface::size::HINT).wrapping(iced::widget::text::Wrapping::None))
            .center_y(Length::Fixed(28.0)),
    )
    .height(Length::Fixed(28.0))
    .padding(pad(0.0, 10.0, 0.0, 10.0))
    .on_press_maybe(on_press)
    .style(move |_theme: &IcedTheme, status| {
        let hot = enabled && matches!(status, button::Status::Hovered | button::Status::Pressed);
        let (fill, ink) = if hot { (tokens.fill2, tokens.t1) } else { (tokens.fill, tokens.t2) };
        button::Style {
            background: Some(Background::Color(fade(fill, alpha))),
            text_color: fade(ink, alpha),
            border: Border { radius: 8.0.into(), ..Border::default() },
            ..button::Style::default()
        }
    });
    tooltip(button, text(tip.to_owned()).size(crate::typeface::size::HINT), tooltip::Position::Top)
        .style(tokens.tip())
        .into()
}

fn source_seg_press<'a>(
    tokens: Faded,
    ink: Color,
    label: &'a str,
    on: bool,
    on_press: Option<Message>,
) -> Element<'a, Message> {
    let label =
        text(label).size(crate::typeface::size::CONTROL).color(if on { ink } else { Color { a: ink.a, ..tokens.t2 } });
    button(container(label).center_x(Length::Fill).center_y(Length::Fill))
        .width(Length::Fill)
        .height(Length::Fill)
        .padding(0)
        .on_press_maybe(on_press)
        .style(move |_theme: &IcedTheme, status| {
            let hot = matches!(status, button::Status::Hovered | button::Status::Pressed);
            button::Style {
                background: Some(Background::Color(if on {
                    tokens.raise
                } else if hot {
                    tokens.fill2
                } else {
                    Color::TRANSPARENT
                })),
                border: Border { radius: 6.0.into(), ..Border::default() },
                ..button::Style::default()
            }
        })
        .into()
}

fn source_seg<'a>(tokens: Faded, ink: Color, label: &'a str, on: bool, tip: Option<&'a str>) -> Element<'a, Message> {
    let label =
        text(label).size(crate::typeface::size::CONTROL).color(if on { ink } else { fade(tokens.t2, ink.a * 0.4) });
    let button = button(container(label).center_x(Length::Fill).center_y(Length::Fill))
        .width(Length::Fill)
        .height(Length::Fill)
        .padding(0)
        .on_press_maybe(None)
        .style(move |_theme: &IcedTheme, _status| button::Style {
            background: on.then_some(Background::Color(tokens.raise)),
            border: Border { radius: 6.0.into(), ..Border::default() },
            ..button::Style::default()
        });
    match tip {
        Some(tip) => tooltip(button, text(tip).size(crate::typeface::size::HINT), tooltip::Position::Top)
            .style(tokens.tip())
            .into(),
        None => button.into(),
    }
}

fn day_night<'a>(tokens: Faded, ink: Color, daylight: bool) -> Element<'a, Message> {
    let track = tokens.fill;
    let one = |lucide: Icon, on: bool, message: Message| {
        button(container(icon(lucide, 15.0, if on { ink } else { fade(tokens.t2, ink.a) })).center(Length::Fill))
            .width(Length::Fixed(30.0))
            .height(Length::Fill)
            .padding(0)
            .on_press(message)
            .style(move |_theme: &IcedTheme, _status| button::Style {
                background: on.then_some(Background::Color(tokens.raise)),
                border: Border { radius: 6.0.into(), ..Border::default() },
                ..button::Style::default()
            })
    };
    let choices = row![
        one(Icon::Sun, daylight, Message::Theme(ThemeMode::Light)),
        one(Icon::Moon, !daylight, Message::Theme(ThemeMode::Dark))
    ]
    .spacing(0)
    .height(Length::Fill);
    container(choices)
        .height(Length::Fixed(28.0))
        .padding(2)
        .style(move |_theme: &IcedTheme| container::Style {
            background: Some(track.into()),
            border: Border { radius: 8.0.into(), ..Border::default() },
            ..container::Style::default()
        })
        .into()
}

fn off_switch<'a>(tokens: Faded) -> Element<'a, Message> {
    let knob = container(space()).width(18).height(18).style(move |_theme: &IcedTheme| container::Style {
        background: Some(tokens.knob.into()),
        border: Border { radius: 9.0.into(), ..Border::default() },
        ..container::Style::default()
    });
    container(knob)
        .width(40)
        .height(24)
        .padding(pad(3.0, 0.0, 0.0, 3.0))
        .style(move |_theme: &IcedTheme| container::Style {
            background: Some(fade(tokens.track, 0.4).into()),
            border: Border { radius: 12.0.into(), ..Border::default() },
            ..container::Style::default()
        })
        .into()
}

fn fade(color: Color, open: f32) -> Color {
    Color { a: color.a * open, ..color }
}

fn title_album<'a>(view: View<'a>) -> (&'a str, &'a str) {
    match view.track {
        Some(track) => (track.title.as_str(), track.album.as_deref().unwrap_or("")),
        None => (view.i18n.t("ui.noTrack"), ""),
    }
}

fn shown_tab(tab: Tab, local: bool, online: bool) -> Tab {
    if tab == Tab::Source && !local && !online { Tab::Cover } else { tab }
}

/// `measured` is the tab body's laid-out height; 0 falls back to the estimate below.
fn measure(tab: Tab, queue_len: usize, signed: bool, inner: f32, max_h: f32, measured: f32) -> (f32, f32, f32) {
    let under = 14.0 + 26.0 + 20.0 + 18.0 + 14.0 + 32.0;
    let room = (max_h - 24.0 - 12.0 - 36.0).max(under + 96.0);
    let cover_px = (room - under).clamp(96.0, inner);
    if tab == Tab::Cover {
        return (cover_px + under, 0.0, cover_px);
    }
    let body = match tab {
        _ if measured > 0.0 => measured,
        Tab::Controls => 460.0,
        Tab::Queue => 36.0 + queue_island::list_height(queue_len),
        Tab::Source => 210.0,
        Tab::Quality => quality_body(signed),
        Tab::Cover => 0.0,
    };
    let chrome = 24.0 + 48.0 + body + 12.0 + 36.0;
    let body = if chrome > max_h { (body - (chrome - max_h)).max(48.0) } else { body };
    (48.0, 12.0 + 1.0 + 8.0 + body, cover_px)
}

/// `AccountTab`: the account block, the group line's 8px above and below, the 44px quality row and the
/// 48px sync row.
fn quality_body(signed: bool) -> f32 {
    let account = if signed { 56.0 } else { 44.0 };
    let sync = if signed { 48.0 } else { 0.0 };
    account + 17.0 + 44.0 + sync
}

/// The queue list's height: what `measure` leaves the queue tab under its 36px head.
fn queue_list_h(width: f32, height: f32, bottom: f32, chrome: bool, len: usize) -> f32 {
    let under = if chrome { HANDLE_ROOM } else { BARE_GAP };
    let panel_w = (width - 32.0).clamp(200.0, 360.0);
    let inner = (panel_w - 24.0).max(96.0);
    let max_h = (height - under - bottom - TOP_MARGIN).max(220.0);
    let (_, body, _) = measure(Tab::Queue, len, false, inner, max_h, 0.0);
    queue_island::list_height(len).min((body - 21.0 - 36.0).max(0.0))
}

fn tokens_of(view: &View<'_>) -> Tokens {
    Tokens::for_panel(view.theme, view.daylight)
}

fn pad(top: f32, right: f32, bottom: f32, left: f32) -> iced::Padding {
    iced::Padding { top, right, bottom, left }
}

/// Where a mode row's trigger is in the face: padding 12, the folded header, the body's gap, hairline and
/// gap, then volume 40, tiles 64 and the hairline with their 8px gaps, and the row's own `py-1`.
fn picker_place(view: &View<'_>, row: Option<mode_picker::Row>, size: Size, header_h: f32) -> mode_picker::Place {
    let style_top = 12.0 + header_h + 21.0 + 40.0 + 8.0 + 64.0 + 8.0 + 1.0 + 8.0 + 4.0;
    let params = if view.style_id == "random" { mode_picker::PARAMS_H + 8.0 } else { 0.0 };
    let top = match row {
        Some(mode_picker::Row::Background) => style_top + mode_picker::ROW_H + 8.0 + params,
        _ => style_top,
    };
    mode_picker::Place {
        width: size.width,
        height: size.height,
        trigger_top: top,
        trigger_bottom: top + 48.0,
        origin: (0.0, 0.0),
    }
}
