// One visualiser frame of loudness, on the same 0–255 scale as Web Audio byte frequency data.

/// Bands and `power` are `0.0..=255.0`. `spectrum` is the 1024 analyser bins; it is meaningful only when
/// `has_spectrum` is set. While playback is stopped the bins are empty and every level is the idle breath.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AudioFrame {
    pub power: f32,
    pub bass: f32,
    pub low_mid: f32,
    pub mid: f32,
    pub vocal: f32,
    pub treble: f32,
    pub spectrum: [u8; 1024],
    pub has_spectrum: bool,
}

impl AudioFrame {
    pub const SILENT: Self = Self {
        power: 0.0,
        bass: 0.0,
        low_mid: 0.0,
        mid: 0.0,
        vocal: 0.0,
        treble: 0.0,
        spectrum: [0; 1024],
        has_spectrum: false,
    };

    /// Idle breath from the original bridge: `(sin(wall_ms / 2000) + 1) * 20`, with an empty spectrum.
    pub fn breath(wall_ms: f64) -> Self {
        let value = ((wall_ms / 2000.0).sin() + 1.0) * 20.0;
        let value = value as f32;
        Self {
            power: value,
            bass: value,
            low_mid: value,
            mid: value,
            vocal: value,
            treble: value,
            spectrum: [0; 1024],
            has_spectrum: false,
        }
    }
}

impl Default for AudioFrame {
    fn default() -> Self {
        Self::SILENT
    }
}
