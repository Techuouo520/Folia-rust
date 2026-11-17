// Original built-in dual-theme generator, retaining its hue schemes, OKLCH accents and contrast floors.

use folia_core::Rgb;
use folia_store::{DualTheme, StoredTheme};

use crate::color;

#[derive(Clone, Copy)]
struct Palette {
    hue: f64,
    saturation: f64,
    chroma: f64,
    accent: f64,
    tone: usize,
}

struct Random(u64);

impl Random {
    fn next(&mut self) -> f64 {
        self.0 = self.0.wrapping_add(0x9e3779b97f4a7c15);
        let mut value = self.0;
        value = (value ^ (value >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
        value = (value ^ (value >> 27)).wrapping_mul(0x94d049bb133111eb);
        ((value ^ (value >> 31)) >> 11) as f64 / ((1_u64 << 53) as f64)
    }

    fn between(&mut self, min: f64, max: f64) -> f64 {
        min + self.next() * (max - min)
    }

    fn weighted(&mut self, weights: &[f64]) -> usize {
        let mut cursor = self.next() * weights.iter().sum::<f64>();
        for (index, weight) in weights.iter().enumerate() {
            cursor -= weight;
            if cursor <= 0.0 {
                return index;
            }
        }
        weights.len() - 1
    }
}

pub fn cover_theme(colors: &[Rgb], seed: u64) -> DualTheme {
    let mut random = Random(seed);
    let palette = palette(colors, &mut random);
    let names = crate::palette_words::names(palette.hue, palette.saturation, || random.next());
    let descriptions = crate::palette_words::descriptions(palette.hue, palette.saturation, || random.next());
    DualTheme {
        light: mode(palette, false, names.0, descriptions.0, &mut random),
        dark: mode(palette, true, names.1, descriptions.1, &mut random),
    }
}

fn palette(colors: &[Rgb], random: &mut Random) -> Palette {
    let parsed: Vec<_> = colors.iter().map(|color| (color::hsl(*color), color::lch(*color)[1])).collect();
    let ([hue, saturation, _], chroma) = parsed
        .iter()
        .find(|([_, _, light], chroma)| chroma * (1.0 - (light - 0.5).abs() * 1.2).max(0.0) > 0.012)
        .copied()
        .or_else(|| parsed.first().copied())
        .unwrap_or_else(|| {
            let hsl = [random.next() * 360.0, random.between(0.35, 0.65), 0.5];
            (hsl, color::lch(color::from_hsl(hsl))[1])
        });
    let support = parsed
        .iter()
        .find(|([h, _, _], c)| *c >= 0.035 && (25.0..=160.0).contains(&color::hue_distance(*h, hue)))
        .map(|(hsl, _)| hsl[0]);
    let scheme = if support.is_some() && random.next() < 0.8 {
        5
    } else {
        random.weighted(&[3.0, 2.0, 2.0, 1.5, if saturation < 0.18 { 2.5 } else { 0.0 }])
    };
    let jitter = if scheme == 4 { 0.0 } else { random.between(-8.0, 8.0) };
    let sign = if random.next() < 0.5 { -1.0 } else { 1.0 };
    let accent = match scheme {
        0 => hue + sign * random.between(20.0, 35.0),
        1 => hue + random.between(150.0, 180.0),
        2 => hue + if sign < 0.0 { 150.0 } else { 210.0 },
        3 => hue + sign * 120.0,
        5 => support.unwrap_or(hue),
        _ => hue,
    };
    Palette {
        hue,
        saturation,
        chroma,
        accent: (accent + jitter).rem_euclid(360.0),
        tone: random.weighted(&[3.0, 4.0, 2.5]),
    }
}

fn mode(palette: Palette, dark: bool, name: String, description: String, random: &mut Random) -> StoredTheme {
    let (light, saturation) = match (dark, palette.tone) {
        (true, 0) => ((0.055, 0.085), (0.10, 0.22)),
        (true, 1) => ((0.075, 0.110), (0.20, 0.36)),
        (true, _) => ((0.095, 0.140), (0.32, 0.50)),
        (false, 0) => ((0.940, 0.965), (0.05, 0.12)),
        (false, 1) => ((0.920, 0.950), (0.10, 0.20)),
        (false, _) => ((0.900, 0.935), (0.18, 0.30)),
    };
    let weight = 0.55 + 0.45 * (palette.saturation / 0.7).clamp(0.0, 1.0);
    let background = color::from_hsl([
        palette.hue,
        (random.between(saturation.0, saturation.1) * weight).max(0.04),
        random.between(light.0, light.1),
    ]);
    let primary = color::solve_hsl(
        [
            palette.hue,
            if dark { random.between(0.1, 0.3) } else { random.between(0.15, 0.4) },
            if dark { random.between(0.90, 0.96) } else { random.between(0.10, 0.20) },
        ],
        background,
        9.0,
    );
    let accent = accent(palette, dark, background, primary, random);
    let hue = palette.hue + ((palette.accent - palette.hue + 540.0) % 360.0 - 180.0) * random.between(0.2, 0.6);
    let lower = if dark { 0.62 } else { 0.32 };
    let light = random.between(lower, if dark { 0.76 } else { 0.46 });
    let mut secondary = color::solve_hsl([hue, random.between(0.15, 0.45), light], background, 4.5);
    if color::distance(secondary, accent) < 0.07 {
        secondary = color::solve_hsl([hue, 0.04, light], background, 4.5);
        if color::distance(secondary, accent) < 0.07 {
            secondary = color::solve_hsl([hue, 0.04, lower], background, 4.5);
        }
    }
    StoredTheme {
        name,
        background,
        primary,
        accent,
        secondary,
        description: Some(description),
        word_colors: Vec::new(),
        lyrics_icons: Vec::new(),
    }
}

fn accent(palette: Palette, dark: bool, background: Rgb, primary: Rgb, random: &mut Random) -> Rgb {
    let light = if dark { random.between(0.74, 0.82) } else { random.between(0.52, 0.60) };
    let (min, max) = if dark { (0.07, 0.13) } else { (0.08, 0.15) };
    let chroma = (min + (max - min) * (palette.chroma / 0.2).clamp(0.0, 1.0)) * random.between(0.9, 1.05);
    let solve = |hue| {
        let hue = color::lch(color::from_hsl([hue, 0.9, 0.5]))[2];
        let hue = if dark { hue } else { steer(hue) };
        color::solve_lch([light, chroma, hue], background, 3.2)
    };
    let color = solve(palette.accent);
    if color::hue_distance(palette.accent, palette.hue) < 15.0 && color::contrast(color, primary) < 1.3 {
        solve(palette.accent + 40.0)
    } else {
        color
    }
}

fn steer(hue: f64) -> f64 {
    let smooth = |low: f64, high: f64| {
        let value = ((hue - low) / (high - low)).clamp(0.0, 1.0);
        value * value * (3.0 - 2.0 * value)
    };
    if hue < 118.0 {
        hue + (70.0 - hue) * 0.75 * smooth(78.0, 92.0)
    } else {
        hue + (150.0 - hue) * 0.75 * (1.0 - smooth(134.0, 148.0))
    }
}
