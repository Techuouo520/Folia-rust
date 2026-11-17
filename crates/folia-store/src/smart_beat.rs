// SmartBeat mode and per-effect strengths stored with shared visualizer settings.
use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

pub const SMART_BEAT_KEY: &str = "visualizer_smart_beat";
pub const NEBBIA_DRIZZLE_RIDE: &str = "nebbia.drizzleRide";
pub const NEBBIA_AQUA_BEAT: &str = "nebbia.aquaBeat";

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SmartBeatMode {
    #[default]
    Off,
    Cached,
    On,
}

impl SmartBeatMode {
    pub const ALL: [Self; 3] = [Self::Off, Self::Cached, Self::On];

    pub fn active(self) -> bool {
        self != Self::Off
    }
    pub fn may_analyse(self) -> bool {
        self == Self::On
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct BeatEffectSetting {
    pub on: bool,
    pub strength: f32,
    pub parts: BTreeMap<String, f32>,
}

static DEFAULT_EFFECT: BeatEffectSetting = BeatEffectSetting::DEFAULT;

impl BeatEffectSetting {
    pub const DEFAULT: Self = Self { on: true, strength: 1.0, parts: BTreeMap::new() };

    pub fn part(&self, key: &str) -> f32 {
        strength(self.parts.get(key).copied().unwrap_or(1.0))
    }
}

impl Default for BeatEffectSetting {
    fn default() -> Self {
        Self::DEFAULT
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SmartBeatSettings {
    pub mode: SmartBeatMode,
    pub effects: BTreeMap<String, BeatEffectSetting>,
}

impl SmartBeatSettings {
    pub fn read(value: Option<&toml::Value>) -> Self {
        let Some(table) = value.and_then(toml::Value::as_table) else { return Self::default() };
        let mode = match table.get("mode").and_then(toml::Value::as_str) {
            Some("on") => SmartBeatMode::On,
            Some("cached") => SmartBeatMode::Cached,
            Some("off") => SmartBeatMode::Off,
            _ if table.get("enabled").and_then(toml::Value::as_bool) == Some(true) => SmartBeatMode::On,
            _ => SmartBeatMode::Off,
        };
        let mut effects = BTreeMap::new();
        if let Some(saved) = table.get("effects").and_then(toml::Value::as_table) {
            for (id, value) in saved {
                let Some(value) = value.as_table() else { continue };
                let mut setting = BeatEffectSetting {
                    on: value.get("on").and_then(toml::Value::as_bool).unwrap_or(true),
                    strength: number(value.get("strength")),
                    parts: BTreeMap::new(),
                };
                if let Some(parts) = value.get("parts").and_then(toml::Value::as_table) {
                    setting.parts.extend(parts.iter().map(|(name, value)| (name.clone(), number(Some(value)))));
                }
                effects.insert(id.clone(), setting);
            }
        }
        Self { mode, effects }
    }

    pub fn effect(&self, id: &str) -> &BeatEffectSetting {
        self.effects.get(id).unwrap_or(&DEFAULT_EFFECT)
    }

    pub fn set_part(&mut self, effect: &str, part: &str, value: f32) {
        self.effects.entry(effect.to_owned()).or_default().parts.insert(part.to_owned(), strength(value));
    }

    pub fn set_enabled(&mut self, effect: &str, on: bool) {
        self.effects.entry(effect.to_owned()).or_default().on = on;
    }

    pub fn table(&self) -> toml::map::Map<String, toml::Value> {
        use toml::Value;
        let mode = match self.mode {
            SmartBeatMode::Off => "off",
            SmartBeatMode::Cached => "cached",
            SmartBeatMode::On => "on",
        };
        let effects = self
            .effects
            .iter()
            .map(|(id, setting)| {
                let parts = setting
                    .parts
                    .iter()
                    .map(|(key, value)| (key.clone(), Value::Float(f64::from(strength(*value)))))
                    .collect();
                let table = toml::map::Map::from_iter([
                    ("on".to_owned(), Value::Boolean(setting.on)),
                    ("strength".to_owned(), Value::Float(f64::from(strength(setting.strength)))),
                    ("parts".to_owned(), Value::Table(parts)),
                ]);
                (id.clone(), Value::Table(table))
            })
            .collect();
        toml::map::Map::from_iter([
            ("mode".to_owned(), Value::String(mode.to_owned())),
            ("effects".to_owned(), Value::Table(effects)),
        ])
    }
}

fn strength(value: f32) -> f32 {
    if value.is_finite() { value.clamp(0.0, 2.0) } else { 1.0 }
}

fn number(value: Option<&toml::Value>) -> f32 {
    strength(
        value.and_then(|value| value.as_float().or_else(|| value.as_integer().map(|value| value as f64))).unwrap_or(1.0)
            as f32,
    )
}
