// The window's light marks as coverage images: soft dot, four-point star (light/sprites.ts) and a track stroke.

use folia_render::glyphs::Image;

pub const DOT: u16 = 0;
pub const STAR: u16 = 1;
pub const TRACK: u16 = 2;
/// The track image spans this many stroke widths across, so its edges stay soft.
pub const TRACK_SPAN: f32 = 3.2 / 1.2;

/// All marks, indexed by the constants above.
pub fn images() -> Vec<folia_render::glyphs::Image> {
    vec![dot(128), star(256), track()]
}

/// `drawDot`: radial gradient 1 → 0.72 (0.18) → 0.2 (0.45) → 0.
fn dot(size: u32) -> Image {
    let r = size as f32 / 2.0;
    let stops = [(0.0, 1.0), (0.18, 0.72), (0.45, 0.2), (1.0, 0.0)];
    image(size, size, |x, y| ramp(&stops, ((x - r).hypot(y - r)) / r))
}

/// `drawStar`: a radial halo of radius r/2, then four long rays and four short diagonal ones added on top.
fn star(size: u32) -> Image {
    let r = size as f32 / 2.0;
    let halo = [(0.0, 1.0), (0.12, 0.8), (0.4, 0.14), (1.0, 0.0)];
    let s = size as f32;
    image(size, size, |x, y| {
        let mut alpha = ramp(&halo, (x - r).hypot(y - r) / (r * 0.5));
        for k in 0..4 {
            let angle = std::f32::consts::FRAC_PI_2 * k as f32;
            alpha += ray(x - r, y - r, angle, r * 0.98, s * 0.018, 0.95);
            alpha += ray(x - r, y - r, angle + std::f32::consts::FRAC_PI_4, r * 0.42, s * 0.01, 0.45);
        }
        alpha.min(1.0)
    })
}

/// A 1.2-pixel stroke seen across `TRACK_SPAN` of its width: coverage `clamp(0.6 + 0.5 − |d|)` in pixels.
fn track() -> Image {
    let (width, height) = (64, 32);
    image(width, height, |_, y| {
        let d = (y / height as f32 - 0.5) * 3.2;
        (1.1 - d.abs()).clamp(0.0, 1.0)
    })
}

/// A thin diamond from the center along `angle`: alpha falls linearly to 0 at `length`, half-thickness `thick` at 0.
fn ray(x: f32, y: f32, angle: f32, length: f32, thick: f32, alpha: f32) -> f32 {
    let (sin, cos) = angle.sin_cos();
    let u = x * cos + y * sin;
    let v = -x * sin + y * cos;
    if u < 0.0 || u > length {
        return 0.0;
    }
    let half = thick * (1.0 - u / length);
    let cover = (half + 0.5 - v.abs()).clamp(0.0, 1.0);
    cover * alpha * (1.0 - u / length)
}

fn ramp(stops: &[(f32, f32)], t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    for pair in stops.windows(2) {
        let ((t0, a0), (t1, a1)) = (pair[0], pair[1]);
        if t <= t1 {
            return a0 + (a1 - a0) * ((t - t0) / (t1 - t0).max(1e-6));
        }
    }
    0.0
}

/// Samples `alpha` at four points per texel, as canvas antialiasing would.
fn image(width: u32, height: u32, alpha: impl Fn(f32, f32) -> f32) -> Image {
    let mut pixels = Vec::with_capacity((width * height) as usize);
    for row in 0..height {
        for col in 0..width {
            let mut sum = 0.0;
            for (dx, dy) in [(0.25, 0.25), (0.75, 0.25), (0.25, 0.75), (0.75, 0.75)] {
                sum += alpha(col as f32 + dx, row as f32 + dy);
            }
            pixels.push((sum / 4.0 * 255.0).round().clamp(0.0, 255.0) as u8);
        }
    }
    Image { width, height, pixels }
}
