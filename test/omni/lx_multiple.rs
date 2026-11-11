// Independent activation, ordered fallback and migration of the single-source catalog.

use super::*;

struct Catalog {
    temp: tempfile::TempDir,
    store: Store,
    omni: Omni,
    calls: Arc<AtomicUsize>,
}

impl Catalog {
    fn new(native_url: &str) -> Self {
        let temp = tempfile::tempdir().unwrap();
        let store = Store::at(temp.path().join("config"));
        let calls = Arc::new(AtomicUsize::new(0));
        let omni = Omni::with_providers(
            temp.path().join("cache"),
            "unused".into(),
            vec![Box::new(Native { calls: calls.clone(), url: native_url.into() })],
        )
        .unwrap();
        omni.restore_sources(store.clone(), temp.path().join("data")).unwrap();
        Self { temp, store, omni, calls }
    }

    fn add(&self, name: &str, handler: &str, enabled: bool) -> String {
        let code =
            format!("{}\nlx.on('request', {handler});", BASIC.replace("@name Folia fixture", &format!("@name {name}")));
        self.add_code(name, &code, enabled)
    }

    fn add_code(&self, name: &str, code: &str, enabled: bool) -> String {
        let path = self.temp.path().join(format!("{name}.js"));
        std::fs::write(&path, code).unwrap();
        let settings = self.omni.import_source(&path).unwrap();
        let id = settings.scripts.last().unwrap().id.clone();
        if enabled {
            self.omni.enable_source(&id, true).unwrap();
        }
        id
    }
}

fn track() -> Track {
    Track { online: Some(OnlineId { provider: "fixture".into(), id: "track".into() }), ..Default::default() }
}

#[tokio::test]
async fn activation_order_priority_disable_delete_and_restore_are_independent() {
    let catalog = Catalog::new("https://example.invalid/native");
    let first = catalog.add("first", "async () => 'https://example.invalid/first'", false);
    let second = catalog.add("second", "async () => 'https://example.invalid/second'", true);
    catalog.omni.enable_source(&first, true).unwrap();
    assert_eq!(catalog.omni.custom_sources().enabled.len(), 2);
    assert_eq!(
        catalog.omni.resolve_audio_source(&track(), AudioQuality::High).await.unwrap().url,
        "https://example.invalid/first"
    );
    catalog.omni.move_source(&second, 0).unwrap();
    assert_eq!(
        catalog.omni.resolve_audio_source(&track(), AudioQuality::High).await.unwrap().url,
        "https://example.invalid/second"
    );
    catalog.omni.restore_sources(catalog.store.clone(), catalog.temp.path().join("data")).unwrap();
    assert_eq!(
        catalog.omni.resolve_audio_source(&track(), AudioQuality::High).await.unwrap().url,
        "https://example.invalid/second"
    );
    catalog.omni.enable_source(&second, false).unwrap();
    assert_eq!(catalog.omni.custom_sources().enabled, vec![first.clone()]);
    assert_eq!(
        catalog.omni.resolve_audio_source(&track(), AudioQuality::High).await.unwrap().url,
        "https://example.invalid/first"
    );
    catalog.omni.enable_source(&second, true).unwrap();
    catalog.omni.enable_source(&second, true).unwrap();
    assert_eq!(catalog.omni.custom_sources().enabled.len(), 2);
    catalog.omni.remove_source(&second).unwrap();
    assert_eq!(catalog.store.load::<SourceSettings>().unwrap().enabled, vec![first]);
    assert_eq!(
        catalog.omni.resolve_audio_source(&track(), AudioQuality::High).await.unwrap().url,
        "https://example.invalid/first"
    );
    assert_eq!(catalog.calls.load(Ordering::Relaxed), 0);
}

#[tokio::test]
async fn failed_and_unsupported_sources_are_skipped_before_native_fallback() {
    let catalog = Catalog::new("https://example.invalid/native");
    catalog.add("failed", "async () => { throw new Error('fixture'); }", true);
    let unsupported = BASIC.replace("@name Folia fixture", "@name other-platform").replace("wy:", "tx:");
    catalog.add_code("other-platform", &unsupported, true);
    let last = catalog.add("last", "async () => 'https://example.invalid/last'", true);
    assert_eq!(
        catalog.omni.resolve_audio_source(&track(), AudioQuality::High).await.unwrap().url,
        "https://example.invalid/last"
    );
    assert_eq!(catalog.calls.load(Ordering::Relaxed), 0);
    catalog.omni.enable_source(&last, false).unwrap();
    assert_eq!(
        catalog.omni.resolve_audio_source(&track(), AudioQuality::High).await.unwrap().url,
        "https://example.invalid/native"
    );
    assert_eq!(catalog.calls.load(Ordering::Relaxed), 1);
}

#[tokio::test]
async fn a_failed_download_advances_to_the_next_source_and_its_own_cache() {
    let (native_url, native_server) = server(b"native", "200 OK");
    let catalog = Catalog::new(&native_url);
    let first = catalog.add("bad-download", "async () => 'http://127.0.0.1:1/'", true);
    let (url, source_server) = server(b"second source", "200 OK");
    let second = catalog.add("good-download", &format!("async () => {}", json!(url)), true);
    let cached = catalog.omni.fetch_track_audio(&track(), AudioQuality::High).await.unwrap();
    assert_eq!(std::fs::read(&cached).unwrap(), b"second source");
    assert!(cached.file_name().unwrap().to_str().unwrap().contains(&second));
    assert!(!cached.file_name().unwrap().to_str().unwrap().contains(&first));
    source_server.join().unwrap();
    assert_eq!(catalog.omni.fetch_track_audio(&track(), AudioQuality::High).await.unwrap(), cached);
    assert_eq!(catalog.calls.load(Ordering::Relaxed), 0);
    catalog.omni.enable_source(&second, false).unwrap();
    let path = catalog.omni.fetch_track_audio(&track(), AudioQuality::High).await.unwrap();
    assert_eq!(std::fs::read(path).unwrap(), b"native");
    native_server.join().unwrap();
    assert_eq!(catalog.calls.load(Ordering::Relaxed), 1);
}

#[tokio::test]
async fn one_restore_or_enable_failure_does_not_stop_the_other_workers() {
    let catalog = Catalog::new("https://example.invalid/native");
    let first = catalog.add("missing", "async () => 'https://example.invalid/missing'", true);
    let second = catalog.add("survives", "async () => 'https://example.invalid/survives'", true);
    std::fs::remove_file(catalog.temp.path().join("data/sources").join(format!("{first}.js"))).unwrap();
    assert!(catalog.omni.restore_sources(catalog.store.clone(), catalog.temp.path().join("data")).is_err());
    assert_eq!(
        catalog.omni.resolve_audio_source(&track(), AudioQuality::High).await.unwrap().url,
        "https://example.invalid/survives"
    );
    assert!(catalog.omni.enable_source(&first, true).is_err());
    assert!(catalog.omni.custom_sources().enabled.contains(&second));
    assert_eq!(
        catalog.omni.resolve_audio_source(&track(), AudioQuality::High).await.unwrap().url,
        "https://example.invalid/survives"
    );
    assert_eq!(catalog.calls.load(Ordering::Relaxed), 0);
}

#[tokio::test]
async fn a_twenty_second_timeout_advances_to_the_next_enabled_source() {
    let catalog = Catalog::new("https://example.invalid/native");
    catalog.add("pending", "() => new Promise(() => {})", true);
    catalog.add("ready", "async () => 'https://example.invalid/ready'", true);
    let began = Instant::now();
    assert_eq!(
        catalog.omni.resolve_audio_source(&track(), AudioQuality::High).await.unwrap().url,
        "https://example.invalid/ready"
    );
    assert!(began.elapsed() >= Duration::from_secs(19));
    assert!(began.elapsed() < Duration::from_secs(30));
    assert_eq!(catalog.calls.load(Ordering::Relaxed), 0);
}

#[tokio::test]
async fn version_one_selection_is_migrated_and_restored() {
    let catalog = Catalog::new("https://example.invalid/native");
    let id = catalog.add("legacy", "async () => 'https://example.invalid/legacy'", false);
    let legacy = format!(
        "schema_version = 1\nenabled = \"{id}\"\n[[scripts]]\nid = \"{id}\"\n[scripts.metadata]\nname = \"legacy\"\n[scripts.sources.wy]\ntype = \"music\"\nactions = [\"musicUrl\"]\nqualitys = [\"128k\"]\n"
    );
    std::fs::write(catalog.store.dir().join("sources.toml"), legacy).unwrap();
    let settings = catalog.omni.restore_sources(catalog.store.clone(), catalog.temp.path().join("data")).unwrap();
    assert_eq!(settings.enabled, vec![id]);
    assert_eq!(settings.scripts[0].metadata.name, "legacy");
    assert_eq!(catalog.store.load::<SourceSettings>().unwrap(), settings);
    assert!(
        std::fs::read_to_string(catalog.store.dir().join("sources.toml")).unwrap().starts_with("schema_version = 2\n")
    );
    assert_eq!(
        catalog.omni.resolve_audio_source(&track(), AudioQuality::Standard).await.unwrap().url,
        "https://example.invalid/legacy"
    );
}

#[test]
fn disabled_legacy_settings_migrate_and_unknown_versions_are_untouched() {
    let temp = tempfile::tempdir().unwrap();
    let store = Store::at(temp.path());
    let path = temp.path().join("sources.toml");
    std::fs::write(&path, "schema_version = 1\nscripts = []\n").unwrap();
    assert_eq!(SourceSettings::load(&store).unwrap(), SourceSettings::default());
    assert_eq!(store.load::<SourceSettings>().unwrap(), SourceSettings::default());
    let future = "schema_version = 3\nscripts = []\nenabled = []\n";
    std::fs::write(&path, future).unwrap();
    assert!(matches!(SourceSettings::load(&store), Err(folia_store::StoreError::SchemaVersion { found: 3, .. })));
    assert_eq!(std::fs::read_to_string(&path).unwrap(), future);
}
