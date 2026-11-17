// Condensed drops on the fog and the raindrop sprite. Drops are drawn on the GPU as sprites, as the
// original nebbiaGl.ts does with raindrop-fx's normal-map raindrop (MIT, assets/nebbia).

use std::sync::{Mutex, OnceLock};

use super::Rng;
use super::place::Drop;

/// Condensed drops per 2200 square pixels (`NEBBIA_FIXED.fieldDrops`).
const FIELD: f32 = 0.5;

/// The sprite and its mip chain, RGBA8, largest first.
pub struct Sprite {
    pub levels: Vec<(u32, u32, Vec<u8>)>,
}

/// Condensed drops for this CSS size, slightly more and bigger towards the bottom (the original `buildField`).
/// `read` sees them only when the size's key differs from `known`; returns the key.
pub fn field(width: f32, height: f32, known: u64, read: impl FnOnce(&[Drop])) -> u64 {
    static FIELD_DROPS: Mutex<(u64, Vec<Drop>)> = Mutex::new((0, Vec::new()));
    let key = (u64::from(width.to_bits()) << 32) | u64::from(height.to_bits());
    if key == known {
        return key;
    }
    let mut held = FIELD_DROPS.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    if held.0 != key {
        held.1.clear();
        let mut rnd = Rng(4242);
        let count = (width * height / 2200.0 * FIELD).round() as usize;
        for _ in 0..count {
            let big = rnd.unit() < 0.12;
            let w = if big { 40.0 + rnd.unit() * 34.0 } else { 14.0 + rnd.unit().powf(1.4) * 22.0 };
            let sp = rnd.unit() * 0.25;
            let x = rnd.unit() * width;
            let y = height * (1.0 - rnd.unit().powf(1.25));
            let dw = w * (1.0 + sp * 0.5);
            held.1.push([x, y, dw, w * (1.0 + sp), dw / 100.0, 1.0]);
        }
        held.0 = key;
    }
    read(&held.1);
    key
}

pub fn sprite() -> Option<&'static Sprite> {
    static SPRITE: OnceLock<Option<Sprite>> = OnceLock::new();
    SPRITE
        .get_or_init(|| {
            let image =
                image::load_from_memory(include_bytes!("../../../../../assets/nebbia/nebbiaRaindrop.png")).ok()?;
            let mut level = image.to_rgba8();
            let mut levels = Vec::new();
            loop {
                let (w, h) = level.dimensions();
                let next = (w > 1 || h > 1).then(|| {
                    image::imageops::resize(
                        &level,
                        (w / 2).max(1),
                        (h / 2).max(1),
                        image::imageops::FilterType::Triangle,
                    )
                });
                levels.push((w, h, level.into_raw()));
                match next {
                    Some(smaller) => level = smaller,
                    None => break,
                }
            }
            Some(Sprite { levels })
        })
        .as_ref()
}

struct FallingDrop {
    start: f64,
    y0: f32,
    length: f32,
    duration: f32,
    trail: usize,
}
struct MovingDrop {
    sprite: Drop,
    home: [f32; 2],
    phase: f32,
    fall: Option<FallingDrop>,
    smear: Option<usize>,
}
struct Trail {
    active: bool,
    x: f32,
    y0: f32,
    y1: f32,
    width: f32,
    seed: f32,
    end: Option<f64>,
}
struct Smear {
    active: bool,
    owner: usize,
    x: f32,
    y: f32,
    width: f32,
    from: [f32; 2],
    to: [f32; 2],
    end: Option<f64>,
}

pub(super) struct MotionField {
    parts: Vec<MovingDrop>,
    trails: Vec<Trail>,
    smears: Vec<Smear>,
    free_trails: Vec<usize>,
    free_smears: Vec<usize>,
    random: Rng,
    serial: u64,
    last_trigger: f64,
    pub sprites: Vec<Drop>,
    pub drops: Vec<Drop>,
    pub fog: Vec<super::place::Segment>,
}

impl Default for MotionField {
    fn default() -> Self {
        Self {
            parts: Vec::new(),
            trails: Vec::new(),
            smears: Vec::new(),
            free_trails: Vec::new(),
            free_smears: Vec::new(),
            random: Rng(777),
            serial: 0,
            last_trigger: f64::NEG_INFINITY,
            sprites: Vec::new(),
            drops: Vec::new(),
            fog: Vec::new(),
        }
    }
}

impl MotionField {
    pub fn clear(&mut self) {
        self.parts.clear();
        self.trails.clear();
        self.smears.clear();
        self.free_trails.clear();
        self.free_smears.clear();
        self.sprites.clear();
        self.drops.clear();
        self.fog.clear();
        self.random = Rng(777);
        self.serial = 0;
        self.last_trigger = f64::NEG_INFINITY;
    }

    pub fn update(
        &mut self,
        base: &[Drop],
        motion: &super::beats::DropsMotion,
        bus: f32,
        hold: f32,
        refog: f32,
        height: f32,
    ) -> bool {
        if self.parts.is_empty() {
            if !motion.driven && !motion.active {
                return false;
            }
            let mut phases = Rng(4242);
            for sprite in base {
                for _ in 0..5 {
                    phases.next();
                }
                self.parts.push(MovingDrop {
                    sprite: *sprite,
                    home: [sprite[0], sprite[1]],
                    phase: phases.unit() * std::f32::consts::TAU,
                    fall: None,
                    smear: None,
                });
            }
            self.serial = motion.serial;
        }
        if self.serial != motion.serial {
            self.serial = motion.serial;
            if motion.trigger > 0.0 {
                self.trigger(motion.trigger_time, motion.trigger);
            }
        }
        self.sprites.clear();
        self.drops.clear();
        self.fog.clear();
        for index in 0..self.parts.len() {
            let part = &mut self.parts[index];
            let [mut x, mut y, width, original_height, _, _] = part.sprite;
            let mut stretch = 0.0;
            if let Some(fall) = &part.fall {
                let progress = ((motion.clock - fall.start) as f32 / fall.duration).clamp(0.0, 1.0);
                let trail = &mut self.trails[fall.trail];
                y = fall.y0
                    + fall.length
                        * (progress - (progress * std::f32::consts::PI * 6.0).sin() / (std::f32::consts::PI * 6.0));
                x = trail.x + (y * 0.07 + trail.seed).sin() * 1.4 + (y * 0.021 + trail.seed * 2.0).sin() * 2.2
                    - (y - trail.y0) * 0.6 * bus;
                stretch =
                    if progress < 1.0 { 0.4 * (1.0 - (progress * std::f32::consts::PI * 6.0).cos()) } else { 0.0 };
                trail.y1 = y;
                if progress >= 1.0 {
                    trail.end = Some(motion.clock);
                    part.fall = None;
                    if y - original_height * 0.125 > height {
                        part.sprite[0] = part.home[0];
                        part.sprite[1] = part.home[1];
                    } else {
                        part.sprite[0] = x;
                        part.sprite[1] = y;
                    }
                }
            }
            let [dx, dy, pull] = motion.offset(width * 0.125);
            let wobble = motion.wobble(part.phase) - pull;
            let not_falling = part.fall.is_none();
            self.sprites.push([
                x + dx,
                y + dy,
                width * (1.0 + wobble),
                original_height * (1.0 - wobble + stretch),
                width / 100.0,
                1.0,
            ]);
            if not_falling {
                self.track_smear(index, [dx, dy], motion.clock);
            }
        }
        self.draw_trails(motion.clock, bus, hold, refog, height);
        motion.apply(&mut self.drops);
        true
    }

    fn trigger(&mut self, time: f64, amount: f32) {
        let share = ((time - self.last_trigger) / 7.5).clamp(0.0, 1.0) as f32;
        self.last_trigger = time;
        for part in &mut self.parts {
            let [x, y, width, height, _, _] = part.sprite;
            if part.fall.is_some() || width < 26.0 {
                continue;
            }
            if self.random.unit() >= 0.5 * amount * share * (width - 24.0) / 45.0 {
                continue;
            }
            if let Some(smear) = part.smear.take().and_then(|index| self.smears.get_mut(index)) {
                smear.end.get_or_insert(time);
            }
            let trail = Trail {
                active: true,
                x,
                y0: y - height * 0.05,
                y1: y,
                width,
                seed: self.random.unit() * 100.0,
                end: None,
            };
            let index = match self.free_trails.pop() {
                Some(index) => {
                    self.trails[index] = trail;
                    index
                }
                None => {
                    self.trails.push(trail);
                    self.trails.len() - 1
                }
            };
            part.fall = Some(FallingDrop {
                start: time + f64::from(self.random.unit()) * 0.12,
                y0: y,
                length: (30.0 + self.random.unit() * 150.0) * width / 50.0,
                duration: 1.0 + self.random.unit() * 2.2,
                trail: index,
            });
        }
    }

    fn track_smear(&mut self, owner: usize, offset: [f32; 2], time: f64) {
        let distance = offset[0].hypot(offset[1]);
        let part = &mut self.parts[owner];
        if distance < 0.3 {
            if let Some(smear) = part.smear.and_then(|index| self.smears.get_mut(index)) {
                smear.end.get_or_insert(time);
            }
            return;
        }
        let index = match part.smear {
            Some(index) => index,
            None if distance < 0.6 => return,
            None => {
                let smear = Smear {
                    active: true,
                    owner,
                    x: part.sprite[0],
                    y: part.sprite[1],
                    width: part.sprite[2],
                    from: [0.0; 2],
                    to: [0.0; 2],
                    end: None,
                };
                let index = match self.free_smears.pop() {
                    Some(index) => {
                        self.smears[index] = smear;
                        index
                    }
                    None => {
                        self.smears.push(smear);
                        self.smears.len() - 1
                    }
                };
                part.smear = Some(index);
                index
            }
        };
        let smear = &mut self.smears[index];
        smear.end = None;
        if offset[1] < smear.from[1] {
            smear.from = offset;
        }
        if offset[1] > smear.to[1] {
            smear.to = offset;
        }
    }

    fn draw_trails(&mut self, time: f64, bus: f32, hold: f32, refog: f32, height: f32) {
        for (index, smear) in self.smears.iter_mut().enumerate() {
            if !smear.active {
                continue;
            }
            let alpha =
                smear.end.map_or(1.0, |end| 1.0 - super::smooth((time - end - 8.0) / (f64::from(refog) * 2.5)) as f32);
            if alpha <= 0.002 {
                smear.active = false;
                self.free_smears.push(index);
                if self.parts[smear.owner].smear == Some(index) {
                    self.parts[smear.owner].smear = None;
                }
                continue;
            }
            self.fog.push([
                smear.x + smear.from[0],
                smear.y + smear.from[1],
                smear.x + smear.to[0],
                smear.y + smear.to[1],
                smear.width * 0.14,
                alpha * 0.9,
            ]);
        }
        for (index, trail) in self.trails.iter_mut().enumerate() {
            if !trail.active {
                continue;
            }
            let alpha = trail
                .end
                .map_or(1.0, |end| 1.0 - super::smooth((time - end - f64::from(hold)) / f64::from(refog)) as f32);
            if alpha <= 0.002 {
                trail.active = false;
                self.free_trails.push(index);
                continue;
            }
            if trail.y1 - trail.y0 < 2.0 || trail.y0 > height {
                continue;
            }
            let wobble = |y: f32| {
                (y * 0.07 + trail.seed).sin() * 1.4 + (y * 0.021 + trail.seed * 2.0).sin() * 2.2
                    - (y - trail.y0) * 0.6 * bus
            };
            let mut y = trail.y0;
            while y < trail.y1 {
                let end = (y + 5.0).min(trail.y1);
                let along = (y - trail.y0) / (trail.y1 - trail.y0).max(1.0);
                let width = trail.width
                    * 0.18
                    * (0.72 + 0.28 * along)
                    * (0.85 + 0.3 * (0.5 + 0.5 * (y * 0.19 + trail.seed * 3.0).sin()));
                self.fog.push([trail.x + wobble(y), y, trail.x + wobble(end), end, width, alpha * 0.95]);
                y += 4.0;
            }
            let mut y = trail.y0 + 10.0 + trail.seed % 9.0;
            while y < trail.y1 - trail.width * 0.25 {
                let radius = trail.width * (0.03 + 0.03 * ((y + trail.seed) * 0.37).fract());
                if radius >= 0.5 {
                    self.drops.push([trail.x + wobble(y), y, radius * 8.0, radius * 8.0, radius * 0.08, alpha]);
                }
                y += 16.0 + (y * 7.0 + trail.seed * 13.0) % 13.0;
            }
        }
    }
}
