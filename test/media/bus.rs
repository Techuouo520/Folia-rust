// Exercises the exported service against a private dbus-daemon without touching the desktop bus.
use std::collections::HashMap;
use std::io::{BufRead, BufReader};
use std::process::{Child, Stdio};
use std::sync::Arc;
use std::time::{Duration, Instant};

use folia_core::Track;
use folia_media::mpris::{PATH, PLAYER, Service};
use folia_media::{Artwork, Command, Metadata, Snapshot, Status};
use futures_util::StreamExt;
use tokio::sync::{mpsc, watch};
use zbus::zvariant::{OwnedObjectPath, OwnedValue};
use zbus::{Connection, Proxy};

struct Bus {
    child: Child,
    address: String,
}

impl Bus {
    fn start(socket: &std::path::Path) -> Self {
        let mut child = std::process::Command::new("dbus-daemon")
            .args(["--session", "--nofork", "--print-address=1"])
            .arg(format!("--address=unix:path={}", socket.display()))
            .stdout(Stdio::piped())
            .spawn()
            .expect("Linux MPRIS tests require dbus-daemon");
        let mut address = String::new();
        BufReader::new(child.stdout.take().unwrap()).read_line(&mut address).unwrap();
        assert!(!address.is_empty());
        // The GUID changes when restarting this private bus; clients should reconnect to the socket.
        let address = address.trim().split(",guid=").next().unwrap().to_owned();
        Self { child, address }
    }

    async fn connect(&self) -> Connection {
        zbus::connection::Builder::address(self.address.as_str()).unwrap().build().await.unwrap()
    }
}

impl Drop for Bus {
    fn drop(&mut self) {
        self.child.kill().unwrap();
        self.child.wait().unwrap();
    }
}

async fn wait_name(connection: &Connection, name: &str) {
    let proxy = zbus::fdo::DBusProxy::new(connection).await.unwrap();
    tokio::time::timeout(Duration::from_secs(8), async {
        loop {
            if proxy.name_has_owner(name.try_into().unwrap()).await.unwrap() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
}

async fn properties(connection: &Connection, name: &str) -> HashMap<String, OwnedValue> {
    let proxy = Proxy::new(connection, name, PATH, "org.freedesktop.DBus.Properties").await.unwrap();
    proxy.call("GetAll", &(PLAYER,)).await.unwrap()
}

async fn wait_status(connection: &Connection, name: &str, status: &str) -> HashMap<String, OwnedValue> {
    tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            let values = properties(connection, name).await;
            if <&str>::try_from(&values["PlaybackStatus"]).unwrap() == status {
                return values;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap()
}

fn state() -> Snapshot {
    Snapshot {
        metadata: Some(Metadata::from_track(
            &Track {
                id: 12,
                title: "A song".into(),
                artist: Some("An artist".into()),
                cover_url: Some("https://secret:credential@example.org/art?token=hidden".into()),
                ..Default::default()
            },
            123.5,
        )),
        artwork: Some(Arc::new(Artwork { width: 1, height: 1, rgba: vec![255, 0, 0, 255] })),
        status: Status::Paused,
        position: 12_250_000,
        can_next: true,
        can_previous: true,
        ..Default::default()
    }
}

#[tokio::test]
async fn discovery_metadata_commands_signals_and_local_artwork() {
    let temp = tempfile::tempdir().unwrap();
    let bus = Bus::start(&temp.path().join("bus"));
    let connection = bus.connect().await;
    let service = Service::new(temp.path().join("art space 音乐"));
    let name = service.bus_name().to_owned();
    let (sender, states) = watch::channel(Snapshot::default());
    let (commands, mut receiver) = mpsc::channel(32);
    let running = tokio::spawn(service.run(Some(bus.address.clone()), states, commands));
    wait_name(&connection, &name).await;
    let player = Proxy::new(&connection, name.as_str(), PATH, PLAYER).await.unwrap();
    let root = Proxy::new(&connection, name.as_str(), PATH, "org.mpris.MediaPlayer2").await.unwrap();
    assert_eq!(root.get_property::<String>("Identity").await.unwrap(), "Folia");
    assert!(!root.get_property::<bool>("CanQuit").await.unwrap());
    let empty = properties(&connection, &name).await;
    assert!(!bool::try_from(&empty["CanPlay"]).unwrap());
    assert!(!bool::try_from(&empty["CanSeek"]).unwrap());
    let metadata = HashMap::<String, OwnedValue>::try_from(empty["Metadata"].try_clone().unwrap()).unwrap();
    assert_eq!(
        OwnedObjectPath::try_from(metadata["mpris:trackid"].try_clone().unwrap()).unwrap().as_str(),
        "/org/mpris/MediaPlayer2/TrackList/NoTrack"
    );
    player.call::<_, _, ()>("Play", &()).await.unwrap();
    assert!(receiver.try_recv().is_err());

    sender.send_replace(state());
    let values = wait_status(&connection, &name, "Paused").await;
    let metadata = HashMap::<String, OwnedValue>::try_from(values["Metadata"].try_clone().unwrap()).unwrap();
    assert_eq!(i64::try_from(&metadata["mpris:length"]).unwrap(), 123_500_000);
    assert_eq!(Vec::<String>::try_from(metadata["xesam:artist"].try_clone().unwrap()).unwrap(), ["An artist"]);
    assert_eq!(i64::try_from(&values["Position"]).unwrap(), 12_250_000);
    let url = url::Url::parse(<&str>::try_from(&metadata["mpris:artUrl"]).unwrap()).unwrap();
    assert_eq!(url.scheme(), "file");
    let artwork = url.to_file_path().unwrap();
    assert!(artwork.starts_with(temp.path()));
    assert_eq!(image::open(&artwork).unwrap().to_rgba8().into_raw(), [255, 0, 0, 255]);
    for secret in ["secret", "credential", "token", "hidden", "https"] {
        assert!(!format!("{metadata:?}").contains(secret));
    }

    for (method, expected) in [
        ("Play", Command::Play),
        ("Pause", Command::Pause),
        ("PlayPause", Command::PlayPause),
        ("Stop", Command::Stop),
        ("Next", Command::Next),
        ("Previous", Command::Previous),
    ] {
        player.call::<_, _, ()>(method, &()).await.unwrap();
        assert_eq!(receiver.recv().await.unwrap(), expected);
    }
    root.call::<_, _, ()>("Raise", &()).await.unwrap();
    assert_eq!(receiver.recv().await.unwrap(), Command::Raise);
    player.set_property("Volume", 1.5f64).await.unwrap();
    assert_eq!(receiver.recv().await.unwrap(), Command::Volume(1.0));
    assert!(player.set_property("Volume", f64::NAN).await.is_err());
    assert!(player.set_property("Rate", 2.0f64).await.is_err());
    player.call::<_, _, ()>("Seek", &(1_000_000i64,)).await.unwrap();
    player
        .call::<_, _, ()>("SetPosition", &(OwnedObjectPath::try_from("/stale").unwrap(), 1_000_000i64))
        .await
        .unwrap();
    assert!(receiver.try_recv().is_err());

    let props = Proxy::new(&connection, name.as_str(), PATH, "org.freedesktop.DBus.Properties").await.unwrap();
    let mut signals = props.receive_signal("PropertiesChanged").await.unwrap();
    let mut seeked = player.receive_signal("Seeked").await.unwrap();
    let mut next = sender.borrow().clone();
    next.status = Status::Playing;
    next.advancing = true;
    next.sampled_at = Instant::now();
    sender.send_replace(next.clone());
    let signal = tokio::time::timeout(Duration::from_secs(3), signals.next()).await.unwrap().unwrap();
    let (interface, changed, _): (String, HashMap<String, OwnedValue>, Vec<String>) =
        signal.body().deserialize().unwrap();
    assert_eq!(interface, PLAYER);
    assert_eq!(<&str>::try_from(&changed["PlaybackStatus"]).unwrap(), "Playing");
    assert!(!changed.contains_key("Position"));
    next.position += 500_000;
    next.sampled_at += Duration::from_millis(500);
    sender.send_replace(next.clone());
    assert!(tokio::time::timeout(Duration::from_millis(100), signals.next()).await.is_err());
    next.position = 40_000_000;
    next.seek_revision += 1;
    sender.send_replace(next);
    let signal = tokio::time::timeout(Duration::from_secs(3), seeked.next()).await.unwrap().unwrap();
    assert_eq!(signal.body().deserialize::<(i64,)>().unwrap(), (40_000_000,));

    sender.send_replace(Snapshot::default());
    let values = wait_status(&connection, &name, "Stopped").await;
    let metadata = HashMap::<String, OwnedValue>::try_from(values["Metadata"].try_clone().unwrap()).unwrap();
    assert!(!metadata.contains_key("mpris:artUrl"));
    drop(sender);
    tokio::time::timeout(Duration::from_secs(3), running).await.unwrap().unwrap();
    assert!(!artwork.exists());
    assert!(
        !zbus::fdo::DBusProxy::new(&connection)
            .await
            .unwrap()
            .name_has_owner(name.as_str().try_into().unwrap())
            .await
            .unwrap()
    );
}

#[tokio::test]
async fn independent_instances_reconnect_to_a_restarted_bus_with_latest_state() {
    let temp = tempfile::tempdir().unwrap();
    let socket = temp.path().join("bus");
    let bus = Bus::start(&socket);
    let first = Service::new(temp.path().into());
    let second = Service::new(temp.path().into());
    let names = [first.bus_name().to_owned(), second.bus_name().to_owned()];
    assert_ne!(names[0], names[1]);
    let (sender, states) = watch::channel(state());
    let (commands, _receiver) = mpsc::channel(8);
    let one = tokio::spawn(first.run(Some(bus.address.clone()), states.clone(), commands.clone()));
    let two = tokio::spawn(second.run(Some(bus.address.clone()), states, commands));
    let connection = bus.connect().await;
    for name in &names {
        wait_name(&connection, name).await;
    }
    drop(connection);
    drop(bus);
    let mut latest = state();
    latest.status = Status::Playing;
    latest.position = 88_000_000;
    sender.send_replace(latest);
    let bus = Bus::start(&socket);
    let connection = bus.connect().await;
    for name in &names {
        wait_name(&connection, name).await;
        let values = wait_status(&connection, name, "Playing").await;
        assert_eq!(i64::try_from(&values["Position"]).unwrap(), 88_000_000);
    }
    drop(sender);
    tokio::time::timeout(Duration::from_secs(3), async {
        one.await.unwrap();
        two.await.unwrap();
    })
    .await
    .unwrap();
}
