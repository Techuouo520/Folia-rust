// Wall clock for audio followers. The dev feature can pin it so two runs match.

use std::time::SystemTime;

#[cfg(feature = "dev")]
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

#[cfg(feature = "dev")]
static FORCED: AtomicBool = AtomicBool::new(false);
#[cfg(feature = "dev")]
static MILLIS: AtomicU64 = AtomicU64::new(0);

pub fn wall_ms() -> f64 {
    #[cfg(feature = "dev")]
    if FORCED.load(Ordering::Relaxed) {
        return MILLIS.load(Ordering::Relaxed) as f64;
    }
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|elapsed| elapsed.as_secs_f64() * 1000.0)
        .unwrap_or(0.0)
}

/// `Some` pins [`wall_ms`]. `None` follows the system clock again.
///
/// Release builds omit this. Tests enable the `dev` feature.
#[cfg(feature = "dev")]
pub fn set_test_clock_ms(ms: Option<u64>) {
    match ms {
        Some(ms) => {
            MILLIS.store(ms, Ordering::Relaxed);
            FORCED.store(true, Ordering::Relaxed);
        }
        None => FORCED.store(false, Ordering::Relaxed),
    }
}
