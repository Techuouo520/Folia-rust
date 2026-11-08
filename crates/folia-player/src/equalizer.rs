// The ten-band equalizer and effect chain in the audio callback: parameters arrive over a ring buffer, every
// filter and delay line is allocated when the stream opens, and a frame allocates nothing and takes no lock.

use std::f32::consts::PI;

use folia_core::{
    AudioEffects, BandShape, EQUALIZER_FREQUENCIES, EQUALIZER_MAX_GAIN_DB, EQUALIZER_MIN_GAIN_DB, EQUALIZER_Q,
    EqualizerSettings,
};
use rtrb::{Consumer, Producer, RingBuffer};

/// What the callback is told to play; the newest one wins.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Parameters {
    pub enabled: bool,
    pub gains: [f32; 10],
    pub effects: AudioEffects,
}

impl Default for Parameters {
    fn default() -> Self {
        Self { enabled: false, gains: [0.0; 10], effects: AudioEffects::default() }
    }
}

impl Parameters {
    /// The parameters of saved settings: the switch turned off means a flat curve and neutral effects.
    pub fn of(settings: &EqualizerSettings) -> Self {
        if settings.enabled {
            Self { enabled: true, gains: settings.gains, effects: settings.effects.clamped() }
        } else {
            Self::default()
        }
    }
}

/// Normalised biquad coefficients, as the RBJ cookbook writes them.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Coefficients {
    pub b0: f32,
    pub b1: f32,
    pub b2: f32,
    pub a1: f32,
    pub a2: f32,
}

impl Default for Coefficients {
    fn default() -> Self {
        Coefficients::PASSTHROUGH
    }
}

impl Coefficients {
    pub const PASSTHROUGH: Coefficients = Coefficients { b0: 1.0, b1: 0.0, b2: 0.0, a1: 0.0, a2: 0.0 };

    /// One band's coefficients; `sample_rate` must be positive.
    pub fn new(shape: BandShape, frequency: f32, q: f32, gain_db: f32, sample_rate: f32) -> Self {
        let rate = sample_rate.max(1.0);
        let frequency = frequency.clamp(1.0, rate * 0.475);
        let omega = 2.0 * PI * frequency / rate;
        let (sin, cos) = omega.sin_cos();
        let a = 10f32.powf(gain_db / 40.0);
        let alpha = match shape {
            BandShape::Peaking => sin / (2.0 * q.max(0.01)),
            // Web Audio's shelves take no Q: the cookbook's S = 1.
            BandShape::LowShelf | BandShape::HighShelf => sin / 2.0 * 2f32.sqrt(),
        };
        let (b0, b1, b2, a0, a1, a2) = match shape {
            BandShape::Peaking => {
                (1.0 + alpha * a, -2.0 * cos, 1.0 - alpha * a, 1.0 + alpha / a, -2.0 * cos, 1.0 - alpha / a)
            }
            BandShape::LowShelf => {
                let two = 2.0 * a.sqrt() * alpha;
                (
                    a * ((a + 1.0) - (a - 1.0) * cos + two),
                    2.0 * a * ((a - 1.0) - (a + 1.0) * cos),
                    a * ((a + 1.0) - (a - 1.0) * cos - two),
                    (a + 1.0) + (a - 1.0) * cos + two,
                    -2.0 * ((a - 1.0) + (a + 1.0) * cos),
                    (a + 1.0) + (a - 1.0) * cos - two,
                )
            }
            BandShape::HighShelf => {
                let two = 2.0 * a.sqrt() * alpha;
                (
                    a * ((a + 1.0) + (a - 1.0) * cos + two),
                    -2.0 * a * ((a - 1.0) + (a + 1.0) * cos),
                    a * ((a + 1.0) + (a - 1.0) * cos - two),
                    (a + 1.0) - (a - 1.0) * cos + two,
                    2.0 * ((a - 1.0) - (a + 1.0) * cos),
                    (a + 1.0) - (a - 1.0) * cos - two,
                )
            }
        };
        Coefficients { b0: b0 / a0, b1: b1 / a0, b2: b2 / a0, a1: a1 / a0, a2: a2 / a0 }
    }

    /// The effect chain's cutoffs: highpass and lowpass with the given Q.
    pub fn cutoff(highpass: bool, frequency: f32, q: f32, sample_rate: f32) -> Self {
        let rate = sample_rate.max(1.0);
        let frequency = frequency.clamp(1.0, rate * 0.475);
        let omega = 2.0 * PI * frequency / rate;
        let (sin, cos) = omega.sin_cos();
        let alpha = sin / (2.0 * q.max(0.01));
        let a0 = 1.0 + alpha;
        if highpass {
            Coefficients {
                b0: (1.0 + cos) / 2.0 / a0,
                b1: -(1.0 + cos) / a0,
                b2: (1.0 + cos) / 2.0 / a0,
                a1: -2.0 * cos / a0,
                a2: (1.0 - alpha) / a0,
            }
        } else {
            Coefficients {
                b0: (1.0 - cos) / 2.0 / a0,
                b1: (1.0 - cos) / a0,
                b2: (1.0 - cos) / 2.0 / a0,
                a1: -2.0 * cos / a0,
                a2: (1.0 - alpha) / a0,
            }
        }
    }
}

/// A transposed direct-form II biquad; the state is carried between frames.
#[derive(Debug, Clone, Copy, Default)]
pub struct Biquad {
    coefficients: Coefficients,
    z1: f32,
    z2: f32,
}

impl Biquad {
    pub fn set(&mut self, coefficients: Coefficients) {
        self.coefficients = coefficients;
    }

    #[inline]
    pub fn process(&mut self, input: f32) -> f32 {
        let c = self.coefficients;
        let output = c.b0 * input + self.z1;
        self.z1 = c.b1 * input - c.a1 * output + self.z2;
        self.z2 = c.b2 * input - c.a2 * output;
        output
    }
}

/// The control side: the newest parameters go to the audio callback through a small ring buffer.
pub struct Control {
    producer: Producer<Parameters>,
    latest: Parameters,
}

impl Control {
    /// Returns the control side and the audio callback's half.
    pub fn pair() -> (Control, Consumer<Parameters>) {
        let (producer, consumer) = RingBuffer::new(8);
        (Control { producer, latest: Parameters::default() }, consumer)
    }

    /// Queues settings for the callback. A full queue keeps the old `latest`, so the same settings are retried next time.
    pub fn set(&mut self, parameters: Parameters) {
        if parameters == self.latest {
            return;
        }
        if self.producer.push(parameters).is_ok() {
            self.latest = parameters;
        }
    }
}

/// `WOW_BASE_DELAY_SECONDS`.
const WOW_BASE_SECONDS: f32 = 0.012;
/// Seconds of wow delay the callback can address.
const WOW_DELAY_SECONDS: f32 = 0.05;
/// The vinyl noise buffer is three seconds long in the original.
const NOISE_SECONDS: f32 = 3.0;

/// The audio side; created once when the output stream opens.
pub struct Processor {
    rate: f32,
    parameters: Parameters,
    bands: [Biquad; 10],
    highpass: Biquad,
    lowpass: Biquad,
    drive: f32,
    crush_steps: f32,
    wow_base: f32,
    wow_depth: f32,
    flutter_depth: f32,
    wow_phase: f32,
    delay: Vec<f32>,
    delay_position: usize,
    noise: Vec<f32>,
    noise_position: usize,
    noise_level: f32,
    noise_filter: Biquad,
    width: f32,
    reverb: Reverb,
    compress: bool,
    threshold_db: f32,
    ratio: f32,
    makeup: f32,
    envelope: f32,
}

impl Processor {
    /// Allocates every filter and delay line for `sample_rate`; effects start neutral.
    pub fn new(sample_rate: f32) -> Self {
        let rate = sample_rate.max(8000.0);
        let delay_len = (rate * WOW_DELAY_SECONDS) as usize + 2;
        let mut noise = vec![0.0; (rate * NOISE_SECONDS) as usize * 2];
        let mut state = 0x9e37_79b9_7f4a_7c15u64;
        for sample in &mut noise {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            let white = (state >> 40) as f32 / 8_388_608.0 - 1.0;
            let crackle = if (state & 0xffff) < 46 { white } else { 0.0 };
            *sample = white * 0.12 + crackle * 0.9;
        }
        let mut processor = Self {
            rate,
            parameters: Parameters::default(),
            bands: [Biquad::default(); 10],
            highpass: Biquad::default(),
            lowpass: Biquad::default(),
            drive: 0.0,
            crush_steps: 0.0,
            wow_base: 0.0,
            wow_depth: 0.0,
            flutter_depth: 0.0,
            wow_phase: 0.0,
            delay: vec![0.0; delay_len * 2],
            delay_position: 0,
            noise,
            noise_position: 0,
            noise_level: 0.0,
            noise_filter: Biquad::default(),
            width: 1.0,
            reverb: Reverb::new(rate),
            compress: false,
            threshold_db: 0.0,
            ratio: 1.0,
            makeup: 1.0,
            envelope: 0.0,
        };
        processor.apply(Parameters::default());
        processor
    }

    /// Takes every queued update, keeping the newest.
    pub fn drain(&mut self, consumer: &mut Consumer<Parameters>) {
        while let Ok(parameters) = consumer.pop() {
            self.apply(parameters);
        }
    }

    /// Recomputes coefficients and stage settings from `parameters`.
    pub fn apply(&mut self, parameters: Parameters) {
        if parameters == self.parameters {
            return;
        }
        self.parameters = parameters;
        let active = parameters.enabled;
        for (index, band) in self.bands.iter_mut().enumerate() {
            let gain = if active { parameters.gains[index] } else { 0.0 };
            let coefficients = if gain.abs() < f32::EPSILON {
                Coefficients::PASSTHROUGH
            } else {
                Coefficients::new(
                    BandShape::of(index),
                    EQUALIZER_FREQUENCIES[index],
                    EQUALIZER_Q,
                    gain.clamp(EQUALIZER_MIN_GAIN_DB, EQUALIZER_MAX_GAIN_DB),
                    self.rate,
                )
            };
            band.set(coefficients);
        }
        let effects = if active { parameters.effects.clamped() } else { AudioEffects::default() };
        self.highpass.set(Coefficients::cutoff(true, effects.highpass, 0.7, self.rate));
        self.lowpass.set(Coefficients::cutoff(false, effects.lowpass, 0.7, self.rate));
        self.drive = effects.drive;
        self.crush_steps = if effects.crush > 0.0 {
            let bits = 16.0 - effects.crush.sqrt() * 12.0;
            (2f32.powf(bits - 1.0)).max(2.0)
        } else {
            0.0
        };
        self.wow_base = if effects.wow > 0.0 { WOW_BASE_SECONDS } else { 0.0 };
        self.wow_depth = effects.wow * 0.0045;
        self.flutter_depth = effects.wow * 0.0006;
        self.noise_filter.set(Coefficients::cutoff(true, 2600.0, 0.5, self.rate));
        self.width = effects.width;
        self.reverb.set_amount(effects.space);
        self.compress = effects.punch > 0.0;
        self.threshold_db = -6.0 - effects.punch * 26.0;
        self.ratio = 1.0 + effects.punch * 7.0;
        self.makeup = 10f32.powf(effects.punch * 3.0 / 20.0);
        self.noise_level = if effects.noise > 0.0 { effects.noise.powf(1.4) * 0.55 } else { 0.0 };
    }

    /// Processes one stereo frame in place. Allocates nothing and takes no lock.
    #[inline]
    pub fn process(&mut self, left: &mut f32, right: &mut f32) {
        let mut l = *left;
        let mut r = *right;
        for band in &mut self.bands {
            l = band.process(l);
            r = band.process(r);
        }
        l = self.highpass.process(l);
        r = self.highpass.process(r);
        l = self.lowpass.process(l);
        r = self.lowpass.process(r);
        if self.drive > 0.0 {
            let shape = 1.0 + self.drive * 4.0;
            let scale = shape.tanh();
            let compensation = saturation_compensation(shape, scale);
            l = (shape * l).tanh() / scale * compensation;
            r = (shape * r).tanh() / scale * compensation;
        }
        if self.crush_steps > 0.0 {
            l = (l * self.crush_steps).round() / self.crush_steps;
            r = (r * self.crush_steps).round() / self.crush_steps;
        }
        if self.wow_base > 0.0 {
            (l, r) = self.wow(l, r);
        }
        self.delay_write(l, r);
        if self.noise_level > 0.0 {
            (l, r) = self.add_noise(l, r);
        }
        let direct = (1.0 + self.width) / 2.0;
        let cross = (1.0 - self.width) / 2.0;
        let (out_l, out_r) = (l * direct + r * cross, r * direct + l * cross);
        l = out_l;
        r = out_r;
        if self.compress {
            let gain = self.compressor(l.abs().max(r.abs()));
            l *= gain * self.makeup;
            r *= gain * self.makeup;
        }
        let (wet_l, wet_r) = self.reverb.process(l, r);
        *left = l * self.reverb.dry() + wet_l;
        *right = r * self.reverb.dry() + wet_r;
    }

    fn wow(&mut self, l: f32, r: f32) -> (f32, f32) {
        let frames = self.delay.len() / 2;
        if frames < 4 {
            return (l, r);
        }
        self.wow_phase = (self.wow_phase + 2.0 * PI * 0.7 / self.rate) % (2.0 * PI);
        let flutter_phase = (self.wow_phase * 9.714) % (2.0 * PI);
        let modulation = self.wow_phase.sin() * self.wow_depth + flutter_phase.sin() * self.flutter_depth;
        let samples = ((self.wow_base + modulation) * self.rate).max(1.0);
        let mut at = self.delay_position as f32 - samples;
        while at < 0.0 {
            at += frames as f32;
        }
        let first = at.floor() as usize % frames;
        let second = (first + 1) % frames;
        let fraction = at - at.floor();
        let read = |channel: usize| {
            self.delay[first * 2 + channel] * (1.0 - fraction) + self.delay[second * 2 + channel] * fraction
        };
        (read(0), read(1))
    }

    fn delay_write(&mut self, l: f32, r: f32) {
        let frames = self.delay.len() / 2;
        if frames == 0 {
            return;
        }
        self.delay[self.delay_position * 2] = l;
        self.delay[self.delay_position * 2 + 1] = r;
        self.delay_position = (self.delay_position + 1) % frames;
    }

    fn add_noise(&mut self, l: f32, r: f32) -> (f32, f32) {
        let frames = self.noise.len() / 2;
        if frames == 0 {
            return (l, r);
        }
        let at = self.noise_position % frames;
        let left = self.noise[at * 2];
        let right = self.noise[at * 2 + 1];
        self.noise_position = (at + 1) % frames;
        let filtered = self.noise_filter.process((left + right) * 0.5);
        let gain = self.noise_level;
        (l + (left + filtered * 0.35) * gain, r + (right + filtered * 0.35) * gain)
    }

    fn compressor(&mut self, peak: f32) -> f32 {
        let attack = 1.0 - (-1.0 / (self.rate * 0.008)).exp();
        let release = 1.0 - (-1.0 / (self.rate * 0.18)).exp();
        let target = peak.max(1e-6);
        let coefficient = if target > self.envelope { attack } else { release };
        self.envelope += (target - self.envelope) * coefficient;
        let level_db = 20.0 * self.envelope.log10();
        if level_db <= self.threshold_db {
            return 1.0;
        }
        let over = level_db - self.threshold_db;
        let gain_db = self.threshold_db + over / self.ratio - level_db;
        10f32.powf(gain_db / 20.0)
    }
}

/// The saturation curve is loudness matched; measured once per parameter change, never per frame.
fn saturation_compensation(shape: f32, scale: f32) -> f32 {
    let samples = 256;
    let mut energy = 0.0;
    for index in 0..samples {
        let input = 0.5 * (2.0 * PI * index as f32 / samples as f32).sin();
        let shaped = (shape * input).tanh() / scale;
        energy += shaped * shaped;
    }
    let rms = (energy / samples as f32).sqrt();
    if rms > 0.0 { (0.5 / 2f32.sqrt()) / rms } else { 1.0 }
}

/// A small stereo reverb in the original's place: four damped combs and two allpasses a side.
struct Reverb {
    combs: [[Comb; 4]; 2],
    allpasses: [[Allpass; 2]; 2],
    amount: f32,
}

impl Reverb {
    fn new(rate: f32) -> Self {
        let comb_delays = [0.0297, 0.0371, 0.0411, 0.0437];
        let allpass_delays = [0.005, 0.0017];
        let mut combs = [std::array::from_fn(|_| Comb::default()), std::array::from_fn(|_| Comb::default())];
        let mut allpasses = [std::array::from_fn(|_| Allpass::default()), std::array::from_fn(|_| Allpass::default())];
        for channel in 0..2 {
            for (index, delay) in comb_delays.iter().enumerate() {
                let spread = 1.0 + channel as f32 * 0.031;
                combs[channel][index] = Comb::new((rate * delay * spread) as usize);
            }
            for (index, delay) in allpass_delays.iter().enumerate() {
                let spread = 1.0 + channel as f32 * 0.019;
                allpasses[channel][index] = Allpass::new((rate * delay * spread) as usize);
            }
        }
        Self { combs, allpasses, amount: 0.0 }
    }

    fn set_amount(&mut self, amount: f32) {
        self.amount = amount;
    }

    fn dry(&self) -> f32 {
        1.0 - self.amount * 0.2
    }

    fn process(&mut self, l: f32, r: f32) -> (f32, f32) {
        if self.amount <= 0.0 {
            return (0.0, 0.0);
        }
        let input = (l + r) * 0.5;
        let mut out = [0.0; 2];
        for (channel, slot) in out.iter_mut().enumerate() {
            let mut sum = 0.0;
            for comb in &mut self.combs[channel] {
                sum += comb.process(input);
            }
            sum *= 0.25;
            for allpass in &mut self.allpasses[channel] {
                sum = allpass.process(sum);
            }
            *slot = sum * self.amount * 0.75;
        }
        (out[0], out[1])
    }
}

#[derive(Clone)]
struct Comb {
    buffer: Vec<f32>,
    position: usize,
    damp: f32,
}

impl Comb {
    fn new(len: usize) -> Self {
        Self { buffer: vec![0.0; len.max(1)], position: 0, damp: 0.0 }
    }

    fn process(&mut self, input: f32) -> f32 {
        let delayed = self.buffer[self.position];
        self.damp = delayed * 0.7 + self.damp * 0.3;
        self.buffer[self.position] = input + self.damp * 0.8;
        self.position = (self.position + 1) % self.buffer.len();
        delayed
    }
}

impl Default for Comb {
    fn default() -> Self {
        Self { buffer: vec![0.0; 1], position: 0, damp: 0.0 }
    }
}

#[derive(Clone)]
struct Allpass {
    buffer: Vec<f32>,
    position: usize,
}

impl Allpass {
    fn new(len: usize) -> Self {
        Self { buffer: vec![0.0; len.max(1)], position: 0 }
    }

    fn process(&mut self, input: f32) -> f32 {
        let delayed = self.buffer[self.position];
        self.buffer[self.position] = input + delayed * 0.5;
        self.position = (self.position + 1) % self.buffer.len();
        delayed - input
    }
}

impl Default for Allpass {
    fn default() -> Self {
        Self { buffer: vec![0.0; 1], position: 0 }
    }
}
