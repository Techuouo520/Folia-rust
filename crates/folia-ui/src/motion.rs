// Wall-clock tweens for chrome that eases between two values, like a CSS transition.

use std::time::Instant;

use folia_store::{MotionSettings, MotionSurface};
use folia_visualizer::Ease;

thread_local! {
    // A UI-thread snapshot keeps the shared chrome primitives consistent without cross-thread mutable state.
    static REDUCED: std::cell::Cell<u8> = const { std::cell::Cell::new(0) };
}

pub fn configure(settings: &MotionSettings, system: bool) {
    let flags = MotionSurface::ALL
        .into_iter()
        .enumerate()
        .fold(0, |flags, (index, surface)| flags | (u8::from(settings.reduced(surface, system)) << index));
    REDUCED.set(flags);
}

pub fn reduced(surface: MotionSurface) -> bool {
    REDUCED.get() & (1 << surface as u8) != 0
}

/// Tailwind's default `transition` curve.
pub const CSS_TRANSITION: Ease = Ease::new(0.4, 0.0, 0.2, 1.0);

/// A value that eases from where it was towards `goal` whenever the goal changes.
#[derive(Debug, Clone, Copy)]
pub struct Tween {
    from: f32,
    goal: f32,
    at: Instant,
    ms: f64,
    ease: Ease,
    reduce_micro_motion: bool,
}

impl Tween {
    pub fn new(value: f32, ms: f64, ease: Ease, now: Instant) -> Self {
        Self { from: value, goal: value, at: now, ms, ease, reduce_micro_motion: true }
    }

    pub fn independent(mut self) -> Self {
        self.reduce_micro_motion = false;
        self
    }

    pub fn value(&self, now: Instant) -> f32 {
        if self.reduce_micro_motion && reduced(MotionSurface::UiMicroMotion) {
            return self.goal;
        }
        let elapsed = now.saturating_duration_since(self.at).as_secs_f64() * 1000.0;
        folia_visualizer::tween(f64::from(self.from), f64::from(self.goal), elapsed, self.ms, self.ease) as f32
    }

    /// Starts easing towards `goal` from the current value; the same goal keeps the running tween.
    pub fn set(&mut self, goal: f32, now: Instant) {
        if (goal - self.goal).abs() < f32::EPSILON {
            return;
        }
        self.from = self.value(now);
        self.goal = goal;
        self.at = now;
    }

    /// `set` with this transition's own duration, for an enter and an exit that differ.
    pub fn set_with(&mut self, goal: f32, ms: f64, ease: Ease, now: Instant) {
        if (goal - self.goal).abs() < f32::EPSILON {
            return;
        }
        self.from = self.value(now);
        self.goal = goal;
        self.at = now;
        self.ms = ms;
        self.ease = ease;
    }

    /// Jumps to `from` and eases back to the goal, for content that enters again.
    pub fn restart(&mut self, from: f32, now: Instant) {
        self.from = from;
        self.at = now;
    }

    /// The value `set` is easing towards.
    pub fn goal(&self) -> f32 {
        self.goal
    }

    pub fn running(&self, now: Instant) -> bool {
        !(self.reduce_micro_motion && reduced(MotionSurface::UiMicroMotion))
            && (self.from - self.goal).abs() > 0.0001
            && now.saturating_duration_since(self.at).as_secs_f64() * 1000.0 < self.ms
    }
}
