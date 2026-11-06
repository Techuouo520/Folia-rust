// Theme colours for the night street, from the original `nebbiaPalette.ts`.

use folia_core::{Rgb, Theme};

pub struct Palette {
    pub sky0: [f32; 3],
    pub sky1: [f32; 3],
    pub la: [f32; 3],
    pub lb: [f32; 3],
    pub lc: [f32; 3],
}

pub fn palette(theme: &Theme) -> Palette {
    let accent = rgb(theme.accent);
    let bg = rgb(theme.background);
    let [ah, as_, al] = to_hsl(accent);
    let lb = from_hsl([(ah + 0.55) % 1.0, as_, al]);
    let lc = from_hsl([(ah + 0.9) % 1.0, as_, al]);
    let sky_mix = [bg[0] * 0.7 + accent[0] * 0.3, bg[1] * 0.7 + accent[1] * 0.3, bg[2] * 0.7 + accent[2] * 0.3];
    Palette {
        sky0: as_sky(bg, 0.05),
        sky1: as_sky(sky_mix, 0.14),
        la: as_light(accent),
        lb: as_light(lb),
        lc: as_light(lc),
    }
}

fn rgb(color: Rgb) -> [f32; 3] {
    [f32::from(color.r) / 255.0, f32::from(color.g) / 255.0, f32::from(color.b) / 255.0]
}

fn to_hsl([r, g, b]: [f32; 3]) -> [f32; 3] {
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let l = (max + min) / 2.0;
    let d = max - min;
    if d == 0.0 {
        return [0.0, 0.0, l];
    }
    let s = d / (1.0 - (2.0 * l - 1.0).abs());
    let h = if max == r {
        ((g - b) / d + 6.0) % 6.0
    } else if max == g {
        (b - r) / d + 2.0
    } else {
        (r - g) / d + 4.0
    };
    [h / 6.0, s, l]
}

fn from_hsl([h, s, l]: [f32; 3]) -> [f32; 3] {
    let k = |n: f32| (n + h * 12.0) % 12.0;
    let a = s * l.min(1.0 - l);
    let f = |n: f32| l - a * (-1.0_f32).max((k(n) - 3.0).min((9.0 - k(n)).min(1.0)));
    [f(0.0), f(8.0), f(4.0)]
}

fn as_light(color: [f32; 3]) -> [f32; 3] {
    let [h, s, l] = to_hsl(color);
    from_hsl([h, 0.55_f32.max(s), 0.78_f32.min(0.6_f32.max(l))])
}

fn as_sky(color: [f32; 3], lightness: f32) -> [f32; 3] {
    let [h, s, _] = to_hsl(color);
    from_hsl([h, 0.55_f32.min(s), lightness])
}
