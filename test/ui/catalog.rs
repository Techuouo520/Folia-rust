// Public search and catalog messages reject stale responses and keep submitted pagination identities.

use folia_core::{AudioQuality, OnlineId, Track};
use folia_omni::{Artist, BoxFuture, Capabilities, Collection, CollectionDetail, Omni, OmniError, Page, Provider};
use folia_ui::catalog::{CatalogPage, Data, Message};
use folia_ui::{SearchMessage, SearchPage};
use iced::futures::StreamExt;

struct Fixture;

fn unsupported<T: Send + 'static>() -> BoxFuture<'static, Result<T, OmniError>> {
    Box::pin(async { Err(OmniError::Unsupported("fixture")) })
}

impl Provider for Fixture {
    fn id(&self) -> &'static str {
        "fixture"
    }
    fn display_name(&self) -> &'static str {
        "Fixture"
    }
    fn capabilities(&self) -> Capabilities {
        Capabilities { artist_search: true, artists: true, albums: true, ..Default::default() }
    }
    fn search_songs<'a>(
        &'a self,
        query: &'a str,
        _: u32,
        offset: u32,
    ) -> BoxFuture<'a, Result<Page<Track>, OmniError>> {
        Box::pin(async move {
            Ok(Page {
                items: vec![Track { id: offset as i64, title: format!("{query}-{offset}"), ..Default::default() }],
                total: Some(2),
                has_more: offset == 0,
                next_offset: offset + 1,
            })
        })
    }
    fn artist_detail<'a>(&'a self, artist: &'a OnlineId) -> BoxFuture<'a, Result<Artist, OmniError>> {
        Box::pin(async move {
            Ok(Artist {
                provider: artist.provider.clone(),
                id: artist.id.clone(),
                name: format!("Artist {}", artist.id),
                ..Default::default()
            })
        })
    }
    fn artist_tracks<'a>(
        &'a self,
        _: &'a OnlineId,
        limit: u32,
        offset: u32,
    ) -> BoxFuture<'a, Result<Page<Track>, OmniError>> {
        self.search_songs("popular", limit, offset)
    }
    fn artist_albums<'a>(
        &'a self,
        _: &'a OnlineId,
        _: u32,
        offset: u32,
    ) -> BoxFuture<'a, Result<Page<Collection>, OmniError>> {
        Box::pin(async move { Ok(Page::empty(offset)) })
    }
    fn audio_source<'a>(
        &'a self,
        _: &'a OnlineId,
        _: AudioQuality,
    ) -> BoxFuture<'a, Result<folia_omni::AudioSource, OmniError>> {
        unsupported()
    }
    fn lyrics<'a>(&'a self, _: &'a OnlineId) -> BoxFuture<'a, Result<Option<folia_omni::ProviderLyrics>, OmniError>> {
        unsupported()
    }
    fn login_code(&self) -> BoxFuture<'_, Result<folia_omni::LoginCode, OmniError>> {
        unsupported()
    }
    fn check_login<'a>(&'a self, _: &'a str) -> BoxFuture<'a, Result<folia_omni::LoginState, OmniError>> {
        unsupported()
    }
    fn account(&self) -> BoxFuture<'_, Result<Option<folia_omni::Account>, OmniError>> {
        unsupported()
    }
    fn logout(&self) -> BoxFuture<'_, Result<(), OmniError>> {
        unsupported()
    }
    fn session(&self) -> Option<String> {
        None
    }
    fn restore_session(&self, _: &str) -> Result<(), OmniError> {
        Ok(())
    }
    fn user_playlists<'a>(&'a self, _: &'a str) -> BoxFuture<'a, Result<Vec<Collection>, OmniError>> {
        unsupported()
    }
    fn collection_detail<'a>(&'a self, _: &'a Collection) -> BoxFuture<'a, Result<CollectionDetail, OmniError>> {
        unsupported()
    }
    fn set_subscribed<'a>(&'a self, _: &'a Collection, _: bool) -> BoxFuture<'a, Result<(), OmniError>> {
        unsupported()
    }
    fn favorite_albums(&self) -> BoxFuture<'_, Result<Vec<Collection>, OmniError>> {
        unsupported()
    }
    fn radio_feed(&self, _: u32) -> BoxFuture<'_, Result<folia_omni::RadioFeed, OmniError>> {
        unsupported()
    }
    fn explore(&self) -> BoxFuture<'_, Result<Vec<folia_omni::ExploreSection>, OmniError>> {
        unsupported()
    }
    fn sized_cover(&self, _: &str, _: u32) -> Option<String> {
        None
    }
}

fn outputs<M: Send + 'static>(task: iced::Task<M>) -> Vec<M> {
    pollster::block_on(async {
        let mut messages = Vec::new();
        if let Some(mut stream) = iced_runtime::task::into_stream(task) {
            while let Some(action) = stream.next().await {
                if let iced_runtime::Action::Output(message) = action {
                    messages.push(message);
                }
            }
        }
        messages
    })
}

fn omni(path: &std::path::Path) -> Omni {
    Omni::with_providers(path.into(), "unused".into(), vec![Box::new(Fixture)]).unwrap()
}

#[test]
fn search_paging_keeps_the_committed_query_and_rejects_superseded_results() {
    let temp = tempfile::tempdir().unwrap();
    let omni = omni(temp.path());
    let mut search = SearchPage::default();
    search.begin("old".into());
    let (old, _) = search.update(SearchMessage::Submit, Some(&omni), "fixture");
    search.begin("committed".into());
    let (current, _) = search.update(SearchMessage::Submit, Some(&omni), "fixture");
    for message in outputs(old) {
        drop(search.update(message, Some(&omni), "fixture"));
    }
    assert!(search.results.is_empty());
    for message in outputs(current) {
        drop(search.update(message, Some(&omni), "fixture"));
    }
    assert_eq!(search.results[0].title, "committed-0");
    drop(search.update(SearchMessage::Query("unfinished edit".into()), Some(&omni), "fixture"));
    let (more, _) = search.update(SearchMessage::LoadMore, Some(&omni), "fixture");
    let messages = outputs(more);
    for message in &messages {
        drop(search.update(message.clone(), Some(&omni), "fixture"));
    }
    for message in messages {
        drop(search.update(message, Some(&omni), "fixture"));
    }
    assert_eq!(
        search.results.iter().map(|track| track.title.as_str()).collect::<Vec<_>>(),
        ["committed-0", "committed-1"]
    );
    assert_eq!(search.query(), "unfinished edit");
}

#[test]
fn playing_all_waits_for_remaining_pages_and_cancellation_rejects_queued_completion() {
    let temp = tempfile::tempdir().unwrap();
    let omni = omni(temp.path());
    let open = || Message::OpenArtist(OnlineId { provider: "fixture".into(), id: "one".into() });
    let (_, old) = CatalogPage::open(open(), Some(&omni)).unwrap();
    let (mut page, task) = CatalogPage::open(open(), Some(&omni)).unwrap();
    for message in outputs(old) {
        drop(page.update(message, Some(&omni)));
    }
    assert!(page.tracks.is_empty());
    for message in outputs(task) {
        drop(page.update(message, Some(&omni)));
    }
    assert_eq!(page.tracks.len(), 1);
    let task = page.prepare_all(false, Some(&omni)).unwrap();
    assert_eq!(page.update(Message::MoreTracks, Some(&omni)).units(), 0);
    let mut messages = outputs(task);
    let completion = messages.pop().unwrap();
    assert!(page.accepts_all(&completion));
    page.suspend();
    drop(page.activate(Some(&omni)));
    assert!(!page.accepts_all(&completion));
    drop(page.update(completion, Some(&omni)));
    assert_eq!(page.tracks.len(), 1);
    let task = page.prepare_all(false, Some(&omni)).unwrap();
    for message in outputs(task) {
        drop(page.update(message, Some(&omni)));
    }
    assert_eq!(page.tracks.len(), 2);
    assert!(page.prepare_all(false, Some(&omni)).is_none());
}

#[test]
fn artist_cover_cache_releases_offscreen_results_before_new_downloads() {
    let temp = tempfile::tempdir().unwrap();
    let omni = omni(temp.path());
    let (mut page, task) =
        CatalogPage::open(Message::OpenArtist(OnlineId { provider: "fixture".into(), id: "one".into() }), Some(&omni))
            .unwrap();
    drop(task);
    let tracks: Vec<_> = (0..128)
        .map(|id| Track { id, cover_url: Some(format!("https://fixture.invalid/{id}.png")), ..Default::default() })
        .collect();
    let generation = page.generation();
    drop(page.update(
        Message::Data(
            generation,
            Box::new(Data::Tracks(0, Ok(Page { items: tracks, total: Some(128), has_more: false, next_offset: 128 }))),
        ),
        Some(&omni),
    ));
    for id in 0..128 {
        let cover = folia_library::Cover { width: 256, height: 256, rgba: vec![255; 256 * 256 * 4] };
        drop(
            page.update(
                Message::Cover(generation, format!("https://fixture.invalid/{id}.png"), Some(cover)),
                Some(&omni),
            ),
        );
    }
    use folia_ui::catalog::artist_page::Message as ArtistMessage;
    drop(page.update(Message::Artist(ArtistMessage::Home), Some(&omni)));
    let mut requests = 0;
    for y in [1000.0, -1000.0, 2000.0, -2000.0] {
        drop(page.update(Message::Artist(ArtistMessage::Press(iced::Point::ORIGIN)), Some(&omni)));
        requests += page.update(Message::Artist(ArtistMessage::Move(iced::Point::new(0.0, y))), Some(&omni)).units();
        drop(page.update(Message::Artist(ArtistMessage::Release), Some(&omni)));
    }
    assert!(requests > 0, "offscreen cover replies remained pinned above the cache budget");
}

#[derive(Default)]
struct BiographyLayout {
    text: Option<iced::Rectangle>,
    scroll: Option<iced::Rectangle>,
}

impl iced::advanced::widget::Operation for BiographyLayout {
    fn traverse(&mut self, operate: &mut dyn FnMut(&mut dyn iced::advanced::widget::Operation)) {
        operate(self);
    }

    fn text(&mut self, _: Option<&iced::widget::Id>, bounds: iced::Rectangle, text: &str) {
        if text.starts_with("Long biography") {
            self.text = Some(bounds);
        }
    }

    fn scrollable(
        &mut self,
        _: Option<&iced::widget::Id>,
        bounds: iced::Rectangle,
        _: iced::Rectangle,
        _: iced::Vector,
        _: &mut dyn iced::advanced::widget::operation::Scrollable,
    ) {
        self.scroll = Some(bounds);
    }
}

#[test]
#[cfg_attr(feature = "ci", ignore = "needs a renderer for biography layout")]
fn biography_scrollbar_reserves_text_space() {
    use iced::advanced::renderer::Headless;
    use iced::{Color, Size};
    use iced_runtime::user_interface::{Cache, UserInterface};
    folia_ui::install_fonts();
    let temp = tempfile::tempdir().unwrap();
    let omni = omni(temp.path());
    let (mut page, task) =
        CatalogPage::open(Message::OpenArtist(OnlineId { provider: "fixture".into(), id: "one".into() }), Some(&omni))
            .unwrap();
    drop(task);
    let artist = Artist {
        name: "Artist".into(),
        description: Some("Long biography with enough text to overflow. ".repeat(40)),
        ..Default::default()
    };
    drop(page.update(Message::Data(page.generation(), Box::new(Data::Artist(Ok(artist)))), Some(&omni)));
    let mut renderer =
        pollster::block_on(<iced::Renderer as Headless>::new(folia_ui::INTERFACE_FONT, iced::Pixels(16.0), None))
            .unwrap();
    let i18n = folia_i18n::I18n::new(folia_core::Language::En).unwrap();
    let size = Size::new(683.0, 384.0);
    let view = folia_ui::catalog::View {
        i18n: &i18n,
        ink: Color::BLACK,
        muted: Color::BLACK,
        accent: Color::BLACK,
        background: Color::WHITE,
        daylight: true,
        width: size.width,
        height: size.height,
    };
    for expanded in [false, true] {
        drop(page.update(Message::Artist(folia_ui::catalog::artist_page::Message::Biography(expanded)), Some(&omni)));
        let mut ui = UserInterface::build(page.view(view), size, Cache::default(), &mut renderer);
        let mut geometry = BiographyLayout::default();
        ui.operate(&mut renderer, &mut geometry);
        let text = geometry.text.unwrap();
        if expanded {
            let viewport = geometry.scroll.unwrap();
            assert!(text.x + text.width <= viewport.x + viewport.width - 16.0);
            assert!(viewport.y >= 0.0 && viewport.y + viewport.height <= size.height);
        } else {
            assert!(geometry.scroll.is_none(), "summary must not create an inner scrollbar");
        }
    }
}
