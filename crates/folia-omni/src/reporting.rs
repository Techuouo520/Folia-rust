// Account-scoped listening reports, with one in-flight request and bounded report frequency.

use std::collections::VecDeque;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use folia_core::{AudioQuality, Track};

use crate::{Omni, OmniError};

#[derive(Debug, Clone, Copy)]
pub struct PlaybackReport {
    pub played_seconds: f64,
    pub total_seconds: f64,
    pub quality: AudioQuality,
}

#[derive(Default)]
pub(crate) struct ReportGate {
    recent: Mutex<VecDeque<Instant>>,
    active: tokio::sync::Mutex<()>,
}

impl Omni {
    pub fn playback_reporting_ready(&self, provider: &str) -> bool {
        self.provider(provider).is_ok_and(|provider| provider.playback_reporting_ready())
    }

    pub fn can_report_playback(&self, track: &Track) -> bool {
        track
            .online
            .as_ref()
            .is_some_and(|song| self.provider(&song.provider).is_ok_and(|provider| provider.can_report_playback(track)))
    }

    /// Reports earned listening time. Rate limits and cancellation apply across cloned facades.
    pub async fn report_playback(&self, track: &Track, report: PlaybackReport) -> Result<(), OmniError> {
        let song = track.online.as_ref().ok_or(OmniError::Unsupported("local listening reports"))?;
        let provider = self.provider(&song.provider)?;
        if !provider.playback_reporting_ready() {
            return Err(OmniError::SignedOut);
        }
        if !provider.can_report_playback(track) || !report.played_seconds.is_finite() || report.played_seconds < 30.0 {
            return Err(OmniError::Unsupported("this listening report"));
        }
        let _active = self.report_gate.active.try_lock().map_err(|_| OmniError::ReportThrottled)?;
        {
            let now = Instant::now();
            let mut recent = self.report_gate.recent.lock().map_err(|_| OmniError::ReportThrottled)?;
            while recent.front().is_some_and(|at| now.duration_since(*at) >= Duration::from_secs(3600)) {
                recent.pop_front();
            }
            if recent.len() >= 60 || recent.back().is_some_and(|at| now.duration_since(*at) < Duration::from_secs(5)) {
                return Err(OmniError::ReportThrottled);
            }
            recent.push_back(now);
        }
        tokio::time::timeout(Duration::from_secs(15), provider.report_playback(track, report))
            .await
            .map_err(|_| OmniError::ReportTimeout)?
    }
}
