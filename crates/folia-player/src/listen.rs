// Counts continuous audio progress without crediting pauses, buffering or seeks.

use std::time::Instant;

#[derive(Default)]
pub struct ListenTracker {
    previous: Option<(f64, Instant, bool)>,
    seconds: f64,
    elapsed: f64,
}

impl ListenTracker {
    /// Samples actual output position; `playing` excludes buffering and restored bookmarks.
    pub fn observe(&mut self, position: f64, now: Instant, playing: bool) {
        let previous = self.previous.replace((position, now, playing));
        let Some((before, at, was_playing)) = previous else { return };
        let step = position - before;
        let elapsed = now.saturating_duration_since(at).as_secs_f64();
        if playing && was_playing {
            self.elapsed += elapsed;
            if position.is_finite() && step > 0.0 && step <= 2.0 {
                self.seconds += step;
            }
        }
    }

    /// Reanchors explicit seeks and playback state changes, including small forward seeks.
    pub fn discontinuity(&mut self) {
        self.previous = None;
    }

    /// Settles once; less than thirty seconds of real audio is a skip.
    pub fn settle(&mut self, duration: f64) -> Option<f64> {
        let seconds = std::mem::take(&mut self.seconds);
        let elapsed = std::mem::take(&mut self.elapsed);
        self.previous = None;
        (seconds >= 30.0 && seconds <= elapsed * 1.25 + 2.0)
            .then(|| if duration.is_finite() && duration > 0.0 { seconds.min(duration) } else { seconds })
    }
}
