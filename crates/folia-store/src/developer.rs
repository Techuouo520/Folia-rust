// The developer tools page: what is recorded, whether it is written to a file, and how often memory is sampled.

use serde::{Deserialize, Serialize};

use crate::Domain;

/// How a log file treats what an earlier run left behind.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LogMode {
    #[default]
    Append,
    Overwrite,
}

impl LogMode {
    pub fn is_append(self) -> bool {
        self == LogMode::Append
    }
}

/// The defaults, in the original `electron/debug/debugHost.cjs` `DEFAULTS`: everything that writes costs
/// something, so only the in-memory session log starts on.
pub const DEFAULT_INTERVAL_MS: u64 = 2000;
pub const MIN_INTERVAL_MS: u64 = 1000;
pub const MAX_INTERVAL_MS: u64 = 60_000;

/// `clampInterval`: whole seconds between 1 and 60, the default where there is no number at all.
pub fn clamp_interval_ms(value: u64) -> u64 {
    if value == 0 { DEFAULT_INTERVAL_MS } else { value.clamp(MIN_INTERVAL_MS, MAX_INTERVAL_MS) }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct DeveloperSettings {
    /// `debug_runtime_log_enabled`; the original opens this off so a normal run writes nothing.
    #[serde(rename = "debug_runtime_log_enabled")]
    pub runtime_log_enabled: bool,
    /// `debug_runtime_log_mode`.
    #[serde(rename = "debug_runtime_log_mode")]
    pub runtime_log_mode: LogMode,
    /// `debug_console_capture_enabled`; the session log kept in memory, on unless switched off.
    #[serde(rename = "debug_console_capture_enabled")]
    pub console_capture_enabled: bool,
    /// `debug_memory_monitor_enabled`.
    #[serde(rename = "debug_memory_monitor_enabled")]
    pub memory_monitor_enabled: bool,
    /// `debug_memory_log_enabled`.
    #[serde(rename = "debug_memory_log_enabled")]
    pub memory_log_enabled: bool,
    /// `debug_memory_log_mode`.
    #[serde(rename = "debug_memory_log_mode")]
    pub memory_log_mode: LogMode,
    /// `debug_memory_interval_ms`.
    #[serde(rename = "debug_memory_interval_ms")]
    pub memory_interval_ms: u64,
}

impl Default for DeveloperSettings {
    fn default() -> Self {
        Self {
            runtime_log_enabled: false,
            runtime_log_mode: LogMode::Append,
            console_capture_enabled: true,
            memory_monitor_enabled: false,
            memory_log_enabled: false,
            memory_log_mode: LogMode::Overwrite,
            memory_interval_ms: DEFAULT_INTERVAL_MS,
        }
    }
}

impl DeveloperSettings {
    /// The interval as sampling uses it, whatever the file happens to hold.
    pub fn interval_ms(&self) -> u64 {
        clamp_interval_ms(self.memory_interval_ms)
    }
}

impl Domain for DeveloperSettings {
    const FILE: &'static str = "developer.toml";
    const SCHEMA_VERSION: u32 = 1;
}
