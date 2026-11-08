// Full-screen search from `SearchWorkspace`: a tinted page, one field, the platform in use, then the result rows.

use std::collections::HashSet;
use std::time::Instant;

use folia_core::Track;
use folia_i18n::I18n;
use folia_library::Cover;
use folia_omni::Omni;
use folia_render::Plate;
use folia_visualizer::Ease;
use iced::widget::scrollable::Viewport;
use iced::widget::{button, column, container, row, shader, space, stack, svg, text, text_input};
use iced::{Alignment, Background, Border, Color, Element, Length, Radians, Task, Theme};
use lucide_static_svg::Icon;

use crate::background::{error_chain, online_cover_sized};
use crate::icons::icon;
use crate::search_results::{self, List};

const PAGE_SIZE: u32 = 30;
const COVER_PX: u32 = 48;
const FETCH_PX: u32 = 120;
const FADE_MS: f64 = 200.0;
const FADE_EASE: Ease = Ease::new(0.0, 0.0, 0.2, 1.0);

#[derive(Debug, Clone)]
pub enum Message {
    Query(String),
    Submit,
    Results(Result<folia_omni::Page<Track>, String>),
    LoadMore,
    More(Result<folia_omni::Page<Track>, String>),
    Scrolled(Viewport),
    Hover(Option<usize>),
    Plus(Option<usize>),
    Play(usize),
    Enqueue(usize),
    Cover(i64, Option<Cover>),
    Close,
    Frame,
}

/// What a result row asked the shell to do.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    None,
    Play(usize),
    Enqueue(usize),
}

pub struct SearchPage {
    query: String,
    pub results: Vec<Track>,
    covers: Vec<Option<iced::widget::image::Handle>>,
    cache: folia_library::CoverCache<i64, iced::widget::image::Handle>,
    pending: HashSet<i64>,
    busy: bool,
    loading_more: bool,
    more_error: bool,
    has_more: bool,
    next_offset: u32,
    submitted: bool,
    error: bool,
    list: List,
    fade_from: f32,
    fade_goal: f32,
    fade_at: Instant,
    busy_at: Instant,
}

impl Default for SearchPage {
    fn default() -> Self {
        let now = Instant::now();
        Self {
            query: String::new(),
            results: Vec::new(),
            covers: Vec::new(),
            cache: folia_library::CoverCache::new(4 * 1024 * 1024),
            pending: HashSet::new(),
            busy: false,
            loading_more: false,
            more_error: false,
            has_more: false,
            next_offset: 0,
            submitted: false,
            error: false,
            list: List::default(),
            fade_from: 0.0,
            fade_goal: 0.0,
            fade_at: now,
            busy_at: now,
        }
    }
}

impl SearchPage {
    pub fn query(&self) -> &str {
        &self.query
    }

    /// Takes a query committed from the home seam (`onSearchCommitted`): the page opens straight on its results.
    pub fn begin(&mut self, query: String) {
        self.query = query;
        self.clear_results();
        self.busy = false;
        self.submitted = true;
        self.error = false;
    }

    pub fn reveal(&mut self) {
        let now = Instant::now();
        self.fade_from = self.fade(now);
        self.fade_goal = 1.0;
        self.fade_at = now;
    }

    pub fn close(&mut self) {
        let now = Instant::now();
        self.fade_from = self.fade(now);
        self.fade_goal = 0.0;
        self.fade_at = now;
    }

    pub fn animating(&self, now: Instant) -> bool {
        (self.fade_from - self.fade_goal).abs() > 0.001
            && now.saturating_duration_since(self.fade_at).as_secs_f64() * 1000.0 < FADE_MS
    }

    /// The searching mark spins, so the page keeps a frame subscription.
    pub fn spinning(&self) -> bool {
        self.busy && self.results.is_empty()
    }

    pub fn gone(&self, now: Instant) -> bool {
        self.fade_goal == 0.0 && !self.animating(now) && self.fade(now) < 0.02
    }

    pub fn fade(&self, now: Instant) -> f32 {
        let t = (now.saturating_duration_since(self.fade_at).as_secs_f64() * 1000.0 / FADE_MS).clamp(0.0, 1.0);
        self.fade_from + (self.fade_goal - self.fade_from) * FADE_EASE.at(t) as f32
    }

    /// Handles a page message. A play or an enqueue is handed back for the shell.
    pub fn update(
        &mut self,
        message: Message,
        omni: Option<&Omni>,
        provider: &'static str,
    ) -> (Task<Message>, Outcome) {
        match message {
            Message::Query(query) => self.query = query,
            Message::Submit => return (self.submit(omni, provider), Outcome::None),
            Message::Results(result) => return self.land(result, false, omni),
            Message::LoadMore => return (self.load_more(omni, provider), Outcome::None),
            Message::More(result) => return self.land(result, true, omni),
            Message::Scrolled(viewport) => {
                self.list.scrolled(viewport);
                return (self.load_covers(omni), Outcome::None);
            }
            Message::Hover(index) => self.list.hovered = index,
            Message::Plus(index) => self.list.plus(index),
            Message::Play(index) => return (Task::none(), Outcome::Play(index)),
            Message::Enqueue(index) => return (Task::none(), Outcome::Enqueue(index)),
            Message::Cover(id, cover) => self.store_cover(id, cover),
            Message::Close | Message::Frame => {}
        }
        (Task::none(), Outcome::None)
    }

    pub fn view<'a>(&'a self, view: View<'a>, now: Instant) -> Element<'a, Message> {
        let shown = self.fade(now);
        let narrow = view.width < 640.0;
        let (pad_x, pad_y) = if narrow { (12.0, 16.0) } else { (24.0, 24.0) };
        let shift = 28.0 * (1.0 - shown);
        let ink = Color { a: view.text.a * shown, ..view.text };
        let field = self.field(view, ink, shown);
        let close = close_button(ink, view.daylight, shown);
        let source = source_pill(view, shown);
        let body = self.results(view, ink, now);
        let header = column![row![field, close].spacing(12).align_y(Alignment::Center), source].spacing(12);
        let page = column![header, body]
            .spacing(12)
            .width(Length::Fixed(view.width.min(1024.0)))
            .padding(iced::Padding { top: pad_y + shift, right: pad_x, bottom: pad_y, left: pad_x });
        let page = container(page).width(Length::Fill).height(Length::Fill).align_x(Alignment::Center);
        if shown <= 0.0 {
            return page.into();
        }
        // `SearchWorkspace` `backdropFilter: blur(24px)`. `shown` fades the blur and the veil together.
        let fill = if view.daylight {
            [250.0 / 255.0, 250.0 / 255.0, 250.0 / 255.0, 0.96]
        } else {
            [8.0 / 255.0, 8.0 / 255.0, 10.0 / 255.0, 0.94]
        };
        let plate = Plate {
            radius: 0.0,
            fill_origin: [0.0, 0.0],
            fill_size: [0.0, 0.0],
            fill,
            shadow: [0.0; 4],
            shadow_blur: 0.0,
            shadow_y: 0.0,
            shadow_spread: 0.0,
            ring: [0.0; 4],
            backdrop: 24.0,
            fade: shown,
            saturate: 1.0,
        };
        stack![shader(plate).width(Length::Fill).height(Length::Fill), page]
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }

    fn submit(&mut self, omni: Option<&Omni>, provider: &'static str) -> Task<Message> {
        let query = self.query.trim().to_owned();
        let Some(omni) = omni.cloned() else { return Task::none() };
        if query.is_empty() || self.busy {
            return Task::none();
        }
        self.busy = true;
        self.busy_at = Instant::now();
        self.submitted = true;
        self.error = false;
        self.more_error = false;
        self.loading_more = false;
        let search = async move {
            omni.search(provider, &query, PAGE_SIZE, 0).await.map_err(|err| {
                tracing::warn!(error = &err as &dyn std::error::Error, "search failed");
                error_chain(&err)
            })
        };
        Task::perform(search, Message::Results)
    }

    fn load_more(&mut self, omni: Option<&Omni>, provider: &'static str) -> Task<Message> {
        let Some(omni) = omni.cloned() else { return Task::none() };
        if !self.has_more || self.busy || self.loading_more || self.query.trim().is_empty() {
            return Task::none();
        }
        self.loading_more = true;
        self.more_error = false;
        let (query, offset) = (self.query.trim().to_owned(), self.next_offset);
        let search = async move {
            omni.search(provider, &query, PAGE_SIZE, offset).await.map_err(|err| {
                tracing::warn!(error = &err as &dyn std::error::Error, "search page failed");
                error_chain(&err)
            })
        };
        Task::perform(search, Message::More)
    }

    fn land(
        &mut self,
        result: Result<folia_omni::Page<Track>, String>,
        more: bool,
        omni: Option<&Omni>,
    ) -> (Task<Message>, Outcome) {
        if more {
            self.loading_more = false;
            match result {
                Ok(page) => {
                    self.more_error = false;
                    self.has_more = page.has_more;
                    self.next_offset = page.next_offset;
                    self.results.extend(page.items);
                    self.sync_covers();
                }
                Err(_) => self.more_error = true,
            }
        } else {
            self.busy = false;
            match result {
                Ok(page) => {
                    self.error = false;
                    self.has_more = page.has_more;
                    self.next_offset = page.next_offset;
                    self.results = page.items;
                    self.list.reset();
                    self.sync_covers();
                }
                Err(_) => {
                    self.error = true;
                    self.clear_results();
                    self.submitted = true;
                }
            }
        }
        (self.load_covers(omni), Outcome::None)
    }

    fn clear_results(&mut self) {
        self.results.clear();
        self.covers.clear();
        self.pending.clear();
        self.cache.clear();
        self.has_more = false;
        self.next_offset = 0;
        self.list.reset();
    }

    fn sync_covers(&mut self) {
        self.covers.resize(self.results.len(), None);
        for (index, track) in self.results.iter().enumerate() {
            if let Some(hit) = self.cache.peek(&track.id).cloned() {
                self.covers[index] = hit;
            }
        }
    }

    fn store_cover(&mut self, id: i64, cover: Option<Cover>) {
        self.pending.remove(&id);
        let bytes = cover.as_ref().map_or(0, |cover| cover.rgba.len());
        let handle = cover.map(|cover| iced::widget::image::Handle::from_rgba(cover.width, cover.height, cover.rgba));
        self.cache.insert(id, handle.clone(), bytes);
        for (index, track) in self.results.iter().enumerate() {
            if track.id == id
                && let Some(slot) = self.covers.get_mut(index)
            {
                *slot = handle.clone();
            }
        }
    }

    fn load_covers(&mut self, omni: Option<&Omni>) -> Task<Message> {
        self.cache.begin_pass();
        let mut tasks = Vec::new();
        for index in self.list.visible(self.results.len()) {
            let Some(track) = self.results.get(index) else { continue };
            let id = track.id;
            self.cache.touch(&id);
            if self.cache.peek(&id).is_some() || self.pending.contains(&id) {
                continue;
            }
            let Some(url) = track.cover_url.clone() else {
                self.cache.insert(id, None, 0);
                continue;
            };
            let Some(omni) = omni.cloned() else {
                self.cache.insert(id, None, 0);
                continue;
            };
            self.pending.insert(id);
            tasks.push(Task::perform(online_cover_sized(omni, url, FETCH_PX, COVER_PX), move |cover| {
                Message::Cover(id, cover)
            }));
        }
        Task::batch(tasks)
    }

    fn field<'a>(&'a self, view: View<'a>, ink: Color, shown: f32) -> Element<'a, Message> {
        let border = if view.daylight {
            Color { a: 0.10 * shown, ..Color::BLACK }
        } else {
            Color { r: 1.0, g: 1.0, b: 1.0, a: 0.10 * shown }
        };
        let fill = if view.daylight {
            Color { a: 0.04 * shown, ..Color::BLACK }
        } else {
            Color { r: 1.0, g: 1.0, b: 1.0, a: 0.05 * shown }
        };
        let input = text_input(view.i18n.t("search.placeholder"), &self.query)
            .on_input(Message::Query)
            .on_submit(Message::Submit)
            .size(14)
            .padding(iced::Padding { top: 14.0, right: 16.0, bottom: 14.0, left: 44.0 })
            .style(move |_theme: &Theme, _status| text_input::Style {
                background: Background::Color(fill),
                border: Border { color: border, width: 1.0, radius: 16.0.into() },
                icon: ink,
                placeholder: Color { a: 0.45 * ink.a, ..ink },
                value: ink,
                selection: Color { a: 0.3, ..view.accent },
            });
        let mark = if self.busy { Icon::LoaderCircle } else { Icon::Search };
        let glyph = container(icon(mark, 16.0, Color { a: 0.45 * ink.a, ..ink }))
            .padding(iced::Padding { top: 0.0, right: 0.0, bottom: 0.0, left: 16.0 })
            .center_y(Length::Fill);
        stack![input, glyph].into()
    }

    fn results<'a>(&'a self, view: View<'a>, ink: Color, now: Instant) -> Element<'a, Message> {
        if self.busy && self.results.is_empty() {
            return center(spinner(ink, now, self.busy_at));
        }
        if self.error && self.results.is_empty() {
            return center(error_block(view, ink, Message::Submit));
        }
        if !self.submitted {
            return space().height(Length::Fill).into();
        }
        if self.results.is_empty() {
            return center(text(view.i18n.t("home.noResults")).size(14).color(Color { a: 0.5 * ink.a, ..ink }));
        }
        let look = search_results::Look {
            text: ink,
            secondary: view.secondary,
            daylight: view.daylight,
            wide: view.width >= 640.0,
            queue_tip: view.i18n.t("navidrome.addToQueue"),
        };
        let bindings = search_results::Bindings {
            scroll: Message::Scrolled,
            hover: Message::Hover,
            play: Message::Play,
            enqueue: Message::Enqueue,
            plus: Message::Plus,
        };
        let list = self.list.view(&self.results, &self.covers, look, bindings);
        column![list, more_row(self, view, ink)].height(Length::Fill).into()
    }
}

fn center<'a>(body: impl Into<Element<'a, Message>>) -> Element<'a, Message> {
    container(body).center(Length::Fill).height(Length::Fill).into()
}

fn spinner<'a>(ink: Color, now: Instant, from: Instant) -> Element<'a, Message> {
    let turn = now.saturating_duration_since(from).as_secs_f32() * std::f32::consts::TAU;
    svg(svg::Handle::from_memory(Icon::LoaderCircle.svg_bytes()))
        .width(Length::Fixed(36.0))
        .height(Length::Fixed(36.0))
        .rotation(Radians(turn))
        .style(move |_, _| svg::Style { color: Some(Color { a: 1.0, ..ink }) })
        .opacity(0.45 * ink.a)
        .into()
}

fn error_block<'a>(view: View<'a>, ink: Color, retry: Message) -> Element<'a, Message> {
    let mark = icon(Icon::CircleAlert, 32.0, Color { a: 0.65 * ink.a, ..ink });
    let words = text(view.i18n.t("search.error")).size(14).color(Color { a: 0.65 * ink.a, ..ink });
    column![mark, words, retry_button(view, ink, retry)].spacing(12).align_x(Alignment::Center).into()
}

fn retry_button<'a>(view: View<'a>, ink: Color, message: Message) -> Element<'a, Message> {
    button(text(view.i18n.t("search.retry")).size(14).color(ink))
        .padding([8, 16])
        .on_press(message)
        .style(move |_, _| button::Style {
            background: None,
            border: Border { color: Color { a: 0.15 * ink.a, ..ink }, width: 1.0, radius: 999.0.into() },
            ..button::Style::default()
        })
        .into()
}

fn more_row<'a>(page: &'a SearchPage, view: View<'a>, ink: Color) -> Element<'a, Message> {
    if page.more_error {
        let words = text(view.i18n.t("search.error")).size(14).color(Color { a: 0.6 * ink.a, ..ink });
        return container(
            row![words, retry_button(view, ink, Message::LoadMore)].spacing(12).align_y(Alignment::Center),
        )
        .width(Length::Fill)
        .align_x(Alignment::Center)
        .padding([12.0, 0.0])
        .into();
    }
    if !page.has_more {
        return space().height(0).into();
    }
    let loading = page.loading_more;
    let fade = if loading { 0.5 } else { 1.0 };
    let label = if loading { "localMusic.searching" } else { "home.loadMore" };
    let label_ink = Color { a: ink.a * fade, ..ink };
    let button = button(text(view.i18n.t(label)).size(14).color(label_ink))
        .padding([8, 20])
        .on_press_maybe((!loading).then_some(Message::LoadMore))
        .style(move |_, status| {
            let hot = !loading && matches!(status, button::Status::Hovered | button::Status::Pressed);
            let wash = if view.daylight { Color::BLACK } else { Color::WHITE };
            let fill = if hot { 0.10 } else { 0.05 };
            button::Style {
                background: Some(Background::Color(Color { a: fill * fade, ..wash })),
                border: Border { color: Color { a: 0.10 * fade, ..wash }, width: 1.0, radius: 999.0.into() },
                text_color: label_ink,
                ..button::Style::default()
            }
        });
    container(button).width(Length::Fill).align_x(Alignment::Center).padding([12.0, 0.0]).into()
}

#[derive(Clone, Copy)]
pub struct View<'a> {
    pub i18n: &'a I18n,
    pub text: Color,
    pub secondary: Color,
    pub background: Color,
    pub accent: Color,
    pub daylight: bool,
    pub width: f32,
    /// The short name of the platform searched.
    pub source: &'static str,
}

fn source_pill<'a>(view: View<'a>, shown: f32) -> Element<'a, Message> {
    let label = text(view.source).size(12).font(medium()).color(view.background);
    button(label)
        .padding([8, 16])
        .style(move |_theme: &Theme, _status| button::Style {
            background: Some(Background::Color(Color { a: shown, ..view.accent })),
            border: Border { radius: 999.0.into(), ..Border::default() },
            ..button::Style::default()
        })
        .into()
}

fn close_button<'a>(ink: Color, daylight: bool, shown: f32) -> Element<'a, Message> {
    let fill = if daylight {
        Color { a: 0.05 * shown, ..Color::BLACK }
    } else {
        Color { r: 1.0, g: 1.0, b: 1.0, a: 0.10 * shown }
    };
    button(icon(Icon::X, 20.0, ink))
        .padding(12)
        .on_press(Message::Close)
        .style(move |_theme: &Theme, status| {
            let hot = matches!(status, button::Status::Hovered | button::Status::Pressed);
            button::Style {
                background: Some(Background::Color(Color { a: if hot { fill.a + 0.05 } else { fill.a }, ..fill })),
                border: Border { radius: 999.0.into(), ..Border::default() },
                ..button::Style::default()
            }
        })
        .into()
}

fn medium() -> iced::Font {
    crate::typeface::font(iced::font::Weight::Medium)
}
