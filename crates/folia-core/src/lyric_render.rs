// Resolution and antialiasing preferences shared by lyric settings and native renderers.

use serde::{Deserialize, Serialize};

pub const LYRIC_RENDER_SCALES: [f32; 5] = [0.5, 0.75, 1.0, 1.5, 2.0];

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LyricAntialias {
    #[default]
    Auto,
    On,
    Off,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct LyricRenderQuality {
    pub scale: f32,
    pub antialias: LyricAntialias,
}

impl Default for LyricRenderQuality {
    fn default() -> Self {
        Self { scale: 1.0, antialias: LyricAntialias::Auto }
    }
}

impl LyricRenderQuality {
    pub fn normalized(self) -> Self {
        Self { scale: if LYRIC_RENDER_SCALES.contains(&self.scale) { self.scale } else { 1.0 }, ..self }
    }

    /// Preserves the original density at 1x, including displays above the supersampling cap.
    pub fn pixel_ratio(self, base: f32) -> f32 {
        let base = if base.is_finite() && base > 0.0 { base } else { 1.0 };
        let scale = self.normalized().scale;
        let ratio = base * scale;
        if scale > 1.0 { ratio.min(3.0).max(base) } else { ratio }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct LyricRenderOverride {
    pub scale: Option<f32>,
    pub antialias: Option<LyricAntialias>,
}

impl LyricRenderOverride {
    pub fn resolve(self, global: LyricRenderQuality) -> LyricRenderQuality {
        LyricRenderQuality {
            scale: self.scale.filter(|scale| LYRIC_RENDER_SCALES.contains(scale)).unwrap_or(global.scale),
            antialias: self.antialias.filter(|mode| *mode != LyricAntialias::Auto).unwrap_or(global.antialias),
        }
        .normalized()
    }
}
