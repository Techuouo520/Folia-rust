// The ten-band equalizer and the sound-effect chain, as the original's `audioEqualizer.ts` and `audioEffects.ts` define them.

use serde::{Deserialize, Serialize};

/// The ten band frequencies in Hz (`AUDIO_EQUALIZER_BANDS`).
pub const EQUALIZER_FREQUENCIES: [f32; 10] = [31.0, 62.0, 125.0, 250.0, 500.0, 1000.0, 2000.0, 4000.0, 8000.0, 16000.0];
/// `AUDIO_EQUALIZER_MIN_GAIN_DB` and `AUDIO_EQUALIZER_MAX_GAIN_DB`.
pub const EQUALIZER_MIN_GAIN_DB: f32 = -12.0;
pub const EQUALIZER_MAX_GAIN_DB: f32 = 12.0;
/// The Q every band uses (`audioEqualizerGraph.ts`).
pub const EQUALIZER_Q: f32 = 1.4;

/// The filter shape of a band: the first is a low shelf, the last a high shelf, the rest peaking.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BandShape {
    LowShelf,
    Peaking,
    HighShelf,
}

impl BandShape {
    pub fn of(index: usize) -> BandShape {
        if index == 0 {
            BandShape::LowShelf
        } else if index + 1 == EQUALIZER_FREQUENCIES.len() {
            BandShape::HighShelf
        } else {
            BandShape::Peaking
        }
    }
}

/// `AudioEffectSettings`, in signal-flow order.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AudioEffects {
    pub highpass: f32,
    pub lowpass: f32,
    pub drive: f32,
    pub crush: f32,
    pub wow: f32,
    pub noise: f32,
    pub width: f32,
    pub space: f32,
    pub punch: f32,
}

impl Default for AudioEffects {
    fn default() -> Self {
        Self {
            highpass: 20.0,
            lowpass: 20000.0,
            drive: 0.0,
            crush: 0.0,
            wow: 0.0,
            noise: 0.0,
            width: 1.0,
            space: 0.0,
            punch: 0.0,
        }
    }
}

impl AudioEffects {
    /// Neutral values or in-range ones (`normalizeAudioEffects`).
    pub fn clamped(self) -> Self {
        let linear = |value: f32| if value.is_finite() { value.clamp(0.0, 1.0) } else { 0.0 };
        Self {
            highpass: if self.highpass.is_finite() { self.highpass.clamp(20.0, 2000.0) } else { 20.0 },
            lowpass: if self.lowpass.is_finite() { self.lowpass.clamp(500.0, 20000.0) } else { 20000.0 },
            drive: linear(self.drive),
            crush: linear(self.crush),
            wow: linear(self.wow),
            noise: linear(self.noise),
            width: if self.width.is_finite() { self.width.clamp(0.0, 2.0) } else { 1.0 },
            space: linear(self.space),
            punch: linear(self.punch),
        }
    }
}

/// One custom slot's curve and effects.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct EqualizerSlot {
    #[serde(default)]
    pub gains: [f32; 10],
    #[serde(default)]
    pub effects: AudioEffects,
}

impl Default for EqualizerSlot {
    fn default() -> Self {
        Self { gains: [0.0; 10], effects: AudioEffects::default() }
    }
}

/// `audio_equalizer_settings`: the switch, the curve, the chosen sound and both custom slots.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct EqualizerSettings {
    pub enabled: bool,
    pub gains: [f32; 10],
    pub preset: String,
    pub effects: AudioEffects,
    pub custom_slots: [EqualizerSlot; 2],
}

impl Default for EqualizerSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            gains: [0.0; 10],
            preset: "flat".to_owned(),
            effects: AudioEffects::default(),
            custom_slots: [EqualizerSlot::default(); 2],
        }
    }
}

impl EqualizerSettings {
    /// Ten gains clamped to ±12 dB, with anything unreadable at 0.
    pub fn normalized_gains(gains: &[f32]) -> [f32; 10] {
        let mut out = [0.0; 10];
        for (index, slot) in out.iter_mut().enumerate() {
            *slot = gains.get(index).copied().filter(|gain| gain.is_finite()).unwrap_or(0.0);
            *slot = slot.clamp(EQUALIZER_MIN_GAIN_DB, EQUALIZER_MAX_GAIN_DB);
        }
        out
    }

    pub fn clamped(mut self) -> Self {
        self.gains = Self::normalized_gains(&self.gains);
        self.effects = self.effects.clamped();
        for slot in &mut self.custom_slots {
            slot.gains = Self::normalized_gains(&slot.gains);
            slot.effects = slot.effects.clamped();
        }
        if preset(self.preset.as_str()).is_none() && !self.preset.starts_with("custom") {
            self.preset = "custom1".to_owned();
        }
        self
    }
}

/// A built-in sound: its curve and its effects (`AUDIO_SOUND_PRESETS`).
#[derive(Debug, Clone, Copy)]
pub struct SoundPreset {
    pub id: &'static str,
    pub gains: [f32; 10],
    pub effects: AudioEffects,
}

/// The six built-in sounds, in the original's order (`AUDIO_SOUND_PRESET_IDS`).
pub const SOUND_PRESETS: [SoundPreset; 6] = [
    SoundPreset { id: "flat", gains: [0.0; 10], effects: AudioEffects::DEFAULT_NEUTRAL },
    SoundPreset {
        id: "lofi",
        gains: [-4.0, -2.0, 0.0, 2.0, 3.0, 2.0, 0.0, -3.0, -7.0, -10.0],
        effects: AudioEffects {
            highpass: 90.0,
            lowpass: 6000.0,
            drive: 0.28,
            crush: 0.45,
            wow: 0.32,
            noise: 0.22,
            width: 0.7,
            space: 0.16,
            punch: 0.4,
        },
    },
    SoundPreset {
        id: "radio",
        gains: [-12.0, -8.0, -3.0, 2.0, 4.0, 4.0, 2.0, -3.0, -8.0, -12.0],
        effects: AudioEffects {
            highpass: 420.0,
            lowpass: 3800.0,
            drive: 0.42,
            crush: 0.3,
            wow: 0.1,
            noise: 0.2,
            width: 0.12,
            space: 0.0,
            punch: 0.6,
        },
    },
    SoundPreset {
        id: "hall",
        gains: [1.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 0.0],
        effects: AudioEffects {
            lowpass: 16000.0,
            width: 1.3,
            space: 0.55,
            punch: 0.15,
            ..AudioEffects::DEFAULT_NEUTRAL
        },
    },
    SoundPreset {
        id: "vocal",
        gains: [-2.0, -1.0, 0.0, 2.0, 4.0, 4.0, 3.0, 1.0, 0.0, -1.0],
        effects: AudioEffects {
            highpass: 110.0,
            width: 0.9,
            space: 0.08,
            punch: 0.35,
            ..AudioEffects::DEFAULT_NEUTRAL
        },
    },
    SoundPreset {
        id: "bass",
        gains: [6.0, 5.0, 3.0, 1.0, 0.0, -1.0, -1.0, 0.0, 1.0, 2.0],
        effects: AudioEffects { drive: 0.1, width: 1.15, punch: 0.45, ..AudioEffects::DEFAULT_NEUTRAL },
    },
];

impl AudioEffects {
    const DEFAULT_NEUTRAL: AudioEffects = AudioEffects {
        highpass: 20.0,
        lowpass: 20000.0,
        drive: 0.0,
        crush: 0.0,
        wow: 0.0,
        noise: 0.0,
        width: 1.0,
        space: 0.0,
        punch: 0.0,
    };
}

pub fn preset(id: &str) -> Option<SoundPreset> {
    SOUND_PRESETS.iter().copied().find(|preset| preset.id == id)
}
