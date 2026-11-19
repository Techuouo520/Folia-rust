// The original Grid artist wall: upper songs, central portrait and biography, lower albums.

use std::time::Instant;

use iced::widget::canvas::{Action, Geometry};
use iced::widget::scrollable::Viewport;
use iced::widget::{
    Id, button, canvas, column, container, image, mouse_area, pin, row, scrollable, shader, space, stack, tooltip,
};
use iced::{
    Alignment, Background, Border, Color, Element, Event, Length, Point, Rectangle, Shadow, Size, Task, Theme, Vector,
    mouse,
};
use lucide_static_svg::Icon;

use super::{CatalogPage, Message as CatalogMessage, View};
use crate::icons::icon;
use crate::smooth_scroll::Scroll;
use crate::typeface::text;
use crate::wall_pan::Glide;

#[derive(Debug, Clone)]
pub enum Message {
    Size(Size),
    Press(Point),
    Move(Point),
    Release,
    Wheel(mouse::ScrollDelta),
    Home,
    Biography(bool),
    BioScrolled(Viewport),
    BioWheel(mouse::ScrollDelta),
}

#[derive(Clone)]
pub(super) struct State {
    pub(super) backdrop: Option<image::Handle>,
    pub(super) backdrop_url: Option<String>,
    size: Size,
    pan: Vector,
    press: Option<(Point, Point)>,
    dragging: bool,
    glide: Glide,
    songs: Vec<Point>,
    albums: Vec<Point>,
    biography: bool,
    bio_scroll: Scroll,
}

impl Default for State {
    fn default() -> Self {
        Self {
            backdrop: None,
            backdrop_url: None,
            size: Size::new(1200.0, 800.0),
            pan: Vector::ZERO,
            press: None,
            dragging: false,
            glide: Glide::default(),
            songs: Vec::new(),
            albums: Vec::new(),
            biography: false,
            bio_scroll: Scroll::default(),
        }
    }
}

impl State {
    pub(super) fn refresh(&mut self, songs: usize, albums: usize) {
        let config = Config::new(self.size);
        if self.songs.len() != songs {
            self.songs = coordinates(songs, -1.0, config);
        }
        if self.albums.len() != albums {
            self.albums = coordinates(albums, 1.0, config);
        }
    }

    pub(super) fn suspend(&mut self) {
        self.backdrop = None;
        self.backdrop_url = None;
        self.press = None;
        self.dragging = false;
        self.glide = Glide::default();
        self.biography = false;
    }

    pub(super) fn close_biography(&mut self) -> bool {
        std::mem::take(&mut self.biography)
    }

    pub(super) fn snapshot(&self) -> Self {
        let mut state = self.clone();
        state.suspend();
        state
    }

    pub(super) fn update(&mut self, message: Message) {
        match message {
            Message::Size(size) => self.size = size,
            Message::Press(point) => {
                self.glide = Glide::default();
                self.press = Some((point, point));
            }
            Message::Move(point) => {
                if let Some((start, last)) = self.press {
                    if self.dragging {
                        self.pan += point - last;
                    } else if start.distance(point) > crate::wall_pan::DRAG_THRESHOLD {
                        self.dragging = true;
                        self.pan += point - start;
                    }
                    self.press = Some((start, point));
                }
            }
            Message::Release => {
                self.press = None;
                self.dragging = false;
            }
            Message::Wheel(delta) => {
                let movement = match delta {
                    mouse::ScrollDelta::Lines { x, y } => Vector::new(x * 100.0, y * 100.0),
                    mouse::ScrollDelta::Pixels { x, y } => Vector::new(x, y),
                };
                self.glide.add(movement);
            }
            Message::Home => {
                self.pan = Vector::ZERO;
                self.glide = Glide::default();
            }
            Message::Biography(open) => {
                if self.biography != open {
                    self.bio_scroll.reset();
                }
                self.biography = open;
            }
            Message::BioScrolled(viewport) => {
                self.bio_scroll.scrolled(&viewport, Instant::now());
            }
            Message::BioWheel(delta) => {
                self.bio_scroll.wheel(delta, Instant::now());
            }
        }
        self.clamp();
    }

    pub(super) fn animating(&self, now: Instant) -> bool {
        self.glide.pending() != Vector::ZERO || self.bio_scroll.animating(now)
    }

    pub(super) fn frame(&mut self, now: Instant) {
        if let Some(step) = self.glide.step(now) {
            self.pan += step;
            self.clamp();
        }
        self.bio_scroll.frame(now);
    }

    pub(super) fn scroll_task(&self) -> Task<CatalogMessage> {
        if self.biography { self.bio_scroll.apply(Id::new("artist-biography")) } else { Task::none() }
    }

    fn clamp(&mut self) {
        let config = Config::new(self.size);
        let mut minimum = Point::new(-0.95 * config.spacing.width, 0.0);
        let mut maximum = Point::new(0.65 * config.spacing.width, 0.0);
        for point in self.songs.iter().chain(&self.albums) {
            let point = config.world(*point);
            minimum.x = minimum.x.min(point.x);
            minimum.y = minimum.y.min(point.y);
            maximum.x = maximum.x.max(point.x);
            maximum.y = maximum.y.max(point.y);
        }
        let origin = 0.65 * config.spacing.width;
        self.pan.x = self.pan.x.clamp(origin - maximum.x, origin - minimum.x);
        self.pan.y = self.pan.y.clamp(-maximum.y, -minimum.y);
    }

    fn frame_at(&self, point: Point, dimensions: Size, viewport: Size) -> Placement {
        let config = Config::new(viewport);
        let center = config.world(point) + self.pan - Vector::new(config.spacing.width * 0.65, 0.0);
        let t = (center.distance(Point::ORIGIN) / config.distance).min(1.0);
        let scale = 1.1 - (1.1 - 0.45) * t;
        let center = center + Vector::new(viewport.width / 2.0, viewport.height / 2.0);
        Placement {
            rect: Rectangle::new(
                center - Vector::new(dimensions.width * scale / 2.0, dimensions.height * scale / 2.0),
                dimensions * scale,
            ),
            scale,
            opacity: 1.0 - 0.6 * t,
        }
    }

    pub(super) fn visible_tracks(&self) -> impl Iterator<Item = usize> + '_ {
        self.visible(&self.songs)
    }
    pub(super) fn visible_albums(&self) -> impl Iterator<Item = usize> + '_ {
        self.visible(&self.albums)
    }

    fn visible<'a>(&'a self, coordinates: &'a [Point]) -> impl Iterator<Item = usize> + 'a {
        let bounds = Rectangle::new(Point::new(-100.0, -100.0), self.size + Size::new(200.0, 200.0));
        let dimensions = Config::new(self.size).card;
        coordinates.iter().enumerate().filter_map(move |(index, point)| {
            self.frame_at(*point, dimensions, self.size).rect.intersects(&bounds).then_some(index)
        })
    }
}

#[derive(Clone, Copy)]
struct Config {
    card: Size,
    spacing: Size,
    avatar: f32,
    bio: Size,
    distance: f32,
}

impl Config {
    fn new(size: Size) -> Self {
        let (card, spacing, avatar, bio, distance) = if size.width < 768.0 {
            (Size::new(180.0, 280.0), Size::new(205.0, 270.0), 200.0, Size::new(360.0, 200.0), 420.0)
        } else if size.width < 1440.0 {
            (Size::new(220.0, 330.0), Size::new(250.0, 320.0), 280.0, Size::new(480.0, 260.0), 500.0)
        } else if size.width < 2000.0 {
            (Size::new(250.0, 375.0), Size::new(285.0, 365.0), 320.0, Size::new(540.0, 280.0), 580.0)
        } else {
            (Size::new(280.0, 420.0), Size::new(320.0, 410.0), 360.0, Size::new(600.0, 300.0), 660.0)
        };
        let fit = ((size.width - 32.0) / (bio.width * 1.1))
            .min(((size.height - 100.0) / (bio.height * 1.1)).max(0.35))
            .min(1.0);
        Self {
            card: card * fit,
            spacing: spacing * fit,
            avatar: avatar * fit,
            bio: bio * fit,
            distance: distance * fit,
        }
    }

    fn world(self, point: Point) -> Point {
        Point::new(point.x * self.spacing.width, point.y * self.spacing.height)
    }
}

struct Placement {
    rect: Rectangle,
    scale: f32,
    opacity: f32,
}

// Keep both halves ordered by distance from the artist, as buildArtistGridCoords does.
fn coordinates(count: usize, direction: f32, config: Config) -> Vec<Point> {
    let mut candidates = Vec::new();
    for z in 1..=count.div_ceil(10).max(10) {
        for x in -20..=20 {
            let z = z as f32 * direction;
            candidates.push(Point::new(x as f32 + z / 2.0, z));
        }
    }
    candidates.sort_by(|left, right| {
        config.world(*left).distance(Point::ORIGIN).total_cmp(&config.world(*right).distance(Point::ORIGIN))
    });
    candidates.truncate(count);
    candidates
}

impl CatalogPage {
    pub(super) fn artist_view<'a>(&'a self, view: View<'a>) -> Element<'a, CatalogMessage> {
        let size = Size::new(view.width, view.height);
        let config = Config::new(size);
        let state = &self.artist_page;
        let wash = folia_render::Wash {
            seam: [0.0; 3],
            base: [view.background.r, view.background.g, view.background.b],
            glows: [
                folia_render::Glow {
                    center: [0.05, 0.5],
                    reach: 0.9,
                    rgba: [view.accent.r, view.accent.g, view.accent.b, 0.10],
                },
                folia_render::Glow {
                    center: [0.9, 0.9],
                    reach: 0.8,
                    rgba: [view.accent.r, view.accent.g, view.accent.b, 0.06],
                },
            ],
        };
        let mut wall = stack![
            shader(wash).width(Length::Fill).height(Length::Fill),
            canvas(Measure(state.size)).width(Length::Fill).height(Length::Fill)
        ];
        if let Some(backdrop) = &state.backdrop {
            wall = wall.push(
                pin(image(backdrop.clone())
                    .width(size.width * 1.1)
                    .height(size.height * 1.1)
                    .content_fit(iced::ContentFit::Cover)
                    .opacity(if view.daylight { 0.18_f32 } else { 0.12 }))
                .x(-size.width * 0.05)
                .y(-size.height * 0.05),
            );
        }
        let bounds = Rectangle::new(Point::ORIGIN, size);
        for (index, point) in state.songs.iter().enumerate() {
            let placement = state.frame_at(*point, config.card, size);
            if placement.rect.intersects(&bounds)
                && let Some(track) = self.tracks.get(index)
            {
                wall = wall.push(place(self.song_card(index, track, &placement, view), placement.rect));
            }
        }
        for (index, point) in state.albums.iter().enumerate() {
            let placement = state.frame_at(*point, config.card, size);
            if placement.rect.intersects(&bounds)
                && let Some(album) = self.albums.get(index)
            {
                wall = wall.push(place(self.album_card(album, &placement, view), placement.rect));
            }
        }
        let portrait = state.frame_at(Point::new(-0.95, 0.0), Size::new(config.avatar, config.avatar), size);
        wall = wall.push(place(self.artist_portrait(portrait.rect.width, view), portrait.rect));
        let biography = state.frame_at(Point::new(0.65, 0.0), config.bio, size);
        wall = wall.push(place(self.biography_card(&biography, view), biography.rect));
        for (key, y, visible) in [("catalog.hotSongs", -0.56, true), ("catalog.albums", 0.63, self.capabilities.albums)]
        {
            if !visible {
                continue;
            }
            let label = state.frame_at(Point::new(0.65, y), Size::new(config.bio.width, 40.0), size);
            wall = wall.push(place(
                container(
                    text(view.i18n.t(key))
                        .size(28)
                        .font(crate::typeface::adapt(crate::typeface::BOLD))
                        .color(fade(view.ink, 0.08)),
                )
                .center(Length::Fill)
                .into(),
                label.rect,
            ));
        }
        let capture = crate::drag_capture::Capture::new(
            wall,
            bounds,
            |at| wrap(Message::Press(at)),
            |at| wrap(Message::Move(at)),
            wrap(Message::Release),
            wrap(Message::Release),
        )
        .held(state.press.is_some(), state.dragging)
        .cursor(mouse::Interaction::Idle);
        let viewport = crate::smooth_scroll::smooth(capture, |delta| wrap(Message::Wheel(delta)));
        let mut layers = stack![viewport, self.artist_toolbar(view)];
        if self.error {
            layers = layers.push(
                container(
                    row![
                        text(view.i18n.t("catalog.error")).size(12).color(view.muted),
                        button(text(view.i18n.t("search.retry")).size(12).color(view.ink))
                            .on_press(CatalogMessage::Retry)
                            .style(button::text)
                    ]
                    .spacing(8),
                )
                .width(Length::Fill)
                .align_x(Alignment::Center)
                .padding(iced::Padding::ZERO.top(82)),
            );
        }
        if state.biography {
            layers = layers.push(self.biography_modal(view));
        }
        container(layers).width(Length::Fill).height(Length::Fill).clip(true).into()
    }

    fn artist_portrait<'a>(&'a self, size: f32, view: View<'a>) -> Element<'a, CatalogMessage> {
        let face = self.picture(self.picture_url(), (size - 6.0).max(1.0), true, view.ink);
        button(container(face).padding(3).style(move |_| container::Style {
            border: Border { radius: (size / 2.0).into(), width: 3.0, color: fade(view.ink, 0.10) },
            shadow: Shadow { color: fade(Color::BLACK, 0.25), offset: Vector::new(0.0, 16.0), blur_radius: 36.0 },
            ..Default::default()
        }))
        .padding(0)
        .on_press(wrap(Message::Home))
        .style(button::text)
        .into()
    }

    fn biography_card<'a>(&'a self, placement: &Placement, view: View<'a>) -> Element<'a, CatalogMessage> {
        let scale = placement.scale;
        let small = view.width < 768.0 || view.height < 560.0;
        let padding = if small { 16.0 } else { 24.0 } * scale;
        let heading = text(self.title())
            .size(if small { 22.0 } else { 30.0 } * scale)
            .font(crate::typeface::adapt(crate::typeface::BOLD))
            .color(view.ink)
            .wrapping(text::Wrapping::None);
        let underline = container(space()).width(48.0 * scale).height(2.0 * scale).style(move |_| container::Style {
            background: Some(fade(view.accent, 0.65).into()),
            ..Default::default()
        });
        let description = self.artist.as_ref().and_then(|artist| artist.description.as_deref()).unwrap_or("");
        let copy = container(text(description).size(12.0 * scale).line_height(1.625).color(fade(view.ink, 0.65)))
            .height(Length::Fill)
            .width(Length::Fill)
            .clip(true);
        let songs = self.track_total.unwrap_or(self.tracks.len() as u64).to_string();
        let albums = self.album_total.unwrap_or(self.albums.len() as u64).to_string();
        let stats = view.i18n.format("catalog.counts", &[("songs", &songs), ("albums", &albums)]);
        let rule = container(space())
            .height(1)
            .width(Length::Fill)
            .style(move |_| container::Style { background: Some(fade(view.ink, 0.06).into()), ..Default::default() });
        let content =
            column![heading, underline, copy, rule, text(stats).size(10.0 * scale).color(fade(view.ink, 0.4))]
                .spacing(8.0 * scale)
                .padding(padding)
                .width(Length::Fill)
                .height(Length::Fill);
        button(content)
            .padding(0)
            .on_press(wrap(Message::Biography(true)))
            .style(move |_, status| {
                let hot = matches!(status, button::Status::Hovered | button::Status::Pressed);
                button::Style {
                    background: Some(
                        fade(
                            if view.daylight { Color::WHITE } else { Color::from_rgb8(24, 24, 27) },
                            if hot { 0.8 } else { 0.65 },
                        )
                        .into(),
                    ),
                    border: Border { radius: (24.0 * scale).into(), width: 1.0, color: fade(view.ink, 0.10) },
                    shadow: Shadow {
                        color: fade(Color::BLACK, if view.daylight { 0.08 } else { 0.24 }),
                        offset: Vector::new(0.0, 12.0),
                        blur_radius: 30.0,
                    },
                    ..Default::default()
                }
            })
            .into()
    }

    fn song_card<'a>(
        &'a self,
        index: usize,
        track: &'a folia_core::Track,
        placement: &Placement,
        view: View<'a>,
    ) -> Element<'a, CatalogMessage> {
        let scale = placement.scale;
        let ink = fade(view.ink, placement.opacity);
        let title = button(
            text(&track.title).size(14.0 * scale).font(crate::typeface::adapt(crate::typeface::BOLD)).color(ink),
        )
        .padding(0)
        .on_press(CatalogMessage::Play(index))
        .style(button::text);
        let mut names = row![].spacing(3.0 * scale);
        for (index, (name, action)) in crate::credits::artists(track, self.capabilities.artists).into_iter().enumerate()
        {
            if index > 0 {
                names = names.push(text("/").size(10.0 * scale).color(fade(ink, 0.5)));
            }
            names = names.push(
                button(text(name).size(10.0 * scale).color(fade(ink, 0.7)).wrapping(text::Wrapping::None))
                    .padding(0)
                    .on_press_maybe(action)
                    .style(button::text),
            );
        }
        let album = button(
            text(track.album.as_deref().unwrap_or(""))
                .size(9.0 * scale)
                .color(fade(ink, 0.55))
                .wrapping(text::Wrapping::None),
        )
        .padding(0)
        .on_press_maybe(crate::credits::album(track, self.capabilities.albums))
        .style(button::text);
        let queue = button(icon(Icon::Plus, 16.0 * scale, ink))
            .padding(4.0 * scale)
            .on_press(CatalogMessage::Enqueue(index))
            .style(button::text);
        let meta = row![
            column![
                album,
                text(crate::search_results::format_duration(track.duration)).size(9.0 * scale).color(fade(ink, 0.55))
            ]
            .width(Length::Fill),
            queue
        ]
        .align_y(Alignment::End);
        let copy = column![container(title).max_height(56.0 * scale).clip(true), container(names).clip(true), meta]
            .spacing(4.0 * scale);
        self.poster(track.cover_url.as_deref(), copy.into(), placement, view, CatalogMessage::Play(index))
    }

    fn album_card<'a>(
        &'a self,
        album: &'a folia_omni::Collection,
        placement: &Placement,
        view: View<'a>,
    ) -> Element<'a, CatalogMessage> {
        let ink = fade(view.ink, placement.opacity);
        let action = CatalogMessage::OpenAlbum(Box::new(album.clone()));
        let copy = column![
            text(&album.name)
                .size(14.0 * placement.scale)
                .font(crate::typeface::adapt(crate::typeface::BOLD))
                .color(ink),
            text(album.creator.as_deref().unwrap_or(""))
                .size(10.0 * placement.scale)
                .color(fade(ink, 0.7))
                .wrapping(text::Wrapping::None),
        ]
        .spacing(4.0 * placement.scale);
        self.poster(
            album.cover_url.as_deref(),
            button(copy).padding(0).on_press(action.clone()).style(button::text).into(),
            placement,
            view,
            action,
        )
    }

    fn poster<'a>(
        &'a self,
        url: Option<&str>,
        copy: Element<'a, CatalogMessage>,
        placement: &Placement,
        view: View<'a>,
        action: CatalogMessage,
    ) -> Element<'a, CatalogMessage> {
        let dimensions = placement.rect.size();
        let face: Element<'a, CatalogMessage> = match self.cover(url) {
            Some(handle) => image(handle.clone())
                .width(dimensions.width)
                .height(dimensions.height)
                .content_fit(iced::ContentFit::Cover)
                .border_radius(12.0 * placement.scale)
                .opacity(placement.opacity)
                .into(),
            None => container(icon(Icon::Disc3, 32.0 * placement.scale, fade(view.ink, 0.2)))
                .center(Length::Fill)
                .style(move |_| container::Style {
                    background: Some(fade(view.ink, 0.06).into()),
                    ..Default::default()
                })
                .into(),
        };
        let paint = if view.daylight { Color::WHITE } else { Color::BLACK };
        let radius = 12.0 * placement.scale;
        let gradient = iced::gradient::Linear::new(iced::Radians::PI)
            .add_stop(0.26, fade(paint, 0.0))
            .add_stop(0.5, fade(paint, 0.30))
            .add_stop(0.74, fade(paint, 0.74))
            .add_stop(1.0, fade(paint, 0.93));
        let mask = container(space()).width(Length::Fill).height(Length::Fill).style(move |_| container::Style {
            background: Some(Background::Gradient(gradient.into())),
            border: Border { radius: radius.into(), ..Default::default() },
            ..Default::default()
        });
        let base = button(stack![face, mask]).padding(0).on_press(action).style(button::text);
        let caption = container(copy)
            .width(Length::Fill)
            .height(Length::Fill)
            .align_y(Alignment::End)
            .padding(12.0 * placement.scale)
            .clip(true);
        stack![base, caption].width(dimensions.width).height(dimensions.height).clip(true).into()
    }

    fn artist_toolbar<'a>(&'a self, view: View<'a>) -> Element<'a, CatalogMessage> {
        let enabled = !self.tracks.is_empty() && !self.tracks_busy && self.all_request.is_none();
        let back = tool(Icon::ChevronLeft, None, Some(CatalogMessage::Back), view);
        let enqueue = tool(
            Icon::ListPlus,
            (view.width > 900.0).then(|| view.i18n.t("catalog.enqueueHot")),
            enabled.then_some(CatalogMessage::EnqueueAll),
            view,
        );
        let play = tool(Icon::Play, None, enabled.then_some(CatalogMessage::PlayAll), view);
        let left = pin(row![back, enqueue, play].spacing(8)).x(24).y(24);
        let badge = button(
            column![
                text(self.title())
                    .size(18)
                    .font(crate::typeface::adapt(crate::typeface::BOLD))
                    .color(view.ink)
                    .wrapping(text::Wrapping::None),
                text(view.i18n.t("catalog.artists")).size(12).color(fade(view.ink, 0.5))
            ]
            .align_x(Alignment::Center),
        )
        .padding([8, 20])
        .on_press(wrap(Message::Home))
        .style(move |_, _| crate::settings_kit::rounded_button(fade(view.background, 0.75), 16.0));
        let top = container(container(badge).max_width((view.width - 360.0).max(120.0)))
            .width(Length::Fill)
            .align_x(Alignment::Center)
            .padding(iced::Padding::ZERO.top(20));
        let mut controls = stack![top, left];
        let mut paging = row![].spacing(8);
        if self.more_tracks {
            paging = paging.push(tool(
                Icon::ListMusic,
                Some(view.i18n.t("catalog.moreSongs")),
                (!self.tracks_busy && self.all_request.is_none()).then_some(CatalogMessage::MoreTracks),
                view,
            ));
        }
        if self.more_albums {
            paging = paging.push(tool(
                Icon::Disc3,
                Some(view.i18n.t("catalog.moreAlbums")),
                (!self.albums_busy).then_some(CatalogMessage::MoreAlbums),
                view,
            ));
        }
        if self.header_busy || self.tracks_busy || self.albums_busy || self.all_request.is_some() {
            paging = paging.push(text(view.i18n.t("localMusic.searching")).size(12).color(view.muted));
        }
        controls = controls.push(
            container(paging)
                .width(Length::Fill)
                .height(Length::Fill)
                .align_y(Alignment::End)
                .align_x(Alignment::Center)
                .padding(iced::Padding::ZERO.bottom(88)),
        );
        controls.into()
    }

    fn biography_modal<'a>(&'a self, view: View<'a>) -> Element<'a, CatalogMessage> {
        let close = wrap(Message::Biography(false));
        let dim =
            mouse_area(container(space()).width(Length::Fill).height(Length::Fill).style(|_| container::Style {
                background: Some(fade(Color::BLACK, 0.55).into()),
                ..Default::default()
            }))
            .on_press(close.clone());
        let heading = row![
            text(self.title())
                .size(24)
                .font(crate::typeface::adapt(crate::typeface::BOLD))
                .color(view.ink)
                .width(Length::Fill),
            tool(Icon::X, None, Some(close), view)
        ]
        .spacing(16)
        .align_y(Alignment::Center);
        let description = self.artist.as_ref().and_then(|artist| artist.description.as_deref()).unwrap_or("");
        let copy = container(text(description).size(14).line_height(1.625).color(fade(view.ink, 0.8)))
            .padding(iced::Padding::ZERO.right(20))
            .width(Length::Fill);
        let scroll = scrollable(copy)
            .id("artist-biography")
            .height(Length::Fill)
            .on_scroll(|viewport| wrap(Message::BioScrolled(viewport)))
            .direction(crate::scrollbar::direction())
            .style(crate::scrollbar::style(view.daylight));
        let body = column![heading, crate::smooth_scroll::smooth(scroll, |delta| wrap(Message::BioWheel(delta)))]
            .spacing(20)
            .padding(24);
        let card = container(body)
            .width((view.width - 40.0).clamp(200.0, 680.0))
            .height((view.height - 80.0).max(160.0))
            .style(move |_| container::Style {
                background: Some(view.background.into()),
                border: Border { radius: 24.0.into(), width: 1.0, color: fade(view.ink, 0.12) },
                ..Default::default()
            });
        stack![dim, container(mouse_area(card).on_press(wrap(Message::Biography(true)))).center(Length::Fill)].into()
    }
}

fn tool<'a>(
    glyph: Icon,
    label: Option<&'a str>,
    action: Option<CatalogMessage>,
    view: View<'a>,
) -> Element<'a, CatalogMessage> {
    let ink = fade(view.ink, if action.is_some() { 1.0 } else { 0.4 });
    let mut face = row![icon(glyph, 16.0, ink)].spacing(6).align_y(Alignment::Center);
    if let Some(label) = label {
        face = face.push(text(label).size(12).color(ink));
    }
    let press = button(container(face).center_y(40)).padding([0, 12]).on_press_maybe(action).style(move |_, status| {
        let opacity = if matches!(status, button::Status::Hovered | button::Status::Pressed) { 0.9 } else { 0.65 };
        crate::settings_kit::rounded_button(fade(view.background, opacity), 24.0)
    });
    let key = match glyph {
        Icon::ChevronLeft => "ui.back",
        Icon::Play => "libraryBravais.playAll",
        Icon::ListPlus => "catalog.enqueueHot",
        Icon::X => "ui.close",
        _ => "home.loadMore",
    };
    tooltip(press, text(label.unwrap_or_else(|| view.i18n.t(key))).size(12), tooltip::Position::Bottom).into()
}

fn wrap(message: Message) -> CatalogMessage {
    CatalogMessage::Artist(message)
}
fn fade(color: Color, opacity: f32) -> Color {
    Color { a: color.a * opacity, ..color }
}
fn place(content: Element<'_, CatalogMessage>, rect: Rectangle) -> Element<'_, CatalogMessage> {
    pin(container(content).width(rect.width).height(rect.height)).x(rect.x).y(rect.y).into()
}

struct Measure(Size);

impl canvas::Program<CatalogMessage> for Measure {
    type State = ();
    fn update(&self, _: &mut (), event: &Event, bounds: Rectangle, _: mouse::Cursor) -> Option<Action<CatalogMessage>> {
        if matches!(event, Event::Window(iced::window::Event::RedrawRequested(_))) && self.0 != bounds.size() {
            Some(Action::publish(wrap(Message::Size(bounds.size()))))
        } else {
            None
        }
    }
    fn draw(&self, _: &(), _: &iced::Renderer, _: &Theme, _: Rectangle, _: mouse::Cursor) -> Vec<Geometry> {
        Vec::new()
    }
}
