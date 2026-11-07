// The Monet poster background from `monetBackgroundPaint.ts`. Call it off the UI and render threads.

use folia_core::{Rgb, Theme};

pub const WIDTH: u32 = 960;
pub const HEIGHT: u32 = 540;

/// Defaults from `DEFAULT_MONET_BACKGROUND_TUNING`. The lyric style does not import that panel.
#[derive(Clone, Copy)]
pub struct Wash {
    pub blur: f32,
    pub overlay: f32,
    pub grayscale: f32,
    pub saturation: f32,
    pub wash: f32,
    pub streaks: bool,
}

impl Default for Wash {
    fn default() -> Self {
        Self { blur: 6.0, overlay: 0.74, grayscale: 0.0, saturation: 1.05, wash: 0.34, streaks: true }
    }
}

/// Cover pixels painted into a `WIDTH`×`HEIGHT` buffer. `source` may be empty: the field is then the theme.
pub fn paint(source: &[u8], source_w: u32, source_h: u32, theme: &Theme, tuning: Wash) -> Vec<u8> {
    let mut pixels = vec![0u8; WIDTH as usize * HEIGHT as usize * 4];
    fill(&mut pixels, theme.background);
    if source_w > 0 && source_h > 0 && source.len() >= source_w as usize * source_h as usize * 4 {
        blit_cover(&mut pixels, source, source_w, source_h);
        box_blur(&mut pixels, tuning.blur.round().clamp(0.0, 12.0) as i32);
    }
    grade(&mut pixels, theme, tuning);
    overlay(&mut pixels, theme, tuning);
    pixels
}

fn fill(pixels: &mut [u8], color: Rgb) {
    for px in pixels.as_chunks_mut::<4>().0 {
        px[0] = color.r;
        px[1] = color.g;
        px[2] = color.b;
        px[3] = 255;
    }
}

fn blit_cover(dest: &mut [u8], source: &[u8], source_w: u32, source_h: u32) {
    let scale = (WIDTH as f32 / source_w as f32).max(HEIGHT as f32 / source_h as f32);
    let draw_w = source_w as f32 * scale;
    let draw_h = source_h as f32 * scale;
    let origin_x = (WIDTH as f32 - draw_w) / 2.0;
    let origin_y = (HEIGHT as f32 - draw_h) / 2.0;
    for y in 0..HEIGHT {
        for x in 0..WIDTH {
            let sx = ((x as f32 - origin_x) / scale).clamp(0.0, source_w as f32 - 1.0) as u32;
            let sy = ((y as f32 - origin_y) / scale).clamp(0.0, source_h as f32 - 1.0) as u32;
            let at = (sy * source_w + sx) as usize * 4;
            let to = (y * WIDTH + x) as usize * 4;
            dest[to..to + 4].copy_from_slice(&source[at..at + 4]);
        }
    }
}

fn box_blur(pixels: &mut [u8], radius: i32) {
    if radius <= 0 {
        return;
    }
    let mut scratch = vec![0u8; pixels.len()];
    blur_axis(pixels, &mut scratch, radius, true);
    blur_axis(&scratch, pixels, radius, false);
}

fn blur_axis(src: &[u8], dest: &mut [u8], radius: i32, horizontal: bool) {
    let (w, h) = (WIDTH as i32, HEIGHT as i32);
    let span = radius * 2 + 1;
    for y in 0..h {
        for x in 0..w {
            let (mut r, mut g, mut b) = (0u32, 0u32, 0u32);
            for delta in -radius..=radius {
                let (sx, sy) =
                    if horizontal { ((x + delta).clamp(0, w - 1), y) } else { (x, (y + delta).clamp(0, h - 1)) };
                let at = (sy as u32 * WIDTH + sx as u32) as usize * 4;
                r += u32::from(src[at]);
                g += u32::from(src[at + 1]);
                b += u32::from(src[at + 2]);
            }
            let to = (y as u32 * WIDTH + x as u32) as usize * 4;
            dest[to] = (r / span as u32) as u8;
            dest[to + 1] = (g / span as u32) as u8;
            dest[to + 2] = (b / span as u32) as u8;
            dest[to + 3] = 255;
        }
    }
}

fn grade(pixels: &mut [u8], theme: &Theme, tuning: Wash) {
    let background = theme.background;
    let accent = theme.accent;
    let primary = theme.primary;
    let (shadow, highlight) =
        if lum(background) <= lum(primary) { (background, primary) } else { (primary, background) };
    for px in pixels.as_chunks_mut::<4>().0 {
        let (mut r, mut g, mut b) = (f32::from(px[0]), f32::from(px[1]), f32::from(px[2]));
        let luminance = 0.2126 * r + 0.7152 * g + 0.0722 * b;
        if tuning.grayscale > 0.0 {
            r = mix(r, luminance, tuning.grayscale);
            g = mix(g, luminance, tuning.grayscale);
            b = mix(b, luminance, tuning.grayscale);
        }
        if (tuning.saturation - 1.0).abs() >= 0.001 {
            let next = 0.2126 * r + 0.7152 * g + 0.0722 * b;
            r = next + (r - next) * tuning.saturation;
            g = next + (g - next) * tuning.saturation;
            b = next + (b - next) * tuning.saturation;
        }
        if tuning.wash > 0.0 {
            let t = (luminance / 255.0).clamp(0.0, 1.0);
            let wash = if t <= 0.55 {
                mix_rgb(shadow, accent, t / 0.55)
            } else {
                mix_rgb(accent, highlight, (t - 0.55) / 0.45)
            };
            r = mix(r, f32::from(wash.r), tuning.wash);
            g = mix(g, f32::from(wash.g), tuning.wash);
            b = mix(b, f32::from(wash.b), tuning.wash);
        }
        px[0] = r.clamp(0.0, 255.0).round() as u8;
        px[1] = g.clamp(0.0, 255.0).round() as u8;
        px[2] = b.clamp(0.0, 255.0).round() as u8;
    }
}

fn overlay(pixels: &mut [u8], theme: &Theme, tuning: Wash) {
    let overlay = tuning.overlay.clamp(0.0, 1.0);
    let w = WIDTH as f32;
    let h = HEIGHT as f32;
    for y in 0..HEIGHT {
        for x in 0..WIDTH {
            let xf = x as f32 / w;
            let yf = y as f32 / h;
            let diag = (xf + yf) / 2.0;
            let mut color = if diag < 0.44 {
                mix_rgb(theme.accent, theme.background, diag / 0.44)
            } else {
                mix_rgb(theme.background, theme.primary, (diag - 0.44) / 0.56)
            };
            let alpha = if diag < 0.44 { 0.08 + overlay * 0.62 } else { 0.16 + overlay * 0.58 };
            let at = (y * WIDTH + x) as usize * 4;
            let base = Rgb { r: pixels[at], g: pixels[at + 1], b: pixels[at + 2] };
            color = mix_rgb(base, color, alpha);
            let dx = xf - 0.18;
            let dy = yf - 0.34;
            let bloom = (-((dx * dx + dy * dy) / (0.55 * 0.55))).exp() * overlay * 0.42;
            color = mix_rgb(color, theme.accent, bloom);
            if xf > 0.45 {
                color = mix_rgb(color, theme.background, (xf - 0.45) / 0.55 * (0.18 + overlay * 0.38));
            }
            if tuning.streaks && (x * 127) % w as u32 == 0 {
                color = mix_rgb(color, theme.background, 0.1 + overlay * 0.08);
            }
            pixels[at] = color.r;
            pixels[at + 1] = color.g;
            pixels[at + 2] = color.b;
        }
    }
}

fn lum(color: Rgb) -> f32 {
    0.2126 * f32::from(color.r) + 0.7152 * f32::from(color.g) + 0.0722 * f32::from(color.b)
}

fn mix(from: f32, to: f32, amount: f32) -> f32 {
    from + (to - from) * amount
}

fn mix_rgb(from: Rgb, to: Rgb, amount: f32) -> Rgb {
    let amount = amount.clamp(0.0, 1.0);
    let channel = |a: u8, b: u8| mix(f32::from(a), f32::from(b), amount).round() as u8;
    Rgb { r: channel(from.r, to.r), g: channel(from.g, to.g), b: channel(from.b, to.b) }
}
