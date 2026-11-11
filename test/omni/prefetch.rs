// Prefetch bounds, URL lifetime, reuse and progressive download under controlled network latency.
use super::*;
use folia_core::{Queue, Repeat};
use folia_omni::{PrefetchPlan, prefetch_indices, url_is_fresh};
use std::time::{Duration, Instant};

fn track(id: i64) -> Track {
    Track { id, online: Some(OnlineId { provider: "fixture".into(), id: id.to_string() }), ..Default::default() }
}

#[tokio::test]
async fn progressing_audio_is_not_cut_off_at_sixty_seconds() {
    use std::io::{Read, Write};
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}/audio", listener.local_addr().unwrap());
    let (send, receive) = std::sync::mpsc::channel();
    let worker = std::thread::spawn(move || {
        let (mut socket, _) = listener.accept().unwrap();
        socket.read(&mut [0; 4096]).unwrap();
        socket.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 69632\r\nConnection: close\r\n\r\n").unwrap();
        socket.write_all(&vec![7; 65536]).unwrap();
        for _ in 0..4 {
            if receive.recv_timeout(Duration::from_secs(5)).is_err() {
                return;
            }
            if socket.write_all(&[9; 1024]).is_err() {
                return;
            }
        }
    });
    let temp = tempfile::tempdir().unwrap();
    let omni = Omni::with_providers(
        temp.path().into(),
        "unused".into(),
        vec![Box::new(Native { url, calls: Default::default() })],
    )
    .unwrap();
    let input = omni.prepare_track_audio(&track(1), AudioQuality::High).await.unwrap();
    let download = input.download.as_ref().unwrap();
    tokio::time::pause();
    for index in 1..=4 {
        tokio::time::advance(Duration::from_secs(20)).await;
        send.send(()).unwrap();
        let deadline = Instant::now() + Duration::from_secs(3);
        loop {
            let (bytes, done, error) = download.progress();
            assert!(error.is_none(), "healthy stream aborted at {}s: {error:?}", index * 20);
            if bytes >= 65536 + index * 1024 {
                break;
            }
            assert!(!done && Instant::now() < deadline, "stream did not progress");
            tokio::task::yield_now().await;
        }
    }
    tokio::time::resume();
    let path = omni.fetch_track_audio(&track(1), AudioQuality::High).await.unwrap();
    assert_eq!(std::fs::metadata(path).unwrap().len(), 69632);
    worker.join().unwrap();
}

#[tokio::test]
async fn stalled_audio_still_times_out_before_and_after_playback_starts() {
    use std::io::{Read, Write};
    for prefix in [0, 65536] {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/audio", listener.local_addr().unwrap());
        let (release, wait) = std::sync::mpsc::channel();
        let worker = std::thread::spawn(move || {
            let (mut socket, _) = listener.accept().unwrap();
            socket.read(&mut [0; 4096]).unwrap();
            socket.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 100000\r\nConnection: close\r\n\r\n").unwrap();
            socket.write_all(&vec![7; prefix]).unwrap();
            let _ = wait.recv_timeout(Duration::from_secs(5));
        });
        let temp = tempfile::tempdir().unwrap();
        let omni = Omni::with_providers(
            temp.path().into(),
            "unused".into(),
            vec![Box::new(Native { url, calls: Default::default() })],
        )
        .unwrap();
        let mut pending = tokio::spawn(async move { omni.prepare_track_audio(&track(1), AudioQuality::High).await });
        let deadline = Instant::now() + Duration::from_secs(3);
        while !temp.path().join("audio/fixture-1-high.part").exists() {
            assert!(Instant::now() < deadline);
            tokio::task::yield_now().await;
        }
        let input = if prefix > 0 { Some((&mut pending).await.unwrap().unwrap()) } else { None };
        tokio::time::pause();
        tokio::time::advance(Duration::from_secs(61)).await;
        if let Some(input) = input {
            let download = input.download.unwrap();
            while !download.progress().1 {
                assert!(Instant::now() < deadline);
                tokio::task::yield_now().await;
            }
            assert!(download.progress().2.unwrap().contains("more audio data"));
        } else {
            // The body task removes the partial file on the initial-buffer timeout.
            while temp.path().join("audio/fixture-1-high.part").exists() {
                assert!(Instant::now() < deadline);
                tokio::task::yield_now().await;
            }
            assert!(pending.await.unwrap().unwrap_err().to_string().contains("initial audio data"));
        }
        tokio::time::resume();
        assert!(!temp.path().join("audio/fixture-1-high.mp3").exists());
        release.send(()).unwrap();
        worker.join().unwrap();
    }
}

#[tokio::test]
async fn lx_prefetch_uses_the_same_source_cache_as_playback() {
    let temp = tempfile::tempdir().unwrap();
    let calls = Arc::new(AtomicUsize::new(0));
    let (url, server) = server(b"LX prefetched audio", "200 OK");
    let omni = Omni::with_providers(
        temp.path().join("cache"),
        "unused".into(),
        vec![Box::new(Native { url: "http://unused.invalid".into(), calls: calls.clone() })],
    )
    .unwrap();
    omni.restore_sources(Store::at(temp.path().join("config")), temp.path().join("data")).unwrap();
    let code = format!("{BASIC}\nlet count = 0; lx.on('request', async () => '{}' + '?call=' + (++count));", url);
    let path = temp.path().join("source.js");
    std::fs::write(&path, code).unwrap();
    let settings = omni.import_source(&path).unwrap();
    let id = &settings.scripts[0].id;
    omni.enable_source(id, true).unwrap();
    let plan = PrefetchPlan { tracks: vec![track(1)], quality: AudioQuality::High };
    omni.prefetch(plan).await;
    assert!(omni.audio_is_prefetched(&track(1), AudioQuality::High));
    let path = omni.fetch_track_audio(&track(1), AudioQuality::High).await.unwrap();
    assert!(path.to_string_lossy().contains(id));
    assert_eq!(std::fs::read(path).unwrap(), b"LX prefetched audio");
    assert!(omni.resolve_audio_source(&track(1), AudioQuality::High).await.unwrap().url.ends_with("?call=2"));
    assert_eq!(calls.load(Ordering::Relaxed), 0);
    server.join().unwrap();
    omni.enable_source(id, false).unwrap();
    assert!(!omni.audio_is_prefetched(&track(1), AudioQuality::High));
}

#[tokio::test]
async fn truncated_download_never_becomes_a_cache_hit() {
    use std::io::{Read, Write};
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}/audio", listener.local_addr().unwrap());
    let server = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        stream.read(&mut [0; 4096]).unwrap();
        stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 9999\r\nConnection: close\r\n\r\nshort").unwrap();
    });
    let temp = tempfile::tempdir().unwrap();
    let calls = Arc::new(AtomicUsize::new(0));
    let omni =
        Omni::with_providers(temp.path().into(), "unused".into(), vec![Box::new(Native { url, calls })]).unwrap();
    assert!(omni.prepare_track_audio(&track(1), AudioQuality::High).await.is_err());
    assert!(!omni.audio_is_prefetched(&track(1), AudioQuality::High));
    assert_eq!(std::fs::read_dir(temp.path().join("audio")).unwrap().count(), 0);
    server.join().unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn failure_after_the_prefix_continues_with_the_native_source() {
    use std::io::{Read, Write};
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}/audio", listener.local_addr().unwrap());
    let (release, wait) = std::sync::mpsc::channel();
    let primary = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        stream.read(&mut [0; 4096]).unwrap();
        stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 131072\r\nConnection: close\r\n\r\n").unwrap();
        stream.write_all(&vec![7; 65536]).unwrap();
        wait.recv_timeout(Duration::from_secs(10)).unwrap();
    });
    let temp = tempfile::tempdir().unwrap();
    let calls = Arc::new(AtomicUsize::new(0));
    let (native_url, native) = server(b"native fallback audio", "200 OK");
    let omni = Omni::with_providers(
        temp.path().join("cache"),
        "unused".into(),
        vec![Box::new(Native { url: native_url, calls: calls.clone() })],
    )
    .unwrap();
    omni.restore_sources(Store::at(temp.path().join("config")), temp.path().join("data")).unwrap();
    let path = temp.path().join("source.js");
    std::fs::write(&path, format!("{BASIC}\nlx.on('request', async () => '{url}');")).unwrap();
    let settings = omni.import_source(&path).unwrap();
    omni.enable_source(&settings.scripts[0].id, true).unwrap();
    let input = omni.prepare_track_audio(&track(1), AudioQuality::High).await.unwrap();
    release.send(()).unwrap();
    let download = input.download.as_ref().unwrap();
    tokio::time::timeout(Duration::from_secs(3), async {
        while !download.progress().1 {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap();
    assert!(download.progress().2.is_some());
    let fallback = omni.prepare_track_audio_after(&track(1), AudioQuality::High, &[input.path]).await.unwrap();
    assert_eq!(std::fs::read(fallback.path).unwrap(), b"native fallback audio");
    assert_eq!(calls.load(Ordering::Relaxed), 1);
    primary.join().unwrap();
    native.join().unwrap();
}

#[test]
fn prefetch_order_queue_edits_and_expiry() {
    assert_eq!(prefetch_indices(8, Some(3), Repeat::Off), [4, 5, 2]);
    assert_eq!(prefetch_indices(8, Some(0), Repeat::Off), [1, 2]);
    assert_eq!(prefetch_indices(8, Some(7), Repeat::Off), [6]);
    assert_eq!(prefetch_indices(8, Some(7), Repeat::All), [0, 6]);
    assert!(prefetch_indices(0, None, Repeat::All).is_empty());
    assert!(prefetch_indices(1, Some(0), Repeat::All).is_empty());
    let mut queue = Queue::default();
    queue.replace((0..6).map(track).collect(), 2);
    let old = PrefetchPlan::new(&queue, AudioQuality::High);
    queue.move_to(5, 3);
    let new = PrefetchPlan::new(&queue, AudioQuality::High);
    assert_ne!(old, new);
    assert_eq!(new.tracks.iter().map(|track| track.id).collect::<Vec<_>>(), [5, 3, 1]);
    let now = Instant::now();
    assert!(url_is_fresh(now, now + Duration::from_millis(1_199_999)));
    assert!(!url_is_fresh(now, now + Duration::from_secs(1200)));
}

#[tokio::test]
async fn prefetch_reuses_address_and_quality_change_resolves_again() {
    let temp = tempfile::tempdir().unwrap();
    let calls = Arc::new(AtomicUsize::new(0));
    let (url, server) = server(b"prefetched audio", "200 OK");
    let omni =
        Omni::with_providers(temp.path().into(), "unused".into(), vec![Box::new(Native { url, calls: calls.clone() })])
            .unwrap();
    let plan = PrefetchPlan { tracks: vec![track(1)], quality: AudioQuality::High };
    omni.prefetch(plan.clone()).await;
    omni.prefetch(plan.clone()).await;
    assert_eq!(calls.load(Ordering::Relaxed), 1);
    let path = omni.fetch_track_audio(&track(1), AudioQuality::High).await.unwrap();
    assert_eq!(std::fs::read(&path).unwrap(), b"prefetched audio");
    assert_eq!(calls.load(Ordering::Relaxed), 1);
    server.join().unwrap();
    omni.prefetch(PrefetchPlan { quality: AudioQuality::Standard, ..plan }).await;
    assert_eq!(calls.load(Ordering::Relaxed), 2);
}

#[tokio::test(flavor = "multi_thread")]
async fn progressive_reader_starts_before_eof_and_downloads_once() {
    use std::io::{Read, Write};
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}/audio", listener.local_addr().unwrap());
    let (release, wait) = std::sync::mpsc::channel();
    let server = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let mut request = [0; 4096];
        stream.read(&mut request).unwrap();
        write!(stream, "HTTP/1.1 200 OK\r\nContent-Length: 131072\r\nConnection: close\r\n\r\n").unwrap();
        stream.write_all(&vec![7; 65536]).unwrap();
        wait.recv_timeout(Duration::from_secs(10)).unwrap();
        stream.write_all(&vec![9; 65536]).unwrap();
    });
    let temp = tempfile::tempdir().unwrap();
    let calls = Arc::new(AtomicUsize::new(0));
    let omni =
        Omni::with_providers(temp.path().into(), "unused".into(), vec![Box::new(Native { url, calls: calls.clone() })])
            .unwrap();
    let input = omni.prepare_track_audio(&track(1), AudioQuality::High).await.unwrap();
    assert!(!input.path.exists(), "partial file must not appear in completed cache");
    let download = input.download.as_ref().unwrap();
    assert!(!download.progress().1);
    let mut reader = download.reader(Default::default());
    let mut prefix = [0; 32];
    reader.read_exact(&mut prefix).unwrap();
    assert_eq!(prefix, [7; 32]);
    let second = omni.prepare_track_audio(&track(1), AudioQuality::High).await.unwrap();
    assert_eq!(second.path, input.path);
    assert_eq!(calls.load(Ordering::Relaxed), 1);
    release.send(()).unwrap();
    let path = omni.fetch_track_audio(&track(1), AudioQuality::High).await.unwrap();
    let mut rest = Vec::new();
    reader.read_to_end(&mut rest).unwrap();
    assert_eq!(rest.len(), 131072 - 32);
    assert_eq!(rest.last(), Some(&9));
    assert_eq!(std::fs::metadata(path).unwrap().len(), 131072);
    server.join().unwrap();
}
