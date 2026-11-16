// Artist and album detail routes with independent requests, paging and restored scroll positions.

use std::collections::HashSet;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

use folia_core::{OnlineId, Track};
use folia_i18n::I18n;
use folia_library::Cover;
use folia_omni::{Artist, Collection, CollectionDetail, Omni, Page, TrackCatalog};
use iced::widget::scrollable::Viewport;
use iced::widget::{button, column, container, image, row, scrollable, space};
use iced::{Alignment, Background, Border, Color, Element, Length, Task, mouse};
use lucide_static_svg::Icon;

use crate::background::{error_chain, online_cover_sized};
use crate::icons::icon;
use crate::search_results::{self, List};
use crate::typeface::text;

#[path = "artist_page.rs"]
pub mod artist_page;

const PAGE_SIZE: u32 = 30;
static GENERATION: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Clone)]
pub enum Message {
    Artist(artist_page::Message),
    TrackArtists(Box<Track>),
    TrackAlbum(Box<Track>),
    OpenArtist(OnlineId),
    OpenAlbum(Box<Collection>),
    Back,
    Retry,
    Data(u64, Box<Data>),
    Cover(u64, String, Option<Cover>),
    Backdrop(u64, String, Option<image::Handle>),
    AllReady(u64, u64, bool, Result<Vec<Track>, String>),
    Play(usize),
    PlayAll,
    Enqueue(usize),
    EnqueueAll,
    MoreTracks,
    MoreAlbums,
    Scrolled(Viewport),
    Wheel(mouse::ScrollDelta),
    Hover(Option<usize>),
    Plus(Option<usize>),
    Frame,
}

#[derive(Debug, Clone)]
pub enum Data {
    Artist(Result<Artist, String>),
    Album(Result<Collection, String>),
    Tracks(u32, Result<Page<Track>, String>),
    Albums(u32, Result<Page<Collection>, String>),
    Detail(Result<CollectionDetail, String>),
    References(Result<TrackCatalog, String>),
}

#[derive(Debug, Clone)]
enum Route {
    Artist(OnlineId),
    Album(Collection),
    References(Track, bool),
}

pub struct CatalogPage {
    generation: u64,
    active: bool,
    all_request: Option<iced::task::Handle>,
    all_ticket: u64,
    route: Route,
    artist: Option<Artist>,
    choices: Vec<folia_core::ArtistRef>,
    pub tracks: Vec<Track>,
    albums: Vec<Collection>,
    published: Option<i64>,
    album_artists: Vec<folia_core::ArtistRef>,
    capabilities: folia_omni::Capabilities,
    track_covers: Vec<Option<image::Handle>>,
    list: List,
    tracks_busy: bool,
    albums_busy: bool,
    header_busy: bool,
    error: bool,
    next_track: u32,
    next_album: u32,
    more_tracks: bool,
    more_albums: bool,
    covers: folia_library::CoverCache<String, image::Handle>,
    pending: HashSet<String>,
    artist_page: artist_page::State,
    track_total: Option<u64>,
    album_total: Option<u64>,
}

impl CatalogPage {
    pub fn open(message: Message, omni: Option<&Omni>) -> Option<(Self, Task<Message>)> {
        let route = match message {
            Message::OpenArtist(id) => Route::Artist(id),
            Message::OpenAlbum(album) => Route::Album(*album),
            Message::TrackArtists(track) => Route::References(*track, true),
            Message::TrackAlbum(track) => Route::References(*track, false),
            _ => return None,
        };
        let mut page = Self {
            generation: GENERATION.fetch_add(1, Ordering::Relaxed),
            active: true,
            all_request: None,
            all_ticket: 0,
            route,
            artist: None,
            choices: Vec::new(),
            tracks: Vec::new(),
            albums: Vec::new(),
            published: None,
            album_artists: Vec::new(),
            capabilities: folia_omni::Capabilities::default(),
            track_covers: Vec::new(),
            list: List::default(),
            tracks_busy: false,
            albums_busy: false,
            header_busy: false,
            error: false,
            next_track: 0,
            next_album: 0,
            more_tracks: false,
            more_albums: false,
            covers: folia_library::CoverCache::new(8 * 1024 * 1024),
            pending: HashSet::new(),
            artist_page: artist_page::State::default(),
            track_total: None,
            album_total: None,
        };
        let task = page.load(omni);
        Some((page, task))
    }

    pub fn title(&self) -> &str {
        match &self.route {
            Route::Artist(_) => self.artist.as_ref().map_or("", |artist| artist.name.as_str()),
            Route::Album(album) => &album.name,
            Route::References(track, _) => &track.title,
        }
    }

    pub fn accepts(&self, message: &Message) -> bool {
        matches!(message, Message::Data(id, _) | Message::Cover(id, ..) | Message::Backdrop(id, ..) if *id == self.generation)
    }

    pub fn accepts_all(&self, message: &Message) -> bool {
        matches!(message, Message::AllReady(generation, ticket, ..) if self.all_request.is_some() && *generation == self.generation && *ticket == self.all_ticket)
    }

    pub fn generation(&self) -> u64 {
        self.generation
    }

    pub(crate) fn close_biography(&mut self) -> bool {
        self.artist_page.close_biography()
    }

    pub fn suspend(&mut self) {
        self.artist_page.suspend();
        self.active = false;
        self.all_request = None;
        self.all_ticket = self.all_ticket.wrapping_add(1);
        self.track_covers.fill(None);
        self.covers.clear();
        self.pending.clear();
    }

    pub fn activate(&mut self, omni: Option<&Omni>) -> Task<Message> {
        self.active = true;
        let reload = if self.header_busy || self.tracks_busy || self.albums_busy {
            self.tracks_busy = false;
            self.albums_busy = false;
            self.load(omni)
        } else {
            Task::none()
        };
        Task::batch([reload, self.restore(), self.load_covers(omni)])
    }

    pub fn snapshot(&self) -> Self {
        Self {
            generation: GENERATION.fetch_add(1, Ordering::Relaxed),
            active: false,
            all_request: None,
            all_ticket: 0,
            route: self.route.clone(),
            artist: self.artist.clone(),
            choices: self.choices.clone(),
            tracks: self.tracks.clone(),
            albums: self.albums.clone(),
            published: self.published,
            album_artists: self.album_artists.clone(),
            capabilities: self.capabilities,
            track_covers: vec![None; self.tracks.len()],
            list: self.list.clone(),
            tracks_busy: self.tracks_busy,
            albums_busy: self.albums_busy,
            header_busy: self.header_busy,
            error: self.error,
            next_track: self.next_track,
            next_album: self.next_album,
            more_tracks: self.more_tracks,
            more_albums: self.more_albums,
            covers: folia_library::CoverCache::new(8 * 1024 * 1024),
            pending: HashSet::new(),
            artist_page: self.artist_page.snapshot(),
            track_total: self.track_total,
            album_total: self.album_total,
        }
    }

    pub fn prepare_all(&mut self, enqueue: bool, omni: Option<&Omni>) -> Option<Task<Message>> {
        if !self.more_tracks {
            return None;
        }
        if self.tracks_busy || self.all_request.is_some() {
            return Some(Task::none());
        }
        let (Some(omni), Route::Artist(id)) = (omni.cloned(), &self.route) else { return Some(Task::none()) };
        let (id, generation, offset, tracks) = (id.clone(), self.generation, self.next_track, self.tracks.clone());
        self.all_ticket = self.all_ticket.wrapping_add(1);
        let ticket = self.all_ticket;
        let task = Task::perform(all_tracks(omni, id, offset, tracks), move |result| {
            Message::AllReady(generation, ticket, enqueue, result)
        });
        let (task, handle) = task.abortable();
        self.all_request = Some(handle.abort_on_drop());
        Some(task)
    }

    pub fn restore(&self) -> Task<Message> {
        if matches!(self.route, Route::Artist(_)) { Task::none() } else { self.list.apply() }
    }

    pub fn animating(&self, now: Instant) -> bool {
        self.list.animating(now) || self.artist_page.animating(now)
    }

    fn load(&mut self, omni: Option<&Omni>) -> Task<Message> {
        let Some(omni) = omni.cloned() else {
            self.error = true;
            return Task::none();
        };
        self.capabilities = omni.capabilities(match &self.route {
            Route::Artist(id) => &id.provider,
            Route::Album(album) => &album.provider,
            Route::References(track, _) => track.online.as_ref().map_or("", |id| id.provider.as_str()),
        });
        self.error = false;
        self.header_busy = true;
        let generation = self.generation;
        match self.route.clone() {
            Route::Artist(id) => {
                let client = omni.clone();
                let heading = Task::perform(
                    async move { Data::Artist(client.artist_detail(&id).await.map_err(catalog_error)) },
                    move |data| Message::Data(generation, Box::new(data)),
                );
                Task::batch([heading, self.load_tracks(Some(&omni)), self.load_albums(Some(&omni))])
            }
            Route::Album(album) => {
                let id = OnlineId { provider: album.provider.clone(), id: album.id.clone() };
                Task::perform(async move { Data::Album(omni.album(&id).await.map_err(catalog_error)) }, move |data| {
                    Message::Data(generation, Box::new(data))
                })
            }
            Route::References(track, _) => Task::perform(
                async move { Data::References(omni.track_catalog(&track).await.map_err(catalog_error)) },
                move |data| Message::Data(generation, Box::new(data)),
            ),
        }
    }

    fn load_tracks(&mut self, omni: Option<&Omni>) -> Task<Message> {
        let (Some(omni), Route::Artist(id)) = (omni.cloned(), &self.route) else { return Task::none() };
        if self.tracks_busy || self.all_request.is_some() {
            return Task::none();
        }
        self.tracks_busy = true;
        let (generation, id, offset) = (self.generation, id.clone(), self.next_track);
        Task::perform(
            async move { Data::Tracks(offset, omni.artist_tracks(&id, PAGE_SIZE, offset).await.map_err(catalog_error)) },
            move |data| Message::Data(generation, Box::new(data)),
        )
    }

    fn load_albums(&mut self, omni: Option<&Omni>) -> Task<Message> {
        let (Some(omni), Route::Artist(id)) = (omni.cloned(), &self.route) else { return Task::none() };
        if self.albums_busy || !omni.capabilities(&id.provider).albums {
            return Task::none();
        }
        self.albums_busy = true;
        let (generation, id, offset) = (self.generation, id.clone(), self.next_album);
        Task::perform(
            async move { Data::Albums(offset, omni.artist_albums(&id, PAGE_SIZE, offset).await.map_err(catalog_error)) },
            move |data| Message::Data(generation, Box::new(data)),
        )
    }

    pub fn update(&mut self, message: Message, omni: Option<&Omni>) -> Task<Message> {
        match message {
            Message::Artist(message) => {
                self.artist_page.update(message);
                return Task::batch([self.artist_page.scroll_task(), self.load_covers(omni)]);
            }
            Message::Backdrop(generation, url, cover)
                if self.active
                    && generation == self.generation
                    && self.artist_page.backdrop_url.as_ref() == Some(&url) =>
            {
                self.artist_page.backdrop = cover;
            }
            Message::Data(generation, data) if generation == self.generation => return self.land(*data, omni),
            Message::Cover(generation, url, cover) if self.active && generation == self.generation => {
                self.pending.remove(&url);
                let bytes = cover.as_ref().map_or(0, |cover| cover.rgba.len());
                self.covers.insert(
                    url,
                    cover.map(|cover| image::Handle::from_rgba(cover.width, cover.height, cover.rgba)),
                    bytes,
                );
                self.sync_covers();
            }
            Message::AllReady(generation, ticket, _, result)
                if self.all_request.is_some() && generation == self.generation && ticket == self.all_ticket =>
            {
                self.all_request = None;
                match result {
                    Ok(tracks) => {
                        self.tracks = tracks;
                        self.more_tracks = false;
                        self.sync_covers();
                    }
                    Err(_) => self.error = true,
                }
                return self.load_covers(omni);
            }
            Message::Retry => {
                self.generation = GENERATION.fetch_add(1, Ordering::Relaxed);
                self.all_request = None;
                self.pending.clear();
                self.tracks_busy = false;
                self.albums_busy = false;
                self.next_track = 0;
                self.next_album = 0;
                self.tracks.clear();
                self.albums.clear();
                self.list.reset();
                return self.load(omni);
            }
            Message::MoreTracks if self.more_tracks => return self.load_tracks(omni),
            Message::MoreAlbums if self.more_albums => return self.load_albums(omni),
            Message::Scrolled(viewport) => {
                self.list.scrolled(&viewport, Instant::now());
                return self.load_covers(omni);
            }
            Message::Wheel(delta) => {
                self.list.wheel(delta, Instant::now());
                return self.restore();
            }
            Message::Hover(index) => self.list.hovered = index,
            Message::Plus(index) => self.list.plus(index),
            Message::Frame => {
                if matches!(self.route, Route::Artist(_)) {
                    self.artist_page.frame(Instant::now());
                    return Task::batch([self.artist_page.scroll_task(), self.load_covers(omni)]);
                }
                return self.list.frame(Instant::now());
            }
            _ => {}
        }
        Task::none()
    }

    fn land(&mut self, data: Data, omni: Option<&Omni>) -> Task<Message> {
        match data {
            Data::Artist(result) => {
                self.header_busy = false;
                match result {
                    Ok(artist) => self.artist = Some(artist),
                    Err(_) => self.error = true,
                }
            }
            Data::Album(result) => {
                self.header_busy = false;
                let album = match result {
                    Ok(album) => album,
                    Err(_) => {
                        self.error = true;
                        match &self.route {
                            Route::Album(album) => album.clone(),
                            _ => return Task::none(),
                        }
                    }
                };
                self.route = Route::Album(album.clone());
                if let Some(client) = omni.cloned() {
                    self.tracks_busy = true;
                    let generation = self.generation;
                    let task = Task::perform(
                        async move { Data::Detail(client.collection_detail(&album).await.map_err(catalog_error)) },
                        move |data| Message::Data(generation, Box::new(data)),
                    );
                    return Task::batch([task, self.load_covers(omni)]);
                }
            }
            Data::Detail(result) => {
                self.tracks_busy = false;
                match result {
                    Ok(detail) => {
                        self.tracks = detail.tracks;
                        self.published = detail.published;
                        self.album_artists = detail.artists;
                    }
                    Err(_) => self.error = true,
                }
            }
            Data::Tracks(offset, result) if self.tracks_busy && offset == self.next_track => {
                self.tracks_busy = false;
                match result {
                    Ok(page) => {
                        self.track_total = page.total;
                        self.more_tracks = page.has_more && page.next_offset > offset;
                        self.next_track = page.next_offset;
                        for track in page.items {
                            if !self.tracks.iter().any(|old| old.online == track.online && old.id == track.id) {
                                self.tracks.push(track);
                            }
                        }
                    }
                    Err(_) => {
                        self.error = true;
                        self.more_tracks = true;
                    }
                }
            }
            Data::Albums(offset, result) if self.albums_busy && offset == self.next_album => {
                self.albums_busy = false;
                match result {
                    Ok(page) => {
                        self.album_total = page.total;
                        self.more_albums = page.has_more && page.next_offset > offset;
                        self.next_album = page.next_offset;
                        for album in page.items {
                            if !self.albums.iter().any(|old| old.provider == album.provider && old.id == album.id) {
                                self.albums.push(album);
                            }
                        }
                    }
                    Err(_) => {
                        self.error = true;
                        self.more_albums = true;
                    }
                }
            }
            Data::References(result) => {
                self.header_busy = false;
                let Route::References(track, artists) = &self.route else { return Task::none() };
                let provider = track.online.as_ref().map(|id| id.provider.clone()).unwrap_or_default();
                match result {
                    Ok(references) if *artists => {
                        self.choices = references.artists;
                        if self.choices.len() == 1 {
                            self.route = Route::Artist(OnlineId { provider, id: self.choices[0].id.clone() });
                            return self.load(omni);
                        }
                        self.error = self.choices.is_empty();
                    }
                    Ok(references) => {
                        if let Some(album) = references.album {
                            self.route = Route::Album(album);
                            return self.load(omni);
                        }
                        self.error = true;
                    }
                    Err(_) => self.error = true,
                }
            }
            _ => {}
        }
        self.sync_covers();
        self.load_covers(omni)
    }

    fn sync_covers(&mut self) {
        self.artist_page.refresh(self.tracks.len(), self.albums.len());
        self.track_covers = self.tracks.iter().map(|track| self.cover(track.cover_url.as_deref()).cloned()).collect();
    }

    fn picture_url(&self) -> Option<&str> {
        match &self.route {
            Route::Artist(_) => self.artist.as_ref()?.picture_url.as_deref(),
            Route::Album(album) => album.cover_url.as_deref(),
            Route::References(track, _) => track.cover_url.as_deref(),
        }
    }

    fn load_covers(&mut self, omni: Option<&Omni>) -> Task<Message> {
        if !self.active {
            return Task::none();
        }
        let Some(omni) = omni.cloned() else { return Task::none() };
        let mut urls = Vec::new();
        if let Some(url) = self.picture_url() {
            urls.push(url.to_owned());
        }
        if matches!(self.route, Route::Artist(_)) {
            urls.extend(
                self.artist_page.visible_tracks().filter_map(|index| self.tracks.get(index)?.cover_url.clone()),
            );
            urls.extend(
                self.artist_page.visible_albums().filter_map(|index| self.albums.get(index)?.cover_url.clone()),
            );
        } else {
            urls.extend(
                self.tracks[self.list.visible(self.tracks.len())].iter().filter_map(|track| track.cover_url.clone()),
            );
        }
        self.covers.begin_pass();
        let mut tasks = Vec::new();
        if matches!(self.route, Route::Artist(_))
            && let Some(url) = self.tracks.first().and_then(|track| track.cover_url.clone())
            && self.artist_page.backdrop_url.as_ref() != Some(&url)
        {
            self.artist_page.backdrop_url = Some(url.clone());
            let client = omni.clone();
            let generation = self.generation;
            let reply_url = url.clone();
            tasks.push(Task::perform(
                async move {
                    let cover = online_cover_sized(client, url, 150, 150).await?;
                    crate::background::blocking(move || {
                        let pixels = ::image::RgbaImage::from_raw(cover.width, cover.height, cover.rgba)?;
                        let blurred = ::image::imageops::blur(&pixels, 4.0);
                        Some(iced::widget::image::Handle::from_rgba(
                            blurred.width(),
                            blurred.height(),
                            blurred.into_raw(),
                        ))
                    })
                    .await
                    .flatten()
                },
                move |cover| Message::Backdrop(generation, reply_url.clone(), cover),
            ));
        }
        for url in urls {
            self.covers.touch(&url);
            if self.covers.peek(&url).is_some() || !self.pending.insert(url.clone()) {
                continue;
            }
            let generation = self.generation;
            tasks.push(Task::perform(online_cover_sized(omni.clone(), url.clone(), 256, 256), move |cover| {
                Message::Cover(generation, url.clone(), cover)
            }));
        }
        Task::batch(tasks)
    }

    fn cover(&self, url: Option<&str>) -> Option<&image::Handle> {
        self.covers.peek(&url?.to_owned()).and_then(Option::as_ref)
    }

    pub fn view<'a>(&'a self, view: View<'a>) -> Element<'a, Message> {
        if matches!(self.route, Route::Artist(_)) {
            return self.artist_view(view);
        }
        let compact = view.height < 560.0;
        let back =
            button(row![icon(Icon::ChevronLeft, 18.0, view.ink), text(view.i18n.t("ui.back")).size(13)].spacing(6))
                .padding([8, 12])
                .on_press(Message::Back)
                .style(button::text);
        let narrow = view.width < 640.0;
        let portrait = self.picture(
            self.picture_url(),
            if compact {
                72.0
            } else if narrow {
                84.0
            } else {
                140.0
            },
            matches!(self.route, Route::Artist(_)),
            view.ink,
        );
        let mut words = column![
            text(self.title())
                .size(if narrow || compact { 22 } else { 28 })
                .font(crate::typeface::adapt(crate::typeface::SEMIBOLD))
                .color(view.ink)
        ]
        .spacing(8);
        match &self.route {
            Route::Artist(_) => {}
            Route::Album(album) => {
                if !self.album_artists.is_empty() {
                    let mut credits = row![].spacing(8);
                    for artist in &self.album_artists {
                        credits = credits.push(
                            button(text(&artist.name).size(14).color(view.muted))
                                .padding(0)
                                .on_press_maybe((self.capabilities.artists && !artist.id.is_empty()).then(|| {
                                    Message::OpenArtist(OnlineId {
                                        provider: album.provider.clone(),
                                        id: artist.id.clone(),
                                    })
                                }))
                                .style(button::text),
                        );
                    }
                    words = words.push(credits);
                } else if let Some(creator) = &album.creator {
                    words = words.push(text(creator).size(14).color(view.muted));
                }
                if let Some(published) = self.published {
                    words = words.push(text(crate::poster::year_of(published).to_string()).size(13).color(view.muted));
                }
                if let Some(description) = &album.description {
                    words = words.push(
                        scrollable(
                            container(text(description).size(13).color(view.muted))
                                .padding(iced::Padding::ZERO.right(20)),
                        )
                        .direction(crate::scrollbar::direction())
                        .style(crate::scrollbar::style(view.daylight))
                        .height(64),
                    );
                }
            }
            Route::References(_, _) => {}
        }
        let words: Element<'a, Message> = if compact {
            scrollable(container(words).padding(iced::Padding::ZERO.right(20)))
                .direction(crate::scrollbar::direction())
                .style(crate::scrollbar::style(view.daylight))
                .height(72)
                .width(Length::Fill)
                .into()
        } else {
            words.width(Length::Fill).into()
        };
        let header = row![portrait, words].spacing(if narrow { 14 } else { 24 }).align_y(Alignment::Center);
        let mut actions = row![].spacing(10).align_y(Alignment::Center);
        let actionable = !self.tracks.is_empty() && !self.tracks_busy && self.all_request.is_none();
        let action_ink = Color { a: view.ink.a * if actionable { 1.0 } else { 0.4 }, ..view.ink };
        for (label, glyph, message) in [
            ("libraryBravais.playAll", Icon::Play, Message::PlayAll),
            ("libraryBravais.enqueueAll", Icon::ListPlus, Message::EnqueueAll),
        ] {
            actions = actions.push(
                button(
                    row![icon(glyph, 16.0, action_ink), text(view.i18n.t(label)).size(13).color(action_ink)]
                        .spacing(8)
                        .align_y(Alignment::Center),
                )
                .padding(if compact { [6, 12] } else { [8, 14] })
                .on_press_maybe(actionable.then_some(message))
                .style(move |_, status| {
                    let hot = matches!(status, button::Status::Hovered | button::Status::Pressed);
                    crate::settings_kit::rounded_button(Color { a: if hot { 0.12 } else { 0.06 }, ..view.ink }, 12.0)
                }),
            );
        }
        let mut body = column![back, header, actions].spacing(if compact { 10 } else { 16 });
        if self.error {
            body = body.push(
                row![
                    text(view.i18n.t("catalog.error")).size(13).color(view.muted),
                    button(text(view.i18n.t("search.retry")).size(13)).on_press(Message::Retry).style(button::text)
                ]
                .spacing(8),
            );
        }
        if !self.choices.is_empty() && matches!(self.route, Route::References(..)) {
            let Route::References(track, _) = &self.route else { return space().into() };
            let provider = track.online.as_ref().map(|id| id.provider.as_str()).unwrap_or_default();
            for artist in &self.choices {
                body = body.push(
                    button(text(&artist.name).size(18).color(view.ink))
                        .on_press(Message::OpenArtist(OnlineId {
                            provider: provider.to_owned(),
                            id: artist.id.clone(),
                        }))
                        .style(button::text),
                );
            }
        } else {
            body = body.push(self.track_list(view));
        }
        if self.header_busy || self.tracks_busy || self.albums_busy || self.all_request.is_some() {
            body = body.push(text(view.i18n.t("localMusic.searching")).size(12).color(view.muted));
        }
        container(container(body).width(Length::Fixed(view.width.min(1080.0))))
            .padding(iced::Padding {
                top: 32.0,
                bottom: if compact { 12.0 } else { 32.0 },
                left: if narrow { 12.0 } else { 24.0 },
                right: if narrow { 12.0 } else { 24.0 },
            })
            .height(Length::Fill)
            .width(Length::Fill)
            .align_x(Alignment::Center)
            .style(move |_| container::Style {
                background: Some(Background::Color(view.background)),
                ..Default::default()
            })
            .into()
    }

    fn picture<'a>(&'a self, url: Option<&str>, size: f32, round: bool, ink: Color) -> Element<'a, Message> {
        match self.cover(url) {
            Some(handle) => image(handle.clone())
                .width(size)
                .height(size)
                .content_fit(iced::ContentFit::Cover)
                .border_radius(if round { size / 2.0 } else { 16.0 })
                .into(),
            None => container(icon(if round { Icon::UserRound } else { Icon::Disc3 }, size * 0.3, ink))
                .center(Length::Fixed(size))
                .style(move |_| container::Style {
                    background: Some(Color { a: 0.06, ..ink }.into()),
                    border: Border { radius: if round { size / 2.0 } else { 16.0 }.into(), ..Default::default() },
                    ..Default::default()
                })
                .into(),
        }
    }

    fn track_list<'a>(&'a self, view: View<'a>) -> Element<'a, Message> {
        if self.tracks.is_empty() && !self.header_busy && !self.tracks_busy {
            return container(text(view.i18n.t("home.noResults")).size(14).color(view.muted))
                .center(Length::Fill)
                .into();
        }
        let list = self.list.view(
            &self.tracks,
            &self.track_covers,
            search_results::Look {
                text: view.ink,
                secondary: view.muted,
                daylight: view.daylight,
                wide: view.width >= 640.0,
                queue_tip: view.i18n.t("navidrome.addToQueue"),
                artists: false,
                catalog: self.capabilities,
            },
            search_results::Bindings {
                scroll: Message::Scrolled,
                wheel: Message::Wheel,
                hover: Message::Hover,
                play: Message::Play,
                enqueue: Message::Enqueue,
                plus: Message::Plus,
                catalog: |message| message,
            },
        );
        let mut column = column![list].height(Length::Fill);
        if self.more_tracks {
            column = column.push(
                button(text(view.i18n.t("home.loadMore")).size(13))
                    .on_press_maybe((!self.tracks_busy && self.all_request.is_none()).then_some(Message::MoreTracks))
                    .style(button::text),
            );
        }
        column.into()
    }
}

#[derive(Clone, Copy)]
pub struct View<'a> {
    pub i18n: &'a I18n,
    pub ink: Color,
    pub muted: Color,
    pub accent: Color,
    pub background: Color,
    pub daylight: bool,
    pub width: f32,
    pub height: f32,
}

fn catalog_error(error: folia_omni::OmniError) -> String {
    tracing::warn!(error = &error as &dyn std::error::Error, "catalog request failed");
    error_chain(&error)
}

async fn all_tracks(
    omni: Omni,
    artist: OnlineId,
    mut offset: u32,
    mut tracks: Vec<Track>,
) -> Result<Vec<Track>, String> {
    loop {
        let page = omni.artist_tracks(&artist, PAGE_SIZE, offset).await.map_err(catalog_error)?;
        for track in page.items {
            if !tracks.iter().any(|old| old.id == track.id && old.online == track.online) {
                tracks.push(track);
            }
        }
        if !page.has_more {
            return Ok(tracks);
        }
        if page.next_offset <= offset {
            return Err(catalog_error(folia_omni::OmniError::Response("artist page did not advance".into())));
        }
        offset = page.next_offset;
    }
}
