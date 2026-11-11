// Offline checks for the QR pictures platforms hand out, and live platform checks behind `--ignored`.

#![cfg(test)]

use folia_core::AudioQuality;
use folia_omni::{LoginQr, Omni, OmniError, cover_cache_key};

mod like_capabilities;

#[test]
fn registry_lists_providers_in_original_order() {
    let omni = Omni::new(std::env::temp_dir(), "Folia-rust-test".to_owned()).unwrap();
    let ids: Vec<_> = omni.providers().into_iter().map(|(id, _)| id).collect();
    assert_eq!(ids, ["netease", "kugou", "qq", "bodian", "spotify", "qishui"]);
    assert!(omni.platforms().iter().all(|platform| platform.available));
    let listed: Vec<_> = omni.platforms().into_iter().map(|platform| platform.id).collect();
    assert_eq!(listed, ids);
    assert!(!listed.contains(&"ytm"));
}

/// Douyin's 256×256 picture behind Qishui's sign-in: 73 modules in a 231px dark square.
#[test]
fn qishui_png_reads_as_73_modules() {
    let qr = LoginQr::decode_png("key".to_owned(), include_bytes!("fixtures/qishui-qr.png")).unwrap();
    assert_eq!(qr.width, 73);
    assert_eq!(qr.modules.len(), 73 * 73);
    for index in 8..=qr.width - 9 {
        let dark = (index - 8) % 2 == 0;
        assert_eq!(qr.dark(index, 6), dark, "timing row at {index}");
        assert_eq!(qr.dark(6, index), dark, "timing column at {index}");
    }
    let content = decode(render(&qr));
    assert!(content.starts_with("aweme://authorizedy"), "{content}");
}

/// QQ's 111×111 ptlogin picture: 33 modules at exactly three pixels each.
#[test]
fn qq_png_reads_as_33_modules() {
    let qr = LoginQr::decode_png("key".to_owned(), include_bytes!("fixtures/qq-qr.png")).unwrap();
    assert_eq!(qr.width, 33);
    let content = decode(render(&qr));
    assert!(content.starts_with("http://txz.qq.com/p?k="), "{content}");
}

#[test]
fn a_picture_without_a_qr_width_is_refused() {
    let solid = image::GrayImage::from_pixel(64, 64, image::Luma([0]));
    let error = LoginQr::decode_png("key".to_owned(), &png(&solid)).unwrap_err();
    assert!(matches!(error, OmniError::QrImage(_)), "{error:?}");
}

/// Paints the matrix four pixels per module, wide enough for the decoder's grid detector.
fn render(qr: &LoginQr) -> image::GrayImage {
    let scale = 4;
    let side = (qr.width * scale) as u32;
    let mut picture = image::GrayImage::from_pixel(side, side, image::Luma([255]));
    for y in 0..qr.width {
        for x in 0..qr.width {
            if qr.dark(x, y) {
                for dy in 0..scale {
                    for dx in 0..scale {
                        picture.put_pixel((x * scale + dx) as u32, (y * scale + dy) as u32, image::Luma([0]));
                    }
                }
            }
        }
    }
    picture
}

fn decode(picture: image::GrayImage) -> String {
    let mut prepared = rqrr::PreparedImage::prepare(picture);
    let grids = prepared.detect_grids();
    assert_eq!(grids.len(), 1);
    let (_, content) = grids[0].decode().unwrap();
    content
}

fn png(picture: &image::GrayImage) -> Vec<u8> {
    use image::ImageEncoder;
    let mut bytes = Vec::new();
    image::codecs::png::PngEncoder::new(&mut bytes)
        .write_image(picture.as_raw(), picture.width(), picture.height(), image::ExtendedColorType::L8)
        .unwrap();
    bytes
}

#[tokio::test]
async fn unknown_provider_is_an_error() {
    let omni = Omni::new(std::env::temp_dir(), "Folia-rust-test".to_owned()).unwrap();
    assert!(matches!(omni.search("nope", "x", 1, 0).await, Err(OmniError::UnknownProvider(_))));
}

#[test]
fn quality_levels_and_the_nearest_level() {
    let omni = Omni::new(std::env::temp_dir(), "Folia-rust-test".to_owned()).unwrap();
    assert_eq!(
        omni.audio_quality_levels("netease"),
        [AudioQuality::Standard, AudioQuality::High, AudioQuality::Lossless, AudioQuality::Hires]
    );
    assert_eq!(omni.audio_quality_levels("spotify"), [AudioQuality::Standard, AudioQuality::High]);
    assert_eq!(omni.audio_quality_levels("qishui"), [AudioQuality::Standard]);
    assert_eq!(omni.resolve_audio_quality("netease", AudioQuality::Hires), AudioQuality::Hires);
    assert_eq!(omni.resolve_audio_quality("spotify", AudioQuality::Hires), AudioQuality::High);
    assert_eq!(omni.resolve_audio_quality("spotify", AudioQuality::Ultra), AudioQuality::High);
    assert_eq!(omni.resolve_audio_quality("qishui", AudioQuality::High), AudioQuality::Standard);
    assert_eq!(omni.resolve_audio_quality("qishui", AudioQuality::Standard), AudioQuality::Standard);
    assert!(omni.audio_quality_levels("nope").is_empty());
}

#[tokio::test]
async fn fetch_cover_returns_the_cached_file() {
    let cache = tempfile::tempdir().unwrap();
    let omni = Omni::new(cache.path().to_owned(), "Folia-rust-test".to_owned()).unwrap();
    let url = "http://cdn.example/cover.jpg";
    let path = cache.path().join("covers").join(cover_cache_key(url));
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, b"jpeg").unwrap();
    let got = omni.fetch_cover(url, 300).await.unwrap();
    assert_eq!(got, path);
    assert_eq!(std::fs::read(got).unwrap(), b"jpeg");
    assert_eq!(cover_cache_key(url), cover_cache_key("https://cdn.example/cover.jpg"));
}

#[tokio::test]
#[ignore]
async fn netease_search_lyrics_and_audio() {
    let cache = tempfile::tempdir().unwrap();
    let omni = Omni::new(cache.path().to_owned(), "Folia-rust-test".to_owned()).unwrap();
    let page = omni.search("netease", "晴天 周杰伦", 5, 0).await.unwrap();
    assert!(!page.items.is_empty(), "search returned nothing");
    let first = &page.items[0];
    let online = first.online.clone().unwrap();
    eprintln!("first result: {} ({}) {}s total={:?}", first.display_name(), online.id, first.duration, page.total);
    assert!(first.duration > 60.0);
    assert!(first.id < 0);

    let lyrics = omni.lyrics(&online).await.unwrap().expect("song has lyrics");
    eprintln!(
        "lyric lines: {} word timed: {} first: {:?}",
        lyrics.lines.len(),
        lyrics.word_timed,
        lyrics.lines.iter().find(|l| !l.is_interlude()).map(|l| &l.text)
    );
    assert!(lyrics.lines.len() > 10);

    let free = omni.search("netease", "Kevin MacLeod", 10, 0).await.unwrap();
    let mut downloaded = None;
    for track in &free.items {
        if let Ok(path) = omni.fetch_audio(track.online.as_ref().unwrap(), AudioQuality::High).await {
            downloaded = Some(path);
            break;
        }
    }
    let path = downloaded.expect("no playable song among the free results");
    let size = std::fs::metadata(&path).unwrap().len();
    eprintln!("downloaded {} ({} bytes)", path.display(), size);
    assert!(size > 100_000);
    let again = omni
        .fetch_audio(
            free.items
                .iter()
                .find_map(|t| t.online.as_ref().filter(|o| path.to_string_lossy().contains(&o.id)))
                .unwrap(),
            AudioQuality::High,
        )
        .await
        .unwrap();
    assert_eq!(again, path, "second fetch reuses the cache");
}

#[tokio::test]
#[ignore]
async fn netease_instrumental_has_no_lyrics() {
    let omni = Omni::new(std::env::temp_dir(), "Folia-rust-test".to_owned()).unwrap();
    let song = folia_core::OnlineId { provider: "netease".into(), id: "548823721".into() };
    assert!(omni.lyrics(&song).await.unwrap().is_none());
}

#[tokio::test]
#[ignore]
async fn netease_gives_a_waiting_qr_code() {
    let omni = Omni::new(std::env::temp_dir(), "Folia-rust-test".to_owned()).unwrap();
    let qr = omni.login_qr("netease").await.unwrap();
    assert!(!qr.key.is_empty() && qr.width >= 21, "{qr:?}");
    let state = omni.check_login("netease", &qr.key).await.unwrap();
    assert_eq!(state, folia_omni::LoginState::Waiting);
}

/// The two platforms whose sign-in arrives as a picture, fetched live and read back; needs the network.
#[tokio::test]
#[ignore]
async fn qishui_and_qq_pictures_read_as_qr_codes() {
    let cache = tempfile::tempdir().unwrap();
    let omni = Omni::new(cache.path().to_owned(), "Folia-rust-test".to_owned()).unwrap();
    for id in ["qishui", "qq"] {
        let qr = omni.login_qr(id).await.unwrap();
        assert!((21..=177).contains(&qr.width) && (qr.width - 21) % 4 == 0, "{id}: {} modules", qr.width);
        eprintln!("[{id}] {} modules", qr.width);
        assert!(!decode(render(&qr)).is_empty(), "{id} decoded to nothing");
    }
}

#[tokio::test]
#[ignore]
async fn netease_explore_reads_as_a_guest() {
    let omni = Omni::new(std::env::temp_dir(), "Folia-rust-test".to_owned()).unwrap();
    let sections = omni.explore("netease").await.unwrap();
    assert!(!sections.is_empty());
    assert!(omni.account("netease").await.unwrap().is_none());
}

#[test]
fn no_kept_session_restores_nothing() {
    let omni = Omni::new(std::env::temp_dir(), "Folia-rust-test-empty".to_owned()).unwrap();
    assert!(!omni.restore_session("netease").unwrap());
    omni.forget_session("netease").unwrap();
}

// Search, download and lyrics on every platform without signing in; prints what each step returned.
#[tokio::test]
#[ignore]
async fn live_guest_smoke_on_every_platform() {
    let cache = tempfile::tempdir().unwrap();
    let omni = Omni::new(cache.path().to_owned(), "Folia-rust-test".to_owned()).unwrap();
    let limit = std::time::Duration::from_secs(40);
    for (id, _) in omni.providers() {
        let query = if matches!(id, "spotify" | "ytm") { "Ed Sheeran Shape of You" } else { "周杰伦 晴天" };
        let query = if id == "kugou" { "海阔天空 Beyond" } else { query };
        let found = tokio::time::timeout(limit, omni.search(id, query, 5, 0)).await;
        let track = match found {
            Ok(Ok(page)) if !page.items.is_empty() => page.items[0].clone(),
            Ok(Ok(_)) => {
                eprintln!("[{id}] search: empty");
                continue;
            }
            Ok(Err(err)) => {
                eprintln!("[{id}] search: {err}");
                continue;
            }
            Err(_) => {
                eprintln!("[{id}] search: timeout");
                continue;
            }
        };
        eprintln!("[{id}] search: {} ({:.0}s)", track.display_name(), track.duration);
        let song = track.online.clone().unwrap();
        match tokio::time::timeout(limit, omni.fetch_audio(&song, AudioQuality::High)).await {
            Ok(Ok(path)) => eprintln!("[{id}] audio: {} bytes", std::fs::metadata(&path).map_or(0, |m| m.len())),
            Ok(Err(err)) => eprintln!("[{id}] audio: {err}"),
            Err(_) => eprintln!("[{id}] audio: timeout"),
        }
        match tokio::time::timeout(limit, omni.lyrics(&song)).await {
            Ok(Ok(Some(lyrics))) => {
                eprintln!("[{id}] lyrics: {} lines, word_timed={}", lyrics.lines.len(), lyrics.word_timed)
            }
            Ok(Ok(None)) => eprintln!("[{id}] lyrics: none"),
            Ok(Err(err)) => eprintln!("[{id}] lyrics: {err}"),
            Err(_) => eprintln!("[{id}] lyrics: timeout"),
        }
    }
}

#[tokio::test]
#[ignore]
async fn qq_guest_search_is_repeatable() {
    let cache = tempfile::tempdir().unwrap();
    let omni = Omni::new(cache.path().to_owned(), "Folia-rust-test-guest".to_owned()).unwrap();
    let mut failures = Vec::new();
    for attempt in 0..18 {
        let query = ["周杰伦 晴天", "海阔天空 Beyond", "Taylor Swift"][attempt % 3];
        let result = tokio::time::timeout(
            std::time::Duration::from_secs(30),
            omni.search("qq", query, if attempt % 2 == 0 { 20 } else { 50 }, if attempt % 6 >= 3 { 20 } else { 0 }),
        )
        .await;
        match result {
            Ok(Ok(page)) if !page.items.is_empty() && page.items.len() <= 20 => {
                eprintln!("QQ guest {attempt}: {} results", page.items.len())
            }
            other => failures.push(format!("{attempt}: {other:?}")),
        }
        tokio::time::sleep(std::time::Duration::from_millis(400)).await;
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// NetEase `/song/chorus` and KuGou `song_climax` for one well-known song, then lyrics marked from them; needs the
/// network.
#[tokio::test]
#[ignore]
async fn netease_and_kugou_chorus_ranges() {
    let cache = tempfile::tempdir().unwrap();
    let omni = Omni::new(cache.path().to_owned(), "Folia-rust-test".to_owned()).unwrap();
    for provider in ["netease", "kugou"] {
        let page = omni.search(provider, "晴天 周杰伦", 5, 0).await.unwrap();
        let song = page.items[0].online.clone().unwrap();
        let ranges = omni.chorus_ranges(&song).await.unwrap();
        let again = omni.chorus_ranges(&song).await.unwrap();
        assert_eq!(ranges, again, "{provider}: the kept answer");
        let lyrics = omni.lyrics(&song).await.unwrap();
        let marked = lyrics.as_ref().map_or(0, |lyrics| lyrics.lines.iter().filter(|line| line.is_chorus).count());
        eprintln!(
            "[{provider}] {} {}: ranges {ranges:?}, chorus lines {marked}",
            page.items[0].display_name(),
            song.id
        );
        assert!(!ranges.is_empty(), "{provider}: no chorus ranges");
        assert!(marked > 0, "{provider}: no chorus lines");
    }
}
#[cfg(test)]
mod lx;
