// HTTP seek ranges must match the original representation and must never expose sparse holes as cache hits.
use super::*;
use std::io::{Read, Seek, SeekFrom, Write};
use std::time::{Duration, Instant};

#[tokio::test(flavor = "multi_thread")]
async fn flac_seek_plays_the_target_before_the_sequential_download_catches_up() {
    let Ok(player) = folia_player::Player::new() else {
        eprintln!("no audio output device; skipping transport check");
        return;
    };
    let data: &'static [u8] = include_bytes!("../player/fixtures/seek.flac");
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}/audio", listener.local_addr().unwrap());
    let (release, wait) = std::sync::mpsc::channel();
    let worker = std::thread::spawn(move || {
        let (mut primary, _) = listener.accept().unwrap();
        primary.read(&mut [0; 4096]).unwrap();
        write!(
            primary,
            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nETag: \"flac\"\r\nConnection: close\r\n\r\n",
            data.len()
        )
        .unwrap();
        primary.write_all(&data[..65536]).unwrap();
        let (mut range, _) = listener.accept().unwrap();
        let mut request = [0; 4096];
        let n = range.read(&mut request).unwrap();
        let request = String::from_utf8_lossy(&request[..n]).to_lowercase();
        let range_value = request.lines().find_map(|line| line.strip_prefix("range: bytes=")).unwrap();
        let offset: usize = range_value.split_once('-').unwrap().0.parse().unwrap();
        assert!(offset >= 65536 && offset < data.len());
        let count = data.len() - offset;
        write!(range, "HTTP/1.1 206 Partial Content\r\nContent-Length: {count}\r\nContent-Range: bytes {offset}-{}/{}\r\nETag: \"flac\"\r\nConnection: close\r\n\r\n", data.len()-1, data.len()).unwrap();
        range.write_all(&data[offset..]).unwrap();
        wait.recv_timeout(Duration::from_secs(10)).unwrap();
        primary.write_all(&data[65536..]).unwrap();
    });
    let temp = tempfile::tempdir().unwrap();
    let omni = Omni::with_providers(
        temp.path().into(),
        "unused".into(),
        vec![Box::new(Native { url, calls: Default::default() })],
    )
    .unwrap();
    let track =
        Track { online: Some(OnlineId { provider: "fixture".into(), id: "flac-range".into() }), ..Default::default() };
    let input = omni.prepare_track_audio(&track, AudioQuality::High).await.unwrap();
    let download = input.download.clone().unwrap();
    player.set_volume(0.0);
    player.load_input(input.clone());
    let until = Instant::now() + Duration::from_secs(2);
    while player.status().position == 0.0 && Instant::now() < until {
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    assert!(player.status().position > 0.0);
    player.seek(1.8);
    let began = Instant::now();
    while (player.is_buffering() || player.status().position <= 1.8) && began.elapsed() < Duration::from_secs(2) {
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    assert!(player.status().position > 1.8 && !player.is_buffering(), "seek did not resume: {:?}", player.status());
    assert_eq!(download.progress().0, 65536);
    assert!(!input.path.exists());
    player.stop();
    release.send(()).unwrap();
    let path = omni.fetch_track_audio(&track, AudioQuality::High).await.unwrap();
    assert_eq!(std::fs::read(path).unwrap(), data);
    worker.join().unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn seek_ranges_validate_status_entity_offsets_and_complete_body() {
    for mode in ["valid", "ignored", "changed", "offset", "truncated"] {
        let data: Arc<Vec<u8>> = Arc::new((0..2 * 1024 * 1024).map(|index| ((index / 1024) % 251) as u8).collect());
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/audio", listener.local_addr().unwrap());
        let (release, wait) = std::sync::mpsc::channel();
        let bytes = data.clone();
        let worker = std::thread::spawn(move || {
            let (mut primary, _) = listener.accept().unwrap();
            primary.read(&mut [0; 4096]).unwrap();
            write!(
                primary,
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nETag: \"fixture\"\r\nConnection: close\r\n\r\n",
                bytes.len()
            )
            .unwrap();
            primary.write_all(&bytes[..65536]).unwrap();
            let (mut range, _) = listener.accept().unwrap();
            let mut request = [0; 4096];
            let n = range.read(&mut request).unwrap();
            let request = String::from_utf8_lossy(&request[..n]).to_lowercase();
            assert!(request.contains("range: bytes=1500000-1762143"), "{request}");
            assert!(request.contains("if-range: \"fixture\""));
            assert!(request.contains("accept-encoding: identity"));
            let count = 262144;
            if mode == "ignored" {
                write!(range, "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", bytes.len())
                    .unwrap();
            } else {
                let tag = if mode == "changed" { "other" } else { "fixture" };
                let offset = if mode == "offset" { 0 } else { 1_500_000 };
                write!(range, "HTTP/1.1 206 Partial Content\r\nContent-Length: {count}\r\nContent-Range: bytes {offset}-1762143/2097152\r\nETag: \"{tag}\"\r\nConnection: close\r\n\r\n").unwrap();
                let body = if mode == "valid" {
                    bytes[1_500_000..1_762_144].to_vec()
                } else if mode == "truncated" {
                    vec![238; 32]
                } else {
                    vec![238; count]
                };
                let _ = range.write_all(&body);
            }
            drop(range);
            wait.recv_timeout(Duration::from_secs(10)).unwrap();
            primary.write_all(&bytes[65536..]).unwrap();
        });
        let temp = tempfile::tempdir().unwrap();
        let omni = Omni::with_providers(
            temp.path().into(),
            "unused".into(),
            vec![Box::new(Native { url, calls: Default::default() })],
        )
        .unwrap();
        let track =
            Track { online: Some(OnlineId { provider: "fixture".into(), id: "range".into() }), ..Default::default() };
        let input = omni.prepare_track_audio(&track, AudioQuality::High).await.unwrap();
        let download = input.download.clone().unwrap();
        let mut reader = download.reader(Default::default());
        let read = tokio::task::spawn_blocking(move || {
            reader.seek(SeekFrom::Start(1_500_000)).unwrap();
            let mut bytes = [0; 32];
            reader.read_exact(&mut bytes).unwrap();
            bytes
        });
        if mode == "valid" {
            let deadline = Instant::now() + Duration::from_secs(3);
            while !read.is_finished() && Instant::now() < deadline {
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
            assert!(read.is_finished(), "seek waited for the sequential download");
            assert_eq!(download.progress().0, 65536, "tail does not make the hole readable");
            assert!(!input.path.exists(), "sparse partial is not a finished cache entry");
        } else {
            tokio::time::sleep(Duration::from_millis(150)).await;
            assert!(!read.is_finished(), "unvalidated range was published: {mode}");
        }
        release.send(()).unwrap();
        assert_eq!(read.await.unwrap(), data[1_500_000..1_500_032]);
        let path = omni.fetch_track_audio(&track, AudioQuality::High).await.unwrap();
        assert_eq!(std::fs::read(path).unwrap(), *data);
        worker.join().unwrap();
    }
}
