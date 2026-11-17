// Procedural snow and frozen-water atlases from nebbiaSnowFlakes.ts, using the same sixteen melt stages.

use super::super::Rng;
use image::{GrayImage, Luma, Rgba, RgbaImage};

pub(super) const SIDE: u32 = 64;
pub(super) const STAGES: u32 = 16;
pub(super) const ROWS: u32 = 15;
const RADIUS: f32 = SIDE as f32 * 0.42;

#[derive(Clone, Copy)]
pub(in crate::styles::nebbia) struct Lump {
    pub x: f32,
    pub y: f32,
    pub weight: f32,
}

pub(in crate::styles::nebbia) struct Atlas {
    pub image: RgbaImage,
    pub lumps: Vec<Vec<Lump>>,
}

struct Disc {
    x: f32,
    y: f32,
    radius: f32,
    alpha: f32,
}
struct Branch {
    at: f32,
    length: f32,
    angle: f32,
    side: i32,
}
struct Arm {
    angle: f32,
    length: f32,
    width: f32,
    branches: Vec<Branch>,
}
struct Shape {
    discs: Vec<Disc>,
    arms: Vec<Arm>,
    specks: Vec<[f32; 3]>,
}

fn random(rng: &mut Rng, low: f32, high: f32) -> f32 {
    low + rng.unit() * (high - low)
}
fn chance(rng: &mut Rng, probability: f32) -> bool {
    rng.unit() < probability
}
fn count(rng: &mut Rng, low: usize, range: usize) -> usize {
    low + (rng.unit() * range as f32).floor() as usize
}

fn branches(rng: &mut Rng, length: f32) -> Vec<Branch> {
    let mut branches = Vec::new();
    let mut at = random(rng, 0.2, 0.4);
    while at < length - 0.08 {
        if !chance(rng, 0.3) {
            branches.push(Branch {
                at,
                length: random(rng, 0.1, 0.3) * (1.0 - at),
                angle: random(rng, 0.85, 1.2),
                side: if chance(rng, 0.25) { if chance(rng, 0.5) { 1 } else { -1 } } else { 0 },
            });
        }
        at += random(rng, 0.14, 0.28);
    }
    branches
}

fn shape(rng: &mut Rng, row: usize) -> Shape {
    let mut shape = Shape { discs: Vec::new(), arms: Vec::new(), specks: Vec::new() };
    let width = random(rng, 0.13, 0.18);
    if row >= 12 {
        shape.discs.push(Disc {
            x: random(rng, -0.05, 0.05),
            y: random(rng, -0.05, 0.05),
            radius: random(rng, 0.45, 0.55),
            alpha: 0.95,
        });
        for _ in 0..count(rng, 4, 3) {
            let angle = rng.unit() * std::f32::consts::TAU;
            let distance = random(rng, 0.15, 0.32);
            shape.discs.push(Disc {
                x: angle.cos() * distance,
                y: angle.sin() * distance,
                radius: random(rng, 0.18, 0.3),
                alpha: random(rng, 0.55, 0.8),
            });
        }
        for _ in 0..count(rng, 5, 3) {
            shape.arms.push(Arm {
                angle: rng.unit() * std::f32::consts::TAU,
                length: random(rng, 0.45, 0.75),
                width: random(rng, 0.04, 0.07),
                branches: Vec::new(),
            });
        }
    } else if row < 7 {
        let (mut x, mut y, mut angle) = (0.0, 0.0, rng.unit() * std::f32::consts::TAU);
        for _ in 0..count(rng, 7, 10) {
            shape.discs.push(Disc { x, y, radius: random(rng, 0.1, 0.22), alpha: random(rng, 0.6, 0.95) });
            angle += random(rng, -1.6, 1.6);
            let step = random(rng, 0.07, 0.15);
            x = (x + angle.cos() * step).clamp(-0.42, 0.42);
            y = (y + angle.sin() * step).clamp(-0.42, 0.42);
        }
    } else {
        shape.discs.push(Disc { x: 0.0, y: 0.0, radius: random(rng, 0.1, 0.22), alpha: 0.9 });
        let base = random(rng, 0.7, 1.0);
        let mut chipped = [false; 6];
        if chance(rng, 0.5) {
            chipped[count(rng, 0, 6)] = true;
        }
        if chipped.contains(&true) && chance(rng, 0.3) {
            chipped[count(rng, 0, 6)] = true;
        }
        for (index, chipped) in chipped.into_iter().enumerate() {
            if chipped && chance(rng, 0.4) {
                continue;
            }
            let length = base * random(rng, 0.86, 1.0) * if chipped { random(rng, 0.3, 0.6) } else { 1.0 };
            shape.arms.push(Arm {
                angle: index as f32 * std::f32::consts::TAU / 6.0 + random(rng, -0.05, 0.05),
                length,
                width: width * random(rng, 0.9, 1.1),
                branches: branches(rng, length),
            });
        }
        if chance(rng, 0.5) {
            shape.discs.push(Disc {
                x: random(rng, -0.2, 0.2),
                y: random(rng, -0.2, 0.2),
                radius: random(rng, 0.12, 0.25),
                alpha: 0.6,
            });
        }
    }
    for _ in 0..count(rng, 6, 14) {
        let speck = if !shape.arms.is_empty() && chance(rng, 0.6) {
            let arm = &shape.arms[count(rng, 0, shape.arms.len())];
            let at = rng.unit() * arm.length;
            [
                arm.angle.cos() * at + random(rng, -0.04, 0.04),
                arm.angle.sin() * at + random(rng, -0.04, 0.04),
                random(rng, 0.025, 0.06),
            ]
        } else {
            let disc = &shape.discs[count(rng, 0, shape.discs.len())];
            let angle = rng.unit() * std::f32::consts::TAU;
            let distance = rng.unit() * disc.radius;
            [disc.x + angle.cos() * distance, disc.y + angle.sin() * distance, random(rng, 0.03, 0.07)]
        };
        shape.specks.push(speck);
    }
    shape
}

fn density(shape: &Shape) -> GrayImage {
    let mut pixels = vec![0.0; (SIDE * SIDE) as usize];
    let mut over = |alpha: &dyn Fn(f32, f32) -> f32| {
        for y in 0..SIDE {
            for x in 0..SIDE {
                let at = (y * SIDE + x) as usize;
                let a = alpha(x as f32 + 0.5 - SIDE as f32 / 2.0, y as f32 + 0.5 - SIDE as f32 / 2.0).clamp(0.0, 1.0);
                pixels[at] = a + pixels[at] * (1.0 - a);
            }
        }
    };
    for disc in &shape.discs {
        over(&|x, y| {
            let distance = (x - disc.x * RADIUS).hypot(y - disc.y * RADIUS) / (disc.radius * RADIUS);
            disc.alpha * if distance <= 0.6 { 1.0 - distance * (0.4 / 0.6) } else { ((1.0 - distance) * 1.5).max(0.0) }
        });
    }
    for arm in &shape.arms {
        let (sin, cos) = arm.angle.sin_cos();
        let mut segment = |a: [f32; 2], b: [f32; 2], width: f32, alpha: f32| {
            let transform = |p: [f32; 2]| [RADIUS * (p[0] * cos - p[1] * sin), RADIUS * (p[0] * sin + p[1] * cos)];
            let (a, b) = (transform(a), transform(b));
            over(&|x, y| {
                alpha
                    * (width * RADIUS * 0.5 + 0.5
                        - super::super::condensation::distance(x, y, &[a[0], a[1], b[0], b[1], width]))
                    .clamp(0.0, 1.0)
            });
        };
        segment([0.0, 0.0], [arm.length, 0.0], arm.width, 0.55);
        for branch in &arm.branches {
            for side in [-1, 1] {
                if branch.side != 0 && branch.side != side {
                    continue;
                }
                segment(
                    [branch.at, 0.0],
                    [branch.at + branch.angle.cos() * branch.length, side as f32 * branch.angle.sin() * branch.length],
                    arm.width * 0.75,
                    0.42,
                );
            }
        }
    }
    for speck in &shape.specks {
        over(&|x, y| {
            0.95 * (speck[2] * RADIUS + 0.5 - (x - speck[0] * RADIUS).hypot(y - speck[1] * RADIUS)).clamp(0.0, 1.0)
        });
    }
    GrayImage::from_fn(SIDE, SIDE, |x, y| Luma([(pixels[(y * SIDE + x) as usize] * 255.0).round() as u8]))
}

fn smooth(a: f32, b: f32, value: f32) -> f32 {
    let t = ((value - a) / (b - a)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

pub(in crate::styles::nebbia) fn build() -> Atlas {
    let mut rng = Rng(5173);
    let mut atlas = Atlas { image: RgbaImage::new(SIDE * STAGES, SIDE * ROWS), lumps: Vec::new() };
    for row in 0..ROWS {
        let mut shape = shape(&mut rng, row as usize);
        let density = density(&shape);
        let solid = image::imageops::blur(&density, 0.35);
        let soft = image::imageops::blur(&density, 2.0);
        shape.discs.sort_by(|left, right| right.radius.total_cmp(&left.radius));
        let sum: f32 = shape.discs.iter().take(3).map(|disc| disc.radius * disc.radius).sum();
        atlas.lumps.push(
            shape
                .discs
                .iter()
                .take(3)
                .map(|disc| Lump { x: disc.x, y: disc.y, weight: disc.radius * disc.radius / sum })
                .collect(),
        );
        for stage in 0..STAGES {
            let melt = stage as f32 / (STAGES - 1) as f32;
            let threshold = melt * 0.85;
            for y in 0..SIDE {
                for x in 0..SIDE {
                    let d = f32::from(solid[(x, y)][0]) / 255.0;
                    let ds = f32::from(soft[(x, y)][0]) / 255.0;
                    let keep = smooth(threshold, threshold + 0.3, d);
                    let ice = keep * (0.88 - 0.45 * melt);
                    let wet = if melt > 0.0 {
                        smooth(threshold - 0.2, threshold, d)
                            * smooth(0.04, 0.12, d)
                            * (1.0 - keep)
                            * 0.32
                            * (melt * 2.0).min(1.0)
                    } else {
                        0.0
                    };
                    let halo = (ds * 2.0).min(1.0) * (1.0 - keep) * 0.2 * (1.0 - melt);
                    let top = ice + wet;
                    if top + halo < 0.004 {
                        continue;
                    }
                    let spec = smooth(0.8, 1.0, d) * (1.0 - melt);
                    let alpha = top + halo * (1.0 - top);
                    let mut rgba = [0; 4];
                    for (channel, ((snow, wet_color), dark)) in
                        [228.0, 236.0, 250.0].into_iter().zip([196.0, 212.0, 236.0]).zip([16.0, 20.0, 30.0]).enumerate()
                    {
                        let snow = snow + (255.0 - snow) * spec;
                        rgba[channel] =
                            ((snow * ice + wet_color * wet + dark * halo * (1.0 - top)) / alpha).round() as u8;
                    }
                    rgba[3] = (alpha * 255.0).round() as u8;
                    atlas.image[(stage * SIDE + x, row * SIDE + y)] = Rgba(rgba);
                }
            }
        }
    }
    atlas
}
