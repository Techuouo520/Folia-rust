// Synthetic LX protocol tests; all HTTP traffic stays on a local std TCP listener.

use std::io::{Read, Write};
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};
use std::time::{Duration, Instant};

use folia_core::{AudioQuality, OnlineId, Track};
use folia_omni::lx::{Limits, MusicInfo, Script, SourceError, parse_metadata};
use folia_omni::{AudioSource, BoxFuture, Omni, OmniError, Page, Provider, ProviderLyrics};
use folia_store::{SourceSettings, Store};
use serde_json::json;

const BASIC: &str = include_str!("fixtures/lx-basic.js");
const HTTP: &str = include_str!("fixtures/lx-http.js");
const UTILS: &str = include_str!("fixtures/lx-utils.js");
#[path = "lx_edges.rs"]
mod edges;
#[path = "lx_multiple.rs"]
mod multiple;
#[path = "prefetch.rs"]
mod prefetch;
#[path = "ranges.rs"]
mod ranges;

fn start(code: &str, limits: Limits) -> Result<Script, SourceError> {
    Script::start(code.into(), parse_metadata(code)?, Default::default(), limits)
}

#[test]
fn header_matches_lx_and_is_bounded() {
    let info = parse_metadata(BASIC).unwrap();
    assert_eq!((&*info.name, &*info.version, &*info.author), ("Folia fixture", "1.2.3", "Folia tests"));
    assert!(matches!(parse_metadata(" // no block"), Err(SourceError::Header)));
    assert!(parse_metadata(&format!("\u{feff}{BASIC}")).is_err());
    let code = format!("/*\n * @name {}\n * @version old\n * @version new\n */", "中".repeat(30));
    let info = parse_metadata(&code).unwrap();
    assert_eq!(info.name, format!("{}...", "中".repeat(24)));
    assert_eq!(info.version, "new");
    assert!(parse_metadata("/* anonymous */").unwrap().name.starts_with("user_api_"));
}

#[tokio::test]
async fn initialization_filters_capabilities_and_dispatches_promises() {
    let script = start(BASIC, Limits::default()).unwrap();
    assert_eq!(script.sources().len(), 2);
    assert_eq!(script.sources()["wy"].actions, ["musicUrl"]);
    assert_eq!(script.sources()["wy"].qualitys, ["128k", "320k", "flac"]);
    assert_eq!(script.sources()["local"].actions, ["musicUrl", "lyric", "pic"]);
    assert_eq!(script.music_url("wy", "128k", json!({})).await.unwrap(), "https://example.invalid/audio.mp3");
    assert!(matches!(script.music_url("wy", "128k", json!({"name":"reject"})).await, Err(SourceError::Script)));
    assert!(script.music_url("wy", "128k", json!({"url":"file:///secret"})).await.is_err());
}

#[tokio::test]
async fn utility_vectors_and_sandbox_globals() {
    let script = start(UTILS, Limits::default()).unwrap();
    assert_eq!(script.music_url("wy", "128k", json!({})).await.unwrap(), "https://example.invalid/utils-ok");
}

#[test]
fn invalid_initialization_and_throw_are_rejected() {
    for code in [
        "/* test */ throw new Error('bad')",
        "/* test */ lx.send('inited', {})",
        "/* test */ lx.send('inited', {sources:{wy:{type:'music'}}})",
    ] {
        assert!(start(code, Limits::default()).is_err());
    }
}

#[tokio::test]
async fn initialization_request_and_heap_limits_are_enforced() {
    let limits = Limits {
        memory_bytes: 4 * 1024 * 1024,
        init_timeout: Duration::from_millis(100),
        request_timeout: Duration::from_millis(100),
    };
    let began = Instant::now();
    assert!(matches!(start("/* loop */ while (true) {}", limits), Err(SourceError::Timeout)));
    assert!(matches!(start("/* never inited */", limits), Err(SourceError::Timeout)));
    for name in ["loop", "pending"] {
        let script = start(BASIC, limits).unwrap();
        assert!(matches!(script.music_url("wy", "128k", json!({"name":name})).await, Err(SourceError::Timeout)));
    }
    let script = start(BASIC, limits).unwrap();
    let error = script.music_url("wy", "128k", json!({"name":"memory"})).await.unwrap_err();
    assert!(matches!(error, SourceError::Memory), "{error:?}");
    assert!(began.elapsed() < Duration::from_secs(5));
}

fn server(body: &'static [u8], status: &str) -> (String, std::thread::JoinHandle<String>) {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}/test", listener.local_addr().unwrap());
    let status = status.to_owned();
    let task = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        stream.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
        let mut request = Vec::new();
        let mut block = [0_u8; 4096];
        loop {
            let n = stream.read(&mut block).unwrap();
            if n == 0 {
                break;
            }
            request.extend_from_slice(&block[..n]);
            if let Some(end) = request.windows(4).position(|b| b == b"\r\n\r\n") {
                let header = String::from_utf8_lossy(&request[..end]).to_ascii_lowercase();
                let length = header
                    .lines()
                    .find_map(|line| line.strip_prefix("content-length:").map(|s| s.trim().parse::<usize>().unwrap()))
                    .unwrap_or(0);
                if request.len() >= end + 4 + length {
                    break;
                }
            }
        }
        write!(stream, "HTTP/1.1 {status}\r\nContent-Length: {}\r\nContent-Type: application/json\r\nX-Fixture: yes\r\nConnection: close\r\n\r\n", body.len()).unwrap();
        stream.write_all(body).unwrap();
        String::from_utf8(request).unwrap()
    });
    (url, task)
}

#[tokio::test]
async fn http_method_headers_body_form_and_form_data_round_trip() {
    let script = start(HTTP, Limits::default()).unwrap();
    for (options, expected) in [
        (json!({"method":"POST","headers":{"X-Test":"value"},"body":"hello"}), "hello"),
        (json!({"method":"POST","form":{"a":"hello world"}}), "a=hello%20world"),
        (json!({"method":"POST","formData":{"a":"hello"}}), "a=hello"),
        (json!({"method":"PUT","headers":{"Content-Type":"application/json"},"body":{"a":1}}), "{\"a\":1}"),
        (json!({"method":"POST","body":{"a":1}}), "a=1"),
        (json!({"method":"POST","fixtureOrder":true}), "z=1&a=2"),
        (
            json!({"method":"POST","fixtureOrder":true,"headers":{"Content-Type":"application/json"}}),
            "{\"z\":1,\"a\":2}",
        ),
        (
            json!({"method":"GET","form":{"nested":{"list":[1,2]},"q":"a b"}}),
            "GET /test?nested[list][]=1&nested[list][]=2&q=a%20b HTTP/1.1",
        ),
    ] {
        let (url, server) = server(br#"{"ok":true}"#, "201 Created");
        assert_eq!(
            script.music_url("wy", "128k", json!({"url":url,"options":options})).await.unwrap(),
            "https://example.invalid/http-ok"
        );
        let request = server.join().unwrap();
        assert!(request.contains(expected), "{request}");
        assert!(request.starts_with(options["method"].as_str().unwrap()));
        if options["headers"]["X-Test"].is_string() {
            assert!(request.to_lowercase().contains("x-test: value"));
        }
    }
}

struct Native {
    calls: Arc<AtomicUsize>,
    url: String,
}

impl Provider for Native {
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
    fn user_playlists<'a>(&'a self, _: &'a str) -> BoxFuture<'a, Result<Vec<folia_omni::Collection>, OmniError>> {
        unsupported()
    }
    fn collection_detail<'a>(
        &'a self,
        _: &'a folia_omni::Collection,
    ) -> BoxFuture<'a, Result<folia_omni::CollectionDetail, OmniError>> {
        unsupported()
    }
    fn set_subscribed<'a>(&'a self, _: &'a folia_omni::Collection, _: bool) -> BoxFuture<'a, Result<(), OmniError>> {
        unsupported()
    }
    fn favorite_albums(&self) -> BoxFuture<'_, Result<Vec<folia_omni::Collection>, OmniError>> {
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
    fn id(&self) -> &'static str {
        "fixture"
    }
    fn display_name(&self) -> &'static str {
        "fixture"
    }
    fn lx_music_info(&self, track: &Track) -> Option<MusicInfo> {
        Some(MusicInfo { source: "wy", data: json!({"name":track.title}) })
    }
    fn search_songs<'a>(&'a self, _: &'a str, _: u32, _: u32) -> BoxFuture<'a, Result<Page<Track>, OmniError>> {
        Box::pin(async { Ok(Page::empty(0)) })
    }
    fn audio_source<'a>(
        &'a self,
        _: &'a OnlineId,
        quality: AudioQuality,
    ) -> BoxFuture<'a, Result<AudioSource, OmniError>> {
        self.calls.fetch_add(1, Ordering::Relaxed);
        Box::pin(async move {
            Ok(AudioSource { url: self.url.clone(), extension: "mp3".into(), preview: false, level: quality })
        })
    }
    fn lyrics<'a>(&'a self, _: &'a OnlineId) -> BoxFuture<'a, Result<Option<ProviderLyrics>, OmniError>> {
        Box::pin(async { Ok(None) })
    }
}

fn unsupported<T>() -> BoxFuture<'static, Result<T, OmniError>> {
    Box::pin(async { Err(OmniError::Unsupported("fixture")) })
}

#[tokio::test]
async fn imports_persist_selection_and_failed_scripts_fall_back_to_native_cache() {
    let temp = tempfile::tempdir().unwrap();
    let store = Store::at(temp.path().join("config"));
    let calls = Arc::new(AtomicUsize::new(0));
    let (url, server) = server(b"native audio", "200 OK");
    let omni = Omni::with_providers(
        temp.path().join("cache"),
        "test-unused".into(),
        vec![Box::new(Native { calls: calls.clone(), url })],
    )
    .unwrap();
    omni.restore_sources(store.clone(), temp.path().join("data")).unwrap();
    let path = temp.path().join("fixture.js");
    std::fs::write(&path, BASIC).unwrap();
    let settings = omni.import_source(&path).unwrap();
    let id = &settings.scripts[0].id;
    assert!(settings.enabled.is_empty());
    assert!(matches!(omni.import_source(&path), Err(SourceError::Duplicate)));
    omni.enable_source(id, true).unwrap();
    assert_eq!(store.load::<SourceSettings>().unwrap().enabled, vec![id.clone()]);
    let track = Track {
        online: Some(OnlineId { provider: "fixture".into(), id: "1".into() }),
        title: "reject".into(),
        ..Default::default()
    };
    let path = omni.fetch_track_audio(&track, AudioQuality::High).await.unwrap();
    assert_eq!(std::fs::read(path).unwrap(), b"native audio");
    server.join().unwrap();
    assert_eq!(calls.load(Ordering::Relaxed), 1);
    omni.enable_source(&id, false).unwrap();
    omni.fetch_track_audio(&track, AudioQuality::High).await.unwrap();
    assert_eq!(calls.load(Ordering::Relaxed), 1);
    omni.remove_source(id).unwrap();
    assert!(store.load::<SourceSettings>().unwrap().scripts.is_empty());
    assert!(!temp.path().join("data/sources").join(format!("{id}.js")).exists());
}
