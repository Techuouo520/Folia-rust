// Model-vocal rain and seek-aware beat cursors for the original Nebbia beat effects.

use super::{
    Rng,
    scene::{self, Hit, Scene},
};
use folia_core::VocalEnvelope;
use std::sync::Arc;

#[derive(Default)]
pub(super) struct VocalRain {
    key: Option<(usize, usize, u32, u32)>,
    scene: Option<Arc<Scene>>,
    pub large: Vec<Hit>,
    levels: Vec<f32>,
    from: u32,
    loudness: f32,
}

impl VocalRain {
    pub fn active(&self) -> bool {
        self.key.is_some()
    }

    pub fn update(&mut self, envelope: Option<&VocalEnvelope>, scene: &Arc<Scene>, size: [f32; 2]) {
        let Some(envelope) = envelope.filter(|envelope| envelope.fps == 50 && !envelope.db.is_empty()) else {
            self.key = None;
            self.scene = None;
            self.large.clear();
            self.levels.clear();
            return;
        };
        let key = (envelope as *const VocalEnvelope as usize, envelope.db.len(), envelope.from, envelope.top.to_bits());
        if self.key == Some(key) && self.scene.as_ref().is_some_and(|held| Arc::ptr_eq(held, scene)) {
            return;
        }
        self.key = Some(key);
        self.scene = Some(Arc::clone(scene));
        self.from = envelope.from;
        self.loudness = 2.0_f32.powf((envelope.top + 17.4) / 8.0).clamp(0.5, 3.0);
        self.levels = energies(envelope);
        self.large.clear();
        let start = f64::from(envelope.from) / f64::from(envelope.fps);
        self.large.extend(scene.big.iter().filter(|hit| hit.t < start).cloned());
        let mut random = Rng((scene.seed ^ 911).wrapping_mul(2654435761));
        for onset in &envelope.onsets {
            if self.levels.get(*onset as usize).copied().unwrap_or(0.0) <= 0.15 && random.unit() >= 0.3 {
                continue;
            }
            let time = f64::from(*onset) / f64::from(envelope.fps);
            let rect =
                scene.lines.iter().find(|line| time >= line.t - 0.3 && time <= line.end + 1.0).map(|line| line.rect);
            self.large.push(scene::rain_hit(time, rect, size, &mut random));
        }
        self.large.sort_by(|left, right| left.t.total_cmp(&right.t));
    }

    pub fn rate(&self, bucket: i64, fallback: f64, density: f32) -> f64 {
        if bucket >= 0 && bucket < i64::from(self.from) {
            return fallback;
        }
        let level = usize::try_from(bucket).ok().and_then(|index| self.levels.get(index)).copied().unwrap_or(0.0);
        f64::from((0.15 + 7.0 * level * level) * density * self.loudness) * 0.02
    }
}

fn energies(envelope: &VocalEnvelope) -> Vec<f32> {
    let reach = (1.5 * f64::from(envelope.fps)).round() as usize;
    let floor = envelope.top - 28.0;
    let mut levels = vec![0.0; envelope.db.len()];
    let mut window: Vec<f32> = envelope.db.iter().take(reach).copied().filter(|value| *value > floor).collect();
    window.sort_by(f32::total_cmp);
    for (index, value) in envelope.db.iter().copied().enumerate() {
        if index > 0 {
            if let Some(value) = envelope.db.get(index + reach - 1).copied().filter(|value| *value > floor) {
                let at = window.partition_point(|held| *held < value);
                window.insert(at, value);
            }
            if let Some(value) = index
                .checked_sub(reach + 1)
                .and_then(|index| envelope.db.get(index))
                .copied()
                .filter(|value| *value > floor)
            {
                let at = window.partition_point(|held| *held < value);
                if at < window.len() {
                    window.remove(at);
                }
            }
        }
        if value <= floor || window.is_empty() {
            continue;
        }
        let low = window[(window.len() as f32 * 0.1).floor() as usize];
        let high = window[(window.len() as f32 * 0.95).floor() as usize].max(low + 3.0);
        levels[index] = ((value - low) / (high - low)).clamp(0.0, 1.0).powf(1.2);
    }
    levels
}

#[derive(Clone, Copy, Default)]
pub(super) struct DropsMotion {
    pub driven: bool,
    pub active: bool,
    pub shift: [f32; 2],
    pub serial: u64,
    pub trigger: f32,
    pub trigger_time: f64,
    pub clock: f64,
    springs: [[f32; 2]; 4],
    wobble: [f32; 2],
}

impl DropsMotion {
    pub fn offset(&self, radius: f32) -> [f32; 3] {
        let radii = [1.5, 3.0, 5.0, 8.0];
        let mut index = 0;
        while index < 2 && radius > radii[index + 1] {
            index += 1;
        }
        let fraction = ((radius - radii[index]) / (radii[index + 1] - radii[index])).clamp(0.0, 1.0);
        let free = ((radius - 2.5) / 3.5).clamp(0.0, 1.0);
        let dx = (self.springs[index][0] + (self.springs[index + 1][0] - self.springs[index][0]) * fraction) * free;
        let dy = (self.springs[index][1] + (self.springs[index + 1][1] - self.springs[index][1]) * fraction) * free;
        let stretch = ((dy.abs() - dx.abs() * 0.5) / radius.max(1.0) * 0.12).clamp(-0.25, 0.35);
        [dx, dy, stretch]
    }

    pub fn wobble(&self, phase: f32) -> f32 {
        self.wobble[0] * phase.sin() + self.wobble[1] * phase.cos()
    }

    pub fn apply(&self, drops: &mut [[f32; 6]]) {
        if !self.active {
            return;
        }
        for drop in drops {
            let [dx, dy, stretch] = self.offset(drop[2] / 8.0);
            let wobble = self.wobble(drop[0] * 0.05 + drop[1] * 0.03) - stretch;
            drop[0] += dx;
            drop[1] += dy;
            drop[2] *= 1.0 + wobble;
            drop[3] *= 1.0 - wobble;
            drop[4] = drop[2] / 100.0;
        }
    }
}

pub(super) struct BeatFrame {
    pub drops: DropsMotion,
    pub jelly: Option<super::aquarium::JellyFrame>,
    pub bursts: [[f32; 4]; 12],
    pub burst_count: f32,
    pub beat_rows: f32,
    pub sway: [f32; 4],
    pub weed_idle: f32,
    pub spot_scale: f32,
}

#[derive(Default)]
struct Pulse {
    from: f64,
    goal: f64,
    age: f64,
}
impl Pulse {
    fn value(&self, attack: f64, decay: f64) -> f64 {
        if self.age < attack {
            self.from + (self.goal - self.from) * self.age / attack
        } else {
            self.goal * (-(self.age - attack) / decay).exp()
        }
    }
    fn hit(&mut self, amount: f64, attack: f64, decay: f64) {
        self.from = self.value(attack, decay);
        self.goal = self.from.max(amount);
        self.age = 0.0;
    }
}

struct Weed {
    x: f64,
    velocity: f64,
    side: f64,
    carry: f64,
    history: [f32; 19],
    head: usize,
}
impl Default for Weed {
    fn default() -> Self {
        Self { x: 0.0, velocity: 0.0, side: 1.0, carry: 0.0, history: [0.0; 19], head: 0 }
    }
}
impl Weed {
    fn kick(&mut self, amount: f64) {
        self.side = -self.side;
        self.velocity = self.side * amount * std::f64::consts::TAU * 2.0 * 1.25;
    }
    fn step(&mut self, dt: f64) -> [f32; 4] {
        let omega = std::f64::consts::TAU * 2.0;
        self.carry += dt.min(0.1);
        while self.carry >= 1.0 / 120.0 {
            self.carry -= 1.0 / 120.0;
            self.velocity += (-omega * omega * self.x - 0.6 * omega * self.velocity) / 120.0;
            self.x += self.velocity / 120.0;
            self.head = (self.head + 1) % 19;
            self.history[self.head] = self.x as f32;
        }
        std::array::from_fn(|index| self.history[(self.head + 38 - index * 6) % 19])
    }
}

pub(super) struct Motion {
    aquarium: bool,
    beats_key: Option<(usize, usize)>,
    beat_next: usize,
    beat_time: f64,
    drums_key: Option<(usize, usize)>,
    drum_next: usize,
    drum_time: f64,
    last_bump: f64,
    previous: Option<f64>,
    clock: f64,
    bumps: Vec<(f64, f64)>,
    trigger_serial: u64,
    trigger_amount: f32,
    trigger_time: f64,
    springs: [[f64; 2]; 4],
    velocity: [[f64; 2]; 4],
    jelly: Option<super::aquarium::LiveJellies>,
    jelly_pulse: Pulse,
    own: f64,
    weed: Weed,
    weed_idle: f64,
    spot: Pulse,
    bursts: [[f32; 4]; 12],
    next_burst: usize,
}
impl Default for Motion {
    fn default() -> Self {
        Self {
            aquarium: false,
            beats_key: None,
            beat_next: 0,
            beat_time: f64::NEG_INFINITY,
            drums_key: None,
            drum_next: 0,
            drum_time: f64::NEG_INFINITY,
            last_bump: f64::NEG_INFINITY,
            previous: None,
            clock: 0.0,
            bumps: Vec::with_capacity(16),
            trigger_serial: 0,
            trigger_amount: 0.0,
            trigger_time: 0.0,
            springs: [[0.0; 2]; 4],
            velocity: [[0.0; 2]; 4],
            jelly: None,
            jelly_pulse: Pulse::default(),
            own: 1.0,
            weed: Weed::default(),
            weed_idle: 1.0,
            spot: Pulse::default(),
            bursts: [[0.0; 4]; 12],
            next_burst: 0,
        }
    }
}

impl Motion {
    pub fn step(
        &mut self,
        mut input: folia_core::SmartBeat<'_>,
        time: f64,
        size: [f32; 2],
        values: &crate::Values,
        palette: &super::palette::Palette,
    ) -> BeatFrame {
        if input.beats.is_none() {
            input.aqua_strength = [0.0; 4];
        }
        let delta = self.previous.map_or(0.0, |previous| time - previous);
        let jumped = !(0.0..=0.5).contains(&delta);
        let dt = if jumped { 0.0 } else { delta.min(0.1) };
        self.previous = Some(time);
        self.clock += dt;
        if jumped {
            for burst in &mut self.bursts {
                burst[1] += delta as f32;
            }
            if self.jelly.is_some() {
                self.jelly = Some(super::aquarium::LiveJellies::at(time));
            }
        }
        let manifest = super::tuning::manifest();
        let aquarium = values.choice(manifest, "behind") == "aquarium";
        if self.aquarium != aquarium {
            self.aquarium = aquarium;
            self.jelly = None;
            self.jelly_pulse = Pulse::default();
            self.own = 1.0;
            self.weed = Weed::default();
            self.weed_idle = 1.0;
            self.spot = Pulse::default();
            self.bursts = [[0.0; 4]; 12];
            self.next_burst = 0;
            self.beats_key = None;
            self.drums_key = None;
            self.bumps.clear();
            self.trigger_serial = self.trigger_serial.wrapping_add(1);
            self.trigger_amount = 0.0;
            self.springs = [[0.0; 2]; 4];
            self.velocity = [[0.0; 2]; 4];
        }
        let bump = f64::from(values.number(manifest, "bump"));
        let driven = input.drums.is_some() && !aquarium;
        let kick =
            if aquarium { 0.0 } else { self.drum_kick(input.drums, input.heard) * f64::from(input.bump_strength) };
        if kick > 0.0 && bump > 0.0 {
            self.bumps.push((self.clock, kick));
            self.trigger_serial = self.trigger_serial.wrapping_add(1);
            self.trigger_amount = kick as f32;
            self.trigger_time = self.clock;
        }
        self.bumps.retain(|(at, _)| self.clock - at <= 3.0);
        self.spring_step(dt, bump);
        let shift = shake(&self.bumps, self.clock, bump).map(|value| value as f32);
        let mut wobble = [0.0; 2];
        for (at, amplitude) in &self.bumps {
            let age = self.clock - at;
            let gain = amplitude * bump * 0.07 * (-age * 3.2).exp();
            wobble[0] += (gain * (age * std::f64::consts::TAU * 7.0).cos()) as f32;
            wobble[1] += (gain * (age * std::f64::consts::TAU * 7.0).sin()) as f32;
        }
        if aquarium {
            self.beat_hits(input, time, size[0] / size[1]);
        }
        let target = if input.aqua_strength[0] > 0.0 { 0.4 } else { 1.0 };
        self.own += (target - self.own).signum() * (target - self.own).abs().min(dt * 0.6 / 1.5);
        self.jelly_pulse.age += dt;
        self.spot.age += dt;
        let idle = if input.aqua_strength[1] > 0.0 { 0.0 } else { 1.0 };
        self.weed_idle += (idle - self.weed_idle) * (1.0 - (-dt / 0.6).exp());
        let jelly = if aquarium && (self.jelly.is_some() || input.aqua_strength[0] > 0.0) {
            let jelly = self.jelly.get_or_insert_with(|| super::aquarium::LiveJellies::at(time));
            Some(jelly.step(
                time,
                dt,
                self.own,
                self.jelly_pulse.value(0.07, 0.3),
                [size[0] / size[1], values.number(manifest, "jellySize")],
                palette,
            ))
        } else {
            None
        };
        let mut burst_count = 0;
        for (index, burst) in self.bursts.iter_mut().enumerate() {
            let age = time - f64::from(burst[1]);
            if burst[2] > 0.0 && (0.0..6.0).contains(&age) {
                burst_count = index + 1;
            } else {
                burst[2] = 0.0;
            }
        }
        BeatFrame {
            drops: DropsMotion {
                driven,
                active: !self.bumps.is_empty() || self.springs.iter().flatten().any(|value| value.abs() > 0.001),
                shift,
                serial: self.trigger_serial,
                trigger: self.trigger_amount,
                trigger_time: self.trigger_time,
                clock: self.clock,
                springs: self.springs.map(|axes| axes.map(|value| value as f32)),
                wobble,
            },
            jelly,
            bursts: self.bursts,
            burst_count: burst_count as f32,
            beat_rows: f32::from(input.aqua_strength[2] > 0.0),
            sway: self.weed.step(dt),
            weed_idle: self.weed_idle as f32,
            spot_scale: (1.0 + 0.45 * self.spot.value(0.06, 0.25)) as f32,
        }
    }

    fn drum_kick(&mut self, drums: Option<&folia_core::DrumHits>, time: f64) -> f64 {
        let Some(drums) = drums else {
            self.drums_key = None;
            return 0.0;
        };
        let key = (drums as *const folia_core::DrumHits as usize, drums.times.len());
        if self.drums_key != Some(key) || time < self.drum_time || time - self.drum_time > 0.5 {
            self.drums_key = Some(key);
            self.drum_time = time;
            self.last_bump = f64::NEG_INFINITY;
            self.drum_next = drums.times.partition_point(|at| *at <= time);
            return 0.0;
        }
        let mut amount = 0.0_f64;
        while self.drum_next < drums.times.len() && drums.times[self.drum_next] <= time {
            let index = self.drum_next;
            self.drum_next += 1;
            let strength = drums.strengths.get(index).copied().unwrap_or(0.0);
            let at = drums.times[index];
            if drums.kinds.get(index) != Some(&0) || strength < 0.35 || at - self.last_bump < 0.3 {
                continue;
            }
            self.last_bump = at;
            amount = amount.max(0.3 + 0.7 * f64::from(strength));
        }
        self.drum_time = time;
        amount
    }

    fn beat_hits(&mut self, input: folia_core::SmartBeat<'_>, time: f64, aspect: f32) {
        let Some(events) = input.beats else {
            self.beats_key = None;
            return;
        };
        let key = (events.as_ptr() as usize, events.len());
        if self.beats_key != Some(key) || input.heard < self.beat_time || input.heard - self.beat_time > 0.5 {
            self.beats_key = Some(key);
            self.beat_time = input.heard;
            self.beat_next = events.partition_point(|event| event.time <= input.heard);
            return;
        }
        self.beat_time = input.heard;
        while self.beat_next < events.len() && events[self.beat_next].time <= input.heard {
            let index = self.beat_next;
            self.beat_next += 1;
            let event = &events[index];
            if event.rhythmic == Some(false) {
                continue;
            }
            let gain = f64::from(event.impact.unwrap_or(1.0).max(0.35));
            let [jelly, weed, bubble, spot] = input.aqua_strength.map(f64::from);
            if jelly > 0.0 {
                self.jelly_pulse.hit(gain * jelly * if event.downbeat { 1.0 } else { 0.6 }, 0.07, 0.3);
            }
            if weed > 0.0 {
                self.weed.kick(gain * weed * if event.downbeat { 1.0 } else { 0.7 });
            }
            if spot > 0.0 {
                self.spot.hit((gain * spot * if event.downbeat { 1.0 } else { 0.7 }).min(1.5), 0.06, 0.25);
            }
            if bubble > 0.0 {
                let random = |value: f64| {
                    let value = (value * 91.7 + 47.3).sin() * 43758.5453;
                    value - value.floor()
                };
                self.bursts[self.next_burst] = [
                    0.17 + random(index as f64) as f32 * (aspect - 0.34).max(0.0),
                    time as f32,
                    (gain * bubble * if event.downbeat { 1.0 } else { 0.75 }).min(1.0) as f32,
                    (random(index as f64 * 3.3 + 1.0) * 100.0) as f32,
                ];
                self.next_burst = (self.next_burst + 1) % 12;
            }
        }
    }

    fn spring_step(&mut self, dt: f64, bump: f64) {
        const STEP: f64 = 1.0 / 480.0;
        let mut time = self.clock - dt.min(0.1);
        while time < self.clock {
            let step = STEP.min(self.clock - time);
            let previous = shake(&self.bumps, time - STEP, bump);
            let current = shake(&self.bumps, time, bump);
            let next = shake(&self.bumps, time + STEP, bump);
            for axis in 0..2 {
                let acceleration = -1.05 * (next[axis] - 2.0 * current[axis] + previous[axis]) / (STEP * STEP);
                for (index, frequency) in [11.0, 8.0, 5.5, 3.6].into_iter().enumerate() {
                    let omega = frequency * std::f64::consts::TAU;
                    let force = acceleration
                        - 0.36 * omega * self.velocity[index][axis]
                        - omega * omega * self.springs[index][axis];
                    self.velocity[index][axis] += force * step;
                    self.springs[index][axis] += self.velocity[index][axis] * step;
                }
            }
            time += step;
        }
    }
}

fn shake(bumps: &[(f64, f64)], time: f64, strength: f64) -> [f64; 2] {
    let mut shift = [0.0; 2];
    for (at, amplitude) in bumps {
        let age = time - at;
        if age < 0.0 {
            continue;
        }
        let gain = amplitude * strength;
        shift[1] += gain * 7.0 * (-age * 4.5).exp() * (age * std::f64::consts::TAU * 3.8).sin();
        shift[0] += gain
            * 2.0
            * (-age * 5.5).exp()
            * (age * std::f64::consts::TAU * 2.7 + 1.2).sin()
            * (1.0 - (-age * 60.0).exp());
    }
    shift
}
