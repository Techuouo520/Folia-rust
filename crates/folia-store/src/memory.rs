// The memory monitor: one sampler thread for this process, sending what it cost to the live window and the log file.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, Sender};
use std::time::{Duration, Instant, SystemTime};

use parking_lot::Mutex;
use sysinfo::{Pid, ProcessRefreshKind, ProcessesToUpdate, System};

use crate::developer::clamp_interval_ms;
use crate::file_log::FileLog;
use crate::log_file::LOG_LINES;
use crate::runtime_log::iso;

/// Points the curve keeps; the original `memorySamples` `HISTORY`.
pub const MEMORY_HISTORY: usize = 600;

/// One reading of this process, with the session figures observed over the simultaneous totals.
#[derive(Debug, Clone)]
pub struct MemorySample {
    pub at: SystemTime,
    pub working_set_mb: u64,
    pub private_mb: Option<u64>,
    pub cpu_percent: f32,
    pub peak_mb: u64,
    pub floor_mb: u64,
    pub avg_mb: u64,
    pub samples: u64,
    pub uptime_sec: u64,
    pub pid: u32,
    pub name: String,
}

#[derive(Debug, Clone, Default)]
pub struct MemoryHistory {
    pub points: Vec<MemorySample>,
    pub latest: Option<MemorySample>,
}

#[derive(Debug, Default)]
struct Shared {
    history: Mutex<MemoryHistory>,
    generation: AtomicU64,
}

enum Command {
    Stop,
}

/// A sampler that runs only while the developer page says it should.
///
/// The thread owns `sysinfo::System`, so the interface thread never makes a sampling syscall; toggling the switch
/// starts or stops it by message. Samples go to the bounded history and, if its switch is on, to [`FileLog`].
#[derive(Debug)]
pub struct MemoryMonitor {
    shared: Arc<Shared>,
    control: Mutex<Option<Sender<Command>>>,
    log: FileLog,
}

impl MemoryMonitor {
    pub fn new(path: PathBuf) -> Self {
        Self { shared: Arc::new(Shared::default()), control: Mutex::new(None), log: FileLog::start(path, LOG_LINES) }
    }

    pub fn log_path(&self) -> &Path {
        self.log.path()
    }

    /// Starts (or restarts) sampling at `interval_ms`, clamped to 1–60 seconds; history starts over with the run.
    pub fn start(&self, interval_ms: u64) {
        self.stop();
        let generation = self.shared.generation.fetch_add(1, Ordering::SeqCst) + 1;
        self.shared.history.lock().points.clear();
        self.shared.history.lock().latest = None;
        let (tx, rx) = mpsc::channel();
        *self.control.lock() = Some(tx);
        let shared = self.shared.clone();
        let log = self.log.clone();
        let interval = Duration::from_millis(clamp_interval_ms(interval_ms));
        let spawned = std::thread::Builder::new()
            .name("folia-memory".to_owned())
            .spawn(move || sample_loop(shared, generation, interval, log, rx));
        if let Err(err) = spawned {
            tracing::warn!(error = &err as &dyn std::error::Error, "memory sampler thread");
        }
    }

    /// Stops the thread; a sample already being taken is dropped by the generation check.
    pub fn stop(&self) {
        self.shared.generation.fetch_add(1, Ordering::SeqCst);
        if let Some(tx) = self.control.lock().take() {
            let _ = tx.send(Command::Stop);
        }
    }

    /// Turns the file on or off without touching the sampling state.
    pub fn set_log(&self, enabled: bool, mode: crate::developer::LogMode) {
        self.log.set(enabled, mode);
    }

    pub fn history(&self) -> MemoryHistory {
        self.shared.history.lock().clone()
    }
}

impl Drop for MemoryMonitor {
    fn drop(&mut self) {
        self.stop();
    }
}

fn sample_loop(shared: Arc<Shared>, generation: u64, interval: Duration, log: FileLog, rx: mpsc::Receiver<Command>) {
    let Ok(pid) = sysinfo::get_current_pid() else { return };
    let cores = std::thread::available_parallelism().map_or(1, |count| count.get()) as f32;
    let mut system = System::new();
    let started = Instant::now();
    let mut stats = Stats::default();
    loop {
        let sample = read(&mut system, pid, cores, started, &mut stats);
        if shared.generation.load(Ordering::SeqCst) != generation {
            return;
        }
        if let Some(sample) = sample {
            log.write(&sample_line(&sample));
            push(&shared, sample);
        }
        match rx.recv_timeout(interval) {
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Ok(Command::Stop) | Err(mpsc::RecvTimeoutError::Disconnected) => return,
        }
    }
}

#[derive(Default)]
struct Stats {
    count: u64,
    sum: u64,
    peak: u64,
    floor: Option<u64>,
}

fn read(system: &mut System, pid: Pid, cores: f32, started: Instant, stats: &mut Stats) -> Option<MemorySample> {
    system.refresh_cpu_all();
    system.refresh_processes_specifics(
        ProcessesToUpdate::Some(&[pid]),
        true,
        ProcessRefreshKind::nothing().with_cpu().with_memory(),
    );
    let process = system.process(pid)?;
    let working_set_mb = process.memory() / (1024 * 1024);
    stats.count += 1;
    stats.sum += working_set_mb;
    stats.peak = stats.peak.max(working_set_mb);
    stats.floor = Some(stats.floor.map_or(working_set_mb, |floor| floor.min(working_set_mb)));
    Some(MemorySample {
        at: SystemTime::now(),
        working_set_mb,
        private_mb: private_mb(process),
        cpu_percent: (process.cpu_usage() / cores * 10.0).round() / 10.0,
        peak_mb: stats.peak,
        floor_mb: stats.floor.unwrap_or(working_set_mb),
        avg_mb: stats.sum / stats.count,
        samples: stats.count,
        uptime_sec: started.elapsed().as_secs(),
        pid: pid.as_u32(),
        name: process.name().to_string_lossy().into_owned(),
    })
}

fn push(shared: &Shared, sample: MemorySample) {
    let mut history = shared.history.lock();
    history.points.push(sample.clone());
    if history.points.len() > MEMORY_HISTORY {
        let drop = history.points.len() - MEMORY_HISTORY;
        history.points.drain(..drop);
    }
    history.latest = Some(sample);
}

fn sample_line(sample: &MemorySample) -> String {
    let private = sample.private_mb.map_or_else(|| "n/a".to_owned(), |mb| format!("{mb}MB"));
    format!(
        "{} ws={}MB private={} cpu={:.1}% processes=1",
        iso(sample.at),
        sample.working_set_mb,
        private,
        sample.cpu_percent
    )
}

/// Windows reports `PrivateUsage` through `virtual_memory`; Linux can sum its private pages; elsewhere it is absent.
#[cfg(target_os = "windows")]
fn private_mb(process: &sysinfo::Process) -> Option<u64> {
    Some(process.virtual_memory() / (1024 * 1024))
}

#[cfg(target_os = "linux")]
fn private_mb(_process: &sysinfo::Process) -> Option<u64> {
    let text = std::fs::read_to_string("/proc/self/smaps_rollup")
        .or_else(|_| std::fs::read_to_string("/proc/self/smaps"))
        .ok()?;
    let mut kb = 0_u64;
    for line in text.lines() {
        let Some((key, rest)) = line.split_once(':') else { continue };
        if matches!(key, "Private_Clean" | "Private_Dirty" | "Private_Hugetlb") {
            kb += rest.split_whitespace().next()?.parse::<u64>().ok()?;
        }
    }
    (kb > 0).then_some(kb / 1024)
}

#[cfg(not(any(target_os = "windows", target_os = "linux")))]
fn private_mb(_process: &sysinfo::Process) -> Option<u64> {
    None
}
