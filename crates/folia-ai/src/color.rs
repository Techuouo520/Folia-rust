// HSL, OKLab and contrast math ported from the original themeColorMath and oklab modules.

use folia_core::Rgb;

pub fn hsl(color: Rgb) -> [f64; 3] {
    let [r, g, b] = [color.r, color.g, color.b].map(|v| f64::from(v) / 255.0);
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let delta = max - min;
    let light = (max + min) / 2.0;
    if delta == 0.0 {
        return [0.0, 0.0, light];
    }
    let hue = if max == r {
        (g - b) / delta
    } else if max == g {
        (b - r) / delta + 2.0
    } else {
        (r - g) / delta + 4.0
    };
    [(hue * 60.0).rem_euclid(360.0), delta / (1.0 - (2.0 * light - 1.0).abs()), light]
}

pub fn from_hsl([hue, saturation, lightness]: [f64; 3]) -> Rgb {
    let hue = hue.rem_euclid(360.0);
    let light = lightness.clamp(0.0, 1.0);
    let chroma = (1.0 - (2.0 * light - 1.0).abs()) * saturation.clamp(0.0, 1.0);
    let second = chroma * (1.0 - ((hue / 60.0) % 2.0 - 1.0).abs());
    let offset = light - chroma / 2.0;
    let channels = match (hue / 60.0) as u32 {
        0 => [chroma, second, 0.0],
        1 => [second, chroma, 0.0],
        2 => [0.0, chroma, second],
        3 => [0.0, second, chroma],
        4 => [second, 0.0, chroma],
        _ => [chroma, 0.0, second],
    }
    .map(|value| ((value + offset) * 255.0).round().clamp(0.0, 255.0) as u8);
    Rgb { r: channels[0], g: channels[1], b: channels[2] }
}

fn linear(channel: u8) -> f64 {
    let value = f64::from(channel) / 255.0;
    if value <= 0.04045 { value / 12.92 } else { ((value + 0.055) / 1.055).powf(2.4) }
}

pub fn lab(color: Rgb) -> [f64; 3] {
    let [r, g, b] = [color.r, color.g, color.b].map(linear);
    let l = (0.4122214708 * r + 0.5363325363 * g + 0.0514459929 * b).cbrt();
    let m = (0.2119034982 * r + 0.6806995451 * g + 0.1073969566 * b).cbrt();
    let s = (0.0883024619 * r + 0.2817188376 * g + 0.6299787005 * b).cbrt();
    [
        0.2104542553 * l + 0.7936177850 * m - 0.0040720468 * s,
        1.9779984951 * l - 2.4285922050 * m + 0.4505937099 * s,
        0.0259040371 * l + 0.7827717662 * m - 0.8086757660 * s,
    ]
}

pub fn lch(color: Rgb) -> [f64; 3] {
    let [light, a, b] = lab(color);
    [light, a.hypot(b), b.atan2(a).to_degrees().rem_euclid(360.0)]
}

fn linear_lch([light, chroma, hue]: [f64; 3]) -> [f64; 3] {
    let a = chroma * hue.to_radians().cos();
    let b = chroma * hue.to_radians().sin();
    let l = (light + 0.3963377774 * a + 0.2158037573 * b).powi(3);
    let m = (light - 0.1055613458 * a - 0.0638541728 * b).powi(3);
    let s = (light - 0.0894841775 * a - 1.2914855480 * b).powi(3);
    [
        4.0767416621 * l - 3.3077115913 * m + 0.2309699292 * s,
        -1.2684380046 * l + 2.6097574011 * m - 0.3413193965 * s,
        -0.0041960863 * l - 0.7034186147 * m + 1.7076147010 * s,
    ]
}

fn in_gamut(channels: [f64; 3]) -> bool {
    channels.iter().all(|value| (-0.0001..=1.0001).contains(value))
}

pub fn from_lch([light, chroma, hue]: [f64; 3]) -> Rgb {
    let light = light.clamp(0.0, 1.0);
    let mut high = chroma.max(0.0);
    let mut low = 0.0;
    let channels = if in_gamut(linear_lch([light, high, hue])) {
        linear_lch([light, high, hue])
    } else {
        for _ in 0..16 {
            let middle = (low + high) / 2.0;
            if in_gamut(linear_lch([light, middle, hue])) {
                low = middle;
            } else {
                high = middle;
            }
        }
        linear_lch([light, low, hue])
    }
    .map(|value| {
        let value = value.clamp(0.0, 1.0);
        let value = if value <= 0.0031308 { value * 12.92 } else { 1.055 * value.powf(1.0 / 2.4) - 0.055 };
        (value * 255.0).round() as u8
    });
    Rgb { r: channels[0], g: channels[1], b: channels[2] }
}

pub fn luminance(color: Rgb) -> f64 {
    0.2126 * linear(color.r) + 0.7152 * linear(color.g) + 0.0722 * linear(color.b)
}

pub fn contrast(a: Rgb, b: Rgb) -> f64 {
    let a = luminance(a);
    let b = luminance(b);
    (a.max(b) + 0.05) / (a.min(b) + 0.05)
}

pub fn distance(a: Rgb, b: Rgb) -> f64 {
    let a = lab(a);
    let b = lab(b);
    ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt()
}

pub fn hue_distance(a: f64, b: f64) -> f64 {
    let difference = (a.rem_euclid(360.0) - b.rem_euclid(360.0)).abs();
    difference.min(360.0 - difference)
}

pub fn solve_hsl(value: [f64; 3], background: Rgb, ratio: f64) -> Rgb {
    let value = hsl(from_hsl(value));
    let direction = if luminance(background) > 0.35 { -1.0 } else { 1.0 };
    solve(background, ratio, 51, |step| from_hsl([value[0], value[1], value[2] + direction * 0.02 * f64::from(step)]))
}

pub fn solve_lch(value: [f64; 3], background: Rgb, ratio: f64) -> Rgb {
    let direction = if luminance(background) > 0.35 { -1.0 } else { 1.0 };
    solve(background, ratio, 101, |step| from_lch([value[0] + direction * 0.01 * f64::from(step), value[1], value[2]]))
}

fn solve(background: Rgb, ratio: f64, steps: u32, candidate: impl Fn(u32) -> Rgb) -> Rgb {
    let mut best = candidate(0);
    let mut best_ratio = contrast(best, background);
    for step in 0..steps {
        let current = candidate(step);
        let current_ratio = contrast(current, background);
        if current_ratio > best_ratio {
            best = current;
            best_ratio = current_ratio;
        }
        if current_ratio >= ratio {
            return current;
        }
    }
    best
}
