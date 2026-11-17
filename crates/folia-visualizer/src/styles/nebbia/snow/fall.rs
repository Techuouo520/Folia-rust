// Two depth layers of falling snow and the shared wind slope from nebbiaSnowFall.ts.

use super::super::Rng;

struct Falling {
    x: f32,
    y: f32,
    radius: f32,
    rotation: f32,
    spin: f32,
    row: usize,
    speed: f32,
    sway: f32,
    frequency: f32,
    phase: f32,
}
struct Layer {
    count: f32,
    radius: [f32; 2],
    speed: [f32; 2],
    alpha: f32,
    focus: f32,
    sway: [f32; 2],
}
const LAYERS: [Layer; 2] = [
    Layer { count: 240.0, radius: [0.9, 1.7], speed: [45.0, 75.0], alpha: 0.3, focus: 0.2, sway: [1.0, 3.0] },
    Layer { count: 70.0, radius: [2.0, 3.2], speed: [95.0, 140.0], alpha: 0.5, focus: 0.55, sway: [2.0, 5.0] },
];

pub(super) fn slope(wind: f32) -> f32 {
    -0.45 * (wind * (1.0 + 0.5 * (wind - 1.0))).max(0.0)
}
fn between(rng: &mut Rng, pair: [f32; 2]) -> f32 {
    pair[0] + rng.unit() * (pair[1] - pair[0])
}

pub(super) struct Fall {
    layers: [Vec<Falling>; 2],
    size: [f32; 2],
    random: Rng,
}

impl Fall {
    pub fn new(size: [f32; 2]) -> Self {
        Self { layers: [Vec::new(), Vec::new()], size, random: Rng(12819) }
    }

    pub fn resize(&mut self, size: [f32; 2]) {
        self.size = size;
        for layer in &mut self.layers {
            layer.clear();
        }
    }

    fn spawn(random: &mut Rng, size: [f32; 2], layer: usize, y: f32) -> Falling {
        let spec = &LAYERS[layer];
        let k = size[1] / 1000.0;
        Falling {
            x: random.unit() * size[0],
            y,
            radius: between(random, spec.radius) * k,
            rotation: random.unit() * std::f32::consts::TAU,
            spin: between(random, [-0.6, 0.6]),
            row: (random.unit() * 12.0).floor() as usize,
            speed: between(random, spec.speed) * k,
            sway: between(random, spec.sway) * k,
            frequency: between(random, [0.8, 1.6]),
            phase: random.unit() * std::f32::consts::TAU,
        }
    }

    pub fn step(&mut self, dt: f32, amount: f32, wind: f32) {
        let area = self.size[0] * self.size[1] / (1280.0 * 720.0);
        let margin = 20.0 * self.size[1] / 1000.0;
        for (index, list) in self.layers.iter_mut().enumerate() {
            let wanted = (LAYERS[index].count * amount * area).round() as usize;
            while list.len() < wanted {
                let y = self.random.unit() * self.size[1];
                list.push(Self::spawn(&mut self.random, self.size, index, y));
            }
            list.truncate(wanted);
            for flake in list {
                flake.y += flake.speed * dt;
                flake.x += flake.speed * slope(wind) * dt;
                flake.rotation += flake.spin * dt;
                if flake.y > self.size[1] + margin {
                    *flake = Self::spawn(&mut self.random, self.size, index, -margin);
                }
                if flake.x < -margin {
                    flake.x += self.size[0] + 2.0 * margin;
                } else if flake.x > self.size[0] + margin {
                    flake.x -= self.size[0] + 2.0 * margin;
                }
            }
        }
    }

    pub fn pack(&self, time: f32, out: &mut Vec<[f32; 10]>) {
        for (index, layer) in self.layers.iter().enumerate() {
            for flake in layer {
                out.push([
                    flake.x + (time * flake.frequency + flake.phase).sin() * flake.sway,
                    flake.y,
                    flake.radius,
                    flake.rotation,
                    0.0,
                    flake.row as f32,
                    LAYERS[index].alpha,
                    LAYERS[index].focus,
                    1.0,
                    1.0,
                ]);
            }
        }
    }
}
