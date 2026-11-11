// Developer settings, the runtime log's file switches and the memory sampler's interval and stop behaviour.

use std::fs;
use std::time::Duration;

use folia_store::{DeveloperSettings, LogMode, MemoryMonitor, RuntimeLog, Store, StoreError, clamp_interval_ms};

#[test]
fn structured_error_causes_reach_both_memory_and_file() {
    use tracing_subscriber::prelude::*;
    struct Capture(RuntimeLog);
    impl<S: tracing::Subscriber> tracing_subscriber::Layer<S> for Capture {
        fn on_event(&self, event: &tracing::Event<'_>, _: tracing_subscriber::layer::Context<'_, S>) {
            self.0.record_event(event);
        }
    }
    #[derive(Debug)]
    struct Failure(std::io::Error);
    impl std::fmt::Display for Failure {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_str("request failed")
        }
    }
    impl std::error::Error for Failure {
        fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
            Some(&self.0)
        }
    }
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("runtime.log");
    let log = RuntimeLog::new(path.clone());
    log.configure(true, true, LogMode::Overwrite);
    let error = Failure(std::io::Error::new(std::io::ErrorKind::TimedOut, "operation timed out"));
    tracing::subscriber::with_default(tracing_subscriber::registry().with(Capture(log.clone())), || {
        tracing::error!(error = &error as &dyn std::error::Error, "audio download failed");
    });
    assert!(log.entries()[0].message.contains("error=request failed: operation timed out"));
    log.sync();
    assert!(std::fs::read_to_string(path).unwrap().contains("error=request failed: operation timed out"));
}
use tracing::Level;

fn lines(path: &std::path::Path) -> Vec<String> {
    fs::read_to_string(path).unwrap_or_default().lines().map(str::to_owned).collect()
}

#[test]
fn defaults_match_the_original_debug_host() {
    let settings = DeveloperSettings::default();
    assert!(!settings.runtime_log_enabled);
    assert_eq!(settings.runtime_log_mode, LogMode::Append);
    assert!(settings.console_capture_enabled);
    assert!(!settings.memory_monitor_enabled);
    assert!(!settings.memory_log_enabled);
    assert_eq!(settings.memory_log_mode, LogMode::Overwrite);
    assert_eq!(settings.memory_interval_ms, 2000);
}

#[test]
fn saves_and_reads_back_under_the_original_keys() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::at(dir.path());
    let settings = DeveloperSettings {
        runtime_log_enabled: true,
        runtime_log_mode: LogMode::Overwrite,
        console_capture_enabled: false,
        memory_monitor_enabled: true,
        memory_log_enabled: true,
        memory_log_mode: LogMode::Append,
        memory_interval_ms: 5000,
    };
    store.save(&settings).unwrap();
    assert_eq!(store.load::<DeveloperSettings>().unwrap(), settings);
    let text = fs::read_to_string(dir.path().join("developer.toml")).unwrap();
    assert!(text.starts_with("schema_version = 1\n"), "{text}");
    for key in [
        "debug_runtime_log_enabled = true",
        "debug_runtime_log_mode = \"overwrite\"",
        "debug_console_capture_enabled = false",
        "debug_memory_monitor_enabled = true",
        "debug_memory_log_enabled = true",
        "debug_memory_log_mode = \"append\"",
        "debug_memory_interval_ms = 5000",
    ] {
        assert!(text.contains(key), "missing {key} in {text}");
    }
}

#[test]
fn rejects_other_schema_versions() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("developer.toml"), "schema_version = 4\n").unwrap();
    let err = Store::at(dir.path()).load::<DeveloperSettings>().unwrap_err();
    assert!(matches!(err, StoreError::SchemaVersion { found: 4, expected: 1, .. }), "{err}");
}

#[test]
fn the_interval_is_clamped_to_one_through_sixty_seconds() {
    assert_eq!(clamp_interval_ms(0), 2000);
    assert_eq!(clamp_interval_ms(1), 1000);
    assert_eq!(clamp_interval_ms(1500), 1500);
    assert_eq!(clamp_interval_ms(60_000), 60_000);
    assert_eq!(clamp_interval_ms(90_000), 60_000);
    let settings = DeveloperSettings { memory_interval_ms: 5, ..DeveloperSettings::default() };
    assert_eq!(settings.interval_ms(), 1000);
}

#[test]
fn a_switched_off_file_writes_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("logs").join("runtime").join("folia.log");
    let log = RuntimeLog::new(path.clone());
    log.configure(true, false, LogMode::Append);
    log.push(Level::INFO, "folia_store", "first");
    log.sync();
    assert!(!path.exists(), "a file appeared with the switch off");
    assert_eq!(log.entries().len(), 1);
}

#[test]
fn turning_the_file_on_writes_and_overwrite_clears_the_old_run() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("logs").join("runtime").join("folia.log");
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(&path, "old a\nold b\n").unwrap();
    let log = RuntimeLog::new(path.clone());
    log.configure(true, true, LogMode::Append);
    log.push(Level::INFO, "folia_store", "new line");
    log.sync();
    let kept = lines(&path);
    assert!(kept.last().is_some_and(|line| line.ends_with("[INFO] [folia_store] new line")), "{kept:?}");
    assert!(kept.iter().any(|line| line.contains("old b")), "append lost the earlier tail");

    log.configure(true, true, LogMode::Overwrite);
    log.sync();
    assert!(lines(&path).is_empty(), "overwrite did not clear the file at once");
    log.push(Level::WARN, "folia_ui", "after clear");
    log.sync();
    let kept = lines(&path);
    assert_eq!(kept.len(), 1);
    assert!(kept[0].contains("after clear"));
}

#[test]
fn the_file_keeps_the_last_five_hundred_lines() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("folia.log");
    let log = RuntimeLog::new(path.clone());
    log.configure(true, true, LogMode::Overwrite);
    for index in 0..600 {
        log.push(Level::DEBUG, "folia_store", &format!("line {index}"));
    }
    log.sync();
    let kept = lines(&path);
    assert_eq!(kept.len(), 500);
    assert!(kept[0].contains("line 100"), "{}", kept[0]);
    assert!(kept[499].contains("line 599"), "{}", kept[499]);
}

#[test]
fn capture_off_drops_the_lines_and_writes_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("folia.log");
    let log = RuntimeLog::new(path.clone());
    log.configure(true, true, LogMode::Overwrite);
    log.push(Level::INFO, "folia_store", "kept");
    log.configure(false, true, LogMode::Overwrite);
    assert!(log.entries().is_empty());
    log.push(Level::INFO, "folia_store", "dropped");
    log.sync();
    assert!(lines(&path).iter().all(|line| !line.contains("dropped")));
}

#[test]
fn the_memory_monitor_samples_and_stops() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("logs").join("memory").join("folia.log");
    let monitor = MemoryMonitor::new(path.clone());
    monitor.set_log(true, LogMode::Overwrite);
    monitor.start(1000);
    std::thread::sleep(Duration::from_millis(1300));
    let first = monitor.history();
    assert!(!first.points.is_empty(), "no sample was taken");
    assert!(first.latest.is_some());
    monitor.stop();
    std::thread::sleep(Duration::from_millis(250));
    let stopped = monitor.history().points.len();
    std::thread::sleep(Duration::from_millis(1300));
    assert_eq!(monitor.history().points.len(), stopped, "samples kept arriving after stop");
    let kept = lines(&path);
    assert!(!kept.is_empty(), "the memory log stayed empty with its switch on");
    assert!(kept.len() <= 500);
    assert!(kept.iter().all(|line| line.contains("ws=") && line.contains("cpu=")));
}

#[test]
fn the_memory_log_stays_absent_with_its_switch_off() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("logs").join("memory").join("folia.log");
    let monitor = MemoryMonitor::new(path.clone());
    monitor.set_log(false, LogMode::Append);
    monitor.start(1000);
    std::thread::sleep(Duration::from_millis(1200));
    monitor.stop();
    assert!(!path.exists(), "a memory file appeared with its switch off");
}
