// Appearance preferences: theme, lyric animation style, how often it may redraw, the lyric and subtitle fonts,
// the bottom subtitle's rows, and the background mode the original app keeps in `visualizerBackgroundMode`.

use folia_core::Theme;
use serde::{Deserialize, Serialize};

use crate::Domain;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ThemeMode {
    #[default]
    Dark,
    Light,
}

impl ThemeMode {
    pub fn theme(self) -> Theme {
        match self {
            ThemeMode::Dark => Theme::MIDNIGHT,
            ThemeMode::Light => Theme::DAYLIGHT,
        }
    }
}

/// Upper bound on visualizer redraws; `Display` follows the monitor's refresh rate.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum FrameRate {
    #[serde(rename = "30")]
    Fps30,
    #[serde(rename = "45")]
    Fps45,
    #[default]
    #[serde(rename = "60")]
    Fps60,
    #[serde(rename = "90")]
    Fps90,
    #[serde(rename = "120")]
    Fps120,
    #[serde(rename = "display")]
    Display,
}

impl FrameRate {
    pub const ALL: [FrameRate; 6] =
        [FrameRate::Fps30, FrameRate::Fps45, FrameRate::Fps60, FrameRate::Fps90, FrameRate::Fps120, FrameRate::Display];

    pub fn fps(self) -> Option<u32> {
        match self {
            FrameRate::Fps30 => Some(30),
            FrameRate::Fps45 => Some(45),
            FrameRate::Fps60 => Some(60),
            FrameRate::Fps90 => Some(90),
            FrameRate::Fps120 => Some(120),
            FrameRate::Display => None,
        }
    }
}

/// The original theme's `fontStyle` (`sans`/`serif`/`mono`) plus the custom family the font picker chooses.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FontStyle {
    #[default]
    Sans,
    Serif,
    Mono,
    Custom,
}

/// A typeface and the fallbacks tried after it, as the original `Theme` carries them.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct FontSettings {
    pub style: FontStyle,
    /// The family picked from the machine's fonts; only read when `style` is `custom`.
    pub family: Option<String>,
    /// `fontFamilyStack`: families tried in order when the chosen one has no glyph.
    pub fallback: Vec<String>,
    /// `fontWeight`; `None` follows the style's own weight (`fontWeightAuto`).
    pub weight: Option<u16>,
}

pub use folia_core::SubtitleContentMode;

/// One row of the bottom subtitle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RowSource {
    Current,
    Next,
    Translation,
    Romanization,
}

impl RowSource {
    fn id(self) -> &'static str {
        match self {
            RowSource::Current => "current",
            RowSource::Next => "next",
            RowSource::Translation => "translation",
            RowSource::Romanization => "romanization",
        }
    }

    fn from_id(id: &str) -> Option<Self> {
        match id {
            "current" => Some(RowSource::Current),
            "next" => Some(RowSource::Next),
            "translation" => Some(RowSource::Translation),
            "romanization" => Some(RowSource::Romanization),
            _ => None,
        }
    }
}

/// The bottom subtitle's rows, stored the way the original `subtitleOverlayLayout` stores them: `current+next`, ...
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubtitleLayout(String);

impl Default for SubtitleLayout {
    fn default() -> Self {
        Self("translation".to_owned())
    }
}

impl SubtitleLayout {
    /// The original `parseSubtitleOverlayLayout`: one single row, or `current` followed by one or two follow rows.
    pub fn parse(value: &str) -> Option<Self> {
        let rows: Vec<&str> = value.split('+').collect();
        match rows.as_slice() {
            [single] if matches!(*single, "current" | "translation" | "romanization") => {
                Some(Self((*single).to_owned()))
            }
            [first, follow @ ..] if *first == "current" && !follow.is_empty() && follow.len() <= 2 => {
                let ok = follow.iter().all(|row| matches!(*row, "next" | "translation" | "romanization"));
                ok.then(|| Self(value.to_owned()))
            }
            _ => None,
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn rows(&self) -> Vec<RowSource> {
        self.0.split('+').filter_map(RowSource::from_id).collect()
    }

    pub fn count(&self) -> usize {
        self.0.split('+').count()
    }

    /// `withSubtitleOverlayRowCount`: shrinking keeps a translation or romanization row, or the current line when
    /// there is none; growing appends the content the stack does not show yet.
    pub fn with_count(&self, count: u8) -> Self {
        let count = count.clamp(1, 3) as usize;
        let rows = self.rows();
        if count == 1 {
            if rows.len() == 1 {
                return self.clone();
            }
            let kept = rows
                .iter()
                .copied()
                .find(|row| matches!(row, RowSource::Translation | RowSource::Romanization))
                .unwrap_or(RowSource::Current);
            return Self(kept.id().to_owned());
        }
        let mut follow: Vec<RowSource> = if rows.len() == 1 {
            vec![match rows[0] {
                RowSource::Current => RowSource::Next,
                other => other,
            }]
        } else {
            rows.into_iter().skip(1).collect()
        };
        for missing in [RowSource::Translation, RowSource::Romanization, RowSource::Next] {
            if follow.len() >= count - 1 {
                break;
            }
            if !follow.contains(&missing) {
                follow.push(missing);
            }
        }
        while follow.len() < count - 1 {
            follow.push(RowSource::Next);
        }
        join(&[RowSource::Current], &follow[..count - 1])
    }

    /// `withSubtitleOverlayRow`: row 0 changes only in a single-row layout.
    pub fn with_row(&self, index: usize, source: RowSource) -> Self {
        let mut rows = self.rows();
        if index >= rows.len() {
            return self.clone();
        }
        rows[index] = source;
        if rows.len() > 1 && rows[0] != RowSource::Current {
            return self.clone();
        }
        let text = rows.iter().map(|row| row.id()).collect::<Vec<_>>().join("+");
        SubtitleLayout::parse(&text).unwrap_or_else(|| self.clone())
    }
}

fn join(first: &[RowSource], follow: &[RowSource]) -> SubtitleLayout {
    let text = first.iter().chain(follow).map(|row| row.id()).collect::<Vec<_>>().join("+");
    SubtitleLayout::parse(&text).unwrap_or_default()
}

impl Serialize for SubtitleLayout {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for SubtitleLayout {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = String::deserialize(deserializer)?;
        Ok(SubtitleLayout::parse(&text).unwrap_or_default())
    }
}

/// The shared bottom subtitle's look, from `VisPlaygroundSettingsPanel.tsx`'s subtitle tab.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SubtitleSettings {
    pub layout: SubtitleLayout,
    pub content_mode: SubtitleContentMode,
    /// `subtitleOverlayOpacity`, 0.2..=1.
    pub opacity: f32,
    pub background: bool,
    pub upcoming_blur: bool,
    pub hide_translation: bool,
    /// `subtitleFontInheritsLyrics`.
    pub font_inherits_lyrics: bool,
    pub font: FontSettings,
    /// `subtitleFontScale`, 0.85..=1.4.
    pub font_scale: f32,
}

impl Default for SubtitleSettings {
    fn default() -> Self {
        Self {
            layout: SubtitleLayout::default(),
            content_mode: SubtitleContentMode::Translation,
            opacity: 0.6,
            background: true,
            upcoming_blur: true,
            hide_translation: false,
            font_inherits_lyrics: true,
            font: FontSettings::default(),
            font_scale: 1.0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AppearanceSettings {
    pub theme: ThemeMode,
    pub frame_rate: FrameRate,
    pub lyrics_font_scale: f32,
    /// Id of the lyric animation style, as the original app names it.
    pub visualizer: String,
    /// `animationIntensity`: how far the lyric animation may go.
    pub animation_intensity: folia_core::AnimationIntensity,
    /// `visualizerBackgroundMode`: a stored mode this build cannot draw reads as `None` and follows the default.
    #[serde(alias = "visualizerBackgroundMode")]
    pub visualizer_background_mode: Option<String>,
    /// `disable_visualizer_vignette` of the common background.
    pub disable_visualizer_vignette: bool,
    /// `disable_visualizer_geometric_background`. Missing in older files means shapes stay on.
    #[serde(default)]
    pub disable_visualizer_geometric_background: bool,
    /// `visualizerOpacity`, 0.2..=1.
    pub visualizer_opacity: f32,
    pub lyrics_font: FontSettings,
    pub subtitle: SubtitleSettings,
}

impl Default for AppearanceSettings {
    fn default() -> Self {
        Self {
            theme: ThemeMode::Dark,
            frame_rate: FrameRate::Fps60,
            lyrics_font_scale: 1.0,
            visualizer: "classic".to_owned(),
            animation_intensity: folia_core::AnimationIntensity::Normal,
            visualizer_background_mode: None,
            disable_visualizer_vignette: false,
            disable_visualizer_geometric_background: false,
            visualizer_opacity: 1.0,
            lyrics_font: FontSettings::default(),
            subtitle: SubtitleSettings::default(),
        }
    }
}

impl AppearanceSettings {
    /// The stored background mode id, when one is saved at all.
    pub fn stored_background_mode(&self) -> Option<&str> {
        self.visualizer_background_mode.as_deref()
    }

    /// Writes the original `visualizerBackgroundMode`; an empty id clears it, so the default follows.
    pub fn set_background_mode(&mut self, mode: Option<&str>) {
        self.visualizer_background_mode = mode.map(str::trim).filter(|mode| !mode.is_empty()).map(str::to_owned);
    }
}

impl Domain for AppearanceSettings {
    const FILE: &'static str = "appearance.toml";
    const SCHEMA_VERSION: u32 = 1;
}
