// The online account and the home tabs' online data, all through the `Omni` facade and off the UI thread.

use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};

use folia_omni::{Account, Collection, ExploreSection, Omni, RadioFeed};
use iced::Task;

use crate::platform_rows::{ProviderRowStatus, order_provider_rows};

use crate::background::{blocking, error_chain};

/// The platform used when none was chosen yet or the one kept is not compiled in.
const FIRST_PROVIDER: &str = "netease";
/// `HOME_FEED_RECOMMENDATION_LIMIT`.
const RADIO_LIMIT: u32 = 35;

/// Whose account the online tabs show (`accountView`).
#[derive(Debug, Clone, PartialEq)]
pub enum AccountState {
    /// The kept session is being read and checked.
    Resolving,
    /// Nobody is signed in; `ran_out` when a kept session no longer works (`needsRelogin`).
    Guest {
        ran_out: bool,
    },
    SignedIn(Account),
}

/// One piece of online data: not asked for, on its way, here, or failed.
#[derive(Debug, Clone, Default, PartialEq)]
pub enum Feed<T> {
    #[default]
    Idle,
    Loading,
    Ready(T),
    Failed(String),
}

impl<T> Feed<T> {
    pub fn ready(&self) -> Option<&T> {
        match self {
            Feed::Ready(value) => Some(value),
            _ => None,
        }
    }

    pub fn loading(&self) -> bool {
        matches!(self, Feed::Loading)
    }
}

#[derive(Debug, Clone)]
pub enum Message {
    /// Each available platform, and whether a kept session was read into it.
    Restored(Vec<(&'static str, Result<bool, String>)>),
    Account(&'static str, Result<Option<Account>, String>),
    Playlists(String, Result<Vec<Collection>, String>),
    Albums(String, Result<Vec<Collection>, String>),
    Radio(String, Result<RadioFeed, String>),
    Explore(String, Result<Vec<ExploreSection>, String>),
    /// The answer to 「换一批」.
    ExploreRefreshed(String, Result<Vec<ExploreSection>, String>),
    Avatar(&'static str, String, Option<folia_library::Cover>),
    /// The sync button's read came back: the account and the playlists kept with it.
    Synced(&'static str, Result<Option<(Account, Vec<Collection>)>, String>),
    /// The account's liked ids.
    Liked(&'static str, Result<Option<Vec<String>>, String>),
    /// A heart's request settled; the id, what was asked and whether the platform took it.
    LikeSettled(&'static str, String, bool, Result<(), String>),
    SyncLeaving(u64),
    SyncGone(u64),
    SyncFrame,
    Forgotten,
}

/// `StatusToast` after the sync button: what it read, and whether the sign-in had run out.
#[derive(Debug, Clone, Copy)]
struct SyncNotice {
    provider: &'static str,
    ok: bool,
    signed_out: bool,
    started: Instant,
}

/// `runStatusMessageTime` and the toast's exit.
const SYNC_NOTICE_MS: u64 = 4000;
const SYNC_LEAVE_MS: u64 = 220;

/// The account picture is decoded at twice the 32px it is drawn at.
const AVATAR_PX: u32 = 64;

pub struct Online {
    omni: Option<Omni>,
    /// The platform the online tabs show (`activeProviderId`).
    provider: &'static str,
    /// Platforms holding a session read from the keyring or signed in during this run.
    kept: HashSet<&'static str>,
    pub account: AccountState,
    accounts: HashMap<&'static str, AccountState>,
    avatars: HashMap<&'static str, iced::widget::image::Handle>,
    pub playlists: Feed<Vec<Collection>>,
    pub albums: Feed<Vec<Collection>>,
    pub radio: Feed<RadioFeed>,
    pub explore: Feed<Vec<ExploreSection>>,
    /// 「换一批」 is on its way; the page on screen stays until it arrives.
    pub explore_refreshing: bool,
    pub avatar: Option<iced::widget::image::Handle>,
    /// The platform whose sync button is reading.
    syncing: Option<&'static str>,
    sync_notice: Option<SyncNotice>,
    sync_serial: u64,
    /// Each platform's liked ids and hearts asked for but not settled yet (`likedSongIds`, `wantedLikes`).
    liked: HashMap<&'static str, LikedSet>,
}

#[derive(Debug, Clone, Default)]
struct LikedSet {
    ids: std::collections::HashSet<String>,
    wanted: HashMap<String, bool>,
    loaded: bool,
}

impl Online {
    /// Restores each platform from the keyring and reads every restored account once.
    pub fn new(omni: Option<Omni>, preferred: Option<&str>) -> (Self, Task<Message>) {
        let available: Vec<&'static str> = omni
            .as_ref()
            .map(|omni| omni.platforms().into_iter().filter(|p| p.available).map(|p| p.id).collect())
            .unwrap_or_default();
        let provider = preferred
            .and_then(|wanted| available.iter().copied().find(|id| *id == wanted))
            .or_else(|| available.first().copied())
            .unwrap_or(FIRST_PROVIDER);
        let task = match omni.clone() {
            Some(omni) => {
                let ids = available.clone();
                let work = blocking(move || {
                    ids.into_iter()
                        .map(|id| (id, omni.restore_session(id).map_err(|err| error_chain(&err))))
                        .collect::<Vec<_>>()
                });
                Task::perform(work, |result| Message::Restored(result.unwrap_or_default()))
            }
            None => Task::none(),
        };
        let account = if omni.is_some() { AccountState::Resolving } else { AccountState::Guest { ran_out: false } };
        let online = Self {
            omni,
            provider,
            kept: HashSet::new(),
            account,
            accounts: HashMap::new(),
            avatars: HashMap::new(),
            playlists: Feed::Idle,
            albums: Feed::Idle,
            radio: Feed::Idle,
            explore: Feed::Idle,
            explore_refreshing: false,
            avatar: None,
            syncing: None,
            sync_notice: None,
            sync_serial: 0,
            liked: HashMap::new(),
        };
        (online, task)
    }

    pub fn omni(&self) -> Option<&Omni> {
        self.omni.as_ref()
    }

    /// The platform the online tabs show.
    pub fn provider(&self) -> &'static str {
        self.provider
    }

    pub fn platforms(&self) -> Vec<folia_omni::Platform> {
        let platforms = self.omni.as_ref().map(Omni::platforms).unwrap_or_default();
        order_provider_rows(
            platforms.into_iter().map(|platform| {
                let status = match self.account_for(platform.id) {
                    AccountState::SignedIn(_) => ProviderRowStatus::SignedIn,
                    AccountState::Guest { ran_out: true } => ProviderRowStatus::RanOut,
                    _ => ProviderRowStatus::SignedOut,
                };
                (platform, status)
            }),
            Some(self.provider),
        )
    }

    pub fn account_for(&self, provider: &str) -> &AccountState {
        self.accounts.get(provider).unwrap_or(&AccountState::Guest { ran_out: false })
    }

    pub fn avatar_for(&self, provider: &str) -> Option<&iced::widget::image::Handle> {
        self.avatars.get(provider)
    }

    /// `isSongLiked`: a heart pressed and not yet settled answers with what was asked for.
    pub fn is_liked(&self, provider: &str, id: &str) -> bool {
        let Some(set) = self.liked.get(provider) else { return false };
        if let Some(wanted) = set.wanted.get(id) {
            return *wanted;
        }
        set.ids.contains(id)
    }

    /// Whether the platform takes hearts at all.
    pub fn can_like(&self, provider: &str) -> bool {
        self.omni.as_ref().is_some_and(|omni| omni.capabilities(provider).likes)
    }

    /// Reads the account's liked ids once, when the platform offers them.
    pub fn ensure_liked(&mut self) -> Task<Message> {
        let (Some(omni), Some(account)) = (self.omni.clone(), self.signed_in().cloned()) else { return Task::none() };
        let provider = self.provider;
        if self.liked.get(provider).is_some_and(|set| set.loaded) {
            return Task::none();
        }
        if !omni.capabilities(provider).likes {
            return Task::none();
        }
        let _ = account;
        self.liked.entry(provider).or_default().loaded = true;
        Task::perform(
            async move { omni.liked_song_ids(provider).await.map_err(|err| error_chain(&err)) },
            move |result| Message::Liked(provider, result),
        )
    }

    /// `pressSongLike`: the heart answers at once and the request follows.
    pub fn press_like(&mut self, provider: &'static str, id: String, liked: bool) -> Task<Message> {
        let Some(omni) = self.omni.clone() else { return Task::none() };
        if !omni.capabilities(provider).likes {
            return Task::none();
        }
        self.liked.entry(provider).or_default().wanted.insert(id.clone(), liked);
        let song = folia_core::OnlineId { provider: provider.to_owned(), id: id.clone() };
        Task::perform(
            async move { omni.like_song(&song, liked).await.map_err(|err| error_chain(&err)) },
            move |result| Message::LikeSettled(provider, id.clone(), liked, result),
        )
    }

    /// `requestSwitch`: shows another platform whose session is kept, reading its account anew.
    pub fn switch_to(&mut self, provider: &'static str) -> Task<Message> {
        if provider == self.provider || !self.kept.contains(provider) {
            return Task::none();
        }
        tracing::info!(provider, "online platform switched");
        self.provider = provider;
        self.account = self.account_for(provider).clone();
        self.avatar = self.avatar_for(provider).cloned();
        self.reset_feeds();
        self.read_account(provider)
    }

    pub fn signed_in(&self) -> Option<&Account> {
        match &self.account {
            AccountState::SignedIn(account) => Some(account),
            _ => None,
        }
    }

    /// Returns whether the account changed, so the wall can be rebuilt.
    pub fn update(&mut self, message: Message) -> (Task<Message>, bool) {
        match message {
            Message::Restored(results) => {
                for (provider, result) in results {
                    match result {
                        Ok(true) => {
                            self.kept.insert(provider);
                        }
                        Ok(false) => {}
                        Err(err) => tracing::warn!(provider, error = %err, "no kept online session"),
                    }
                }
                if !self.kept.contains(self.provider) {
                    self.account = AccountState::Guest { ran_out: false };
                }
                let providers: Vec<_> = self.kept.iter().copied().collect();
                (Task::batch(providers.into_iter().map(|provider| self.read_account(provider))), true)
            }
            Message::Account(provider, _) if !self.kept.contains(provider) => (Task::none(), false),
            Message::Account(provider, Ok(Some(account))) => {
                tracing::info!(provider, "online account read");
                self.accounts.insert(provider, AccountState::SignedIn(account.clone()));
                let avatar = self.load_avatar(provider, &account);
                if provider != self.provider {
                    return (avatar, false);
                }
                self.account = AccountState::SignedIn(account.clone());
                self.reset_feeds();
                (Task::batch([self.load_playlists(&account), avatar, self.ensure_liked()]), true)
            }
            Message::Account(provider, Ok(None)) => {
                let ran_out = self.kept.remove(provider);
                let state = AccountState::Guest { ran_out };
                self.accounts.insert(provider, state.clone());
                self.avatars.remove(provider);
                if provider == self.provider {
                    self.account = state;
                    self.reset_feeds();
                }
                (self.forget(provider), provider == self.provider)
            }
            Message::Account(provider, Err(err)) => {
                tracing::warn!(provider, error = %err, "online account unreadable");
                if !matches!(self.account_for(provider), AccountState::SignedIn(_)) {
                    self.accounts.insert(provider, AccountState::Guest { ran_out: false });
                }
                if provider == self.provider {
                    self.account = self.account_for(provider).clone();
                }
                (Task::none(), provider == self.provider)
            }
            Message::Playlists(owner, result) => {
                (Task::none(), self.settle(&owner, result, |online| &mut online.playlists))
            }
            Message::Albums(owner, result) => (Task::none(), self.settle(&owner, result, |online| &mut online.albums)),
            Message::Radio(owner, result) => (Task::none(), self.settle(&owner, result, |online| &mut online.radio)),
            Message::Explore(owner, result) => {
                (Task::none(), self.settle(&owner, result, |online| &mut online.explore))
            }
            Message::ExploreRefreshed(owner, result) => {
                if !self.explore_refreshing || self.owner().is_none_or(|current| current != owner) {
                    return (Task::none(), false);
                }
                self.explore_refreshing = false;
                let fresh = match result {
                    Ok(fresh) => fresh,
                    Err(err) => {
                        tracing::warn!(error = %err, "explore refresh unavailable");
                        return (Task::none(), false);
                    }
                };
                let Feed::Ready(shown) = &mut self.explore else {
                    self.explore = Feed::Ready(fresh);
                    return (Task::none(), true);
                };
                // `exchanged`: a section that stands takes what it has now in its place, one the platform no longer
                // names keeps what it showed, and the new ones come after.
                let mut joining = Vec::new();
                for section in fresh {
                    match shown.iter_mut().find(|standing| standing.key == section.key) {
                        Some(standing) => *standing = section,
                        None => joining.push(section),
                    }
                }
                shown.extend(joining);
                (Task::none(), true)
            }
            Message::Avatar(provider, user, cover) => {
                if matches!(self.account_for(provider), AccountState::SignedIn(account) if account.id == user)
                    && let Some(cover) = cover
                {
                    let handle = iced::widget::image::Handle::from_rgba(cover.width, cover.height, cover.rgba);
                    self.avatars.insert(provider, handle.clone());
                    if provider == self.provider {
                        self.avatar = Some(handle);
                    }
                }
                (Task::none(), false)
            }
            Message::Synced(provider, result) => {
                if self.syncing == Some(provider) {
                    self.syncing = None;
                }
                if !self.kept.contains(provider) && !matches!(self.account_for(provider), AccountState::SignedIn(_)) {
                    return (Task::none(), false);
                }
                match result {
                    Ok(Some((account, playlists))) => {
                        self.kept.insert(provider);
                        self.accounts.insert(provider, AccountState::SignedIn(account.clone()));
                        let avatar = self.load_avatar(provider, &account);
                        if provider != self.provider {
                            let notice = self.show_sync_notice(provider, true, false);
                            return (Task::batch([avatar, notice]), false);
                        }
                        self.account = AccountState::SignedIn(account);
                        self.playlists = Feed::Ready(playlists);
                        let notice = self.show_sync_notice(provider, true, false);
                        (Task::batch([avatar, notice]), true)
                    }
                    Ok(None) => {
                        self.kept.remove(provider);
                        let state = AccountState::Guest { ran_out: true };
                        self.accounts.insert(provider, state.clone());
                        self.avatars.remove(provider);
                        if provider == self.provider {
                            self.account = state;
                            self.reset_feeds();
                        }
                        let notice = self.show_sync_notice(provider, false, true);
                        (Task::batch([self.forget(provider), notice]), provider == self.provider)
                    }
                    Err(err) => {
                        tracing::warn!(provider, error = %err, "online account sync failed");
                        (self.show_sync_notice(provider, false, false), false)
                    }
                }
            }
            Message::SyncLeaving(serial) if serial == self.sync_serial => {
                let leaving =
                    Task::perform(blocking(|| std::thread::sleep(Duration::from_millis(SYNC_LEAVE_MS))), move |_| {
                        Message::SyncGone(serial)
                    });
                (leaving, false)
            }
            Message::SyncGone(serial) if serial == self.sync_serial => {
                self.sync_notice = None;
                (Task::none(), false)
            }
            Message::SyncLeaving(_) | Message::SyncGone(_) | Message::SyncFrame => (Task::none(), false),
            Message::Liked(provider, result) => {
                match result {
                    Ok(Some(ids)) => {
                        let set = self.liked.entry(provider).or_default();
                        set.ids = ids.into_iter().collect();
                        set.loaded = true;
                    }
                    Ok(None) => {
                        self.liked.remove(provider);
                    }
                    Err(err) => {
                        tracing::warn!(provider, error = %err, "the liked songs could not be read");
                        if let Some(set) = self.liked.get_mut(provider) {
                            set.loaded = false;
                        }
                    }
                }
                (Task::none(), false)
            }
            Message::LikeSettled(provider, id, liked, result) => {
                let took = result.is_ok();
                if let Some(set) = self.liked.get_mut(provider) {
                    set.wanted.remove(&id);
                    match result {
                        Ok(()) => {
                            if liked {
                                set.ids.insert(id);
                            } else {
                                set.ids.remove(&id);
                            }
                        }
                        Err(err) => tracing::warn!(provider, error = %err, "the heart was not kept"),
                    }
                }
                // A saved or removed heart changes the liked list among the account's playlists, which the wall shows.
                let reload = if took { self.reload_playlists() } else { Task::none() };
                (reload, took)
            }
            Message::Forgotten => (Task::none(), false),
        }
    }

    fn load_avatar(&mut self, provider: &'static str, account: &Account) -> Task<Message> {
        self.avatars.remove(provider);
        if provider == self.provider {
            self.avatar = None;
        }
        let (Some(omni), Some(url)) = (self.omni.clone(), account.avatar_url.clone()) else { return Task::none() };
        let user = account.id.clone();
        Task::perform(crate::background::online_cover(omni, url, AVATAR_PX), move |cover| {
            Message::Avatar(provider, user.clone(), cover)
        })
    }

    /// Activates a confirmed account, retaining every other platform's summary.
    pub fn signed_in_now(&mut self, provider: &'static str) -> Task<Message> {
        self.provider = provider;
        self.account = self.account_for(provider).clone();
        self.avatar = self.avatar_for(provider).cloned();
        self.reset_feeds();
        if self.kept.contains(provider) {
            return match self.signed_in().cloned() {
                Some(account) => self.load_playlists(&account),
                None if matches!(self.account_for(provider), AccountState::Resolving) => Task::none(),
                None => self.keep_session(provider),
            };
        }
        self.keep_session(provider)
    }

    pub fn keep_session(&mut self, provider: &'static str) -> Task<Message> {
        let Some(omni) = self.omni.clone() else { return Task::none() };
        self.kept.insert(provider);
        let keep = blocking(move || omni.save_session(provider).map_err(|err| error_chain(&err)));
        let keep = Task::perform(keep, |result| {
            if let Some(Err(err)) = result {
                tracing::error!(error = %err, "cannot keep the online session");
            }
            Message::Forgotten
        });
        Task::batch([keep, self.read_account(provider)])
    }

    /// The panel's sync button: reads the account and its playlists again (`readAccount` with the `sync` reason).
    pub fn sync(&mut self, provider: &'static str) -> Task<Message> {
        let Some(omni) = self.omni.clone() else { return Task::none() };
        if self.syncing.is_some() {
            return Task::none();
        }
        self.syncing = Some(provider);
        Task::perform(
            async move { omni.sync_account(provider).await.map_err(|err| error_chain(&err)) },
            move |result| Message::Synced(provider, result),
        )
    }

    pub fn syncing(&self, provider: &str) -> bool {
        self.syncing == Some(provider)
    }

    /// `holdStatusMessage` while the sync toast is entering or leaving.
    pub fn sync_notice_animating(&self, now: Instant) -> bool {
        self.sync_notice.is_some_and(|notice| {
            let ms = now.saturating_duration_since(notice.started).as_millis() as u64;
            ms < 320 || (SYNC_NOTICE_MS..SYNC_NOTICE_MS + SYNC_LEAVE_MS).contains(&ms)
        })
    }

    /// The sync toast's platform, outcome and `StatusToast` motion.
    pub fn sync_notice(&self, now: Instant) -> Option<(&'static str, bool, bool, f32, f32)> {
        let notice = self.sync_notice?;
        let ms = now.saturating_duration_since(notice.started).as_secs_f64() * 1000.0;
        let entered = folia_visualizer::Ease::new(0.2, 0.8, 0.2, 1.0).at((ms / 320.0).clamp(0.0, 1.0)) as f32;
        let left = folia_visualizer::Ease::new(0.4, 0.0, 1.0, 1.0)
            .at(((ms - SYNC_NOTICE_MS as f64) / SYNC_LEAVE_MS as f64).clamp(0.0, 1.0)) as f32;
        Some((
            notice.provider,
            notice.ok,
            notice.signed_out,
            entered * (1.0 - left),
            -18.0 * (1.0 - entered) - 10.0 * left,
        ))
    }

    fn show_sync_notice(&mut self, provider: &'static str, ok: bool, signed_out: bool) -> Task<Message> {
        self.sync_serial = self.sync_serial.wrapping_add(1);
        let serial = self.sync_serial;
        self.sync_notice = Some(SyncNotice { provider, ok, signed_out, started: Instant::now() });
        Task::perform(blocking(|| std::thread::sleep(Duration::from_millis(SYNC_NOTICE_MS))), move |_| {
            Message::SyncLeaving(serial)
        })
    }

    /// `logout`: tells the platform, drops the kept session, and shows the guest wall.
    pub fn sign_out(&mut self, provider: &'static str) -> Task<Message> {
        let Some(omni) = self.omni.clone() else { return Task::none() };
        if self.syncing == Some(provider) {
            self.syncing = None;
        }
        if provider == self.provider {
            self.account = AccountState::Guest { ran_out: false };
        }
        self.kept.remove(provider);
        self.accounts.insert(provider, AccountState::Guest { ran_out: false });
        self.avatars.remove(provider);
        if provider == self.provider {
            self.reset_feeds();
        }
        let work = async move {
            if let Err(err) = omni.logout(provider).await {
                tracing::warn!(error = &err as &dyn std::error::Error, "platform logout failed");
            }
            let forget = omni.clone();
            if let Some(Err(err)) = blocking(move || forget.forget_session(provider)).await {
                tracing::error!(error = &err as &dyn std::error::Error, "cannot remove the kept session");
            }
        };
        Task::perform(work, |()| Message::Forgotten)
    }

    /// Reads what an online tab shows the first time it is opened for this account (`attachOnlineHomeFeeds`).
    pub fn ensure(&mut self, tab: folia_store::HomeTab) -> Task<Message> {
        let (Some(omni), Some(account)) = (self.omni.clone(), self.signed_in().cloned()) else { return Task::none() };
        let (owner, provider) = (self.owner_of(&account), self.provider);
        match tab {
            folia_store::HomeTab::Albums if self.albums == Feed::Idle => {
                self.albums = Feed::Loading;
                Task::perform(
                    async move { omni.favorite_albums(provider).await.map_err(|err| error_chain(&err)) },
                    move |r| Message::Albums(owner.clone(), r),
                )
            }
            folia_store::HomeTab::Radio if self.radio == Feed::Idle => {
                self.radio = Feed::Loading;
                Task::perform(
                    async move { omni.radio_feed(provider, RADIO_LIMIT).await.map_err(|err| error_chain(&err)) },
                    move |r| Message::Radio(owner.clone(), r),
                )
            }
            folia_store::HomeTab::Explore if self.explore == Feed::Idle => {
                self.explore = Feed::Loading;
                Task::perform(async move { omni.explore(provider).await.map_err(|err| error_chain(&err)) }, move |r| {
                    Message::Explore(owner.clone(), r)
                })
            }
            folia_store::HomeTab::Playlist if self.playlists == Feed::Idle => self.load_playlists(&account),
            _ => Task::none(),
        }
    }

    /// 「换一批」 (`refresh`): the platform's page is read anew, every shelf handing out what it has not shown yet first.
    pub fn refresh_explore(&mut self) -> Task<Message> {
        let (Some(omni), Some(account)) = (self.omni.clone(), self.signed_in().cloned()) else { return Task::none() };
        if self.explore_refreshing || self.explore == Feed::Loading || self.explore == Feed::Idle {
            return Task::none();
        }
        self.explore_refreshing = true;
        let (owner, provider) = (self.owner_of(&account), self.provider);
        Task::perform(async move { omni.explore(provider).await.map_err(|err| error_chain(&err)) }, move |r| {
            Message::ExploreRefreshed(owner.clone(), r)
        })
    }

    pub fn can_refresh_explore(&self) -> bool {
        !self.explore_refreshing && matches!(self.explore, Feed::Ready(_) | Feed::Failed(_))
    }

    /// Reads the playlists again after a change made here (`refreshProviderPlaylists`).
    pub fn reload_playlists(&mut self) -> Task<Message> {
        match self.signed_in().cloned() {
            Some(account) => self.load_playlists(&account),
            None => Task::none(),
        }
    }

    fn load_playlists(&mut self, account: &Account) -> Task<Message> {
        let Some(omni) = self.omni.clone() else { return Task::none() };
        self.playlists = Feed::Loading;
        let (owner, user, provider) = (self.owner_of(account), account.id.clone(), self.provider);
        Task::perform(
            async move { omni.user_playlists(provider, &user).await.map_err(|err| error_chain(&err)) },
            move |result| Message::Playlists(owner.clone(), result),
        )
    }

    fn read_account(&mut self, provider: &'static str) -> Task<Message> {
        let Some(omni) = self.omni.clone() else { return Task::none() };
        if !matches!(self.account_for(provider), AccountState::SignedIn(_)) {
            self.accounts.insert(provider, AccountState::Resolving);
        }
        if provider == self.provider {
            self.account = self.account_for(provider).clone();
        }
        Task::perform(
            async move {
                match omni.account(provider).await {
                    Err(folia_omni::OmniError::SignedOut) => Ok(None),
                    result => result.map_err(|err| error_chain(&err)),
                }
            },
            move |result| Message::Account(provider, result),
        )
    }

    fn forget(&self, provider: &'static str) -> Task<Message> {
        let Some(omni) = self.omni.clone() else { return Task::none() };
        Task::perform(
            blocking(move || {
                if let Err(err) = omni.forget_session(provider) {
                    tracing::error!(error = &err as &dyn std::error::Error, "cannot remove the kept session");
                }
            }),
            |_| Message::Forgotten,
        )
    }

    /// Whose answers are kept: the platform and the account signed in on it.
    fn owner(&self) -> Option<String> {
        self.signed_in().map(|account| self.owner_of(account))
    }

    fn owner_of(&self, account: &Account) -> String {
        format!("{}:{}", self.provider, account.id)
    }

    fn reset_feeds(&mut self) {
        if self.signed_in().is_none() {
            self.avatar = None;
        }
        self.playlists = Feed::Idle;
        self.albums = Feed::Idle;
        self.radio = Feed::Idle;
        self.explore = Feed::Idle;
        self.explore_refreshing = false;
    }

    /// Stores an answer unless it belongs to an account that is no longer signed in; returns whether it was kept.
    fn settle<T>(
        &mut self,
        owner: &str,
        result: Result<T, String>,
        feed: impl FnOnce(&mut Self) -> &mut Feed<T>,
    ) -> bool {
        if self.owner().is_none_or(|current| current != owner) {
            return false;
        }
        *feed(self) = match result {
            Ok(value) => Feed::Ready(value),
            Err(err) => {
                tracing::warn!(error = %err, "online home data unavailable");
                Feed::Failed(err)
            }
        };
        true
    }
}
