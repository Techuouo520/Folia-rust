// Native bounded frame scheduling and the stable grain frame index.

use std::time::{Duration, Instant};

/// Wakes an active visual surface at its cap without accumulating missed frames.
pub fn frames_at(fps: u32) -> iced::Subscription<Instant> {
    iced::Subscription::run_with(fps.max(1), |fps| {
        let interval = Duration::from_secs_f64(1.0 / f64::from(*fps));
        let (mut sender, receiver) = iced::futures::channel::mpsc::channel(0);
        let worker = std::thread::Builder::new().name("folia-frames".into()).spawn(move || {
            while !sender.is_closed() {
                // Rust uses a high-resolution waitable timer on supported Windows hosts.
                std::thread::sleep(interval);
                if let Err(error) = sender.try_send(Instant::now())
                    && error.is_disconnected()
                {
                    break;
                }
            }
        });
        if let Err(error) = worker {
            tracing::error!(error = &error as &dyn std::error::Error, "cannot start visual frame timer");
        }
        receiver
    })
}

/// `time_seconds * 48`, wrapped into `0..4096`, so the hash input stays a small integer.
pub fn grain_frame(time_seconds: f32) -> u32 {
    if !time_seconds.is_finite() || time_seconds <= 0.0 {
        return 0;
    }
    (time_seconds * 48.0).rem_euclid(4096.0) as u32
}
