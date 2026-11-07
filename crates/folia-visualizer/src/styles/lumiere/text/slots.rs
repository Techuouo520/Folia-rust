// Window slots (windowSlots.ts) and the gap around the current line (lineClearance.ts).

use std::f32::consts::PI;

use super::super::art::Rng;
use super::super::profiles::Type;

/// Gap between a crossed neighbor and the current line, in font sizes.
pub const CROSSED_CLEARANCE: f32 = 0.5;
const AWAY_SOFTNESS: f32 = 0.005;
const HOLD: f32 = 0.018;
const PROTECT_FLOOR: f32 = 0.4;
pub const PROTECT_MARGIN: f32 = 0.5;

/// The text region: center and size in height units.
#[derive(Clone, Copy)]
pub struct Region {
    pub cx: f32,
    pub cy: f32,
    pub w: f32,
    pub h: f32,
}

#[derive(Clone, Copy, PartialEq, Default)]
pub struct Slot {
    pub dx: f32,
    pub dy: f32,
    pub scale: f32,
    pub alpha: f32,
    /// 0 across, 1 down.
    pub orient: f32,
    pub rotation: f32,
    /// 0 one line, 1 wrapped.
    pub wrap: f32,
}

/// `freePlacements` for relative −2, −1, 1, 2, in that order.
pub fn free_placements(rng: &mut Rng, region: Region, next_alpha: f32) -> [Slot; 4] {
    let mut placement = |upper: bool, alpha: f32| {
        let angle = if upper { PI * (1.12 + rng.next() * 0.76) } else { PI * (0.12 + rng.next() * 0.76) };
        let rx = region.w * (0.24 + rng.next() * 0.22);
        let ry = region.h * (0.3 + rng.next() * 0.18);
        let vertical = rng.next() < 0.5;
        Slot {
            dx: angle.cos() * rx,
            dy: angle.sin() * ry,
            scale: 0.5 + rng.next() * 0.2,
            alpha,
            orient: if vertical { 1.0 } else { 0.0 },
            rotation: (rng.next() - 0.5) * if vertical { 0.3 } else { 0.7 },
            wrap: 0.0,
        }
    };
    let previous = placement(true, 0.9);
    let next = placement(false, next_alpha);
    let farther =
        |slot: Slot| Slot { dx: slot.dx * 1.45, dy: slot.dy * 1.45, scale: slot.scale * 0.85, alpha: 0.0, ..slot };
    [farther(previous), previous, next, farther(next)]
}

pub fn placement_index(relative: i32) -> usize {
    match relative {
        -2 => 0,
        -1 => 1,
        1 => 2,
        _ => 3,
    }
}

/// `fixedSlot`: across and down layouts. `across(offset, scale)` is the thickness of line `current + offset`.
pub fn fixed_slot(
    kind: Type,
    relative: i32,
    region: Region,
    font_h: f32,
    next_alpha: f32,
    mut across: impl FnMut(i32, f32) -> f32,
) -> Slot {
    let vertical = kind == Type::Vertical;
    let orient = if vertical { 1.0 } else { 0.0 };
    let slot =
        |dx: f32, dy: f32, scale: f32, alpha: f32| Slot { dx, dy, scale, alpha, orient, rotation: 0.0, wrap: 0.0 };
    if relative == 0 {
        return slot(0.0, 0.0, 1.0, 1.0);
    }
    let previous = relative < 0;
    let far = relative.abs() > 1;
    let near_scale = if vertical { 0.58 } else { 0.52 };
    let scale = if far { if vertical { 0.48 } else { 0.44 } } else { near_scale };
    let (gap_previous, gap_next, far_previous, far_next) =
        if vertical { (0.95, 0.88, 0.78, 0.7) } else { (0.93, 0.735, 0.56, 0.56) };
    let sign: i32 = if previous { -1 } else { 1 };
    let mut offset = across(0, 1.0) / 2.0 + if previous { gap_previous } else { gap_next } * font_h;
    if far {
        offset += across(sign, near_scale) + if previous { far_previous } else { far_next } * font_h;
    }
    offset += across(relative, scale) / 2.0;
    let alpha = match (previous, far) {
        (_, true) => 0.0,
        (true, false) => 0.9,
        (false, false) => next_alpha,
    };
    let sign = sign as f32;
    if vertical {
        return slot(-sign * offset, sign * if far { 0.07 } else { 0.03 }, scale, alpha);
    }
    slot(sign * region.w * if far { 0.24 } else { 0.16 }, sign * offset, scale, alpha)
}

/// Stack axis: x for vertical, y otherwise.
pub fn stack_x(kind: Type) -> f32 {
    if kind == Type::Vertical { 1.0 } else { 0.0 }
}

/// Direction away from the current line on the stack axis (±1, 0 for the current line).
pub fn away_of(index: usize, current: i64, kind: Type) -> f32 {
    let index = index as i64;
    let side = if index == current {
        0.0
    } else if index < current {
        -1.0
    } else {
        1.0
    };
    side * if kind == Type::Vertical { -1.0 } else { 1.0 }
}

fn soft_abs(a: f32) -> f32 {
    (a * a + AWAY_SOFTNESS * AWAY_SOFTNESS).sqrt() - AWAY_SOFTNESS
}

/// `awayDrift`: the part of a neighbor's drift toward the current line turns away.
pub fn away_drift(drift: f32, away: f32) -> f32 {
    if away == 0.0 {
        return drift;
    }
    let sign = away.signum();
    let along = drift * sign;
    drift + away.abs() * (soft_abs(along) - along) * sign
}

/// `heldDrift`: the current line's drift bends into a circle of radius `HOLD` at the same speed.
pub fn held_drift(vx: f32, vy: f32, age: f32, axis: usize) -> f32 {
    let speed = vx.hypot(vy);
    if speed < 1e-9 {
        return 0.0;
    }
    let phi = speed * age / HOLD;
    let along = HOLD * phi.sin();
    let turn = HOLD * (1.0 - phi.cos());
    let (ux, uy) = (vx / speed, vy / speed);
    if axis == 0 { along * ux - turn * uy } else { along * uy + turn * ux }
}

/// A current line's ink box in logical pixels, with its hand-over weight.
#[derive(Clone, Copy, Default)]
pub struct ProtectBox {
    pub line: usize,
    pub x: f32,
    pub y: f32,
    pub cos: f32,
    pub sin: f32,
    pub half_w: f32,
    pub half_h: f32,
    pub margin: f32,
    pub weight: f32,
}

/// `protectionAt` then `protectedAlpha`, with flight lifting the dimming.
pub fn shield(boxes: &[ProtectBox], line: usize, x: f32, y: f32, half: f32, flying: f32) -> f32 {
    if boxes.is_empty() {
        return 1.0;
    }
    let mut protect = 0.0;
    for b in boxes {
        if b.line == line || b.weight <= 0.0 {
            continue;
        }
        let (dx, dy) = (x - b.x, y - b.y);
        let u = (dx * b.cos + dy * b.sin).abs() - b.half_w - half;
        let v = (dy * b.cos - dx * b.sin).abs() - b.half_h - half;
        let outside = if u > 0.0 && v > 0.0 { u.hypot(v) } else { u.max(v).max(0.0) };
        protect += b.weight * (1.0 - smooth(outside / b.margin));
    }
    1.0 - (1.0 - PROTECT_FLOOR) * protect.min(1.0) * (1.0 - flying)
}

pub fn smooth(value: f32) -> f32 {
    let t = value.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}
