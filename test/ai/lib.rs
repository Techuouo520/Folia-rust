// Offline contracts for AI protocols, source-aware theme caching and generated palette contrast.
#![cfg(test)]

use folia_core::{OnlineId, Rgb, Track};
use folia_store::{AiProtocol, AiSettings};
use serde_json::json;
use std::io::{Read, Write};

#[test]
fn theme_requests_use_all_three_protocols_and_reject_truncated_results() {
    let runtime = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
    let theme = json!({"light":{"name":"light","backgroundColor":"#fff","primaryColor":"#111"},"dark":{"name":"dark","accentColor":"#abc"}});
    for protocol in [AiProtocol::Openai, AiProtocol::Gemini, AiProtocol::Anthropic] {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let endpoint = format!("http://{}/v1", listener.local_addr().unwrap());
        let answer = theme.to_string();
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            stream.set_read_timeout(Some(std::time::Duration::from_secs(10))).unwrap();
            let mut request = Vec::new();
            let mut buf = [0; 4096];
            let (header_end, length) = loop {
                let read = stream.read(&mut buf).unwrap();
                assert_ne!(read, 0);
                request.extend_from_slice(&buf[..read]);
                if let Some(index) = request.windows(4).position(|part| part == b"\r\n\r\n") {
                    let header = String::from_utf8_lossy(&request[..index]).to_ascii_lowercase();
                    let length = header
                        .lines()
                        .find_map(|line| line.strip_prefix("content-length: "))
                        .unwrap()
                        .parse::<usize>()
                        .unwrap();
                    break (index + 4, length);
                }
            };
            while request.len() < header_end + length {
                let read = stream.read(&mut buf).unwrap();
                assert_ne!(read, 0);
                request.extend_from_slice(&buf[..read]);
            }
            let header = String::from_utf8_lossy(&request[..header_end]).to_ascii_lowercase();
            let body: serde_json::Value = serde_json::from_slice(&request[header_end..header_end + length]).unwrap();
            let response = match protocol {
                AiProtocol::Openai => {
                    assert!(header.starts_with("post /v1/chat/completions "));
                    assert!(header.contains("authorization: bearer fixture-secret"));
                    assert_eq!(body["response_format"]["type"], "json_object");
                    assert!(body["messages"][1]["content"].as_str().unwrap().contains("test lyrics"));
                    json!({"choices":[{"finish_reason":"stop","message":{"content":answer}}]})
                }
                AiProtocol::Gemini => {
                    assert!(header.starts_with("post /v1/models/fixture:generatecontent "));
                    assert!(header.contains("x-goog-api-key: fixture-secret"));
                    assert_eq!(body["generationConfig"]["responseMimeType"], "application/json");
                    json!({"candidates":[{"finishReason":"STOP","content":{"parts":[{"text":"hidden reasoning","thought":true},{"text":answer}]}}]})
                }
                AiProtocol::Anthropic => {
                    assert!(header.starts_with("post /v1/messages "));
                    assert!(header.contains("x-api-key: fixture-secret"));
                    assert_eq!(body["model"], "fixture");
                    json!({"stop_reason":"end_turn","content":[{"type":"text","text":answer}]})
                }
            }.to_string();
            write!(stream, "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}", response.len(), response).unwrap();
        });
        let client = folia_ai::Client::with_key(
            AiSettings { protocol, endpoint, model: "fixture".into(), ..Default::default() },
            "fixture-secret".into(),
        )
        .unwrap();
        assert!(!format!("{client:?}").contains("fixture-secret"));
        let result = runtime.block_on(client.generate_theme("title", "test lyrics")).unwrap();
        assert_eq!(result.light.name, "light");
        assert_eq!(result.dark.accent, Rgb::hex(0xaabbcc));
        server.join().unwrap();
    }
    assert!(folia_ai::parse_theme("{\"light\":{}}").is_err());
    let settings = AiSettings { endpoint: "https://api.example.com/v1".into(), ..Default::default() };
    let mut changed = settings.clone();
    changed.endpoint.push('/');
    assert!(folia_ai::same_endpoint(&settings, &changed));
    changed.endpoint = "https://other.example.com/v1".into();
    assert!(!folia_ai::same_endpoint(&settings, &changed));
    changed.endpoint = settings.endpoint.clone();
    changed.protocol = AiProtocol::Anthropic;
    assert!(!folia_ai::same_endpoint(&settings, &changed));
    assert!(folia_ai::Client::with_key(settings, "secret\r\nx-injected: value".into()).is_err());
    assert!(folia_ai::parse_theme("{\"light\":{},\"dark\":").is_err());
    assert!(
        folia_ai::Client::with_key(
            AiSettings { endpoint: "https://secret@example.com/api".into(), ..Default::default() },
            "key".into()
        )
        .is_err()
    );
}

#[test]
fn cache_keeps_song_platforms_separate_and_persists_both_theme_sides() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("themes.db");
    let song = |provider: &str| Track {
        id: -1,
        online: Some(OnlineId { provider: provider.into(), id: "same-id".into() }),
        ..Default::default()
    };
    let qq = folia_ai::song_key(&song("qq"));
    let netease = folia_ai::song_key(&song("netease"));
    assert_ne!(qq, netease);
    let theme = folia_ai::cover_theme(&[Rgb::hex(0xf25580)], 7);
    let cache = folia_ai::ThemeCache::open(&path).unwrap();
    cache.put(&qq, &theme).unwrap();
    let earlier = folia_ai::ThemeCache::reserve_revision();
    let later = folia_ai::ThemeCache::reserve_revision();
    cache.put_revision(&qq, &theme, later).unwrap();
    cache.put_revision(&qq, &folia_store::DualTheme::default(), earlier).unwrap();
    assert_eq!(cache.get(&qq).unwrap(), Some(theme.clone()));
    cache.remember(&theme, later).unwrap();
    assert_eq!(cache.get(&netease).unwrap(), None);
    drop(cache);
    assert_eq!(folia_ai::ThemeCache::open(&path).unwrap().get(&qq).unwrap(), Some(theme));
    assert!(folia_ai::ThemeCache::open(&path).unwrap().last().unwrap().is_some());
}

#[test]
fn cover_themes_keep_the_original_contrast_floors() {
    let fixtures: serde_json::Value = serde_json::from_str(include_str!("fixtures/palettes.json")).unwrap();
    for case in fixtures.as_array().unwrap() {
        let colors = case["colors"]
            .as_array()
            .unwrap()
            .iter()
            .map(|color| folia_core::parse_color(color.as_str().unwrap()).unwrap())
            .collect::<Vec<_>>();
        let expected = folia_ai::parse_theme(&case["theme"].to_string()).unwrap();
        assert_eq!(folia_ai::cover_theme(&colors, case["seed"].as_u64().unwrap()), expected, "original palette {case}");
    }
    let luminance = |color: Rgb| {
        let [r, g, b] = [color.r, color.g, color.b].map(|value| {
            let value = f64::from(value) / 255.0;
            if value <= 0.04045 { value / 12.92 } else { ((value + 0.055) / 1.055).powf(2.4) }
        });
        0.2126 * r + 0.7152 * g + 0.0722 * b
    };
    let contrast = |a: Rgb, b: Rgb| {
        let a = luminance(a);
        let b = luminance(b);
        (a.max(b) + 0.05) / (a.min(b) + 0.05)
    };
    for palette in [
        vec![],
        vec![Rgb::hex(0xffffff)],
        vec![Rgb::hex(0x02030a)],
        vec![Rgb::hex(0xffff00), Rgb::hex(0xe14a85)],
        vec![Rgb::hex(0x046bea), Rgb::hex(0x9832a8)],
    ] {
        for seed in 0..64 {
            let theme = folia_ai::cover_theme(&palette, seed);
            for side in [&theme.light, &theme.dark] {
                assert!(contrast(side.primary, side.background) >= 9.0);
                assert!(contrast(side.secondary, side.background) >= 4.5);
                assert!(contrast(side.accent, side.background) >= 3.2);
            }
            assert!(luminance(theme.light.background) > luminance(theme.dark.background));
            assert_eq!(theme, folia_ai::cover_theme(&palette, seed));
        }
    }
}
