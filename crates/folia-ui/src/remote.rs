// Companion window state and commands; playback remains owned by Shell.

use std::time::{Duration, Instant};

use folia_render::desktop_window::{Capabilities, Geometry};
pub use folia_store::RemoteChrome as Chrome;
use folia_store::{RemoteSettings, RemoteSource};
use iced::{Point, Rectangle, Size, Task, window};

use crate::motion::Tween;
pub use crate::queue_island::Message as QueueMessage;
pub use crate::remote_lyrics::{Rows as LyricRows, line_at, rows as lyric_rows};

#[derive(Debug, Clone)]
pub enum Message {
    Open,
    Close,
    OpenSettings,
    Unlock,
    Background,
    Pin,
    Taskbar(bool),
    Titlebar(bool),
    ClickThrough(bool),
    ExpandCover(bool),
    TextShade(bool),
    Pairs(u8),
    Source(usize, RemoteSource),
    ExportAppearance,
    ImportAppearance,
    Imported(Option<String>),
    Loaded(Box<RemoteSettings>),
    Saved(Result<(), String>),
    SlidersFinished,
    Attached(window::Id, u64),
    Event(window::Id, iced::Event, iced::event::Status),
    Geometry(window::Id, u64),
    Idle(window::Id, u64),
    QueueFolded(window::Id, u64),
    QueueShift(window::Id, u64, bool),
    QueueHidden(window::Id, u64, bool),
    WashReady(window::Id, u64, Result<iced::widget::image::Handle, String>),
    Drag,
    Resize(window::Direction),
    Queue,
    Query(String),
    QueueList(QueueMessage),
    QueueAction(QueueAction, usize, i64),
    More,
    Tab(Tab),
    MainVisible,
    MainTransparent(bool),
    MainBorder(bool),
    MainThrough(bool),
    MainPin(bool),
    MainChrome(Chrome),
    Preset(usize),
    Width(String),
    Height(String),
    SaveSize,
    Like,
    Shuffle,
    Volume,
    Hover(Option<Preview>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QueueAction {
    Play,
    Next,
    End,
    Remove,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Window,
    Size,
    Export,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Preview {
    Previous,
    Next,
    Like,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Expansion {
    pub up: bool,
    pub height: f32,
}

#[derive(Clone, Copy)]
pub(crate) enum Shift {
    Fading,
    Hidden { open: bool, frames: u8 },
    Revealing(u8),
}

pub(crate) struct QueueGeometry {
    pub position: Option<Point>,
    pub size: Size,
    pub expansion: Option<Expansion>,
}

pub(crate) fn queue_geometry(geometry: &Geometry, old: Option<Expansion>, open: bool, wanted: f32) -> QueueGeometry {
    let scale = geometry.scale;
    let position = geometry.position.map(|position| {
        Point::new(position.x, position.y + old.filter(|old| old.up).map_or(0.0, |old| old.height * scale))
    });
    let height = geometry.size.height - old.map_or(0.0, |old| old.height);
    let expansion = open.then(|| {
        queue_expansion(
            Rectangle::new(
                position.map_or(Point::ORIGIN, |p| Point::new(p.x / scale, p.y / scale)),
                Size::new(geometry.size.width, height),
            ),
            geometry.current_work_area.map(|area| Rectangle {
                x: area.x / scale,
                y: area.y / scale,
                width: area.width / scale,
                height: area.height / scale,
            }),
            wanted,
            old.map(|old| old.up),
        )
    });
    QueueGeometry {
        position: position
            .map(|p| Point::new(p.x, p.y - expansion.filter(|next| next.up).map_or(0.0, |next| next.height * scale))),
        size: Size::new(geometry.size.width, height + expansion.map_or(0.0, |next| next.height)),
        expansion,
    }
}

/// Chooses the side with room for a search field and three rows, preserving an existing anchor.
pub fn queue_expansion(card: Rectangle, work: Option<Rectangle>, wanted: f32, preferred: Option<bool>) -> Expansion {
    let Some(work) = work else { return Expansion { up: false, height: wanted } };
    let down = work.y + work.height - card.y - card.height;
    let up = card.y - work.y;
    let least = wanted.min(160.0);
    let upwards = match preferred {
        Some(true) if up >= least => true,
        Some(false) if down >= least => false,
        _ if down >= wanted => false,
        _ if up >= wanted => true,
        _ => up > down,
    };
    Expansion { up: upwards, height: wanted.min(if upwards { up } else { down }.max(least)) }
}

/// Rejects unplugged-monitor positions and keeps the visible card inside the destination work area.
pub fn restored_position(settings: &RemoteSettings, geometry: &Geometry) -> Option<Point> {
    if !geometry.capabilities.position {
        return None;
    }
    let size = Size::new(settings.size[0] * geometry.scale, settings.size[1] * geometry.scale);
    let inset = if settings.appearance.expand_cover { 0.0 } else { 174.0 * geometry.scale };
    if let Some([x, y]) = settings.position {
        let drawn = Rectangle::new(Point::new(x as f32 + inset, y as f32), Size::new(size.width - inset, size.height));
        if geometry.work_areas.iter().any(|area| {
            drawn.x >= area.x
                && drawn.y >= area.y
                && drawn.x + drawn.width <= area.x + area.width
                && drawn.y + drawn.height <= area.y + area.height
        }) {
            return Some(Point::new(x as f32, y as f32));
        }
    }
    geometry.current_work_area.or_else(|| geometry.work_areas.first().copied()).map(|area| {
        let strip = 174.0 * geometry.scale;
        let min_x = area.x - inset;
        Point::new(
            (area.x + (area.width - size.width + strip) * 0.5 - strip)
                .clamp(min_x, (area.x + area.width - size.width).max(min_x)),
            area.y + ((area.height - size.height) * 0.5).max(0.0),
        )
    })
}

pub(crate) struct State {
    pub id: Option<window::Id>,
    pub raw_id: Option<u64>,
    pub settings: RemoteSettings,
    pub capabilities: Capabilities,
    pub loaded: bool,
    pub deferred: Vec<Message>,
    pub open_pending: bool,
    pub saving: bool,
    pub close_application: bool,
    pub dirty: bool,
    pub geometry_serial: u64,
    pub idle_serial: u64,
    pub queue_serial: u64,
    pub queue_target: bool,
    pub queue_reveal: Tween,
    pub window_fade: Tween,
    pub shift: Option<Shift>,
    pub typing: bool,
    pub error_at: Option<Instant>,
    pub icon: Option<window::Icon>,
    pub expansion: Option<Expansion>,
    pub query: String,
    pub queue: crate::queue_island::QueueList,
    pub queue_revision: u64,
    pub sheet: Option<Tab>,
    pub volume: bool,
    pub width: String,
    pub height: String,
    pub main_border: bool,
    pub main_through: bool,
    pub main_pin: bool,
    pub main_chrome: Chrome,
    pub pointer: Option<Point>,
    pub drag_from: Option<Point>,
    pub held: bool,
    pub seek_drag: bool,
    pub volume_drag: bool,
    pub finishing_sliders: u32,
    pub focused: bool,
    pub left_at: Option<Instant>,
    pub hover: Option<(Preview, Instant)>,
    pub idle: Tween,
    pub lit: Tween,
    pub frame_time: Instant,
    pub current_face: Option<(folia_core::Track, Option<iced::widget::image::Handle>)>,
    pub outgoing: Option<(folia_core::Track, Option<iced::widget::image::Handle>)>,
    pub wash: Option<iced::widget::image::Handle>,
    pub wash_outgoing: Option<iced::widget::image::Handle>,
    pub wash_fade: Tween,
    pub wash_source: Option<iced::widget::image::Handle>,
    pub wash_serial: u64,
    pub shape: Vec<Rectangle>,
    pub shape_scale: f64,
    pub lyric: crate::remote_lyrics::Presentation,
    pub handoff: Tween,
}

impl Default for State {
    fn default() -> Self {
        let now = Instant::now();
        Self {
            id: None,
            raw_id: None,
            settings: RemoteSettings::default(),
            capabilities: Capabilities::default(),
            loaded: false,
            deferred: Vec::new(),
            open_pending: false,
            saving: false,
            close_application: false,
            dirty: false,
            geometry_serial: 0,
            idle_serial: 0,
            queue_serial: 0,
            queue_target: false,
            queue_reveal: Tween::new(0.0, 340.0, folia_visualizer::Ease::new(0.2, 0.8, 0.2, 1.0), now),
            window_fade: Tween::new(1.0, 140.0, folia_visualizer::Ease::OUT, now),
            shift: None,
            typing: false,
            error_at: None,
            icon: None,
            expansion: None,
            query: String::new(),
            queue: crate::queue_island::QueueList::compact(),
            queue_revision: u64::MAX,
            sheet: None,
            volume: false,
            width: "1920".into(),
            height: "1080".into(),
            main_border: true,
            main_through: false,
            main_pin: false,
            main_chrome: Chrome::Auto,
            pointer: None,
            drag_from: None,
            held: false,
            seek_drag: false,
            volume_drag: false,
            finishing_sliders: 0,
            focused: false,
            left_at: None,
            hover: None,
            idle: Tween::new(0.0, 620.0, folia_visualizer::Ease::new(0.32, 0.72, 0.0, 1.0), now),
            lit: Tween::new(1.0, 200.0, folia_visualizer::Ease::OUT, now),
            handoff: Tween::new(1.0, 300.0, folia_visualizer::Ease::new(0.42, 0.0, 0.58, 1.0), now),
            frame_time: now,
            current_face: None,
            outgoing: None,
            wash: None,
            wash_outgoing: None,
            wash_fade: Tween::new(1.0, 300.0, folia_visualizer::Ease::new(0.42, 0.0, 0.58, 1.0), now),
            wash_source: None,
            wash_serial: 0,
            shape: Vec::new(),
            shape_scale: 0.0,
            lyric: crate::remote_lyrics::Presentation::default(),
        }
    }
}

impl State {
    pub fn opacity(&self) -> f32 {
        self.window_fade.value(self.frame_time)
    }
    pub fn reset_presentation(&mut self) {
        let now = Instant::now();
        self.idle.snap(0.0, now);
        self.lit.snap(1.0, now);
        self.handoff.snap(1.0, now);
        self.wash_fade.snap(1.0, now);
        self.queue_reveal.snap(0.0, now);
        self.window_fade.snap(1.0, now);
        self.shift = None;
        self.drag_from = None;
        self.held = false;
        self.focused = false;
        self.hover = None;
        self.typing = false;
        self.volume = false;
        self.query.clear();
        self.queue = crate::queue_island::QueueList::compact();
        self.left_at = Some(now);
        self.frame_time = now;
        self.queue_serial = self.queue_serial.wrapping_add(1);
    }
    pub fn active(&self) -> bool {
        self.raw_id
            .and_then(folia_render::backend::native_window)
            .is_some_and(|window| window.is_visible() != Some(false) && window.is_minimized() != Some(true))
    }

    pub fn animating(&self, now: Instant) -> bool {
        self.id.is_some()
            && (self.idle.running(self.frame_time)
                || self.lyric.animating(self.frame_time)
                || self.expansion.is_some() && self.queue.animating(now)
                || self.queue_reveal.running(self.frame_time)
                || self.window_fade.running(self.frame_time)
                || self.shift.is_some()
                || self.lit.running(self.frame_time)
                || self.handoff.running(self.frame_time)
                || self.wash_fade.running(self.frame_time)
                || self.left_at.is_some_and(|at| now.saturating_duration_since(at) < Duration::from_millis(2800))
                || self.hover.is_some_and(|(_, at)| now.saturating_duration_since(at) < Duration::from_millis(400)))
    }

    pub fn frame(&mut self, now: Instant) {
        self.frame_time = now;
        let resting = self.settings.click_through
            || (!self.held
                && !(self.focused && self.typing)
                && self.pointer.is_none()
                && self.left_at.is_none_or(|at| now.saturating_duration_since(at) >= Duration::from_millis(800)));
        let idle = resting && self.sheet.is_none() && !self.queue_target;
        self.idle.set_with(
            f32::from(idle),
            if idle { 620.0 } else { 320.0 },
            if idle {
                folia_visualizer::Ease::new(0.32, 0.72, 0.0, 1.0)
            } else {
                folia_visualizer::Ease::new(0.2, 0.8, 0.2, 1.0)
            },
            now,
        );
        self.lit.set(f32::from(!resting), now);
        if !self.handoff.running(now) {
            self.outgoing = None;
        }
        if !self.wash_fade.running(now) {
            self.wash_outgoing = None;
        }
    }

    pub fn persist(&mut self, store: &folia_store::Store) -> Task<crate::Message> {
        self.loaded = true;
        self.settings.main_border = self.main_border;
        self.settings.main_pin = self.main_pin;
        self.settings.main_chrome = self.main_chrome;
        self.dirty = true;
        if self.saving {
            return Task::none();
        }
        self.dirty = false;
        self.saving = true;
        let store = store.clone();
        let settings = self.settings.clone();
        Task::perform(
            crate::background::blocking(move || store.save(&settings).map_err(|error| crate::error_chain(&error))),
            |result| {
                crate::Message::Remote(Message::Saved(
                    result.unwrap_or_else(|| Err("remote settings worker stopped".into())),
                ))
            },
        )
    }
}

pub(crate) fn load(store: folia_store::Store) -> Task<crate::Message> {
    Task::perform(
        crate::background::blocking(move || {
            let mut settings: RemoteSettings = store.load_or_default();
            settings.normalize();
            settings
        }),
        |settings| {
            crate::Message::Remote(Message::Loaded(Box::new(settings.unwrap_or_else(|| {
                tracing::warn!("remote settings worker stopped; using defaults");
                RemoteSettings::default()
            }))))
        },
    )
}
