// Nomand's five Paper image effects, original tuning keys, palette rules and edge overscan.

use folia_core::Theme;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Effect {
    Dithering,
    FlutedGlass,
    PaperTexture,
    HalftoneDots,
    LensDistortion,
}
impl Effect {
    pub const ALL: [Self; 5] =
        [Self::Dithering, Self::FlutedGlass, Self::PaperTexture, Self::HalftoneDots, Self::LensDistortion];
    pub fn id(self) -> &'static str {
        match self {
            Self::Dithering => "dithering",
            Self::FlutedGlass => "fluted-glass",
            Self::PaperTexture => "paper-texture",
            Self::HalftoneDots => "halftone-dots",
            Self::LensDistortion => "lens-distortion",
        }
    }
    pub fn index(self) -> usize {
        Self::ALL.iter().position(|value| *value == self).unwrap_or(0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(usize)]
pub enum Number {
    Size,
    ColorSteps,
    GlassSize,
    GlassDistortion,
    GlassBlur,
    PaperContrast,
    PaperRoughness,
    PaperFiber,
    DotsSize,
    DotsRadius,
    DotsContrast,
    LensSpread,
    LensBulge,
    LensDispersion,
    Overlay,
}

pub struct Spec {
    pub field: Number,
    pub key: &'static str,
    pub default: f32,
    pub min: f32,
    pub max: f32,
    pub step: f32,
}
pub const NUMBERS: [Spec; 15] = [
    Spec { field: Number::Size, key: "size", default: 3.0, min: 0.5, max: 20.0, step: 0.5 },
    Spec { field: Number::ColorSteps, key: "colorSteps", default: 4.0, min: 1.0, max: 7.0, step: 1.0 },
    Spec { field: Number::GlassSize, key: "flutedGlassSize", default: 0.5, min: 0.1, max: 1.0, step: 0.05 },
    Spec { field: Number::GlassDistortion, key: "flutedGlassDistortion", default: 0.5, min: 0.0, max: 1.0, step: 0.05 },
    Spec { field: Number::GlassBlur, key: "flutedGlassBlur", default: 0.08, min: 0.0, max: 1.0, step: 0.01 },
    Spec { field: Number::PaperContrast, key: "paperTextureContrast", default: 0.32, min: 0.0, max: 1.0, step: 0.01 },
    Spec { field: Number::PaperRoughness, key: "paperTextureRoughness", default: 0.42, min: 0.0, max: 1.0, step: 0.01 },
    Spec { field: Number::PaperFiber, key: "paperTextureFiber", default: 0.3, min: 0.0, max: 1.0, step: 0.05 },
    Spec { field: Number::DotsSize, key: "halftoneDotsSize", default: 0.5, min: 0.1, max: 1.0, step: 0.05 },
    Spec { field: Number::DotsRadius, key: "halftoneDotsRadius", default: 1.25, min: 0.1, max: 2.0, step: 0.05 },
    Spec { field: Number::DotsContrast, key: "halftoneDotsContrast", default: 0.4, min: 0.0, max: 1.0, step: 0.05 },
    Spec { field: Number::LensSpread, key: "lensDistortionSpread", default: 0.45, min: 0.0, max: 1.0, step: 0.05 },
    Spec { field: Number::LensBulge, key: "lensDistortionBulge", default: 0.3, min: -1.0, max: 1.0, step: 0.05 },
    Spec {
        field: Number::LensDispersion,
        key: "lensDistortionDispersion",
        default: 0.65,
        min: 0.0,
        max: 1.0,
        step: 0.05,
    },
    Spec { field: Number::Overlay, key: "overlayOpacity", default: 0.35, min: 0.0, max: 1.0, step: 0.05 },
];

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Tuning {
    pub uploaded: bool,
    pub effect: Effect,
    pub dither: u8,
    pub original: bool,
    pub inverted: bool,
    pub dots_original: bool,
    pub dots_inverted: bool,
    pub overlay: bool,
    numbers: [f32; 15],
}

impl Default for Tuning {
    fn default() -> Self {
        Self {
            uploaded: false,
            effect: Effect::Dithering,
            dither: 8,
            original: false,
            inverted: false,
            dots_original: false,
            dots_inverted: false,
            overlay: true,
            numbers: std::array::from_fn(|i| NUMBERS[i].default),
        }
    }
}

impl Tuning {
    pub fn number(self, field: Number) -> f32 {
        self.numbers[field as usize]
    }
    pub fn set(&mut self, field: Number, value: f32) {
        let spec = &NUMBERS[field as usize];
        self.numbers[field as usize] = if value.is_finite() { value.clamp(spec.min, spec.max) } else { spec.default };
    }
    pub fn read(value: Option<&toml::Value>) -> Self {
        let mut result = Self::default();
        let Some(toml::Value::Table(table)) = value else { return result };
        for spec in &NUMBERS {
            if let Some(value) =
                table.get(spec.key).and_then(|v| v.as_float().or_else(|| v.as_integer().map(|v| v as f64)))
            {
                result.set(spec.field, value as f32);
            }
        }
        let text = |key| table.get(key).and_then(toml::Value::as_str);
        let flag = |key, default| table.get(key).and_then(toml::Value::as_bool).unwrap_or(default);
        result.uploaded = text("imageSource") == Some("uploaded-global");
        result.effect =
            Effect::ALL.into_iter().find(|effect| Some(effect.id()) == text("effect")).unwrap_or(Effect::Dithering);
        result.dither = match text("ditheringType") {
            Some("2x2") => 2,
            Some("4x4") => 4,
            _ => 8,
        };
        result.original = flag("originalColors", false);
        result.inverted = flag("inverted", false);
        result.dots_original = flag("halftoneDotsOriginalColors", false);
        result.dots_inverted = flag("halftoneDotsInverted", false);
        result.overlay = flag("overlayEnabled", true);
        result
    }
    pub fn table(self) -> toml::map::Map<String, toml::Value> {
        let mut table = toml::map::Map::new();
        for spec in &NUMBERS {
            table.insert(spec.key.to_owned(), toml::Value::Float(f64::from(self.number(spec.field))));
        }
        for (key, value) in [
            ("originalColors", self.original),
            ("inverted", self.inverted),
            ("halftoneDotsOriginalColors", self.dots_original),
            ("halftoneDotsInverted", self.dots_inverted),
            ("overlayEnabled", self.overlay),
        ] {
            table.insert(key.to_owned(), toml::Value::Boolean(value));
        }
        for (key, value) in [
            ("imageSource", if self.uploaded { "uploaded-global" } else { "cover-derived" }),
            ("effect", self.effect.id()),
            (
                "ditheringType",
                match self.dither {
                    2 => "2x2",
                    4 => "4x4",
                    _ => "8x8",
                },
            ),
        ] {
            table.insert(key.to_owned(), toml::Value::String(value.to_owned()));
        }
        table
    }
}

/// Uniform order is recorded beside the translated shaders in uniforms.json.
pub fn uniforms(t: Tuning, theme: &Theme, size: [f32; 2], aspect: f32, daylight: bool, image: bool) -> [[f32; 4]; 72] {
    let mut p = [[0.0; 4]; 72];
    let rgb = |c: folia_core::Rgb| [f32::from(c.r) / 255.0, f32::from(c.g) / 255.0, f32::from(c.b) / 255.0, 1.0];
    p[0] = [size[0], size[1], 0.0, 0.0];
    p[11] = rgb(theme.accent);
    p[12] = rgb(theme.background);
    p[13] = rgb(theme.primary);
    p[20] = rgb(theme.secondary);
    for (i, v) in [(1, 1.0), (2, 0.5), (3, 0.5), (6, 2.0), (7, 1.0), (14, aspect)] {
        p[i][0] = v;
    }
    match t.effect {
        Effect::Dithering => {
            for (i, v) in [
                (
                    15,
                    match t.dither {
                        2 => 2.0,
                        4 => 3.0,
                        _ => 4.0,
                    },
                ),
                (16, t.number(Number::Size)),
                (17, f32::from(t.original)),
                (18, f32::from(t.inverted ^ (daylight && !t.original))),
                (19, t.number(Number::ColorSteps)),
            ] {
                p[i][0] = v;
            }
        }
        Effect::FlutedGlass => {
            for (i, v) in [
                (21, t.number(Number::GlassSize)),
                (22, 0.25),
                (25, 1.0),
                (26, t.number(Number::GlassDistortion)),
                (27, 0.1),
                (28, 1.0),
                (30, t.number(Number::GlassBlur)),
                (31, 0.25),
            ] {
                p[i][0] = v;
            }
        }
        Effect::PaperTexture => {
            for (i, v) in [
                (7, paper_overscan(t.number(Number::PaperRoughness), t.number(Number::PaperFiber))),
                (38, t.number(Number::PaperContrast)),
                (39, t.number(Number::PaperRoughness)),
                (40, t.number(Number::PaperFiber)),
                (41, 0.2),
                (42, 0.35),
                (43, 0.35),
                (44, 0.65),
                (45, 5.0),
                (46, 0.2),
                (47, 5.8),
            ] {
                p[i][0] = v;
            }
        }
        Effect::HalftoneDots => {
            for (i, v) in [
                (15, 1.0),
                (17, f32::from(t.dots_original)),
                (18, f32::from(!t.dots_inverted ^ (daylight && !t.dots_original))),
                (21, t.number(Number::DotsSize)),
                (36, 0.12),
                (37, 0.06),
                (38, t.number(Number::DotsContrast)),
                (50, t.number(Number::DotsRadius)),
                (51, 0.5),
                (52, 1.0),
            ] {
                p[i][0] = v;
            }
        }
        Effect::LensDistortion => {
            for (i, v) in [
                (7, lens_overscan(t.number(Number::LensBulge), t.number(Number::LensSpread))),
                (53, t.number(Number::LensSpread)),
                (54, 1.0),
                (55, 0.4),
                (56, 20.0),
                (57, t.number(Number::LensDispersion)),
                (59, 0.6),
                (60, 0.55),
                (61, 0.8),
                (62, 0.08),
                (64, 0.25),
                (66, t.number(Number::LensBulge)),
                (67, 0.1),
            ] {
                p[i][0] = v;
            }
        }
    }
    p[71] = [f32::from(image), if t.overlay { t.number(Number::Overlay) } else { 0.0 }, 0.0, 0.0];
    p
}

fn margin(scale: f32) -> f32 {
    if scale <= 1.0 { 1.0 } else { (scale + 0.01).min(1.8) }
}
pub fn paper_overscan(roughness: f32, fiber: f32) -> f32 {
    let normal = 2.0 * 0.65 + 1.5 * 0.35 + 0.2 * 0.2 + 0.75 * roughness.clamp(0.0, 1.0) + 0.1 * fiber.clamp(0.0, 1.0);
    margin(1.0 / (1.0 - 2.0 * (0.02 * normal).clamp(0.0, 0.45)))
}
pub fn lens_overscan(bulge: f32, spread: f32) -> f32 {
    let radius = (1.0_f32 + (16.0_f32 / 9.0).powi(2)).sqrt() * 0.5;
    let spread = spread.clamp(0.0, 1.0);
    let headroom = (1.0 - 0.7 * spread.powf(1.3 + 2.7 * spread) * 0.2 / radius).max(0.35);
    if bulge <= 0.0 {
        return margin(1.0 / headroom);
    }
    let amount = bulge.min(1.0) * 1.4;
    let push = |scale: f32| {
        let corner = 2.0 * radius / scale;
        (corner * amount).tan() / amount.tan() / corner
    };
    let mut low = (2.0 * radius * amount / 1.45).max(1.0);
    if low >= 1.8 || push(1.8) > 1.8 * headroom {
        return 1.8;
    }
    if low == 1.0 && push(1.0) <= headroom {
        return margin(1.0 / headroom);
    }
    let mut high = 1.8;
    for _ in 0..32 {
        let middle = (low + high) * 0.5;
        if push(middle) > middle * headroom {
            low = middle;
        } else {
            high = middle;
        }
    }
    margin(high)
}
