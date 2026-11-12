// Monet's static image treatment and 240-second drift from monetBackgroundPaint.ts and monetBackgroundDrift.ts.

use folia_core::{Rgb, Theme};
use iced::widget::image::Handle;
use image::{Rgba, RgbaImage};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Tuning {
    pub uploaded: bool,
    pub half: bool,
    pub blur: f32,
    pub overlay: f32,
    pub grayscale: f32,
    pub saturation: f32,
    pub wash: f32,
    pub offset: f32,
    pub custom_wash: bool,
    pub wash_color: Rgb,
    pub drift: bool,
    pub drift_strength: f32,
    pub streaks: bool,
}

impl Tuning {
    pub const DEFAULT: Self = Self {
        uploaded: false,
        half: false,
        blur: 6.0,
        overlay: 0.74,
        grayscale: 0.0,
        saturation: 1.05,
        wash: 0.34,
        offset: 0.0,
        custom_wash: false,
        wash_color: Rgb::hex(0x8fb7ff),
        drift: true,
        drift_strength: 0.5,
        streaks: true,
    };

    pub fn read(value: Option<&toml::Value>) -> Self {
        let Some(toml::Value::Table(table)) = value else { return Self::DEFAULT };
        let number = |key: &str, default: f32, min: f32, max: f32| {
            let value = table.get(key).and_then(|v| v.as_float().or_else(|| v.as_integer().map(|v| v as f64)));
            value.filter(|v| v.is_finite()).map_or(default, |v| (v as f32).clamp(min, max))
        };
        let flag = |key: &str, default| table.get(key).and_then(toml::Value::as_bool).unwrap_or(default);
        let choice = |key: &str, selected: &str| table.get(key).and_then(toml::Value::as_str) == Some(selected);
        let wash_color = table
            .get("backgroundWashCustomColor")
            .and_then(toml::Value::as_str)
            .and_then(parse_color)
            .unwrap_or(Self::DEFAULT.wash_color);
        Self {
            uploaded: choice("backgroundSource", "uploaded-global"),
            half: choice("backgroundLayout", "half-pane-gradient"),
            blur: number("backgroundBlurPx", 6.0, 0.0, 60.0),
            overlay: number("backgroundOverlayOpacity", 0.74, 0.0, 1.0),
            grayscale: number("backgroundGrayscale", 0.0, 0.0, 1.0),
            saturation: number("backgroundSaturation", 1.05, 0.0, 2.0),
            wash: number("backgroundWash", 0.34, 0.0, 1.0),
            offset: number("backgroundHalfPaneOffsetX", 0.0, -40.0, 40.0),
            custom_wash: choice("backgroundWashColorMode", "custom"),
            wash_color,
            drift: flag("backgroundDriftEnabled", true),
            drift_strength: number("backgroundDriftStrength", 0.5, 0.0, 1.0),
            streaks: flag("backgroundStreaksEnabled", true),
        }
    }

    pub fn table(self) -> toml::map::Map<String, toml::Value> {
        let mut table = toml::map::Map::new();
        for (key, value) in [
            ("backgroundBlurPx", self.blur),
            ("backgroundOverlayOpacity", self.overlay),
            ("backgroundGrayscale", self.grayscale),
            ("backgroundSaturation", self.saturation),
            ("backgroundWash", self.wash),
            ("backgroundHalfPaneOffsetX", self.offset),
            ("backgroundDriftStrength", self.drift_strength),
        ] {
            table.insert(key.to_owned(), toml::Value::Float(f64::from(value)));
        }
        for (key, value) in [("backgroundDriftEnabled", self.drift), ("backgroundStreaksEnabled", self.streaks)] {
            table.insert(key.to_owned(), toml::Value::Boolean(value));
        }
        for (key, value) in [
            ("backgroundSource", if self.uploaded { "uploaded-global" } else { "cover-derived" }),
            ("backgroundLayout", if self.half { "half-pane-gradient" } else { "full-overlay" }),
            ("backgroundWashColorMode", if self.custom_wash { "custom" } else { "theme" }),
        ] {
            table.insert(key.to_owned(), toml::Value::String(value.to_owned()));
        }
        table.insert(
            "backgroundWashCustomColor".to_owned(),
            toml::Value::String(format!(
                "#{:02x}{:02x}{:02x}",
                self.wash_color.r, self.wash_color.g, self.wash_color.b
            )),
        );
        table
    }
}

pub fn parse_color(value: &str) -> Option<Rgb> {
    let value = value.trim().trim_start_matches('#');
    (value.len() == 6).then(|| u32::from_str_radix(value, 16).ok()).flatten().map(Rgb::hex)
}

fn channels(rgb: Rgb) -> [f32; 3] {
    [f32::from(rgb.r), f32::from(rgb.g), f32::from(rgb.b)]
}
fn luma(c: [f32; 3]) -> f32 {
    c[0] * 0.2126 + c[1] * 0.7152 + c[2] * 0.0722
}
fn mix(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    std::array::from_fn(|i| a[i] + (b[i] - a[i]) * t)
}

/// The original three-stop watercolour remapping, performed before the overlay gradients.
pub fn wash_color(luminance: f32, theme: &Theme, accent: Rgb) -> [f32; 3] {
    let (bg, primary, accent) = (channels(theme.background), channels(theme.primary), channels(accent));
    let (shadow, highlight) = if luma(bg) <= luma(primary) { (bg, primary) } else { (primary, bg) };
    if luminance <= 0.55 {
        mix(shadow, accent, luminance / 0.55)
    } else {
        mix(accent, highlight, (luminance - 0.55) / 0.45)
    }
}

/// Runs on a background worker. The original always paints a 1920×1080 poster before CSS layout.
pub fn paint(source: &RgbaImage, theme: &Theme, tuning: Tuning) -> Handle {
    const WIDTH: u32 = 1920;
    const HEIGHT: u32 = 1080;
    if source.width() == 0 || source.height() == 0 {
        return Handle::from_rgba(1, 1, vec![theme.background.r, theme.background.g, theme.background.b, 255]);
    }
    let padding = (tuning.blur * 3.0).ceil() as u32 + 2;
    let ratio = (WIDTH as f64 / f64::from(source.width())).max(HEIGHT as f64 / f64::from(source.height()));
    let offset_x = (f64::from(WIDTH) - f64::from(source.width()) * ratio) * 0.5;
    let offset_y = (f64::from(HEIGHT) - f64::from(source.height()) * ratio) * 0.5;
    let mut padded = RgbaImage::from_fn(WIDTH + padding * 2, HEIGHT + padding * 2, |x, y| {
        let sx = (f64::from(x) - f64::from(padding) + 0.5 - offset_x) / ratio;
        let sy = (f64::from(y) - f64::from(padding) + 0.5 - offset_y) / ratio;
        if sx < 0.0 || sy < 0.0 || sx >= f64::from(source.width()) || sy >= f64::from(source.height()) {
            return Rgba([0; 4]);
        }
        let sx = (sx - 0.5).clamp(0.0, f64::from(source.width() - 1));
        let sy = (sy - 0.5).clamp(0.0, f64::from(source.height() - 1));
        let (x0, y0) = (sx.floor() as u32, sy.floor() as u32);
        let (x1, y1) = ((x0 + 1).min(source.width() - 1), (y0 + 1).min(source.height() - 1));
        let pixels =
            [source.get_pixel(x0, y0), source.get_pixel(x1, y0), source.get_pixel(x0, y1), source.get_pixel(x1, y1)];
        let weights = [
            (1.0 - sx.fract()) * (1.0 - sy.fract()),
            sx.fract() * (1.0 - sy.fract()),
            (1.0 - sx.fract()) * sy.fract(),
            sx.fract() * sy.fract(),
        ];
        Rgba(std::array::from_fn(|channel| {
            pixels
                .iter()
                .zip(weights)
                .map(|(p, weight)| {
                    let value = if channel == 3 {
                        f64::from(p[channel])
                    } else {
                        f64::from(p[channel]) * f64::from(p[3]) / 255.0
                    };
                    value * weight
                })
                .sum::<f64>()
                .round() as u8
        }))
    });
    if tuning.blur > 0.0 {
        padded = image::imageops::fast_blur(&padded, tuning.blur);
    }
    let mut picture = image::imageops::crop_imm(&padded, padding, padding, WIDTH, HEIGHT).to_image();
    let bg = channels(theme.background);
    let accent = channels(theme.accent);
    let primary = channels(theme.primary);
    for (x, y, pixel) in picture.enumerate_pixels_mut() {
        let original = [f32::from(pixel[0]), f32::from(pixel[1]), f32::from(pixel[2])];
        let original = std::array::from_fn(|i| original[i] + bg[i] * (1.0 - f32::from(pixel[3]) / 255.0));
        let luminance = luma(original);
        let gray = mix(original, [luminance; 3], tuning.grayscale);
        let colored = mix([luma(gray); 3], gray, tuning.saturation);
        let wash = wash_color(
            (luminance / 255.0).clamp(0.0, 1.0),
            theme,
            if tuning.custom_wash { tuning.wash_color } else { theme.accent },
        );
        let mut rgb = mix(colored, wash, tuning.wash).map(|v| v.round().clamp(0.0, 255.0));
        let t = (x as f32 * WIDTH as f32 + y as f32 * HEIGHT as f32) / (WIDTH * WIDTH + HEIGHT * HEIGHT) as f32;
        let (left, right, alpha_left, alpha_right, at) = if t < 0.44 {
            (accent, bg, 0.08 + tuning.overlay * 0.62, 0.16 + tuning.overlay * 0.58, t / 0.44)
        } else {
            (bg, primary, 0.16 + tuning.overlay * 0.58, 0.06 + tuning.overlay * 0.36, (t - 0.44) / 0.56)
        };
        let alpha = alpha_left + (alpha_right - alpha_left) * at;
        rgb = mix(rgb, mix(left, right, at), alpha);
        let radius = ((x as f32 - WIDTH as f32 * 0.18).powi(2) + (y as f32 - HEIGHT as f32 * 0.34).powi(2)).sqrt();
        let radial = (radius / (WIDTH as f32 * 0.55)).clamp(0.0, 1.0);
        rgb = mix(rgb, mix(accent, bg, radial), tuning.overlay * 0.42 * (1.0 - radial));
        rgb = mix(rgb, bg, (0.18 + tuning.overlay * 0.38) * ((x as f32 / WIDTH as f32 - 0.45) / 0.55).clamp(0.0, 1.0));
        if tuning.streaks
            && (0..18).any(|i| {
                x == (i * 127) % WIDTH
                    && (y as i32) >= ((i * 211) % HEIGHT) as i32 - 40
                    && (y as i32) < ((i * 211) % HEIGHT) as i32 - 40 + (HEIGHT as f32 * 0.28) as i32
            })
        {
            rgb = mix(rgb, bg, 0.1 + tuning.overlay * 0.08);
        }
        *pixel = Rgba([rgb[0].round() as u8, rgb[1].round() as u8, rgb[2].round() as u8, 255]);
    }
    Handle::from_rgba(WIDTH, HEIGHT, picture.into_raw())
}

/// Original sampled drift. Values are translation as fractions of the box and its uniform scale.
pub struct Drift([[f64; 3]; 157]);

impl Default for Drift {
    fn default() -> Self {
        let mut samples = [[0.0; 3]; 157];
        for (i, sample) in samples.iter_mut().enumerate() {
            let t = 76.0 * i as f64 / 156.0;
            for (channel, seed) in [12.9898, 78.233, 39.425].into_iter().enumerate() {
                sample[channel] =
                    (noise(t, 76.0, seed) * 0.68 + noise(t * 2.0, 152.0, seed + 17.31) * 0.32) * 2.0 - 1.0;
            }
        }
        for channel in 0..3 {
            let peak = samples.iter().map(|p| p[channel].abs()).fold(0.0, f64::max);
            for sample in &mut samples {
                sample[channel] /= peak.max(f64::EPSILON);
            }
        }
        Self(samples)
    }
}

impl Drift {
    pub fn at(&self, seconds: f64, strength: f32) -> [f32; 3] {
        let phase = seconds.rem_euclid(240.0) / 240.0 * 156.0;
        let index = phase.floor() as usize;
        let blend = phase.fract();
        let sample: [f64; 3] =
            std::array::from_fn(|i| self.0[index][i] + (self.0[index + 1][i] - self.0[index][i]) * blend);
        let strength = strength.clamp(0.0, 1.0);
        [
            sample[0] as f32 * 0.034 * strength,
            sample[1] as f32 * 0.034 * strength,
            1.01 + 0.068 * strength + 0.035 * strength * (sample[2] as f32 + 1.0) / 2.0,
        ]
    }
}

fn noise(t: f64, points: f64, seed: f64) -> f64 {
    let hash = |cell: f64| {
        let value = (cell.rem_euclid(points) * 127.1 + seed * 311.7).sin() * 43758.5453;
        value - value.floor()
    };
    let f = t.fract();
    hash(t.floor()) + (hash(t.floor() + 1.0) - hash(t.floor())) * f * f * (3.0 - 2.0 * f)
}
