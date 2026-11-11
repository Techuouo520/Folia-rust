// Protocol edge cases, native platform mappings and persisted source restoration.

use super::*;

#[tokio::test]
async fn download_link_import_persists_only_the_script_and_uses_file_deduplication() {
    let temp = tempfile::tempdir().unwrap();
    let store = Store::at(temp.path().join("config"));
    let omni = Omni::new(temp.path().join("cache"), "unused".into()).unwrap();
    omni.restore_sources(store.clone(), temp.path().join("data")).unwrap();
    let (url, server) = server(BASIC.as_bytes(), "200 OK");
    let settings = omni.import_source_url(&format!("  {url}?token=fixture-only  ")).await.unwrap();
    assert!(server.join().unwrap().starts_with("GET /test?token=fixture-only HTTP/1.1"));
    assert_eq!(settings.scripts.len(), 1);
    assert_eq!(settings.scripts[0].metadata.name, "Folia fixture");
    assert!(settings.enabled.is_empty());
    let id = &settings.scripts[0].id;
    assert_eq!(std::fs::read_to_string(temp.path().join("data/sources").join(format!("{id}.js"))).unwrap(), BASIC);
    let saved = std::fs::read_to_string(store.dir().join("sources.toml")).unwrap();
    assert!(!saved.contains("fixture-only"));
    assert!(!saved.contains("127.0.0.1"));
    let path = temp.path().join("source.js");
    std::fs::write(&path, BASIC).unwrap();
    assert!(matches!(omni.import_source(&path), Err(SourceError::Duplicate)));
}

#[tokio::test]
async fn bad_download_links_and_responses_do_not_create_sources() {
    let temp = tempfile::tempdir().unwrap();
    let omni = Omni::new(temp.path().join("cache"), "unused".into()).unwrap();
    omni.restore_sources(Store::at(temp.path().join("config")), temp.path().join("data")).unwrap();
    for url in ["", "file:///tmp/source.js", "not a link", "https://example.invalid/\nscript.js"] {
        assert!(matches!(omni.import_source_url(url).await, Err(SourceError::Url)));
    }
    for (body, status) in [
        (b"<html>not a script</html>".as_slice(), "200 OK"),
        (b"\xff".as_slice(), "200 OK"),
        (b"missing".as_slice(), "404 Not Found"),
    ] {
        let (url, server) = server(body, status);
        assert!(omni.import_source_url(&url).await.is_err());
        server.join().unwrap();
    }
    assert!(omni.custom_sources().scripts.is_empty());
    assert!(!temp.path().join("data/sources").exists());
}

#[tokio::test]
async fn link_import_rejects_an_oversized_response_before_reading_it() {
    let temp = tempfile::tempdir().unwrap();
    let omni = Omni::new(temp.path().join("cache"), "unused".into()).unwrap();
    omni.restore_sources(Store::at(temp.path().join("config")), temp.path().join("data")).unwrap();
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}/script.js", listener.local_addr().unwrap());
    let server = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let mut bytes = [0_u8; 4096];
        stream.read(&mut bytes).unwrap();
        stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2097153\r\nConnection: close\r\n\r\n").unwrap();
    });
    assert!(matches!(omni.import_source_url(&url).await, Err(SourceError::Limit)));
    server.join().unwrap();
    assert!(omni.custom_sources().scripts.is_empty());
}

#[tokio::test]
async fn default_twenty_second_source_timeout_falls_back_once() {
    let temp = tempfile::tempdir().unwrap();
    let calls = Arc::new(AtomicUsize::new(0));
    let omni = Omni::with_providers(
        temp.path().join("cache"),
        "unused".into(),
        vec![Box::new(Native { calls: calls.clone(), url: "https://example.invalid/native".into() })],
    )
    .unwrap();
    omni.restore_sources(Store::at(temp.path().join("config")), temp.path().join("data")).unwrap();
    let path = temp.path().join("pending.js");
    std::fs::write(&path, format!("{BASIC}\nlx.on('request',()=>new Promise(()=>{{}}));")).unwrap();
    let id = omni.import_source(&path).unwrap().scripts[0].id.clone();
    omni.enable_source(&id, true).unwrap();
    let began = Instant::now();
    let track = Track { online: Some(OnlineId { provider: "fixture".into(), id: "1".into() }), ..Default::default() };
    let source = omni.resolve_audio_source(&track, AudioQuality::High).await.unwrap();
    assert_eq!(source.url, "https://example.invalid/native");
    assert_eq!(calls.load(Ordering::Relaxed), 1);
    assert!(began.elapsed() >= Duration::from_secs(19));
    assert!(began.elapsed() < Duration::from_secs(30));
}

#[tokio::test]
async fn idle_source_survives_an_expired_operation_deadline() {
    let limits = Limits {
        init_timeout: Duration::from_millis(50),
        request_timeout: Duration::from_millis(50),
        ..Limits::default()
    };
    let script = start(BASIC, limits).unwrap();
    tokio::time::sleep(Duration::from_millis(250)).await;
    assert!(script.music_url("wy", "128k", json!({})).await.is_ok());
    tokio::time::sleep(Duration::from_millis(250)).await;
    assert!(script.music_url("wy", "128k", json!({})).await.is_ok());
}

#[tokio::test]
async fn custom_addresses_use_the_cache_and_download_failures_use_native_audio() {
    let temp = tempfile::tempdir().unwrap();
    let calls = Arc::new(AtomicUsize::new(0));
    let (native_url, native_server) = server(b"native", "200 OK");
    let omni = Omni::with_providers(
        temp.path().join("cache"),
        "unused".into(),
        vec![Box::new(Native { calls: calls.clone(), url: native_url })],
    )
    .unwrap();
    omni.restore_sources(Store::at(temp.path().join("config")), temp.path().join("data")).unwrap();
    let (url, custom_server) = server(b"custom", "200 OK");
    let code = format!(
        "{BASIC}\nlx.on('request',async({{info}})=> info.musicInfo.name==='bad'?'http://127.0.0.1:1/':{});",
        json!(url)
    );
    let file = temp.path().join("fixture.js");
    std::fs::write(&file, code).unwrap();
    let id = omni.import_source(&file).unwrap().scripts[0].id.clone();
    omni.enable_source(&id, true).unwrap();
    let mut track =
        Track { online: Some(OnlineId { provider: "fixture".into(), id: "good".into() }), ..Default::default() };
    let cached = omni.fetch_track_audio(&track, AudioQuality::High).await.unwrap();
    assert_eq!(std::fs::read(&cached).unwrap(), b"custom");
    custom_server.join().unwrap();
    assert_eq!(cached, omni.fetch_track_audio(&track, AudioQuality::High).await.unwrap());
    assert_eq!(calls.load(Ordering::Relaxed), 0);
    track.online.as_mut().unwrap().id = "bad".into();
    track.title = "bad".into();
    assert_eq!(std::fs::read(omni.fetch_track_audio(&track, AudioQuality::High).await.unwrap()).unwrap(), b"native");
    native_server.join().unwrap();
    assert_eq!(calls.load(Ordering::Relaxed), 1);
}

#[tokio::test]
async fn rsa_matches_node_public_encrypt_without_padding() {
    let vector: serde_json::Value = serde_json::from_str(include_str!("fixtures/lx-rsa.json")).unwrap();
    let code = format!(
        "{BASIC}\nlx.on('request',async()=>{{ const b=lx.utils.buffer; const result=b.bufToString(lx.utils.crypto.rsaEncrypt(b.from('abc'),{}),'hex'); if(result!=={}) throw new Error('RSA'); return 'https://example.invalid/rsa'; }});",
        vector["pem"], vector["expected"]
    );
    let script = start(&code, Limits::default()).unwrap();
    assert_eq!(script.music_url("wy", "128k", json!({})).await.unwrap(), "https://example.invalid/rsa");
}

#[tokio::test]
async fn update_alert_is_once_bounded_and_pumped_while_idle() {
    let code = format!(
        "{BASIC}\nsetTimeout(async()=>{{await lx.send('updateAlert',{{log:'x'.repeat(1200),updateUrl:'javascript:bad'}}); try {{ await lx.send('updateAlert',{{log:'second'}}); }} catch (_) {{}} }},20);"
    );
    let script = start(&code, Limits::default()).unwrap();
    tokio::time::sleep(Duration::from_millis(150)).await;
    let alerts = script.take_update_alerts();
    assert_eq!(alerts.len(), 1);
    assert_eq!(alerts[0].log.len(), 1027);
    assert!(alerts[0].update_url.is_none());
    assert!(script.take_update_alerts().is_empty());
}

#[tokio::test]
async fn http_timeout_and_cancel_use_error_null_null_callback() {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}/", listener.local_addr().unwrap());
    let server = std::thread::spawn(move || {
        let (_stream, _) = listener.accept().unwrap();
        std::thread::sleep(Duration::from_millis(200));
    });
    let code = format!(
        "{BASIC}\nlx.on('request',()=>new Promise((resolve,reject)=>{{lx.request({},{{timeout:20}},(err,resp,body)=>{{if(!err||resp!==null||body!==null)return reject(new Error('signature')); resolve('https://example.invalid/timeout');}});}}));",
        json!(url)
    );
    let script = start(&code, Limits::default()).unwrap();
    assert_eq!(script.music_url("wy", "128k", json!({})).await.unwrap(), "https://example.invalid/timeout");
    server.join().unwrap();
    let code = format!(
        "{BASIC}\nlx.on('request',()=>new Promise((resolve,reject)=>{{const cancel=lx.request('http://127.0.0.1:1/',{{}},()=>reject(new Error('cancel callback')));cancel();cancel();setTimeout(()=>resolve('https://example.invalid/cancel'),30);}}));"
    );
    assert_eq!(
        start(&code, Limits::default()).unwrap().music_url("wy", "128k", json!({})).await.unwrap(),
        "https://example.invalid/cancel"
    );
}

#[tokio::test]
async fn platform_ids_and_quality_hashes_survive_the_facade() {
    let temp = tempfile::tempdir().unwrap();
    let omni = Omni::new(temp.path().join("cache"), "unused".into()).unwrap();
    omni.restore_sources(Store::at(temp.path().join("config")), temp.path().join("data")).unwrap();
    let path = temp.path().join("mapping.js");
    std::fs::write(&path, include_str!("fixtures/lx-mapping.js")).unwrap();
    let id = omni.import_source(&path).unwrap().scripts[0].id.clone();
    omni.enable_source(&id, true).unwrap();
    for (provider, id, expected) in [
        ("netease", "123", "wy/123///album///320k"),
        ("qq", "mid-42-media", "tx/mid/42/media/album///320k"),
        ("kugou", "abc-9-10", "kg/77///9/ABC/HQ/320k"),
    ] {
        let track = Track {
            online: Some(OnlineId { provider: provider.into(), id: id.into() }),
            online_info: Some(folia_core::OnlineTrackInfo {
                album_id: "album".into(),
                audio_id: "77".into(),
                quality_hashes: [(AudioQuality::High, "HQ".into())].into_iter().collect(),
            }),
            ..Default::default()
        };
        let source = omni.resolve_audio_source(&track, AudioQuality::Ultra).await.unwrap();
        assert_eq!(source.url, format!("https://example.invalid/{expected}"));
        assert_eq!(source.level, AudioQuality::High);
    }
}

#[tokio::test]
async fn restart_restores_source_and_selected_quality_degrades() {
    let temp = tempfile::tempdir().unwrap();
    let store = Store::at(temp.path().join("config"));
    let data = temp.path().join("data");
    let calls = Arc::new(AtomicUsize::new(0));
    let make = || {
        Omni::with_providers(
            temp.path().join("cache"),
            "unused".into(),
            vec![Box::new(Native { calls: calls.clone(), url: "https://example.invalid/native".into() })],
        )
        .unwrap()
    };
    let omni = make();
    omni.restore_sources(store.clone(), data.clone()).unwrap();
    let path = temp.path().join("source.js");
    let code = format!(
        "{BASIC}\nlx.on('request',async({{info}})=>{{if(info.type==='flac')throw new Error('quality unavailable');return 'https://example.invalid/'+info.type;}});"
    );
    std::fs::write(&path, code).unwrap();
    let id = omni.import_source(&path).unwrap().scripts[0].id.clone();
    omni.enable_source(&id, true).unwrap();
    drop(omni);
    let omni = make();
    assert_eq!(omni.restore_sources(store, data).unwrap().enabled, vec![id.clone()]);
    let track = Track { online: Some(OnlineId { provider: "fixture".into(), id: "1".into() }), ..Default::default() };
    let source = omni.resolve_audio_source(&track, AudioQuality::Hires).await.unwrap();
    assert_eq!(source.url, "https://example.invalid/320k");
    assert_eq!(source.level, AudioQuality::High);
    assert_eq!(calls.load(Ordering::Relaxed), 0);
    omni.enable_source(&id, false).unwrap();
    assert_eq!(
        omni.resolve_audio_source(&track, AudioQuality::High).await.unwrap().url,
        "https://example.invalid/native"
    );
    assert_eq!(calls.load(Ordering::Relaxed), 1);
}
