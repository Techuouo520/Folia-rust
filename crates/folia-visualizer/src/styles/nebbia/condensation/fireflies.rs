// Dew-glass fireflies: approach, crawl, departure and five-to-ten light cycles from nebbiaFireflies.ts.

use super::super::{Rng, palette::Palette};

const TAU: f32 = std::f32::consts::TAU;

struct Fly {
    x: f32,
    y: f32,
    radius: f32,
    approach: f32,
    offset: [f32; 2],
    zig: f32,
    frequency: f32,
    phase: f32,
    hold: f32,
    leave: f32,
    direction: f32,
    far: f32,
    age: f32,
    crawl: f32,
    blink: f32,
    blink_phase: f32,
    tint: usize,
    previous: [f32; 2],
    head: f32,
}

fn random(rng: &mut Rng, low: f32, high: f32) -> f32 {
    low + rng.unit() * (high - low)
}
fn smooth(value: f32) -> f32 {
    let value = value.clamp(0.0, 1.0);
    value * value * (3.0 - 2.0 * value)
}

impl Fly {
    fn new(rng: &mut Rng, size: [f32; 2]) -> Self {
        let k = size[1] / 1000.0;
        let angle = rng.unit() * TAU;
        let distance = random(rng, 80.0, 220.0) * k;
        let x = rng.unit() * size[0];
        let y = rng.unit() * size[1];
        let offset = [angle.cos() * distance, angle.sin() * distance - random(rng, 0.0, 50.0) * k];
        let approach = random(rng, 1.5, 3.0);
        let blink = random(rng, 3.0, 6.0);
        let flashes = 5.0 + (rng.unit() * 6.0).floor();
        let radius = if rng.unit() < 0.3 { random(rng, 4.5, 6.0) } else { random(rng, 2.5, 4.5) } * k;
        Self {
            x,
            y,
            radius,
            approach,
            offset,
            zig: random(rng, 12.0, 30.0) * k,
            frequency: random(rng, 1.2, 2.2),
            phase: rng.unit() * TAU,
            hold: blink * (flashes - 1.0 + 0.45),
            leave: random(rng, 0.8, 1.4),
            direction: rng.unit() * TAU,
            far: random(rng, 120.0, 260.0) * k,
            age: 0.0,
            crawl: rng.unit() * TAU,
            blink,
            blink_phase: (1.0 - (approach / blink).fract()).fract(),
            tint: (rng.unit() * 5.0).floor() as usize,
            previous: [x + offset[0], y + offset[1]],
            head: (-offset[1]).atan2(-offset[0]),
        }
    }

    fn sample(&mut self, dt: f32, k: f32, rng: &mut Rng) -> [f32; 9] {
        let (mut x, mut y, mut scale, mut alpha, mut focus) = (self.x, self.y, 1.0, 0.92, 1.0);
        let progress = (self.age / self.approach).min(1.0);
        let leaving = (self.age - self.approach - self.hold) / self.leave;
        let flap;
        if progress < 1.0 {
            let cycle = TAU * 2.5;
            let eased = (progress
                + 0.85
                    * ((cycle * progress + self.phase).sin()
                        - self.phase.sin() * (1.0 - progress)
                        - (cycle + self.phase).sin() * progress)
                    / cycle)
                .clamp(0.0, 1.0);
            let remaining = (1.0 - eased).powi(2);
            let length = self.offset[0].hypot(self.offset[1]).max(1.0);
            let zig = self.zig
                * (1.0 - eased)
                * ((self.age * self.frequency * TAU + self.phase).sin()
                    + 0.5 * (self.age * self.frequency * 2.7 * TAU + self.phase * 2.0).sin());
            x += self.offset[0] * remaining - self.offset[1] / length * zig;
            y += self.offset[1] * remaining + self.offset[0] / length * zig;
            scale = 0.7 + 0.3 * progress;
            alpha *= 0.45 + 0.55 * progress;
            focus = progress;
            flap = (0.6 + 0.4 * (self.age * 47.0 + self.phase).sin().abs()) * (1.0 - progress.powi(4));
        } else if leaving < 0.0 {
            let stopped = self.age - self.approach;
            let go = smooth(((stopped * 0.9 + self.phase).sin() - 0.2) / 0.4);
            if dt > 0.0 {
                self.crawl += (rng.unit() - 0.5) * 0.78 * dt.sqrt();
            }
            self.x += self.crawl.cos() * 7.0 * k * go * dt;
            self.y += self.crawl.sin() * 7.0 * k * go * dt;
            x = self.x;
            y = self.y;
            flap =
                0.35 * ((stopped * 1.7 + self.phase * 3.0).sin() - 0.94).max(0.0) / 0.06 * (stopped * 40.0).sin().abs();
        } else {
            let distance = leaving * leaving * self.far;
            let zig = self.zig * 0.6 * leaving * (self.age * self.frequency * TAU + self.phase).sin();
            x += self.direction.cos() * distance - self.direction.sin() * zig;
            y += self.direction.sin() * distance + self.direction.cos() * zig;
            scale = 1.0 - 0.3 * leaving;
            focus = 1.0 - leaving;
            alpha *= 1.0 - leaving.powf(1.5);
            flap = 0.6 + 0.4 * (self.age * 47.0 + self.phase).sin().abs();
        }
        let movement = [x - self.previous[0], y - self.previous[1]];
        if movement[0] * movement[0] + movement[1] * movement[1] > 0.0025 * k * k {
            let difference = movement[1].atan2(movement[0]) - self.head;
            self.head += difference.sin().atan2(difference.cos()) * (dt * 10.0).min(1.0);
        }
        self.previous = [x, y];
        let phase = (self.age / self.blink + self.blink_phase).fract();
        let glow = if phase < 0.15 {
            smooth(phase / 0.15)
        } else if phase < 0.4 {
            1.0 - smooth((phase - 0.15) / 0.25)
        } else {
            0.0
        };
        let remain = if leaving < 0.0 { 1.0 } else { 1.0 - leaving };
        [x, y, self.radius * scale, alpha, focus, remain, self.head, flap, glow * remain]
    }
}

pub(in crate::styles::nebbia) struct Fireflies {
    size: [f32; 2],
    random: Rng,
    spawn: f32,
    flies: Vec<Fly>,
    poses: Vec<[f32; 9]>,
    instances: Vec<[f32; 12]>,
}

impl Default for Fireflies {
    fn default() -> Self {
        Self {
            size: [0.0; 2],
            random: Rng(27183),
            spawn: 0.0,
            flies: Vec::new(),
            poses: Vec::new(),
            instances: Vec::new(),
        }
    }
}

impl Fireflies {
    fn resize(&mut self, size: [f32; 2]) {
        let previous = self.size;
        self.size = size;
        if previous[0] > 0.0 {
            let scale = [size[0] / previous[0], size[1] / previous[1]];
            for fly in &mut self.flies {
                fly.x *= scale[0];
                fly.y *= scale[1];
                fly.previous[0] *= scale[0];
                fly.previous[1] *= scale[1];
            }
            return;
        }
        self.poses.clear();
        let count = (100.0 * size[0] * size[1] / (1280.0 * 720.0)).round() as usize;
        for _ in 0..count {
            let mut fly = Fly::new(&mut self.random, size);
            fly.age = self.random.unit() * (fly.approach + fly.hold + fly.leave);
            fly.previous = [f32::NAN; 2];
            self.poses.push(fly.sample(0.0, size[1] / 1000.0, &mut self.random));
            self.flies.push(fly);
        }
    }

    pub fn update(&mut self, size: [f32; 2], elapsed: f32, palette: &Palette) -> &[[f32; 12]] {
        if self.size != size {
            self.resize(size);
        }
        let dt = elapsed.clamp(0.0, 0.1);
        self.spawn += dt * (100.0 * size[0] * size[1] / (1280.0 * 720.0)) / (3.35 + 4.5 * (7.5 - 1.0 + 0.45));
        while self.spawn >= 1.0 {
            self.flies.push(Fly::new(&mut self.random, size));
            self.spawn -= 1.0;
        }
        self.flies.retain_mut(|fly| {
            fly.age += dt;
            fly.age < fly.approach + fly.hold + fly.leave
        });
        self.poses.clear();
        for fly in &mut self.flies {
            self.poses.push(fly.sample(dt, size[1] / 1000.0, &mut self.random));
        }
        let normalize = |color: [f32; 3]| {
            let peak = color.into_iter().fold(0.05_f32, f32::max);
            color.map(|value| value / peak)
        };
        let colors =
            [normalize(palette.la), normalize(palette.lb), normalize(palette.lc), [1.0, 0.68, 0.32], [1.0, 0.95, 0.85]];
        self.instances.clear();
        self.instances.extend(self.flies.iter().zip(&self.poses).map(|(fly, pose)| {
            let color = colors[fly.tint];
            [
                pose[0], pose[1], pose[2], pose[3], pose[4], pose[5], pose[6], pose[7], pose[8], color[0], color[1],
                color[2],
            ]
        }));
        &self.instances
    }
}
