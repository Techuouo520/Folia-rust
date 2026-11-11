// Progressive input decodes before completion, waits at a temporary EOF, seeks and cancels promptly.
use folia_core::{AudioInput, Download};

#[test]
fn a_new_seek_interrupts_waiting_for_an_uncached_position() {
    let temp = tempfile::tempdir().unwrap();
    let full = temp.path().join("full.wav");
    super::write_wav(&full, 44100, 8.0);
    let bytes = std::fs::read(full).unwrap();
    let path = temp.path().join("partial.wav");
    std::fs::write(&path, &bytes[..65536]).unwrap();
    let download = Download::new(std::fs::File::open(&path).unwrap(), Some(bytes.len() as u64));
    download.publish(65536);
    let Ok(player) = folia_player::Player::new() else {
        eprintln!("no audio output device; skipping transport check");
        return;
    };
    player.set_volume(0.0);
    player.load_input(AudioInput { path, download: Some(download) });
    let ready = super::wait_for(&player, Duration::from_secs(2), |status| status.position > 0.0);
    assert!(ready.position > 0.0);
    player.seek(6.0);
    std::thread::sleep(Duration::from_millis(100));
    assert!(player.is_buffering());
    assert!((player.status().position - 6.0).abs() < 0.05);
    player.pause();
    player.seek(0.1);
    std::thread::sleep(Duration::from_millis(100));
    assert_eq!(player.status().state, folia_player::PlayerState::Paused);
    player.resume();
    let recovered =
        super::wait_for(&player, Duration::from_secs(1), |status| status.position > 0.1 && status.position < 1.0);
    player.stop();
    assert!(recovered.position > 0.1 && recovered.position < 1.0, "new seek stayed behind the old read: {recovered:?}");
}

#[test]
fn flac_seek_cancellation_preserves_the_track_and_can_return_to_the_cached_prefix() {
    let data = include_bytes!("fixtures/seek.flac");
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("seek.flac");
    std::fs::write(&path, &data[..65536]).unwrap();
    let download = Download::new(std::fs::File::open(&path).unwrap(), Some(data.len() as u64));
    download.publish(65536);
    let Ok(player) = folia_player::Player::new() else {
        eprintln!("no audio output device; skipping transport check");
        return;
    };
    player.set_volume(0.0);
    player.load_input(AudioInput { path, download: Some(download) });
    assert!(super::wait_for(&player, Duration::from_secs(2), |s| s.position > 0.0).position > 0.0);
    for _ in 0..5 {
        player.seek(1.8);
        std::thread::sleep(Duration::from_millis(50));
        assert!(player.is_buffering());
        player.seek(0.05);
        let status = super::wait_for(&player, Duration::from_secs(1), |s| s.position > 0.1 && s.position < 0.7);
        assert_eq!(status.state, folia_player::PlayerState::Playing);
        assert!(status.position > 0.1 && status.position < 0.7, "{status:?}");
    }
    player.seek(1.8);
    std::thread::sleep(Duration::from_millis(50));
    player.stop();
    std::thread::sleep(Duration::from_millis(100));
    assert_eq!(player.status().state, folia_player::PlayerState::Idle);
    assert!(!player.is_buffering());
}

#[test]
fn sparse_download_holes_wait_and_published_ranges_merge_with_the_prefix() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("sparse");
    let mut file = std::fs::File::create(&path).unwrap();
    file.write_all(b"head").unwrap();
    file.seek(SeekFrom::Start(8)).unwrap();
    file.write_all(b"tail").unwrap();
    let download = Download::new(std::fs::File::open(&path).unwrap(), Some(12));
    download.publish(4);
    download.publish_range(8, 12);
    let cancel = Arc::new(AtomicBool::new(false));
    let waiting = Arc::new(AtomicBool::new(false));
    let mut reader = download.reader_with_status(cancel.clone(), waiting.clone());
    reader.seek(SeekFrom::Start(8)).unwrap();
    let mut tail = [0; 4];
    reader.read_exact(&mut tail).unwrap();
    assert_eq!(&tail, b"tail");
    reader.seek(SeekFrom::Start(4)).unwrap();
    let worker = std::thread::spawn(move || reader.read(&mut [0; 4]));
    let deadline = Instant::now() + Duration::from_secs(1);
    while !waiting.load(Ordering::Acquire) && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(1));
    }
    assert!(waiting.load(Ordering::Acquire));
    cancel.store(true, Ordering::Release);
    assert_eq!(worker.join().unwrap().unwrap_err().kind(), std::io::ErrorKind::ConnectionAborted);
    assert!(!waiting.load(Ordering::Acquire));
    file.seek(SeekFrom::Start(4)).unwrap();
    file.write_all(b"body").unwrap();
    download.publish_range(4, 8);
    assert_eq!(download.progress().0, 12);
    download.publish(6);
    assert_eq!(download.progress().0, 12, "sequential progress cannot forget already validated bytes");
}

#[test]
fn read_ahead_waiting_is_not_buffering_until_the_output_runs_dry() {
    let Ok(player) = folia_player::Player::new() else { return };
    player.set_volume(0.0);
    let temp = tempfile::tempdir().unwrap();
    let full = temp.path().join("full.wav");
    super::write_wav(&full, 44100, 8.0);
    let data = std::fs::read(full).unwrap();
    let path = temp.path().join("partial.wav");
    std::fs::write(&path, &data[..65536]).unwrap();
    let download = Download::new(std::fs::File::open(&path).unwrap(), Some(data.len() as u64));
    download.publish(65536);
    player.load_input(AudioInput { path, download: Some(download) });
    let playing = super::wait_for(&player, Duration::from_secs(2), |s| s.position > 0.15);
    assert!(playing.position > 0.15 && playing.position < 0.5);
    assert!(!player.is_buffering(), "queued PCM still plays despite a read-ahead wait");
    let until = Instant::now() + Duration::from_secs(2);
    while !player.is_buffering() && Instant::now() < until {
        std::thread::sleep(Duration::from_millis(5));
    }
    assert!(player.is_buffering());
    player.stop();
}
use std::io::{Read, Seek, SeekFrom, Write};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::time::{Duration, Instant};

#[test]
fn incomplete_wav_decodes_without_waiting_for_tail() {
    let temp = tempfile::tempdir().unwrap();
    let full = temp.path().join("full.wav");
    super::write_wav(&full, 44100, 5.0);
    let bytes = std::fs::read(full).unwrap();
    let path = temp.path().join("partial.wav");
    std::fs::write(&path, &bytes[..65536]).unwrap();
    let download = Download::new(std::fs::File::open(&path).unwrap(), Some(bytes.len() as u64));
    download.publish(65536);
    let input = AudioInput { path, download: Some(download.clone()) };
    let cancelled = Arc::new(AtomicBool::new(false));
    let (sent, received) = std::sync::mpsc::channel();
    let token = cancelled.clone();
    let worker = std::thread::spawn(move || {
        let result = folia_player::DecodedSource::open_input(&input, token)
            .and_then(|mut source| source.next_packet().map(|block| block.unwrap().samples.len()));
        sent.send(result).unwrap();
    });
    let result = received.recv_timeout(Duration::from_secs(3));
    cancelled.store(true, Ordering::Release);
    worker.join().unwrap();
    assert!(result.unwrap().unwrap() > 0);
    assert!(!download.progress().1);
}

#[test]
fn m4a_with_trailing_metadata_can_wait_for_the_tail_without_ranges() {
    let data = include_bytes!("fixtures/seek.m4a");
    assert!(data.len() > 65536);
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("partial.m4a");
    std::fs::write(&path, &data[..65536]).unwrap();
    let download = Download::new(std::fs::File::open(&path).unwrap(), Some(data.len() as u64));
    download.publish(65536);
    let input = AudioInput { path: path.clone(), download: Some(download.clone()) };
    let producer = std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(80));
        std::fs::OpenOptions::new().append(true).open(path).unwrap().write_all(&data[65536..]).unwrap();
        download.publish(data.len() as u64);
        download.finish(Ok(()));
    });
    let result = folia_player::DecodedSource::open_input(&input, Default::default());
    producer.join().unwrap();
    let mut source = result.expect("a seekable growing file must not be mistaken for an unseekable MP4 stream");
    assert!(source.next_packet().unwrap().is_some());
}

#[test]
fn temporary_eof_seek_error_and_cancel_are_distinct() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("part");
    std::fs::write(&path, b"head").unwrap();
    let download = Download::new(std::fs::File::open(&path).unwrap(), Some(8));
    download.publish(4);
    let mut reader = download.reader(Default::default());
    assert_eq!(reader.seek(SeekFrom::End(-4)).unwrap(), 4);
    let producer = download.clone();
    let worker = std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(40));
        std::fs::OpenOptions::new().append(true).open(path).unwrap().write_all(b"tail").unwrap();
        producer.publish(8);
        producer.finish(Ok(()));
    });
    let mut tail = Vec::new();
    reader.read_to_end(&mut tail).unwrap();
    assert_eq!(tail, b"tail");
    worker.join().unwrap();

    let cancel = Arc::new(AtomicBool::new(true));
    let mut cancelled = download.reader(cancel);
    let at = Instant::now();
    assert_eq!(cancelled.read(&mut [0]).unwrap_err().kind(), std::io::ErrorKind::ConnectionAborted);
    assert!(at.elapsed() < Duration::from_millis(100));
    download.finish(Err("fixture network failed".into()));
    assert!(reader.read(&mut [0]).unwrap_err().to_string().contains("fixture network failed"));
}

#[test]
fn cancellation_wakes_a_reader_waiting_for_network_bytes() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("part");
    std::fs::write(&path, []).unwrap();
    let download = Download::new(std::fs::File::open(path).unwrap(), Some(100));
    let cancel = Arc::new(AtomicBool::new(false));
    let mut reader = download.reader(cancel.clone());
    let worker = std::thread::spawn(move || reader.read(&mut [0; 16]).unwrap_err().kind());
    std::thread::sleep(Duration::from_millis(30));
    let at = Instant::now();
    cancel.store(true, Ordering::Release);
    assert_eq!(worker.join().unwrap(), std::io::ErrorKind::ConnectionAborted);
    assert!(at.elapsed() < Duration::from_millis(200));
}
