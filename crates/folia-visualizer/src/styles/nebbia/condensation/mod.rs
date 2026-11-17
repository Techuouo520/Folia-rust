// Seeded condensation with a local clock, original growth/evaporation, and ink-driven regrowth.

pub(super) mod fireflies;

use super::{
    Rng,
    place::{Drop, WetSegment},
};

const AREA: f32 = 81.0;
const MAX_DENSITY: f32 = 1.5;
const CELL: f32 = 24.0;
const GROW: f64 = 6.0;
const FADE: f64 = 4.0;

struct Droplet {
    x: f32,
    y: f32,
    radius: f32,
    stretch: f32,
    born: f64,
    life: f64,
    full: bool,
    cell: usize,
}

impl Droplet {
    fn spawn(random: &mut Rng, size: [f32; 2], born: f64) -> Self {
        let y = size[1] * (1.0 - random.unit().powf(1.4));
        let low = y / size[1].max(1.0);
        let radius = if random.unit() < 0.06 + 0.08 * low {
            3.0 + random.unit() * (0.55 + 0.7 * low)
        } else {
            1.2 + random.unit().powf(1.5) * (1.35 + 0.4 * low)
        };
        Self {
            x: random.unit() * size[0],
            y,
            radius,
            stretch: 1.0 + random.unit() * 0.2,
            born,
            life: 12.5 + random.next() * 25.0,
            full: false,
            cell: 0,
        }
    }

    fn instance(&mut self, time: f64, scale: f32) -> Drop {
        let age = time - self.born;
        self.full = age >= GROW && age <= GROW + self.life;
        let grow = (age / GROW).clamp(0.0, 1.0).sqrt();
        let fade = ((GROW + self.life + FADE - age) / FADE).clamp(0.0, 1.0).sqrt();
        let radius = self.radius * scale * grow as f32 * fade as f32;
        let width = if radius < 0.5 { 0.0 } else { radius * 8.0 };
        [self.x, self.y, width, width * self.stretch, width / 100.0, 1.0]
    }
}

pub(super) struct Condensation {
    size: [f32; 2],
    droplets: Vec<Droplet>,
    sprites: Vec<Drop>,
    cells: Vec<Vec<usize>>,
    grid: [usize; 2],
    random: Rng,
    time: f64,
    since_update: f64,
    wet_at: f64,
    settings: [f32; 2],
    count: usize,
    dirty: bool,
}

impl Default for Condensation {
    fn default() -> Self {
        Self {
            size: [0.0; 2],
            droplets: Vec::new(),
            sprites: Vec::new(),
            cells: Vec::new(),
            grid: [1; 2],
            random: Rng(9191),
            time: 0.0,
            since_update: 0.0,
            wet_at: f64::NEG_INFINITY,
            settings: [0.0; 2],
            count: 0,
            dirty: true,
        }
    }
}

impl Condensation {
    fn reset(&mut self, size: [f32; 2]) {
        self.size = size;
        self.time = 0.0;
        self.wet_at = f64::NEG_INFINITY;
        self.grid = size.map(|side| (side / CELL).ceil() as usize + 1);
        self.cells.clear();
        self.cells.resize_with(self.grid[0] * self.grid[1], Vec::new);
        let count = (size[0] * size[1] / AREA * MAX_DENSITY).round() as usize;
        self.droplets.clear();
        self.sprites.resize(count, [0.0; 6]);
        self.sprites.fill([0.0; 6]);
        for index in 0..count {
            let born = self.random.next() * 10.0;
            let mut drop = Droplet::spawn(&mut self.random, size, born);
            drop.cell = self.cell(drop.x, drop.y);
            self.cells[drop.cell].push(index);
            self.droplets.push(drop);
        }
        self.count = 0;
        self.dirty = true;
    }

    fn cell(&self, x: f32, y: f32) -> usize {
        let x = (x / CELL).floor().clamp(0.0, (self.grid[0] - 1) as f32) as usize;
        let y = (y / CELL).floor().clamp(0.0, (self.grid[1] - 1) as f32) as usize;
        x + y * self.grid[0]
    }

    fn wipe(&mut self, segments: &[WetSegment], pad: f32, time: f64) {
        let size = self.settings[1];
        for segment in segments {
            let [x0, y0, x1, y1, width] = *segment;
            let margin = width * 0.5 + pad + 5.0 * size;
            let first = self.cell(x0.min(x1) - margin, y0.min(y1) - margin);
            let last = self.cell(x0.max(x1) + margin, y0.max(y1) + margin);
            for y in first / self.grid[0]..=last / self.grid[0] {
                for x in first % self.grid[0]..=last % self.grid[0] {
                    for index in &self.cells[x + y * self.grid[0]] {
                        let drop = &mut self.droplets[*index];
                        if *index >= self.count
                            || drop.born > time + 1.0
                            || distance(drop.x, drop.y, segment) >= width * 0.5 + pad + drop.radius * size
                        {
                            continue;
                        }
                        if drop.born <= time {
                            drop.full = false;
                        }
                        drop.born = time + 2.0 + self.random.next() * 4.0;
                    }
                }
            }
        }
    }

    fn update_sprites(&mut self, time: f64) -> bool {
        let mut changed = false;
        for index in 0..self.count {
            let old = &self.droplets[index];
            if time - old.born > GROW + old.life + FADE {
                let born = time + self.random.next() * 3.0;
                let mut drop = Droplet::spawn(&mut self.random, self.size, born);
                drop.cell = self.cell(drop.x, drop.y);
                self.cells[self.droplets[index].cell].retain(|held| *held != index);
                self.cells[drop.cell].push(index);
                self.droplets[index] = drop;
            }
            let drop = &mut self.droplets[index];
            let age = time - drop.born;
            if drop.full && age >= GROW && age <= GROW + drop.life {
                continue;
            }
            self.sprites[index] = drop.instance(time, self.settings[1]);
            changed = true;
        }
        changed
    }

    pub fn update(
        &mut self,
        size: [f32; 2],
        elapsed: f32,
        settings: [f32; 2],
        sweeps: &[WetSegment],
        wet: &[WetSegment],
    ) -> Option<&[Drop]> {
        if self.size != size {
            self.reset(size);
        }
        let resized = self.settings[1] != settings[1];
        self.settings = settings;
        self.wipe(sweeps, 0.0, self.time);
        if !wet.is_empty() && self.time - self.wet_at >= 0.05 {
            self.wet_at = self.time;
            self.wipe(wet, 2.0, self.time);
        }
        let dt = f64::from(elapsed.clamp(0.0, 0.1));
        self.time += dt;
        self.since_update += dt;
        let count =
            ((self.droplets.len() as f32 * settings[0] / MAX_DENSITY).round() as usize).min(self.droplets.len());
        let mut changed = std::mem::take(&mut self.dirty) || resized || count != self.count;
        if !changed && self.since_update < 0.05 {
            return None;
        }
        self.since_update = 0.0;
        if resized {
            for drop in &mut self.droplets {
                drop.full = false;
            }
        }
        self.count = count;
        changed |= self.update_sprites(self.time);
        if changed { Some(&self.sprites[..self.count]) } else { None }
    }

    pub fn invalidate(&mut self) {
        self.dirty = true;
    }
}

pub(super) fn distance(x: f32, y: f32, segment: &WetSegment) -> f32 {
    let [x0, y0, x1, y1, _] = *segment;
    let (dx, dy) = (x1 - x0, y1 - y0);
    let t = ((x - x0) * dx + (y - y0) * dy) / (dx * dx + dy * dy).max(1.0e-6);
    let t = t.clamp(0.0, 1.0);
    (x - x0 - dx * t).hypot(y - y0 - dy * t)
}
