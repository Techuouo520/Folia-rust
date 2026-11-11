// The developer page and its two overlays: the session log, the log file switches and the memory monitor.

use crate::typeface::text;
use std::collections::BTreeSet;
use std::path::Path as FilePath;
use std::time::Instant;

use folia_i18n::I18n;
use folia_store::{DeveloperSettings, LogLine, LogMode, MemorySample, RuntimeLog, clock};
use iced::mouse::Cursor;
use iced::widget::canvas::{self, Canvas, Frame, Path as CanvasPath, Stroke};
use iced::widget::{button, column, container, mouse_area, pin, row, scrollable, space};
use iced::{Alignment, Background, Border, Color, Element, Length, Padding, Point, Rectangle, Shadow, Size, Vector};
use lucide_static_svg::Icon;
use tracing::Level;

use crate::settings_kit::{self as kit, Group, Row, Tokens};

const LOG_HEIGHT: f32 = 352.0;
const OVERLAY_LOG_HEIGHT: f32 = 416.0;
const CHART_HEIGHT: f32 = 120.0;
/// `DraggableDebugWindow`: `top-4 right-4`, and at least this much stays on screen.
const ANCHOR: f32 = 16.0;
const EDGE_MARGIN: f32 = 48.0;
/// `MemoryMonitorWindow`'s `defaultOffset`, so it does not open on top of the log.
const MEMORY_OFFSET: Vector = Vector::new(-32.0, 56.0);

/// One of the two debug windows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Panel {
    Log,
    Memory,
}

impl Panel {
    /// The window's width for a window `width` wide: the log is 544 at most, the monitor 512.
    fn width(self, width: f32) -> f32 {
        let most = if self == Panel::Log { 544.0 } else { 512.0 };
        width.min(most) - 24.0
    }
}

/// A title-bar drag: where the pointer and the window's offset were when it started.
#[derive(Debug, Clone, Copy)]
struct Drag {
    panel: Panel,
    from: Point,
    offset: Vector,
}

/// What the two developer panels show and which parts the reader has narrowed down.
#[derive(Debug, Default)]
pub struct Developer {
    pub log_open: bool,
    pub memory_open: bool,
    pub lines: Vec<LogLine>,
    pub hidden_targets: BTreeSet<String>,
    pub samples: Vec<MemorySample>,
    pub latest: Option<MemorySample>,
    pub hidden_series: BTreeSet<Series>,
    pub copied: Option<Instant>,
    /// Offsets from the top-right anchor once dragged; `None` is the window's default place.
    log_offset: Option<Vector>,
    memory_offset: Option<Vector>,
    cursor: Option<Point>,
    drag: Option<Drag>,
}

impl Developer {
    pub fn refresh_log(&mut self, log: &RuntimeLog) {
        self.lines = log.entries();
    }

    pub fn refresh_memory(&mut self, history: folia_store::MemoryHistory) {
        self.samples = history.points;
        self.latest = history.latest;
    }

    /// The lines the module filter leaves on screen.
    pub fn visible(&self) -> impl DoubleEndedIterator<Item = &LogLine> + '_ {
        self.lines.iter().filter(|line| !self.hidden_targets.contains(&line.target))
    }

    pub fn visible_text(&self) -> String {
        self.visible().map(LogLine::text).collect::<Vec<_>>().join("\n")
    }

    fn offset(&self, panel: Panel) -> Vector {
        match panel {
            Panel::Log => self.log_offset.unwrap_or(Vector::ZERO),
            Panel::Memory => self.memory_offset.unwrap_or(MEMORY_OFFSET),
        }
    }

    /// The pointer pressed on a title bar.
    pub fn grab(&mut self, panel: Panel) {
        if let Some(from) = self.cursor {
            self.drag = Some(Drag { panel, from, offset: self.offset(panel) });
        }
    }

    /// The pointer moved; a dragged window follows, kept `EDGE_MARGIN` on screen (`settle`).
    pub fn pointer(&mut self, at: Point, window: Size) {
        self.cursor = Some(at);
        let Some(drag) = self.drag else { return };
        let width = drag.panel.width(window.width);
        let moved = drag.offset + (at - drag.from);
        let right = window.width - ANCHOR;
        let x = moved.x.clamp(EDGE_MARGIN - right, window.width - EDGE_MARGIN - (right - width));
        let y = moved.y.clamp(-ANCHOR, (window.height - EDGE_MARGIN - ANCHOR).max(-ANCHOR));
        let offset = Some(Vector::new(x, y));
        match drag.panel {
            Panel::Log => self.log_offset = offset,
            Panel::Memory => self.memory_offset = offset,
        }
    }

    pub fn release(&mut self) {
        self.drag = None;
    }

    fn copied(&self) -> bool {
        self.copied.is_some_and(|at| at.elapsed().as_millis() < 1500)
    }
}

#[derive(Debug, Clone)]
pub enum Message {
    /// Alt+Shift+D: open or close the session log.
    ChordLog,
    /// Alt+Shift+M: open or close the memory monitor.
    ChordMemory,
    CloseLog,
    CloseMemory,
    /// The pointer went down on a debug window's title bar.
    DragStart(Panel),
    Capture(bool),
    File(bool),
    FileMode(LogMode),
    Memory(bool),
    MemoryFile(bool),
    MemoryFileMode(LogMode),
    MemoryInterval(u64),
    ToggleTarget(String),
    ToggleSeries(Series),
    ClearLog,
    Copy,
    Copied,
    OpenRuntimeFolder,
    OpenMemoryFolder,
    Tick,
}

/// The series the chart draws, the same two the original's single desktop process reports.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Series {
    WorkingSet,
    Private,
}

const SERIES: [Series; 2] = [Series::WorkingSet, Series::Private];

impl Series {
    fn label(self) -> &'static str {
        match self {
            Series::WorkingSet => "Working set",
            Series::Private => "Private (all)",
        }
    }

    fn color(self) -> Color {
        match self {
            Series::WorkingSet => Color::from_rgb8(0x34, 0xd3, 0x99),
            Series::Private => Color::from_rgb8(0x60, 0xa5, 0xfa),
        }
    }

    fn value(self, sample: &MemorySample) -> Option<f32> {
        match self {
            Series::WorkingSet => Some(sample.working_set_mb as f32),
            Series::Private => sample.private_mb.map(|mb| mb as f32),
        }
    }
}

fn dmsg(message: Message) -> crate::Message {
    crate::Message::Developer(message)
}

/// The single card of the developer page, in the kit's row shape.
pub fn settings_groups<'a>(
    i18n: &'a I18n,
    tokens: Tokens,
    developer: &'a Developer,
    settings: &DeveloperSettings,
    log: &'a RuntimeLog,
    memory_path: &'a FilePath,
) -> Vec<Group<'a, crate::Message>> {
    let capturing = settings.console_capture_enabled;
    let mut rows = vec![Row {
        label: i18n.t("options.consoleLogCapture").to_owned(),
        hint: Some(i18n.t("options.consoleLogCaptureDesc").to_owned()),
        sub: false,
        below: None,
        control: row![
            kit::keys(tokens, &["Alt", "Shift", "D"]),
            kit::toggle(tokens, if capturing { 1.0 } else { 0.0 }, dmsg(Message::Capture(!capturing))),
        ]
        .spacing(10)
        .align_y(Alignment::Center)
        .into(),
    }];
    if capturing {
        let index = mode_index(settings.runtime_log_mode);
        let modes = mode_choices(i18n);
        rows.push(Row {
            label: i18n.t("options.debugLogsToFile").to_owned(),
            hint: Some(log.path().display().to_string()),
            sub: true,
            below: Some(log_panel(i18n, tokens, developer, LOG_HEIGHT)),
            control: row![
                kit::icon_button(tokens, Icon::FolderOpen, 15.0, dmsg(Message::OpenRuntimeFolder)),
                kit::segmented_enabled(
                    tokens,
                    modes,
                    settings.runtime_log_mode,
                    Some(index as f32),
                    settings.runtime_log_enabled,
                    |mode| dmsg(Message::FileMode(mode)),
                ),
                kit::toggle(
                    tokens,
                    if settings.runtime_log_enabled { 1.0 } else { 0.0 },
                    dmsg(Message::File(!settings.runtime_log_enabled)),
                ),
            ]
            .spacing(8)
            .align_y(Alignment::Center)
            .into(),
        });
    }
    let monitoring = settings.memory_monitor_enabled;
    rows.push(Row {
        label: i18n.t("options.memoryMonitor").to_owned(),
        hint: Some(i18n.t("options.memoryMonitorDesc").to_owned()),
        sub: false,
        below: None,
        control: row![
            kit::keys(tokens, &["Alt", "Shift", "M"]),
            kit::toggle(tokens, if monitoring { 1.0 } else { 0.0 }, dmsg(Message::Memory(!monitoring))),
        ]
        .spacing(10)
        .align_y(Alignment::Center)
        .into(),
    });
    if monitoring {
        let index = mode_index(settings.memory_log_mode);
        rows.push(Row {
            label: i18n.t("options.debugLogsToFile").to_owned(),
            hint: Some(memory_path.display().to_string()),
            sub: true,
            below: None,
            control: row![
                kit::icon_button(tokens, Icon::FolderOpen, 15.0, dmsg(Message::OpenMemoryFolder)),
                kit::segmented_enabled(
                    tokens,
                    mode_choices(i18n),
                    settings.memory_log_mode,
                    Some(index as f32),
                    settings.memory_log_enabled,
                    |mode| dmsg(Message::MemoryFileMode(mode)),
                ),
                kit::toggle(
                    tokens,
                    if settings.memory_log_enabled { 1.0 } else { 0.0 },
                    dmsg(Message::MemoryFile(!settings.memory_log_enabled)),
                ),
            ]
            .spacing(8)
            .align_y(Alignment::Center)
            .into(),
        });
        rows.push(Row {
            label: i18n.t("options.memoryMonitorInterval").to_owned(),
            hint: None,
            sub: true,
            below: None,
            control: kit::segmented(
                tokens,
                interval_choices(),
                settings.interval_ms(),
                interval_mark(settings.interval_ms()),
                |interval| dmsg(Message::MemoryInterval(interval)),
            ),
        });
    }
    vec![Group {
        title: i18n.t("options.developerTools").to_owned(),
        bare: true,
        rows,
        foot: Some(i18n.t("options.consoleLogConvention").to_owned()),
    }]
}

/// The page as one card, for the offscreen painter the layout is checked with.
pub fn page<'a>(
    i18n: &'a I18n,
    tokens: Tokens,
    developer: &'a Developer,
    settings: &DeveloperSettings,
    log: &'a RuntimeLog,
    memory_path: &'a FilePath,
) -> Element<'a, crate::Message> {
    let groups = settings_groups(i18n, tokens, developer, settings, log, memory_path);
    column(groups.into_iter().enumerate().map(|(index, group)| kit::group(tokens, group, index == 0))).into()
}

fn mode_index(mode: LogMode) -> usize {
    match mode {
        LogMode::Append => 0,
        LogMode::Overwrite => 1,
    }
}

fn mode_choices(i18n: &I18n) -> Vec<(LogMode, String)> {
    vec![
        (LogMode::Append, i18n.t("options.debugLogModeAppend").to_owned()),
        (LogMode::Overwrite, i18n.t("options.debugLogModeOverwrite").to_owned()),
    ]
}

fn interval_choices() -> Vec<(u64, String)> {
    vec![(1000, "1s".to_owned()), (2000, "2s".to_owned()), (5000, "5s".to_owned()), (15000, "15s".to_owned())]
}

fn interval_mark(value: u64) -> Option<f32> {
    [1000_u64, 2000, 5000, 15000].iter().position(|interval| *interval == value).map(|index| index as f32)
}

/// The log panel shared by the page and the overlay: module filter, copy, clear and the newest lines.
pub fn log_panel<'a>(
    _i18n: &'a I18n,
    tokens: Tokens,
    developer: &'a Developer,
    height: f32,
) -> Element<'a, crate::Message> {
    let visible = developer.visible().count();
    let total = developer.lines.len();
    let summary =
        if visible == total { format!("Console ({visible})") } else { format!("Console ({visible} of {total})") };
    let heading = crate::typeface::log_caps(&summary, true, tokens.t3);
    let copy_label = if developer.copied() { "Copied".to_owned() } else { format!("Copy {visible}") };
    let copy = pill(tokens, copy_label.clone(), false, dmsg(Message::Copy));
    let clear = pill(tokens, "Clear".to_owned(), false, dmsg(Message::ClearLog));
    let header = row![heading, space().width(Length::Fill), copy, clear].spacing(6).align_y(Alignment::Center);

    let mut counts: Vec<(String, usize)> = Vec::new();
    for line in &developer.lines {
        match counts.iter_mut().find(|(target, _)| *target == line.target) {
            Some((_, count)) => *count += 1,
            None => counts.push((line.target.clone(), 1)),
        }
    }
    counts.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    let mut filters = row![].spacing(4).align_y(Alignment::Center);
    filters = filters.push(pill(tokens, "All".to_owned(), false, dmsg(Message::ToggleTarget(String::new()))));
    filters = filters.push(pill(tokens, "None".to_owned(), false, dmsg(Message::ToggleTarget("*".to_owned()))));
    for (target, count) in &counts {
        let hidden = developer.hidden_targets.contains(target);
        filters = filters.push(pill(
            tokens,
            format!("{target} {count}"),
            !hidden,
            dmsg(Message::ToggleTarget(target.clone())),
        ));
    }
    let filters = scrollable(filters)
        .direction(scrollable::Direction::Horizontal(scrollable::Scrollbar::hidden()))
        .width(Length::Fill);

    let mut list = column![].spacing(0);
    let mut shown = 0;
    for line in developer.visible().rev() {
        if shown >= 400 {
            break;
        }
        shown += 1;
        let color = level_color(line.level, tokens);
        let content = crate::typeface::LOG_ROW
            .text(format!("{} [{}] {}", clock(line.at), line.target, line.message))
            .color(color)
            .width(Length::Fill);
        list = list.push(container(content).width(Length::Fill).padding(Padding {
            top: 2.0,
            right: 8.0,
            bottom: 2.0,
            left: 8.0,
        }));
    }
    let list: Element<'a, crate::Message> = if shown == 0 {
        let words = if total == 0 { "Nothing logged yet." } else { "Nothing matches the module filter." };
        container(text(words).size(crate::typeface::size::CAPTION).color(tokens.t3)).padding([4, 8]).into()
    } else {
        scrollable(list)
            .height(Length::Fixed(height))
            .direction(kit::scrollbar())
            .style(kit::scroll_style(tokens))
            .into()
    };

    container(column![header, filters, list].spacing(6).padding([6, 4]))
        .width(Length::Fill)
        .style(move |_| kit::rounded(tokens.pv, 8.0))
        .into()
}

fn pill<'a>(tokens: Tokens, label: String, active: bool, on_press: crate::Message) -> Element<'a, crate::Message> {
    let ink = if active { tokens.t1 } else { tokens.t2 };
    button(container(crate::typeface::log_caps(&label, false, ink)).padding([4, 10]))
        .padding(0)
        .on_press(on_press)
        .style(move |_, status| button::Style {
            background: Some(Background::Color(match (active, status) {
                (true, _) => tokens.fill2,
                (false, button::Status::Hovered | button::Status::Pressed) => tokens.fill,
                _ => Color::TRANSPARENT,
            })),
            border: Border { color: tokens.line2, width: 1.0, radius: 11.0.into() },
            text_color: ink,
            ..button::Style::default()
        })
        .into()
}

fn level_color(level: Level, tokens: Tokens) -> Color {
    match level {
        Level::ERROR => Color::from_rgb8(0xe0, 0x60, 0x76),
        Level::WARN => Color::from_rgb8(0xd9, 0x9b, 0x3c),
        _ => tokens.t2,
    }
}

/// The session log as the Alt+Shift+D overlay; nothing when it is closed.
pub fn log_overlay<'a>(
    i18n: &'a I18n,
    tokens: Tokens,
    developer: &'a Developer,
    settings: &DeveloperSettings,
    window: iced::Size,
) -> Option<Element<'a, crate::Message>> {
    if !developer.log_open {
        return None;
    }
    let body = if settings.console_capture_enabled {
        log_panel(i18n, tokens, developer, OVERLAY_LOG_HEIGHT)
    } else {
        // `ConsoleLogPanel` with capture off.
        container(
            text("Not recording. Nothing from this session can be read back or copied.")
                .size(crate::typeface::size::CAPTION)
                .color(tokens.t3),
        )
        .padding(12)
        .into()
    };
    Some(debug_window(i18n, tokens, developer, Panel::Log, body, window))
}

/// The memory curve as the Alt+Shift+M overlay; nothing while sampling is off.
pub fn memory_overlay<'a>(
    i18n: &'a I18n,
    tokens: Tokens,
    developer: &'a Developer,
    settings: &DeveloperSettings,
    memory_path: &FilePath,
    window: iced::Size,
) -> Option<Element<'a, crate::Message>> {
    if !developer.memory_open || !settings.memory_monitor_enabled {
        return None;
    }
    let latest = developer.latest.as_ref();
    let unavailable: BTreeSet<Series> = SERIES
        .into_iter()
        .filter(|series| !developer.samples.is_empty() && developer.samples.iter().all(|s| series.value(s).is_none()))
        .collect();
    let shown: Vec<Series> = SERIES
        .into_iter()
        .filter(|series| !developer.hidden_series.contains(series) && !unavailable.contains(series))
        .collect();
    let max = developer
        .samples
        .iter()
        .flat_map(|sample| shown.iter().filter_map(|s| s.value(sample)))
        .fold(0.0_f32, f32::max);
    let max = (max * 1.08).max(1.0);

    let stats = row![
        stat(i18n, tokens, "options.memoryCurrent", mb(latest.map(|s| s.working_set_mb))),
        stat(i18n, tokens, "options.memoryPeak", mb(latest.map(|s| s.peak_mb))),
        stat(i18n, tokens, "options.memoryFloor", mb(latest.map(|s| s.floor_mb))),
        stat(i18n, tokens, "options.memoryAverage", mb(latest.map(|s| s.avg_mb))),
    ]
    .spacing(8)
    .width(Length::Fill);

    let chart = Canvas::new(Chart { samples: &developer.samples, shown: shown.clone(), max, grid: tokens.line2 })
        .width(Length::Fill)
        .height(Length::Fixed(CHART_HEIGHT));
    let first = developer.samples.first().map(|s| clock(s.at)).unwrap_or_default();
    let last = developer.samples.last().map(|s| clock(s.at)).unwrap_or_default();
    let axis = row![
        text(first).size(crate::typeface::size::SMALL_BADGE).color(tokens.t3),
        space().width(Length::Fill),
        text(format!("0 \u{2013} {} MB", max.round())).size(crate::typeface::size::SMALL_BADGE).color(tokens.t3),
        space().width(Length::Fill),
        text(last).size(crate::typeface::size::SMALL_BADGE).color(tokens.t3),
    ]
    .align_y(Alignment::Center);

    let mut legend = row![].spacing(6);
    for series in SERIES {
        let missing = unavailable.contains(&series);
        let on = !missing && !developer.hidden_series.contains(&series);
        let mut chip = row![].spacing(6).align_y(Alignment::Center);
        chip =
            chip.push(container(space()).width(12).height(6).style(move |_| {
                kit::rounded(if missing { Color { a: 0.25, ..tokens.t3 } } else { series.color() }, 3.0)
            }));
        chip = chip.push(text(series.label()).size(crate::typeface::size::SMALL_BADGE).color(if on {
            tokens.t2
        } else {
            tokens.t3
        }));
        if missing {
            chip = chip.push(text("n/a").size(crate::typeface::size::SMALL_BADGE).color(tokens.t3));
        }
        let press = (!missing).then(|| dmsg(Message::ToggleSeries(series)));
        let border = Border { color: tokens.line2, width: 1.0, radius: 11.0.into() };
        let mut chip = button(container(chip).padding([4, 8])).padding(0).style(move |_, status| button::Style {
            background: Some(Background::Color(match (on, status) {
                (false, _) => Color::TRANSPARENT,
                (true, button::Status::Hovered | button::Status::Pressed) => tokens.fill,
                _ => Color::TRANSPARENT,
            })),
            border,
            text_color: if on { tokens.t2 } else { tokens.t3 },
            ..button::Style::default()
        });
        if let Some(press) = press {
            chip = chip.on_press(press);
        }
        legend = legend.push(chip);
    }
    if SERIES.iter().any(|series| unavailable.contains(series)) {
        legend = legend.push(
            text(i18n.t("options.memorySeriesUnavailable")).size(crate::typeface::size::SMALL_BADGE).color(tokens.t3),
        );
    }

    let process = row![
        text(latest.map(|s| format!("{} #{}", s.name, s.pid)).unwrap_or_else(|| "\u{2014}".to_owned()))
            .size(crate::typeface::size::SMALL_BADGE)
            .color(tokens.t2)
            .width(Length::Fill),
        cell(latest.map(|s| s.working_set_mb.to_string()).unwrap_or_else(|| "\u{2014}".to_owned()), true),
        cell(latest.and_then(|s| s.private_mb).map_or_else(|| "\u{2014}".to_owned(), |mb| mb.to_string()), true),
        cell(latest.map(|s| format!("{:.1}%", s.cpu_percent)).unwrap_or_else(|| "\u{2014}".to_owned()), true),
    ]
    .spacing(6)
    .align_y(Alignment::Center);
    let columns = row![
        text("process").size(crate::typeface::size::SMALL_BADGE).color(tokens.t3).width(Length::Fill),
        header_cell("ws", tokens),
        header_cell("priv", tokens),
        header_cell("cpu", tokens),
    ]
    .spacing(6);

    let mut body = column![stats, chart, axis, legend].spacing(6);
    body = body.push(text(i18n.t("options.memoryByProcess")).size(crate::typeface::size::SMALL_BADGE).color(tokens.t3));
    body = body.push(columns);
    body = body.push(process);
    if let Some(latest) = latest {
        body = body.push(
            text(format!(
                "{} samples \u{00b7} {}s \u{00b7} cpu {:.1}%",
                latest.samples, latest.uptime_sec, latest.cpu_percent
            ))
            .size(crate::typeface::size::SMALL_BADGE)
            .color(tokens.t3),
        );
    }
    if settings.memory_log_enabled {
        body = body
            .push(text(memory_path.display().to_string()).size(crate::typeface::size::SMALL_BADGE).color(tokens.t3));
    }
    Some(debug_window(i18n, tokens, developer, Panel::Memory, body.into(), window))
}

fn stat<'a>(i18n: &'a I18n, tokens: Tokens, key: &'a str, value: String) -> Element<'a, crate::Message> {
    column![
        text(i18n.t(key)).size(crate::typeface::size::MICRO).color(tokens.t3),
        text(value).size(crate::typeface::size::CONTROL).color(tokens.t1),
    ]
    .width(Length::Fill)
    .into()
}

// A column title over `cell`: the same width and right edge as the numbers under it.
fn header_cell<'a>(title: &'a str, tokens: Tokens) -> Element<'a, crate::Message> {
    container(text(title).size(crate::typeface::size::SMALL_BADGE).color(tokens.t3))
        .width(Length::Fixed(52.0))
        .align_x(Alignment::End)
        .into()
}

fn cell<'a>(content: String, right: bool) -> Element<'a, crate::Message> {
    let text = text(content).size(crate::typeface::size::SMALL_BADGE);
    container(text).width(Length::Fixed(52.0)).align_x(if right { Alignment::End } else { Alignment::Start }).into()
}

fn mb(value: Option<u64>) -> String {
    value.map_or_else(|| "\u{2014}".to_owned(), |mb| format!("{mb} MB"))
}

/// `DraggableDebugWindow`: anchored top-right, moved by its title bar only, so the body keeps its clicks.
fn debug_window<'a>(
    i18n: &'a I18n,
    tokens: Tokens,
    developer: &Developer,
    panel: Panel,
    body: Element<'a, crate::Message>,
    window: Size,
) -> Element<'a, crate::Message> {
    let (title, shortcut, close) = match panel {
        Panel::Log => ("options.consoleLogCapture", "Alt+Shift+D", Message::CloseLog),
        Panel::Memory => ("options.memoryMonitor", "Alt+Shift+M", Message::CloseMemory),
    };
    let heading = text(i18n.t(title)).size(crate::typeface::size::CONTROL).color(tokens.t1);
    let keys = kit::keys(tokens, &shortcut.split('+').collect::<Vec<_>>());
    let close = kit::icon_button(tokens, Icon::X, 15.0, dmsg(close));
    let header = row![heading, space().width(Length::Fill), keys, close].spacing(8).align_y(Alignment::Center);
    let header =
        mouse_area(header).on_press(dmsg(Message::DragStart(panel))).interaction(iced::mouse::Interaction::Grab);
    let width = panel.width(window.width);
    let offset = developer.offset(panel);
    let card = container(column![header, body].spacing(8)).width(Length::Fixed(width)).padding(12).style(move |_| {
        container::Style {
            background: Some(tokens.menu.into()),
            border: Border { color: tokens.line2, width: 1.0, radius: 12.0.into() },
            shadow: Shadow {
                color: Color { a: 0.36 * tokens.shadow.a, ..Color::BLACK },
                offset: Vector::new(0.0, 18.0),
                blur_radius: 48.0,
            },
            text_color: Some(tokens.t1),
            ..container::Style::default()
        }
    });
    pin(card).x(window.width - ANCHOR - width + offset.x).y(ANCHOR + offset.y).into()
}

/// The curve: one polyline per shown series, scaled to the tallest value on screen.
struct Chart<'a> {
    samples: &'a [MemorySample],
    shown: Vec<Series>,
    max: f32,
    grid: Color,
}

impl canvas::Program<crate::Message> for Chart<'_> {
    type State = ();

    fn draw(
        &self,
        _state: &Self::State,
        renderer: &iced::Renderer,
        _theme: &iced::Theme,
        bounds: Rectangle,
        _cursor: Cursor,
    ) -> Vec<canvas::Geometry> {
        let mut frame = Frame::new(renderer, bounds.size());
        for fraction in [0.25_f32, 0.5, 0.75] {
            let y = bounds.height * fraction;
            let line = CanvasPath::line(Point::new(0.0, y), Point::new(bounds.width, y));
            frame.stroke(
                &line,
                Stroke::default().with_color(Color { a: self.grid.a * 0.6, ..self.grid }).with_width(1.0),
            );
        }
        if self.samples.len() < 2 {
            return vec![frame.into_geometry()];
        }
        let step = bounds.width / (self.samples.len() - 1) as f32;
        for series in &self.shown {
            let path = CanvasPath::new(|builder| {
                let mut started = false;
                for (index, sample) in self.samples.iter().enumerate() {
                    let Some(value) = series.value(sample) else {
                        started = false;
                        continue;
                    };
                    let point = Point::new(
                        index as f32 * step,
                        bounds.height - (value / self.max).clamp(0.0, 1.0) * bounds.height,
                    );
                    if started {
                        builder.line_to(point);
                    } else {
                        builder.move_to(point);
                        started = true;
                    }
                }
            });
            frame.stroke(&path, Stroke::default().with_color(series.color()).with_width(1.5));
        }
        vec![frame.into_geometry()]
    }
}
