// Monet's bottom spectrum (`AudioOverlay.tsx`): the band profiles and the active bar height.

const BARS: usize = 72;
const MIN_RAW_BIN: usize = 6;

/// `clamp(power / 255, 0.08, 1)`.
pub fn energy(power: f64) -> f64 {
    (power / 255.0).clamp(0.08, 1.0)
}

fn clamp01(value: f64) -> f64 {
    value.clamp(0.0, 1.0)
}

/// `buildSpectrumAnchors` then `sampleSpectrumProfile`.
pub fn sample_spectrum_profile(bands: [f64; 5], normalized_index: f64) -> f64 {
    let [bass, low_mid, mid, vocal, treble] = bands;
    let anchors = [
        bass * 1.08,
        bass * 0.96,
        bass * 0.74 + low_mid * 0.26,
        low_mid,
        low_mid * 0.58 + mid * 0.42,
        mid,
        mid * 0.42 + vocal * 0.58,
        vocal,
        vocal * 0.48 + treble * 0.52,
        treble,
        treble * 0.82,
    ]
    .map(clamp01);
    let clamped = clamp01(normalized_index);
    let scaled = (1.0 + clamped * 9.0).log10() * (anchors.len() - 1) as f64;
    let lower = scaled.floor() as usize;
    let upper = (lower + 1).min(anchors.len() - 1);
    let interpolation = scaled - lower as f64;
    let lower_value = anchors[lower];
    let upper_value = anchors[upper];
    let interpolated = lower_value + (upper_value - lower_value) * interpolation;
    let contour = 0.92
        + (clamped * std::f64::consts::PI * 3.4 - std::f64::consts::PI * 0.3).sin() * 0.08
        + (clamped * std::f64::consts::PI * 9.2 + std::f64::consts::PI * 0.2).sin() * 0.035;
    clamp01(interpolated * contour)
}

/// `sampleRawSpectrumProfile`. An array no longer than the skipped bins is 0.
pub fn sample_raw_spectrum_profile(raw: &[u8], normalized_index: f64) -> f64 {
    if raw.len() <= MIN_RAW_BIN {
        return 0.0;
    }
    let clamped = clamp01(normalized_index);
    let usable = (raw.len() - MIN_RAW_BIN).max(1) as f64;
    let mapped = (clamped * (usable + 1.0).ln()).exp_m1();
    let center = MIN_RAW_BIN as f64 + mapped;
    let edge = 1.0 - (clamped - 0.5).abs() * 2.0;
    let radius = (5.0 - edge * 3.0).round().max(1.0);
    let start = MIN_RAW_BIN.max((center - radius).floor() as usize);
    let end = (raw.len() - 1).min((center + radius).ceil() as usize);
    let mut weighted = 0.0;
    let mut weight_total = 0.0;
    for index in start..=end {
        let Some(sample) = raw.get(index) else { continue };
        let distance = (index as f64 - center).abs();
        let weight = (1.0 - distance / (radius + 1.0)).max(0.1);
        weighted += f64::from(*sample) * weight;
        weight_total += weight;
    }
    let averaged = if weight_total > 0.0 { weighted / weight_total } else { 0.0 };
    let noise = 16.0 + (1.0 - clamped) * 12.0;
    let normalized = clamp01((averaged - noise) / (255.0 - noise));
    let power = 1.8 + (1.0 - clamped) * 0.4;
    let processed = normalized.powf(power);
    let compensation = 0.52 + clamped * 0.62;
    let presence = 0.92 + clamped.sqrt() * 0.14;
    clamp01(processed * compensation * presence)
}

/// Active bar height: `h * (0.02 + (energy * 0.04 + band * 0.82 + pulse * 0.02) * envelope)`.
pub fn active_bar_height(height: f64, energy: f64, band: f64, pulse: f64, envelope: f64) -> f64 {
    height * (0.02 + (energy * 0.04 + band * 0.82 + pulse * 0.02) * envelope)
}

/// `pulse` of bar `index` at `millis` (`performance.now()` in the original).
pub fn pulse(index: usize, millis: f64) -> f64 {
    (index as f64 * 0.45 + millis * 0.006).sin() * 0.5 + 0.5
}

/// `envelope` of bar `index`, `sin(π * i / 71)`.
pub fn envelope(index: usize) -> f64 {
    (std::f64::consts::PI * index as f64 / (BARS - 1) as f64).sin()
}

/// The two sines added into a line point.
pub fn wave(index: usize, millis: f64) -> f64 {
    (index as f64 * 0.24 + millis * 0.004).sin() * 0.08 + (index as f64 * 0.68 + millis * 0.0028).sin() * 0.05
}
