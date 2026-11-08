// Attack/release follower for lumiere's bass, treble and power (`lumiereAudio.ts`). Paused frames are zero.

use folia_core::AudioFrame;

const ATTACK: f64 = 0.06;
const RELEASE: f64 = 0.25;

/// Smoothed levels, each 0–1.
#[derive(Clone, Copy, Debug, Default)]
pub struct Heard {
    pub bass: f32,
    pub treble: f32,
    pub power: f32,
}

#[derive(Default)]
pub struct Sampler {
    bass: f32,
    treble: f32,
    power: f32,
    last_ms: f64,
}

impl Sampler {
    pub fn sample(&mut self, now_ms: f64, audio: &AudioFrame) -> Heard {
        let dt = if self.last_ms > 0.0 { ((now_ms - self.last_ms) / 1000.0).clamp(0.0, 0.1) } else { 1.0 };
        self.last_ms = now_ms;
        if !audio.has_spectrum {
            self.bass = 0.0;
            self.treble = 0.0;
            self.power = 0.0;
            return Heard::default();
        }
        self.bass = smooth(self.bass, unit(audio.bass), dt);
        self.treble = smooth(self.treble, unit(audio.treble), dt);
        self.power = smooth(self.power, unit(audio.power), dt);
        Heard { bass: self.bass, treble: self.treble, power: self.power }
    }
}

fn unit(value: f32) -> f32 {
    if value.is_finite() && value > 0.0 { (value / 255.0).min(1.0) } else { 0.0 }
}

fn smooth(current: f32, target: f32, dt: f64) -> f32 {
    let tau = if target > current { ATTACK } else { RELEASE };
    let k = 1.0 - (-dt / tau).exp();
    current + (target - current) * k as f32
}

pub fn wall_ms() -> f64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|elapsed| elapsed.as_secs_f64() * 1000.0)
        .unwrap_or(0.0)
}
