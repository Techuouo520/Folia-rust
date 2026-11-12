// Weighted median-cut palette, matching `extractRepresentativeColorsFromPixels` after a 50×50 resample.

use std::collections::{HashMap, VecDeque};

use folia_core::Rgb;

const SHIFT: u8 = 4;
const MIN_ALPHA: u8 = 128;

#[derive(Clone, Copy)]
struct Sample {
    r: u8,
    g: u8,
    b: u8,
    weight: u32,
}

struct Sum {
    r: u64,
    g: u64,
    b: u64,
    weight: u32,
}

struct Bucket {
    colors: Vec<Sample>,
    weight: u32,
}

/// Canvas side used by `loadImagePixels` before the median cut.
pub const SAMPLE_SIDE: u32 = 50;

/// How many songs keep a palette. Older ids fall out; `PaletteStore::insert` can hold one back.
pub const PALETTE_CAP: usize = 32;

/// Resamples to 50×50 the way `loadImagePixels` draws into a canvas, then runs the median cut.
pub fn cover_palette(pixels: &[u8], width: u32, height: u32, count: usize) -> Vec<Rgb> {
    let sampled = resample_cover(pixels, width, height);
    if sampled.is_empty() {
        return Vec::new();
    }
    cover_colors(&sampled, count)
}

/// Bilinear shrink or grow to 50×50. Empty when the buffer is not `width`×`height` RGBA.
pub fn resample_cover(pixels: &[u8], width: u32, height: u32) -> Vec<u8> {
    let needed = (width as usize).saturating_mul(height as usize).saturating_mul(4);
    if width == 0 || height == 0 || pixels.len() < needed {
        return Vec::new();
    }
    let Some(image) = image::RgbaImage::from_raw(width, height, pixels[..needed].to_vec()) else {
        return Vec::new();
    };
    image::imageops::resize(&image, SAMPLE_SIDE, SAMPLE_SIDE, image::imageops::FilterType::Triangle).into_raw()
}

/// Up to `count` cover colours, heaviest first. Empty when every pixel is transparent.
pub fn cover_colors(pixels: &[u8], count: usize) -> Vec<Rgb> {
    if count == 0 {
        return Vec::new();
    }
    let mut histogram: Vec<(u16, Sum)> = Vec::new();
    let mut index = 0;
    while index + 3 < pixels.len() {
        if pixels[index + 3] >= MIN_ALPHA {
            let (r, g, b) = (pixels[index], pixels[index + 1], pixels[index + 2]);
            let key = (u16::from(r >> SHIFT) << 8) | (u16::from(g >> SHIFT) << 4) | u16::from(b >> SHIFT);
            if let Some((_, sample)) = histogram.iter_mut().find(|(stored, _)| *stored == key) {
                sample.r += u64::from(r);
                sample.g += u64::from(g);
                sample.b += u64::from(b);
                sample.weight += 1;
            } else {
                histogram.push((key, Sum { r: u64::from(r), g: u64::from(g), b: u64::from(b), weight: 1 }));
            }
        }
        index += 4;
    }
    let colors: Vec<Sample> = histogram
        .into_iter()
        .map(|(_, sample)| Sample {
            r: round_div(sample.r, sample.weight),
            g: round_div(sample.g, sample.weight),
            b: round_div(sample.b, sample.weight),
            weight: sample.weight,
        })
        .collect();
    if colors.is_empty() {
        return Vec::new();
    }
    let mut buckets = vec![Bucket { weight: colors.iter().map(|color| color.weight).sum(), colors }];
    while buckets.len() < count.min(buckets.iter().map(|bucket| bucket.colors.len()).sum()) {
        let Some(split_at) = widest(&buckets) else { break };
        let Some((left, right)) = split(&buckets[split_at]) else { break };
        buckets.splice(split_at..=split_at, [left, right]);
    }
    let mut averaged: Vec<Sample> = buckets.iter().map(average).collect();
    averaged.sort_by_key(|color| std::cmp::Reverse(color.weight));
    averaged.into_iter().map(|color| Rgb { r: color.r, g: color.g, b: color.b }).collect()
}

fn widest(buckets: &[Bucket]) -> Option<usize> {
    let mut best = None;
    let mut score = -1.0_f32;
    for (index, bucket) in buckets.iter().enumerate() {
        if bucket.colors.len() < 2 {
            continue;
        }
        let range = range(&bucket.colors, 0).max(range(&bucket.colors, 1)).max(range(&bucket.colors, 2));
        let next = range as f32 * (bucket.weight as f32).sqrt();
        if next > score {
            score = next;
            best = Some(index);
        }
    }
    best
}

fn split(bucket: &Bucket) -> Option<(Bucket, Bucket)> {
    if bucket.colors.len() < 2 {
        return None;
    }
    let channel = widest_channel(&bucket.colors);
    let mut sorted = bucket.colors.clone();
    sorted.sort_by_key(|color| channel_of(*color, channel));
    let midpoint = bucket.weight / 2;
    let mut accumulated = 0;
    let mut at = 1;
    for (index, color) in sorted.iter().enumerate().take(sorted.len() - 1) {
        accumulated += color.weight;
        at = index + 1;
        if accumulated >= midpoint {
            break;
        }
    }
    let right = sorted.split_off(at);
    let left_weight = sorted.iter().map(|color| color.weight).sum();
    let right_weight = right.iter().map(|color| color.weight).sum();
    Some((Bucket { colors: sorted, weight: left_weight }, Bucket { colors: right, weight: right_weight }))
}

fn average(bucket: &Bucket) -> Sample {
    let mut totals = [0_u64; 3];
    for color in &bucket.colors {
        totals[0] += u64::from(color.r) * u64::from(color.weight);
        totals[1] += u64::from(color.g) * u64::from(color.weight);
        totals[2] += u64::from(color.b) * u64::from(color.weight);
    }
    Sample {
        r: round_div(totals[0], bucket.weight),
        g: round_div(totals[1], bucket.weight),
        b: round_div(totals[2], bucket.weight),
        weight: bucket.weight,
    }
}

/// Equal ranges keep the earlier channel: red, then green, then blue.
fn widest_channel(colors: &[Sample]) -> usize {
    let mut chosen = 0;
    let mut best = range(colors, 0);
    for candidate in 1..3 {
        let next = range(colors, candidate);
        if next > best {
            best = next;
            chosen = candidate;
        }
    }
    chosen
}

fn range(colors: &[Sample], channel: usize) -> u8 {
    let mut min = 255_u8;
    let mut max = 0_u8;
    for color in colors {
        let value = channel_of(*color, channel);
        min = min.min(value);
        max = max.max(value);
    }
    max.saturating_sub(min)
}

fn round_div(sum: u64, weight: u32) -> u8 {
    ((sum + u64::from(weight) / 2) / u64::from(weight.max(1))) as u8
}

fn channel_of(color: Sample, channel: usize) -> u8 {
    match channel {
        0 => color.r,
        1 => color.g,
        _ => color.b,
    }
}

/// Cover palettes kept by track id, oldest first, capped at [`PALETTE_CAP`].
#[derive(Debug, Default)]
pub struct PaletteStore {
    colors: HashMap<i64, Vec<Rgb>>,
    order: VecDeque<i64>,
}

impl PaletteStore {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn get(&self, id: i64) -> Option<&[Rgb]> {
        self.colors.get(&id).map(Vec::as_slice)
    }

    pub fn len(&self) -> usize {
        self.colors.len()
    }

    pub fn is_empty(&self) -> bool {
        self.colors.is_empty()
    }

    pub fn remove(&mut self, id: i64) {
        self.colors.remove(&id);
        self.order.retain(|kept| *kept != id);
    }

    /// Stores `colors` for `id` and evicts the oldest other id once the cap is passed. `keep` stays.
    pub fn insert(&mut self, id: i64, colors: Vec<Rgb>, keep: Option<i64>) {
        self.order.retain(|kept| *kept != id);
        self.order.push_back(id);
        self.colors.insert(id, colors);
        let mut guarded = 0;
        while self.colors.len() > PALETTE_CAP {
            let Some(old) = self.order.pop_front() else { break };
            if Some(old) == keep || old == id {
                self.order.push_back(old);
                guarded += 1;
                if guarded > self.order.len() {
                    break;
                }
                continue;
            }
            self.colors.remove(&old);
            guarded = 0;
        }
    }
}

/// A cover result is current only when it is the request still waiting.
pub fn accept_art(pending: Option<(i64, u64)>, id: i64, generation: u64) -> bool {
    pending == Some((id, generation))
}
