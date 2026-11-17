// Meltwater mass, collision, evaporation, freezing and fog trails from nebbiaSnowWater.ts.

use super::super::{
    Rng,
    condensation::distance,
    place::{Drop as Sprite, Segment, WetSegment},
};

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) struct Id {
    index: usize,
    generation: u32,
}

#[derive(Clone, Copy)]
pub(super) struct Drop {
    pub x: f32,
    pub y: f32,
    vx: f32,
    vy: f32,
    pub mass: f32,
    density: f32,
    pub sx: f32,
    pub sy: f32,
    resist: f32,
    shift: f32,
    next_motion: f64,
    trail_x: f32,
    trail_y: f32,
    next_trail: f32,
    parent: Option<Id>,
    pub dead: bool,
    pub pin: Option<[f32; 2]>,
    wipe: Option<usize>,
    awake: bool,
    cell: usize,
    woke_mass: f32,
    woke_x: f32,
    woke_y: f32,
    still: f32,
    pub ice: f32,
    ice_speed: f32,
    pub rotation: f32,
    speck: bool,
    ice_duration: f32,
    slid: f32,
    pub tilt: f32,
}

impl Drop {
    pub fn width(&self) -> f32 {
        (self.sx + 1.0) * self.mass.max(0.0).sqrt() / self.density
    }
    fn height(&self) -> f32 {
        (self.sy + 1.0) * self.mass.max(0.0).sqrt() / self.density
    }
    fn merge_distance(&self) -> f32 {
        self.width() * (1.0 + self.sx) * 0.16
    }
}

struct Slot {
    generation: u32,
    drop: Option<Drop>,
}
struct Trace {
    active: bool,
    points: Vec<[f32; 2]>,
    width: f32,
    end: Option<f64>,
}

pub(super) struct Water {
    slots: Vec<Slot>,
    free: Vec<usize>,
    active: Vec<Id>,
    woken: Vec<Id>,
    cells: Vec<Vec<Id>>,
    grid: [usize; 2],
    cell_size: f32,
    traces: Vec<Trace>,
    free_traces: Vec<usize>,
    size: [f32; 2],
    k: f32,
    time: f64,
    random: Rng,
    frozen: usize,
    specks: usize,
    max_radius: f32,
}

impl Water {
    pub fn new(size: [f32; 2]) -> Self {
        let k = size[1] / 1000.0;
        let cell_size = 21.0 * k;
        let grid = size.map(|value| (value / cell_size).ceil() as usize + 1);
        Self {
            slots: Vec::new(),
            free: Vec::new(),
            active: Vec::new(),
            woken: Vec::new(),
            cells: (0..grid[0] * grid[1]).map(|_| Vec::new()).collect(),
            grid,
            cell_size,
            traces: Vec::new(),
            free_traces: Vec::new(),
            size,
            k,
            time: 0.0,
            random: Rng(8167),
            frozen: 0,
            specks: 0,
            max_radius: 40.0 * k,
        }
    }

    pub fn resize(&mut self, size: [f32; 2]) {
        let (time, random) = (self.time, self.random);
        *self = Self::new(size);
        self.time = time;
        self.random = random;
    }

    fn random(&mut self, low: f32, high: f32) -> f32 {
        low + self.random.unit() * (high - low)
    }
    pub fn get(&self, id: Id) -> Option<&Drop> {
        self.slots
            .get(id.index)
            .filter(|slot| slot.generation == id.generation)
            .and_then(|slot| slot.drop.as_ref())
            .filter(|drop| !drop.dead)
    }
    pub fn get_mut(&mut self, id: Id) -> Option<&mut Drop> {
        self.slots
            .get_mut(id.index)
            .filter(|slot| slot.generation == id.generation)
            .and_then(|slot| slot.drop.as_mut())
            .filter(|drop| !drop.dead)
    }
    fn cell(&self, x: f32, y: f32) -> usize {
        let x = (x / self.cell_size).floor().clamp(0.0, (self.grid[0] - 1) as f32) as usize;
        let y = (y / self.cell_size).floor().clamp(0.0, (self.grid[1] - 1) as f32) as usize;
        x + y * self.grid[0]
    }

    pub fn spawn(&mut self, x: f32, y: f32, size: f32, density: f32, spread: f32) -> Id {
        let index = self.free.pop().unwrap_or_else(|| {
            self.slots.push(Slot { generation: 0, drop: None });
            self.slots.len() - 1
        });
        let id = Id { index, generation: self.slots[index].generation };
        let drop = Drop {
            x,
            y,
            vx: 0.0,
            vy: 0.0,
            mass: (size * density).powi(2),
            density,
            sx: spread,
            sy: spread,
            resist: 0.0,
            shift: 0.0,
            next_motion: 0.0,
            trail_x: x,
            trail_y: y,
            next_trail: self.random(20.0, 30.0) * self.k,
            parent: None,
            dead: false,
            pin: None,
            wipe: None,
            awake: true,
            cell: self.cell(x, y),
            woke_mass: (size * density).powi(2),
            woke_x: x,
            woke_y: y,
            still: 0.0,
            ice: 0.0,
            ice_speed: 1.0,
            rotation: self.random(0.0, std::f32::consts::TAU),
            speck: false,
            ice_duration: 1.0,
            slid: 0.0,
            tilt: 0.0,
        };
        self.cells[drop.cell].push(id);
        self.slots[index].drop = Some(drop);
        self.active.push(id);
        self.woken.push(id);
        id
    }

    pub fn wake(&mut self, id: Id) {
        let Some(drop) = self.get_mut(id) else { return };
        if drop.awake {
            return;
        }
        drop.awake = true;
        drop.woke_mass = drop.mass;
        drop.woke_x = drop.x;
        drop.woke_y = drop.y;
        self.woken.push(id);
    }

    pub fn grew(&mut self, id: Id) {
        if self.get(id).is_some_and(|drop| drop.mass > drop.woke_mass * 1.05) {
            self.wake(id);
        }
    }
    pub fn kill(&mut self, id: Id) {
        if let Some(drop) = self.get_mut(id) {
            drop.dead = true;
        }
    }
    fn cap(&self, drop: &Drop) -> f32 {
        (72.0 * self.k * drop.density).powi(2)
    }

    pub fn step(&mut self, dt: f32) {
        self.time += f64::from(dt);
        let spread = 0.01_f32.powf(dt);
        let mut active = std::mem::take(&mut self.active);
        for id in &active {
            let Some(mut drop) = self.get(*id).copied() else { continue };
            self.move_drop(*id, &mut drop, dt, spread);
            let cell = self.cell(drop.x, drop.y);
            if cell != drop.cell {
                self.cells[drop.cell].retain(|held| held != id);
                self.cells[cell].push(*id);
                drop.cell = cell;
            }
            self.slots[id.index].drop = Some(drop);
        }
        self.collide();
        active.retain(|id| {
            let Some(drop) = self.slots[id.index].drop else { return false };
            if !drop.dead {
                return true;
            }
            self.end_trace(drop.wipe);
            self.cells[drop.cell].retain(|held| held != id);
            self.slots[id.index].drop = None;
            self.slots[id.index].generation = self.slots[id.index].generation.wrapping_add(1);
            self.free.push(id.index);
            false
        });
        active.append(&mut self.active);
        self.active = active;
        self.specks = self.active.iter().filter_map(|id| self.get(*id)).filter(|drop| drop.speck).count();
        self.frozen =
            self.active.iter().filter_map(|id| self.get(*id)).filter(|drop| !drop.speck && drop.ice > 0.0).count();
    }

    fn move_drop(&mut self, id: Id, drop: &mut Drop, dt: f32, spread: f32) {
        drop.mass = drop.mass.min(self.cap(drop));
        drop.mass -= (2.6 * self.k * drop.mass.max(0.0).sqrt() + drop.mass / 150.0) * dt * (1.0 - 0.85 * drop.ice);
        if drop.mass <= 0.0 {
            drop.dead = true;
            return;
        }
        drop.sx *= spread;
        drop.sy *= spread;
        if let Some([x, y]) = drop.pin {
            drop.x = x;
            drop.y = y;
            drop.vx = 0.0;
            drop.vy = 0.0;
            drop.still = 0.0;
            return;
        }
        if self.freeze(id, drop, dt) {
            return;
        }
        if drop.vy == 0.0 && drop.mass < 1600.0 * self.k * self.k {
            self.end_trace(drop.wipe.take());
            return;
        }
        if self.time >= drop.next_motion {
            drop.next_motion = self.time + f64::from(self.random(0.1, 0.4));
            drop.resist = self.random.unit() * 2400.0 * self.k * 70.0_f32.powi(2) * self.k.powi(2) * 4.0;
            drop.shift = self.random.unit() * self.random(0.0, 0.1);
            if drop.mass < 1600.0 * self.k * self.k {
                drop.resist = drop.resist.max(4800.0 * self.k * drop.mass);
            }
        }
        drop.vy = (drop.vy + (2400.0 * self.k * drop.mass - drop.resist) / drop.mass * dt).max(0.0);
        drop.vx = drop.vy * drop.shift;
        drop.x += drop.vx * dt;
        drop.y += drop.vy * dt;
        drop.sy = drop.sy.max(0.6 * (drop.vy * 0.005).atan() / std::f32::consts::PI);
        if drop.y > self.size[1] + 100.0 {
            drop.dead = true;
            return;
        }
        if drop.vy > 8.0 * self.k {
            drop.wipe = Some(self.trace(drop.wipe, drop.x, drop.y, drop.width() * 0.3));
            self.wake_copy(id, drop);
        } else {
            self.end_trace(drop.wipe.take());
        }
        if (drop.x - drop.trail_x).hypot(drop.y - drop.trail_y) > drop.next_trail
            && drop.mass >= 1000.0 * self.k * self.k
        {
            let x = drop.x + self.random(-5.0, 5.0) * self.k;
            let width = drop.width() * self.random(0.3, 0.5);
            let child = self.spawn(x, drop.y - drop.height() / 4.0, width, 0.2, 0.0);
            if let Some(child) = self.get_mut(child) {
                child.sx = 0.1;
                child.sy = drop.vy * 0.006;
                child.parent = Some(id);
                drop.mass -= child.mass;
            }
            drop.trail_x = drop.x;
            drop.trail_y = drop.y;
            drop.next_trail = self.random(20.0, 30.0) * self.k;
        }
    }

    fn wake_copy(&mut self, id: Id, drop: &mut Drop) {
        if drop.awake {
            return;
        }
        drop.awake = true;
        drop.woke_mass = drop.mass;
        drop.woke_x = drop.x;
        drop.woke_y = drop.y;
        self.woken.push(id);
    }

    fn freeze(&mut self, id: Id, drop: &mut Drop, dt: f32) -> bool {
        if drop.speck {
            drop.ice = 1.0;
            return true;
        }
        if drop.ice == 0.0 {
            if drop.vy > 0.0 || drop.width() * 0.25 < 2.5 * self.k {
                drop.still = 0.0;
                return false;
            }
            drop.still += dt;
            let scale = 1.0 + (drop.width() / (10.0 * self.k)).max(1.0).ln();
            if drop.still < 1.5 * scale || self.frozen >= 200 {
                return false;
            }
            self.frozen += 1;
            drop.ice = 0.001;
            drop.ice_speed = self.random(0.65, 1.35);
            drop.ice_duration = (3.0 * scale).max(1.0);
        }
        if drop.ice < 1.0 {
            drop.mass = (drop.mass * 0.75_f32.powf(2.0 * dt / drop.ice_duration)).min(self.cap(drop));
        }
        drop.ice = (drop.ice + dt / drop.ice_duration).min(1.0);
        if drop.ice < 0.3 {
            drop.vx = 0.0;
            drop.vy = 0.0;
            return true;
        }
        let velocity = 3.0 * self.k * drop.ice_speed * ((drop.ice - 0.3) / 0.15).min(1.0);
        drop.vy += (velocity - drop.vy) * (dt * 2.0).min(1.0);
        drop.vx = (self.time as f32 * 0.7 + drop.rotation * 5.0).sin() * drop.vy * 0.15;
        drop.x += drop.vx * dt;
        drop.y += drop.vy * dt;
        if drop.y > self.size[1] + 100.0 {
            drop.dead = true;
            return true;
        }
        drop.slid += drop.vy * dt;
        let over = drop.width() / (4.0 * self.k) - 7.5;
        let amplitude = 0.35 * (over / 6.0).clamp(0.0, 1.0);
        drop.tilt +=
            (amplitude * (drop.slid / (14.0 * self.k) + drop.rotation * 3.0).sin() - drop.tilt) * (dt * 3.0).min(1.0);
        if (drop.x - drop.woke_x).abs() + (drop.y - drop.woke_y).abs() > self.k || drop.mass > drop.woke_mass * 1.05 {
            self.wake_copy(id, drop);
        }
        if (drop.x - drop.trail_x).hypot(drop.y - drop.trail_y) > drop.next_trail
            && drop.mass > 300.0 * self.k * self.k
            && self.specks < 300
        {
            let x = drop.x + self.random(-1.5, 1.5) * self.k;
            let width = drop.width() * self.random(0.18, 0.3);
            let child = self.spawn(x, drop.y - drop.width() * 0.25, width, 1.0, 0.0);
            if let Some(child) = self.get_mut(child) {
                child.speck = true;
                child.ice = 1.0;
                child.parent = Some(id);
                drop.mass -= child.mass;
            }
            drop.trail_x = drop.x;
            drop.trail_y = drop.y;
            drop.next_trail = self.random(10.0, 18.0) * self.k;
            self.specks += 1;
        }
        true
    }

    fn collide(&mut self) {
        let mut woken = std::mem::take(&mut self.woken);
        for id in &woken {
            let Some(drop) = self.get_mut(*id) else { continue };
            drop.awake = false;
            let drop = *drop;
            let cx = (drop.x / self.cell_size).floor() as i32;
            let cy = (drop.y / self.cell_size).floor() as i32;
            for y in (cy - 1).max(0)..=(cy + 1).min(self.grid[1] as i32 - 1) {
                for x in (cx - 1).max(0)..=(cx + 1).min(self.grid[0] as i32 - 1) {
                    let cell = x as usize + y as usize * self.grid[0];
                    for index in 0..self.cells[cell].len() {
                        let other_id = self.cells[cell][index];
                        let (Some(drop), Some(other)) = (self.get(*id).copied(), self.get(other_id).copied()) else {
                            continue;
                        };
                        if other_id == *id
                            || other.parent == Some(*id)
                            || drop.parent == Some(other_id)
                            || drop.parent.is_some() && drop.parent == other.parent
                        {
                            continue;
                        }
                        if (drop.x - other.x).hypot(drop.y - other.y) >= drop.merge_distance() + other.merge_distance()
                        {
                            continue;
                        }
                        let (big_id, mut big, small_id, small) = if drop.mass >= other.mass {
                            (*id, drop, other_id, other)
                        } else {
                            (other_id, other, *id, drop)
                        };
                        let mass = big.mass + small.mass;
                        big.vx = (big.vx * big.mass + small.vx * small.mass) / mass;
                        big.vy = (big.vy * big.mass + small.vy * small.mass) / mass;
                        big.density = (big.density * big.mass + small.density * small.mass) / mass;
                        big.ice =
                            if big.ice > 0.0 { (big.ice * big.mass + small.ice * small.mass) / mass } else { 0.0 };
                        if big.ice < 0.001 {
                            big.ice = 0.0;
                        }
                        big.mass = mass.min(self.cap(&big));
                        big.speck &= small.speck;
                        self.slots[big_id.index].drop = Some(big);
                        self.kill(small_id);
                        self.wake(big_id);
                    }
                }
            }
        }
        woken.clear();
        woken.append(&mut self.woken);
        self.woken = woken;
    }

    pub fn near(&self, x: f32, y: f32, extra: f32) -> Option<Id> {
        let cell = self.cell(x, y);
        let (cx, cy) = (cell % self.grid[0], cell / self.grid[0]);
        for gy in cy.saturating_sub(1)..=(cy + 1).min(self.grid[1] - 1) {
            for gx in cx.saturating_sub(1)..=(cx + 1).min(self.grid[0] - 1) {
                for id in &self.cells[gx + gy * self.grid[0]] {
                    if self.get(*id).is_some_and(|drop| (drop.x - x).hypot(drop.y - y) < drop.width() * 0.25 + extra) {
                        return Some(*id);
                    }
                }
            }
        }
        None
    }

    pub fn absorb(&mut self, segments: &[WetSegment], pad: f32) {
        for segment in segments {
            let [x0, y0, x1, y1, width] = *segment;
            let margin = width * 0.5 + pad + self.max_radius;
            let first = self.cell(x0.min(x1) - margin, y0.min(y1) - margin);
            let last = self.cell(x0.max(x1) + margin, y0.max(y1) + margin);
            for y in first / self.grid[0]..=last / self.grid[0] {
                for x in first % self.grid[0]..=last % self.grid[0] {
                    let cell = x + y * self.grid[0];
                    for index in 0..self.cells[cell].len() {
                        let id = self.cells[cell][index];
                        if self.get(id).is_some_and(|drop| {
                            distance(drop.x, drop.y, segment) < width * 0.5 + pad + drop.width() * 0.25
                        }) {
                            self.kill(id);
                        }
                    }
                }
            }
        }
    }

    fn trace(&mut self, held: Option<usize>, x: f32, y: f32, width: f32) -> usize {
        let index = held.unwrap_or_else(|| {
            let index = self.free_traces.pop().unwrap_or_else(|| {
                self.traces.push(Trace { active: false, points: Vec::new(), width, end: None });
                self.traces.len() - 1
            });
            let trace = &mut self.traces[index];
            trace.active = true;
            trace.points.clear();
            trace.points.push([x, y]);
            trace.width = width;
            trace.end = None;
            index
        });
        let trace = &mut self.traces[index];
        trace.width = (trace.width * 0.98).max(width);
        if trace.points.last().is_some_and(|point| (x - point[0]).hypot(y - point[1]) > 3.0) {
            trace.points.push([x, y]);
        }
        index
    }

    fn end_trace(&mut self, index: Option<usize>) {
        if let Some(trace) = index.and_then(|index| self.traces.get_mut(index)) {
            trace.end.get_or_insert(self.time);
        }
    }

    pub fn outputs(
        &mut self,
        drops: &mut Vec<Sprite>,
        flakes: &mut Vec<[f32; 10]>,
        fog: &mut Vec<Segment>,
        hold: f32,
        refog: f32,
    ) {
        self.max_radius = 0.0;
        for id in &self.active {
            let Some(drop) = self.get(*id) else { continue };
            let width = drop.width();
            if width < 2.0 {
                continue;
            }
            drops.push([drop.x, drop.y, width * 2.0, drop.height() * 2.0, width / 50.0, 1.0]);
            if drop.ice > 0.0 {
                let radius = width * 0.25;
                flakes.push([
                    drop.x,
                    drop.y + radius * (0.42 + 0.2 * drop.sy),
                    radius * 1.05,
                    drop.rotation + drop.tilt,
                    ((1.0 - drop.ice) * 15.0).min(14.999),
                    12.0 + (drop.rotation * 10.0).floor() % 3.0,
                    0.9 * (drop.ice * 5.0).min(1.0),
                    1.0,
                    1.0 + drop.sx * 0.5,
                    1.0 + drop.sy * 0.5,
                ]);
            }
            self.max_radius = self.max_radius.max(width * 0.25);
        }
        self.max_radius += 2.0 * self.k;
        for (index, trace) in self.traces.iter_mut().enumerate() {
            if !trace.active {
                continue;
            }
            let opacity = trace.end.map_or(1.0, |end| {
                1.0 - super::super::smooth((self.time - end - f64::from(hold) * 0.5) / (f64::from(refog) * 0.6)) as f32
            });
            if opacity <= 0.002 {
                trace.active = false;
                self.free_traces.push(index);
                continue;
            }
            for points in trace.points.windows(2) {
                fog.push([points[0][0], points[0][1], points[1][0], points[1][1], trace.width, opacity * 0.5]);
            }
        }
    }
}
