// Lumiere palette and the dark field laid under every shot. Champagne is the original reference gold.

use folia_core::{Rgb, Theme};

pub const CHAMPAGNE: [f32; 3] = [1.0, 0.86, 0.62];
const UNLIT_COOL: [f32; 3] = [0.62, 0.68, 0.8];
const BRIGHT_BACKGROUND: f32 = 0.18;
pub const BRIGHT_DARK_FLOOR: f32 = 0.94;
const DARK_SHADE: f32 = 0.06;

#[derive(Clone, Copy)]
pub struct Palette {
    pub light: [f32; 3],
    pub lit: [f32; 3],
    pub unlit: [f32; 3],
}

#[derive(Clone, Copy)]
pub struct DarkField {
    pub rgb: [f32; 3],
    pub alpha: f32,
}

pub fn palette(theme: &Theme, theme_mix: f32) -> Palette {
    let mix = theme_mix.clamp(0.0, 1.0);
    let accent = channels(theme.accent);
    let mut light = mix3(mix3(CHAMPAGNE, accent, 0.18), glow_of(accent, CHAMPAGNE), mix);
    let peak = light[0].max(light[1]).max(light[2]).max(1e-3);
    light = scale(light, 1.0 / peak);
    let warm = mix3(light, [1.0, 1.0, 1.0], 0.2);
    let primary = glow_of(channels(theme.primary), warm);
    let secondary = glow_of(channels(theme.secondary), primary);
    Palette {
        light,
        lit: mix3(warm, mix3(primary, [1.0, 1.0, 1.0], 0.2), mix),
        unlit: mix3(mix3(light, UNLIT_COOL, 0.55), mix3(secondary, UNLIT_COOL, 0.35), mix),
    }
}

pub fn dark_field(theme: &Theme, amount: f32) -> DarkField {
    let background = channels(theme.background);
    let strength = if amount.is_finite() { amount.clamp(0.0, 1.0) } else { 0.0 };
    let bright = luminance(background) > BRIGHT_BACKGROUND;
    DarkField {
        rgb: scale(background, DARK_SHADE),
        alpha: if bright { strength.max(BRIGHT_DARK_FLOOR) } else { strength },
    }
}

fn channels(rgb: Rgb) -> [f32; 3] {
    [f32::from(rgb.r) / 255.0, f32::from(rgb.g) / 255.0, f32::from(rgb.b) / 255.0]
}

fn glow_of(rgb: [f32; 3], fallback: [f32; 3]) -> [f32; 3] {
    let peak = rgb[0].max(rgb[1]).max(rgb[2]);
    if peak < 0.04 { fallback } else { scale(rgb, 1.0 / peak) }
}

fn luminance(rgb: [f32; 3]) -> f32 {
    0.2126 * rgb[0] + 0.7152 * rgb[1] + 0.0722 * rgb[2]
}

pub fn mix3(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t]
}

fn scale(rgb: [f32; 3], k: f32) -> [f32; 3] {
    [rgb[0] * k, rgb[1] * k, rgb[2] * k]
}
