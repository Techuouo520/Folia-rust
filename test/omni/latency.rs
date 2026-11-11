// Live QQ timing, with separate URL resolution, headers, first bytes and complete download stages.
use folia_core::AudioQuality;
use folia_omni::Omni;
use std::time::Instant;

fn qq_track() -> folia_core::Track {
    folia_core::Track {
        title: "Scheming Weasel (faster version)".into(),
        online: Some(folia_core::OnlineId {
            provider: "qq".into(),
            id: "0037sGh91smErd-200980117-004AfGz930jQYq".into(),
        }),
        ..Default::default()
    }
}

#[tokio::test]
#[ignore = "live QQ CDN range support"]
async fn qq_cdn_range_uses_the_same_representation() {
    let temp = tempfile::tempdir().unwrap();
    let omni = Omni::new(temp.path().into(), "unused".into()).unwrap();
    let source = omni.resolve_audio_source(&qq_track(), AudioQuality::High).await.unwrap();
    let http = reqwest::Client::new();
    let base = http
        .get(&source.url)
        .header("accept-encoding", "identity")
        .timeout(std::time::Duration::from_secs(20))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap();
    let Some(tag) = base.headers().get("etag").cloned() else {
        eprintln!("QQ CDN has no ETag; uses sequential fallback");
        return;
    };
    let range = http
        .get(&source.url)
        .header("accept-encoding", "identity")
        .header("range", "bytes=500000-762143")
        .header("if-range", tag.clone())
        .timeout(std::time::Duration::from_secs(20))
        .send()
        .await
        .unwrap();
    eprintln!(
        "QQ range status={} same_etag={} content_range={:?}",
        range.status(),
        range.headers().get("etag") == Some(&tag),
        range.headers().get("content-range")
    );
    if range.status() == reqwest::StatusCode::PARTIAL_CONTENT {
        assert_eq!(range.headers().get("etag"), Some(&tag));
        assert_eq!(range.bytes().await.unwrap().len(), 262144);
    }
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "live QQ network timing"]
async fn qq_progressive_and_prefetched() {
    let _ = tracing_subscriber::fmt().with_env_filter("folia_omni=debug").with_test_writer().try_init();
    for prefetch in [false, true, false, true, false, true] {
        progressive_once(prefetch).await;
    }
}

#[tokio::test]
#[ignore = "live QQ network timing"]
async fn qq_baseline_full_download() {
    for _ in 0..3 {
        baseline_once().await;
    }
}

async fn progressive_once(prefetch: bool) {
    let cache = tempfile::tempdir().unwrap();
    let omni = Omni::new(cache.path().into(), "Folia-rust-test".into()).unwrap();
    let track = qq_track();
    if prefetch {
        omni.prefetch(folia_omni::PrefetchPlan { tracks: vec![track.clone()], quality: AudioQuality::High }).await;
    }
    let player = folia_player::Player::new().unwrap();
    player.set_volume(0.0);
    let started = Instant::now();
    let input = omni.prepare_track_audio(&track, AudioQuality::High).await.unwrap();
    let ready = started.elapsed();
    let download = input.download.clone();
    player.load_input(input.clone());
    let decoded = tokio::task::spawn_blocking(move || {
        let mut source = folia_player::DecodedSource::open_input(&input, Default::default()).unwrap();
        assert!(!source.next_packet().unwrap().unwrap().samples.is_empty());
        started.elapsed()
    })
    .await
    .unwrap();
    while player.status().position == 0.0 && started.elapsed().as_secs() < 10 {
        tokio::time::sleep(std::time::Duration::from_millis(1)).await;
    }
    assert!(player.status().position > 0.0, "audio callback did not consume samples");
    let output = started.elapsed();
    player.stop();
    let path = omni.fetch_track_audio(&track, AudioQuality::High).await.unwrap();
    eprintln!(
        "QQ AFTER prefetch={prefetch} ready_ms={:.2} first_decoded_ms={:.2} output_ms={:.2} full_file_ms={:.2} bytes={}",
        ready.as_secs_f64() * 1000.0,
        decoded.as_secs_f64() * 1000.0,
        output.as_secs_f64() * 1000.0,
        started.elapsed().as_secs_f64() * 1000.0,
        std::fs::metadata(path).unwrap().len()
    );
    assert!(download.unwrap().progress().1);
}

async fn baseline_once() {
    let cache = tempfile::tempdir().unwrap();
    let omni = Omni::new(cache.path().into(), "Folia-rust-test".into()).unwrap();
    let track = qq_track();
    let player = folia_player::Player::new().unwrap();
    player.set_volume(0.0);
    let started = Instant::now();
    let source = omni.resolve_audio_source(&track, AudioQuality::High).await.unwrap();
    let resolved = started.elapsed();
    let mut response = reqwest::Client::new().get(&source.url).send().await.unwrap().error_for_status().unwrap();
    let headers = started.elapsed();
    let mut first = None;
    let mut bytes = Vec::new();
    while let Some(chunk) = response.chunk().await.unwrap() {
        first.get_or_insert(started.elapsed());
        bytes.extend_from_slice(&chunk);
    }
    let partial = cache.path().join("baseline.part");
    let path = cache.path().join(format!("baseline.{}", source.extension));
    std::fs::write(&partial, &bytes).unwrap();
    std::fs::rename(partial, &path).unwrap();
    let eligible = started.elapsed();
    let mut decoded = folia_player::DecodedSource::open(&path).unwrap();
    assert!(!decoded.next_packet().unwrap().unwrap().samples.is_empty());
    let first_pcm = started.elapsed();
    player.load(&path);
    while player.status().position == 0.0 && started.elapsed().as_secs() < 10 {
        tokio::time::sleep(std::time::Duration::from_millis(1)).await;
    }
    assert!(player.status().position > 0.0);
    let output = started.elapsed();
    player.stop();
    eprintln!(
        "QQ BASELINE track={} id={} ext={} bytes={} resolved_ms={:.2} headers_ms={:.2} first_ms={:.2} decoder_eligible_ms={:.2} first_decoded_ms={:.2} output_ms={:.2}",
        track.title,
        track.online.as_ref().unwrap().id,
        source.extension,
        bytes.len(),
        resolved.as_secs_f64() * 1000.0,
        headers.as_secs_f64() * 1000.0,
        first.unwrap().as_secs_f64() * 1000.0,
        eligible.as_secs_f64() * 1000.0,
        first_pcm.as_secs_f64() * 1000.0,
        output.as_secs_f64() * 1000.0
    );
    assert!(bytes.len() > 10_000);
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "live QQ paired network timing"]
async fn qq_latency_comparison() {
    let _ = tracing_subscriber::fmt().with_env_filter("folia_omni=debug").with_test_writer().try_init();
    for _ in 0..3 {
        baseline_once().await;
        progressive_once(false).await;
        progressive_once(true).await;
    }
}
