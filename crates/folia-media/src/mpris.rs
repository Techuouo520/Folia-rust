// Linux MPRIS object server, bounded command delivery, reconnects and local artwork.
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use futures_util::StreamExt;
use tokio::sync::{mpsc, watch};
use zbus::object_server::SignalEmitter;
use zbus::zvariant::{ObjectPath, Value};
use zbus::{Connection, MessageStream, fdo, interface};

use crate::artwork::ArtCache;
use crate::{Command, Snapshot};

pub const PATH: &str = "/org/mpris/MediaPlayer2";
pub const PLAYER: &str = "org.mpris.MediaPlayer2.Player";
const NO_TRACK: &str = "/org/mpris/MediaPlayer2/TrackList/NoTrack";
static INSTANCE: AtomicU64 = AtomicU64::new(0);

#[derive(Clone)]
struct Published {
    snapshot: Snapshot,
    art_url: Option<String>,
}

#[derive(Clone)]
struct Player {
    state: watch::Receiver<Published>,
    commands: mpsc::Sender<Command>,
}

impl Player {
    fn command(&self, command: Command) -> fdo::Result<()> {
        self.commands
            .try_send(command)
            .map_err(|_| fdo::Error::LimitsExceeded("Media command queue unavailable".into()))
    }
}

#[interface(name = "org.mpris.MediaPlayer2")]
impl Root {
    fn raise(&self) -> fdo::Result<()> {
        self.0.command(Command::Raise)
    }
    fn quit(&self) {}
    #[zbus(property)]
    fn can_quit(&self) -> bool {
        false
    }
    #[zbus(property)]
    fn can_raise(&self) -> bool {
        true
    }
    #[zbus(property)]
    fn has_track_list(&self) -> bool {
        false
    }
    #[zbus(property)]
    fn identity(&self) -> &str {
        "Folia"
    }
    #[zbus(property)]
    fn supported_uri_schemes(&self) -> Vec<&str> {
        vec![]
    }
    #[zbus(property)]
    fn supported_mime_types(&self) -> Vec<&str> {
        vec![]
    }
}

struct Root(Player);

#[interface(name = "org.mpris.MediaPlayer2.Player")]
impl Player {
    fn next(&self) -> fdo::Result<()> {
        if self.can_go_next() { self.command(Command::Next) } else { Ok(()) }
    }
    fn previous(&self) -> fdo::Result<()> {
        if self.can_go_previous() { self.command(Command::Previous) } else { Ok(()) }
    }
    fn play(&self) -> fdo::Result<()> {
        if self.can_play() { self.command(Command::Play) } else { Ok(()) }
    }
    fn pause(&self) -> fdo::Result<()> {
        if self.can_pause() { self.command(Command::Pause) } else { Ok(()) }
    }
    fn play_pause(&self) -> fdo::Result<()> {
        if self.can_play() { self.command(Command::PlayPause) } else { Ok(()) }
    }
    fn stop(&self) -> fdo::Result<()> {
        self.command(Command::Stop)
    }
    fn seek(&self, offset: i64) {
        let _ = offset;
    }
    fn set_position(&self, track_id: ObjectPath<'_>, position: i64) {
        let _ = (track_id, position);
    }
    fn open_uri(&self, uri: &str) -> fdo::Result<()> {
        let _ = uri;
        Err(fdo::Error::NotSupported("Opening URIs is not supported".into()))
    }
    #[zbus(signal)]
    async fn seeked(emitter: &SignalEmitter<'_>, position: i64) -> zbus::Result<()>;
    #[zbus(property)]
    fn playback_status(&self) -> &'static str {
        let state = self.state.borrow();
        if state.snapshot.metadata.is_some() { state.snapshot.status.as_str() } else { "Stopped" }
    }
    #[zbus(property)]
    fn metadata(&self) -> HashMap<String, Value<'static>> {
        metadata(&self.state.borrow())
    }
    #[zbus(property(emits_changed_signal = "false"))]
    fn position(&self) -> i64 {
        self.state.borrow().snapshot.position_at(Instant::now())
    }
    #[zbus(property)]
    fn volume(&self) -> f64 {
        self.state.borrow().snapshot.volume
    }
    #[zbus(property)]
    fn set_volume(&self, value: f64) -> fdo::Result<()> {
        if !value.is_finite() {
            return Err(fdo::Error::InvalidArgs("Volume must be finite".into()));
        }
        self.command(Command::Volume(value.clamp(0.0, 1.0)))
    }
    #[zbus(property)]
    fn rate(&self) -> f64 {
        1.0
    }
    #[zbus(property)]
    fn set_rate(&self, value: f64) -> fdo::Result<()> {
        if value == 1.0 { Ok(()) } else { Err(fdo::Error::NotSupported("Only normal speed is supported".into())) }
    }
    #[zbus(property)]
    fn minimum_rate(&self) -> f64 {
        1.0
    }
    #[zbus(property)]
    fn maximum_rate(&self) -> f64 {
        1.0
    }
    #[zbus(property)]
    fn can_go_next(&self) -> bool {
        self.state.borrow().snapshot.can_next
    }
    #[zbus(property)]
    fn can_go_previous(&self) -> bool {
        self.state.borrow().snapshot.can_previous
    }
    #[zbus(property)]
    fn can_play(&self) -> bool {
        self.state.borrow().snapshot.metadata.is_some()
    }
    #[zbus(property)]
    fn can_pause(&self) -> bool {
        self.can_play()
    }
    #[zbus(property)]
    fn can_seek(&self) -> bool {
        false
    }
    #[zbus(property(emits_changed_signal = "const"))]
    fn can_control(&self) -> bool {
        true
    }
}

fn metadata(state: &Published) -> HashMap<String, Value<'static>> {
    let mut values = HashMap::new();
    let track = state.snapshot.metadata.as_ref();
    // Metadata constructs only the fixed prefix and a hexadecimal hash.
    let path = ObjectPath::from_string_unchecked(track.map_or(NO_TRACK, |track| track.track_id()).to_owned());
    values.insert("mpris:trackid".into(), Value::from(path));
    if let Some(track) = track {
        values.insert("xesam:title".into(), Value::from(track.title.clone()));
        values.insert("xesam:artist".into(), Value::from(track.artists.clone()));
        if let Some(album) = &track.album {
            values.insert("xesam:album".into(), Value::from(album.clone()));
        }
        if track.length > 0 {
            values.insert("mpris:length".into(), track.length.into());
        }
        if let Some(url) = &state.art_url {
            values.insert("mpris:artUrl".into(), Value::from(url.clone()));
        }
    }
    values
}

/// One application instance; the suffix avoids stealing another player's bus name.
pub struct Service {
    name: String,
    cache: PathBuf,
}

impl Service {
    pub fn new(cache: PathBuf) -> Self {
        let serial = INSTANCE.fetch_add(1, Ordering::Relaxed);
        let base = if cfg!(debug_assertions) { "folia_dev" } else { "folia" };
        let name = format!("org.mpris.MediaPlayer2.{base}.instance{}_{}", std::process::id(), serial);
        Self { name, cache }
    }

    pub fn bus_name(&self) -> &str {
        &self.name
    }

    /// Serves a host-selected bus, or the desktop session bus when `address` is absent.
    /// Dropping this future releases the name; dropping the snapshot sender stops it cleanly.
    pub async fn run(
        self,
        address: Option<String>,
        mut states: watch::Receiver<Snapshot>,
        commands: mpsc::Sender<Command>,
    ) {
        let initial = Published { snapshot: Snapshot::default(), art_url: None };
        let (published, state) = watch::channel(initial);
        let player = Player { state, commands };
        let mut retry = Duration::from_secs(1);
        let mut cache = ArtCache::new(self.cache.join("mpris").join(&self.name));
        loop {
            let connected_at = Instant::now();
            let result = self.serve(address.as_deref(), &mut states, &published, &player, &mut cache).await;
            if states.has_changed().is_err() || player.commands.is_closed() {
                break;
            }
            if let Err(error) = result {
                tracing::warn!(error = &error as &dyn std::error::Error, "MPRIS disconnected; retrying");
            }
            if connected_at.elapsed() >= Duration::from_secs(30) {
                retry = Duration::from_secs(1);
            }
            tokio::select! {
                _ = tokio::time::sleep(retry) => {}
                _ = player.commands.closed() => break,
                _ = states.wait_for(|_| false) => break,
            }
            retry = (retry * 2).min(Duration::from_secs(30));
        }
    }

    async fn serve(
        &self,
        address: Option<&str>,
        states: &mut watch::Receiver<Snapshot>,
        published: &watch::Sender<Published>,
        player: &Player,
        cache: &mut ArtCache,
    ) -> zbus::Result<()> {
        let builder = match address {
            Some(address) => zbus::connection::Builder::address(address)?,
            None => zbus::connection::Builder::session()?,
        };
        let snapshot = states.borrow_and_update().clone();
        update(published, snapshot, cache, None).await?;
        let connection = builder
            .serve_at(PATH, Root(player.clone()))?
            .serve_at(PATH, player.clone())?
            .name(self.name.as_str())?
            .build()
            .await?;
        tracing::info!(name = %self.name, "MPRIS session registered");
        let mut messages = MessageStream::from(&connection);
        loop {
            tokio::select! {
                result = states.changed() => {
                    if result.is_err() { return Ok(()); }
                    let snapshot = states.borrow_and_update().clone();
                    update(published, snapshot, cache, Some(&connection)).await?;
                }
                message = messages.next() => {
                    match message { Some(Ok(_)) => {}, Some(Err(error)) => return Err(error), None => return Err(zbus::Error::Failure("Session bus closed".into())) }
                }
                _ = player.commands.closed() => return Ok(()),
            }
        }
    }
}

async fn update(
    published: &watch::Sender<Published>,
    snapshot: Snapshot,
    cache: &mut ArtCache,
    connection: Option<&Connection>,
) -> zbus::Result<()> {
    let before = published.borrow().clone();
    let same_art = match (&before.snapshot.artwork, &snapshot.artwork) {
        (Some(a), Some(b)) => std::sync::Arc::ptr_eq(a, b),
        (None, None) => true,
        _ => false,
    };
    let art_url = if same_art { before.art_url.clone() } else { cache.write(&snapshot).await };
    let after = Published { snapshot, art_url };
    published.send_replace(after.clone());
    let Some(connection) = connection else { return Ok(()) };
    let emitter = SignalEmitter::new(connection, PATH)?;
    let mut changed = HashMap::new();
    if before.snapshot.metadata != after.snapshot.metadata || before.art_url != after.art_url {
        changed.insert("Metadata", Value::from(metadata(&after)));
    }
    if before.snapshot.status != after.snapshot.status {
        changed.insert("PlaybackStatus", Value::from(after.snapshot.status.as_str()));
    }
    if before.snapshot.volume != after.snapshot.volume {
        changed.insert("Volume", Value::from(after.snapshot.volume));
    }
    if before.snapshot.can_next != after.snapshot.can_next {
        changed.insert("CanGoNext", Value::from(after.snapshot.can_next));
    }
    if before.snapshot.can_previous != after.snapshot.can_previous {
        changed.insert("CanGoPrevious", Value::from(after.snapshot.can_previous));
    }
    if before.snapshot.metadata.is_some() != after.snapshot.metadata.is_some() {
        changed.insert("CanPlay", Value::from(after.snapshot.metadata.is_some()));
        changed.insert("CanPause", Value::from(after.snapshot.metadata.is_some()));
    }
    if !changed.is_empty() {
        fdo::Properties::properties_changed(&emitter, PLAYER.try_into()?, changed, [].as_slice().into()).await?;
    }
    let same_track =
        before.snapshot.metadata.as_ref().map(|m| &m.track_id) == after.snapshot.metadata.as_ref().map(|m| &m.track_id);
    if same_track
        && after.snapshot.metadata.is_some()
        && (before.snapshot.seek_revision != after.snapshot.seek_revision
            || (before.snapshot.position_at(after.snapshot.sampled_at) - after.snapshot.position).abs() > 1_500_000)
    {
        Player::seeked(&emitter, after.snapshot.position).await?;
    }
    Ok(())
}
