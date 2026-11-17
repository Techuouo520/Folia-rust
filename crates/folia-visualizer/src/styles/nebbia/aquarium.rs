// Jellyfish motion and colours from nebbiaJellies.ts, integrated analytically for seek-stable frames.

use super::palette::Palette;

const TOP: f64 = -0.35;
const BOTTOM: f64 = 1.35;
const VIVID: [[f32; 3]; 4] = [[1.0, 0.35, 0.72], [0.35, 0.8, 1.0], [0.72, 0.5, 1.0], [1.0, 0.72, 0.35]];
pub(super) type JellyFrame = ([[f32; 4]; 24], [[f32; 4]; 24]);

#[derive(Clone, Copy)]
struct Jelly {
    x: f64,
    depth: f64,
    rate: f64,
    phase: f64,
    rise: f64,
    hue: f64,
}

fn random(value: f64) -> f64 {
    let value = (value * 127.1 + 311.7).sin() * 43758.5453;
    value - value.floor()
}

impl Jelly {
    fn new(index: usize, seed: usize) -> Self {
        let index = index as f64;
        let seed = seed as f64;
        Self {
            x: random(seed * 3.1 + index),
            depth: random(seed * 5.7 + index * 1.3),
            rate: 1.6 + 0.9 * random(seed * 2.3 + index),
            phase: random(seed + index * 7.3) * std::f64::consts::TAU,
            rise: 0.012 + 0.012 * random(seed * 9.1 + index),
            hue: random(seed * 4.4 + index * 11.0),
        }
    }

    fn distance(&self, seconds: f64) -> f64 {
        let integral = |phase: f64| {
            let cycles = (phase / std::f64::consts::TAU).floor();
            let tail = (phase - cycles * std::f64::consts::TAU).min(std::f64::consts::PI);
            2.0 * cycles + 1.0 - tail.cos()
        };
        self.rise * (seconds + 0.9 * (integral(self.phase + self.rate * seconds) - integral(self.phase)) / self.rate)
    }

    fn travel_time(&self, distance: f64) -> f64 {
        let (mut low, mut high) = (0.0, distance / self.rise);
        for _ in 0..24 {
            let middle = (low + high) * 0.5;
            if self.distance(middle) < distance {
                low = middle;
            } else {
                high = middle;
            }
        }
        (low + high) * 0.5
    }
}

pub(super) fn jellies(time: f64, aspect: f32, size: f32, palette: &Palette) -> JellyFrame {
    let mut positions = [[0.0; 4]; 24];
    let mut colors = [[0.0; 4]; 24];
    for index in 0..24 {
        let mut seed = index + 1;
        let mut jelly = Jelly::new(index, seed);
        let mut y = TOP + random(index as f64 * 13.7) * (BOTTOM - TOP);
        let mut elapsed = time.max(0.0);
        while jelly.distance(elapsed) > y - TOP {
            elapsed -= jelly.travel_time(y - TOP);
            seed += 24;
            jelly = Jelly::new(index, seed);
            y = BOTTOM;
        }
        y -= jelly.distance(elapsed);
        let phase = jelly.phase + jelly.rate * elapsed;
        positions[index] = [
            ((jelly.x + 0.02 * (time * 0.15 + jelly.hue * 10.0).sin()) * f64::from(aspect + 0.2) - 0.1) as f32,
            y as f32,
            (0.03 + 0.055 * jelly.depth) as f32 * size,
            phase.sin().max(0.0).powi(3) as f32,
        ];
        colors[index] = jelly_color(&jelly, phase, palette);
    }
    (positions, colors)
}

struct MovingJelly {
    jelly: Jelly,
    seed: usize,
    y: f64,
}

pub(super) struct LiveJellies {
    list: [MovingJelly; 24],
}

impl LiveJellies {
    pub fn at(time: f64) -> Self {
        Self {
            list: std::array::from_fn(|index| {
                let mut seed = index + 1;
                let mut jelly = Jelly::new(index, seed);
                let mut y = TOP + random(index as f64 * 13.7) * (BOTTOM - TOP);
                let mut elapsed = time.max(0.0);
                while jelly.distance(elapsed) > y - TOP {
                    elapsed -= jelly.travel_time(y - TOP);
                    seed += 24;
                    jelly = Jelly::new(index, seed);
                    y = BOTTOM;
                }
                y -= jelly.distance(elapsed);
                jelly.phase += jelly.rate * elapsed;
                MovingJelly { jelly, seed, y }
            }),
        }
    }

    pub fn step(
        &mut self,
        time: f64,
        dt: f64,
        own: f64,
        kick: f64,
        dimensions: [f32; 2],
        palette: &Palette,
    ) -> JellyFrame {
        let [aspect, size] = dimensions;
        let mut positions = [[0.0; 4]; 24];
        let mut colors = [[0.0; 4]; 24];
        for (index, moving) in self.list.iter_mut().enumerate() {
            moving.jelly.phase += dt * moving.jelly.rate;
            let swim = moving.jelly.phase.sin().max(0.0);
            moving.y -= dt * (moving.jelly.rise * (1.0 + 0.9 * swim * own) + 0.06 * kick);
            if moving.y < TOP {
                moving.seed += 24;
                moving.jelly = Jelly::new(index, moving.seed);
                moving.y = BOTTOM;
            }
            let jelly = moving.jelly;
            positions[index] = [
                ((jelly.x + 0.02 * (time * 0.15 + jelly.hue * 10.0).sin()) * f64::from(aspect + 0.2) - 0.1) as f32,
                moving.y as f32,
                (0.03 + 0.055 * jelly.depth) as f32 * size,
                (swim.powi(3) * own).max(kick).min(1.3) as f32,
            ];
            colors[index] = jelly_color(&jelly, jelly.phase, palette);
        }
        (positions, colors)
    }
}

fn jelly_color(jelly: &Jelly, phase: f64, palette: &Palette) -> [f32; 4] {
    let vivid = VIVID[(jelly.hue * 4.0).floor() as usize % 4];
    let theme = [palette.la, palette.lb, palette.lc][((jelly.hue * 3.7).fract() * 3.0).floor() as usize];
    let shade = (0.45 + 0.55 * jelly.depth) as f32;
    [
        (vivid[0] * 0.65 + theme[0] * 0.35) * shade,
        (vivid[1] * 0.65 + theme[1] * 0.35) * shade,
        (vivid[2] * 0.65 + theme[2] * 0.35) * shade,
        phase as f32,
    ]
}
