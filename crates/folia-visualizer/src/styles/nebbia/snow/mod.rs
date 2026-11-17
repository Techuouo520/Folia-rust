// Snow landing, melting and frozen water advance on the mounted environment's local clock.

pub(super) mod atlas;
mod fall;
mod water;

use super::{
    Rng,
    condensation::distance,
    place::{Drop, Segment, WetSegment},
};

#[derive(Clone, Copy)]
struct Flake {
    x: f32,
    y: f32,
    radius: f32,
    rotation: f32,
    spin: f32,
    row: usize,
    age: f32,
    hold: f32,
    duration: f32,
    melt: f32,
    approach: f32,
    offset: [f32; 2],
    curl: f32,
    frequency: f32,
    flutter: f32,
    phase: f32,
    water: f32,
    drops: Option<[Option<water::Id>; 3]>,
    quick: bool,
}

impl Flake {
    fn pack(&self) -> [f32; 10] {
        let (mut x, mut y, mut radius) = (self.x, self.y, self.radius);
        let (mut sx, mut sy, mut alpha, mut focus) = (1.0, 1.0, 0.92, 1.0);
        let progress = (self.age / self.approach).min(1.0);
        if progress < 1.0 {
            let remaining = (1.0 - progress).powf(1.2);
            let bend = (std::f32::consts::PI * progress).sin() * self.curl;
            let flutter = self.flutter * remaining;
            x += self.offset[0] * remaining - self.offset[1] * bend
                + (self.age * self.frequency + self.phase).sin() * flutter;
            y += self.offset[1] * remaining
                + self.offset[0] * bend
                + (self.age * self.frequency * 0.7 + self.phase).cos() * flutter;
            let scale = 1.0 / (1.0 + 4.0 * (1.0 - progress));
            radius *= scale;
            focus = (scale - 0.2) / 0.8;
            alpha *= (progress / 0.15).min(1.0) * (0.35 + 0.65 * focus);
        } else {
            let flatten = (-(self.age - self.approach) * 14.0).exp();
            sx = 1.0 + 0.07 * flatten;
            sy = 1.0 - 0.08 * flatten;
            y -= self.radius * 0.1 * self.melt;
        }
        [x, y, radius, self.rotation, (self.melt * 15.0).min(14.999), self.row as f32, alpha, focus, sx, sy]
    }
}

struct Simulation {
    size: [f32; 2],
    settings: [f32; 5],
    time: f64,
    wet_at: f64,
    random: Rng,
    flakes: Vec<Flake>,
    water: water::Water,
    fall: fall::Fall,
    spawn: f32,
}

impl Simulation {
    fn new(size: [f32; 2], settings: [f32; 5]) -> Self {
        Self {
            size,
            settings,
            time: 0.0,
            wet_at: f64::NEG_INFINITY,
            random: Rng(36189),
            flakes: Vec::new(),
            water: water::Water::new(size),
            fall: fall::Fall::new(size),
            spawn: 0.0,
        }
    }

    fn random(&mut self, low: f32, high: f32) -> f32 {
        low + self.random.unit() * (high - low)
    }

    fn new_flake(&mut self) -> Flake {
        let k = self.size[1] / 1000.0;
        let wind = self.settings[3];
        let clump = self.random.unit() < 0.6;
        let radius =
            if self.random.unit() < 0.3 { self.random(4.5, 6.0) } else { self.random(2.5, 4.5) } * k * self.settings[1];
        let approach = self.random(2.0, 3.2) / (0.75 + 0.25 * wind);
        let distance = self.random(45.0, 75.0) * k * approach;
        let calm = 1.0 / (0.6 + 0.4 * wind);
        Flake {
            x: self.random.unit() * self.size[0],
            y: self.random.unit() * self.size[1],
            radius,
            rotation: self.random(0.0, std::f32::consts::TAU),
            spin: self.random(-1.33, 1.33) * (0.7 + 0.3 * wind),
            row: if clump {
                (self.random.unit() * 7.0).floor() as usize
            } else {
                7 + (self.random.unit() * 5.0).floor() as usize
            },
            approach,
            offset: [self.random(-15.0, 15.0) * k - fall::slope(wind) * distance, -distance],
            curl: self.random(-0.2, 0.2) * calm,
            frequency: self.random(1.5, 3.5),
            flutter: self.random(1.0, 4.0) * k * calm,
            phase: self.random(0.0, std::f32::consts::TAU),
            age: 0.0,
            hold: 0.3 + self.random.unit().powf(2.5) * 7.0,
            duration: self.random(2.5, 4.5) + radius / k * 0.18,
            melt: 0.0,
            water: 0.0,
            drops: None,
            quick: false,
        }
    }

    fn water_of(&self, radius: f32) -> f32 {
        let k = self.size[1] / 1000.0;
        (18.0 * (1.0 + radius / k / 2.5).ln() * k).powi(2)
    }

    fn touches(flake: &Flake, segments: &[WetSegment]) -> bool {
        segments.iter().any(|segment| distance(flake.x, flake.y, segment) < segment[4] * 0.5 + flake.radius * 0.6)
    }

    fn melt(&mut self, flake: &mut Flake, amount: f32, lumps: &[atlas::Lump]) -> bool {
        flake.melt = (flake.melt + amount / flake.duration).min(1.0);
        let total = self.water_of(flake.radius) * flake.melt;
        let delta = total - flake.water;
        flake.water = total;
        if flake.drops.is_none() && flake.melt > 0.12 {
            let mut drops = [None; 3];
            let (sin, cos) = flake.rotation.sin_cos();
            for (index, lump) in lumps.iter().enumerate().take(3) {
                let x = flake.x + (lump.x * cos - lump.y * sin) * flake.radius;
                let y = flake.y + (lump.x * sin + lump.y * cos) * flake.radius + flake.radius * 0.25;
                let id = self.water.spawn(x, y, (total * lump.weight).max(1.0).sqrt(), 1.0, 0.0);
                if let Some(drop) = self.water.get_mut(id) {
                    drop.pin = Some([x, y]);
                }
                drops[index] = Some(id);
            }
            flake.drops = Some(drops);
        } else if let Some(drops) = &mut flake.drops {
            let mut weight = 0.0;
            for (index, held) in drops.iter_mut().enumerate() {
                if held.is_some_and(|id| self.water.get(id).is_none()) {
                    *held = None;
                }
                if held.is_some() {
                    weight += lumps[index].weight;
                }
            }
            for (index, id) in drops.iter().enumerate().filter_map(|(index, id)| id.map(|id| (index, id))) {
                if let Some(drop) = self.water.get_mut(id) {
                    drop.mass += delta * lumps[index].weight / weight.max(0.0001);
                }
                self.water.grew(id);
            }
        }
        if flake.melt < 1.0 {
            return false;
        }
        for id in flake.drops.into_iter().flatten().flatten() {
            if let Some(drop) = self.water.get_mut(id) {
                drop.pin = None;
            }
        }
        true
    }

    fn step(&mut self, dt: f32, lumps: &[Vec<atlas::Lump>], sweeps: &[WetSegment], wet: &[WetSegment]) {
        self.fall.step(dt, self.settings[0] * self.settings[4], self.settings[3]);
        self.spawn += dt * self.settings[0] * 25.0 * self.size[0] * self.size[1] / (1280.0 * 720.0);
        while self.spawn >= 1.0 {
            let flake = self.new_flake();
            self.flakes.push(flake);
            self.spawn -= 1.0;
        }
        for index in (0..self.flakes.len()).rev() {
            let mut flake = self.flakes[index];
            let landing = flake.age < flake.approach;
            flake.age += dt;
            if flake.age < flake.approach {
                flake.rotation += flake.spin * (1.0 - flake.age / flake.approach) * dt;
                self.flakes[index] = flake;
                continue;
            }
            if landing {
                let near = self.water.near(flake.x, flake.y, flake.radius * 0.6);
                flake.quick = Self::touches(&flake, sweeps) || Self::touches(&flake, wet) || near.is_some();
                if let Some(id) = near {
                    let mass = self.water_of(flake.radius);
                    if let Some(drop) = self.water.get_mut(id) {
                        drop.mass += mass;
                    }
                    self.water.wake(id);
                }
            }
            if flake.quick {
                flake.melt = (flake.melt + dt / 0.25).min(1.0);
            }
            let wiped = !flake.quick && Self::touches(&flake, sweeps);
            if wiped {
                for id in flake.drops.into_iter().flatten().flatten() {
                    self.water.kill(id);
                }
            }
            let row = flake.row;
            let finished = if flake.quick {
                flake.melt >= 1.0
            } else {
                wiped
                    || flake.age >= flake.approach + flake.hold
                        && self.melt(&mut flake, dt * self.settings[2], &lumps[row])
            };
            if finished {
                self.flakes.swap_remove(index);
            } else {
                self.flakes[index] = flake;
            }
        }
        self.water.step(dt);
        self.time += f64::from(dt);
        self.water.absorb(sweeps, 0.0);
        if self.time - self.wet_at >= 0.05 {
            self.wet_at = self.time;
            self.water.absorb(wet, 3.0 * self.size[1] / 1000.0);
        }
    }
}

pub(super) struct Snow {
    lumps: Vec<Vec<atlas::Lump>>,
    simulation: Option<Simulation>,
    pub flakes: Vec<[f32; 10]>,
    pub drops: Vec<Drop>,
    pub fog: Vec<Segment>,
}

impl Snow {
    pub fn new(lumps: Vec<Vec<atlas::Lump>>) -> Self {
        Self { lumps, simulation: None, flakes: Vec::new(), drops: Vec::new(), fog: Vec::new() }
    }

    pub fn clear(&mut self) {
        self.simulation = None;
        self.flakes.clear();
        self.drops.clear();
        self.fog.clear();
    }

    pub fn prepare(&mut self, pass: &super::gpu::Pass) {
        let uniform = &pass.uniforms;
        let settings = [
            uniform.snow[0],
            uniform.snow[1],
            uniform.snow[2],
            if uniform.environment[0] > 0.5 { 0.0 } else { uniform.bus },
            if uniform.environment[0] > 0.5 { 0.0 } else { 1.0 },
        ];
        if self.simulation.is_none() {
            self.simulation = Some(Simulation::new(uniform.resolution, settings));
        }
        let Some(simulation) = &mut self.simulation else { return };
        if simulation.size != uniform.resolution {
            simulation.size = uniform.resolution;
            simulation.flakes.clear();
            simulation.fall.resize(uniform.resolution);
            simulation.water.resize(uniform.resolution);
        }
        simulation.settings = settings;
        simulation.step(pass.elapsed, &self.lumps, &pass.sweeps, &pass.wet);
        self.flakes.clear();
        self.drops.clear();
        self.fog.clear();
        simulation.fall.pack(simulation.time as f32, &mut self.flakes);
        self.flakes.extend(simulation.flakes.iter().map(Flake::pack));
        simulation.water.outputs(
            &mut self.drops,
            &mut self.flakes,
            &mut self.fog,
            uniform.snow[3],
            pass.knobs.refog as f32,
        );
        for ice in &pass.ice {
            self.flakes.push([
                ice[0],
                ice[1],
                ice[2],
                ice[4],
                ((1.0 - ice[3]) * 15.0).min(14.999),
                12.0 + (ice[4].abs() * 10.0).floor() % 3.0,
                0.9 * (ice[3] * 5.0).min(1.0) * ice[5],
                1.0,
                1.0,
                1.0,
            ]);
        }
    }
}
