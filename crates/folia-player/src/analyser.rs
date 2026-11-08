// Web Audio AnalyserNode for one animation frame: 2048-point Blackman FFT, byte bins, five bands.

use folia_core::AudioFrame;
use realfft::num_complex::Complex;
use realfft::{RealFftPlanner, RealToComplex};
use rtrb::{Consumer, Producer, RingBuffer};

/// Samples the output callback keeps for the analyser. A full buffer drops new samples.
pub const TAP_CAPACITY: usize = 16384;

const FFT: usize = 2048;
const BINS: usize = 1024;
const SMOOTHING: f32 = 0.6;
/// Original `getEnergy` divisor, written for 44.1 kHz (`44100 / 2048 ≈ 21.53`). Not the device rate.
const HZ_PER_BIN: f32 = 21.5;

/// Copies what fits into `producer` and drops the rest. Does not allocate or block.
pub fn push_samples(producer: &mut Producer<f32>, samples: &[f32]) {
    let count = samples.len().min(producer.slots());
    if count == 0 {
        return;
    }
    if let Ok(chunk) = producer.write_chunk_uninit(count) {
        chunk.fill_from_iter(samples[..count].iter().copied());
    }
}

/// Reads the post-volume tap and turns the latest 2048 mono frames into one [`AudioFrame`].
pub struct Analyser {
    consumer: Consumer<f32>,
    history: [f32; FFT],
    /// Next write, and the oldest sample once `filled` reaches `FFT`.
    write: usize,
    filled: usize,
    window: [f32; FFT],
    smooth: [f32; BINS],
    plan: std::sync::Arc<dyn RealToComplex<f32>>,
    input: Vec<f32>,
    output: Vec<Complex<f32>>,
    scratch: Vec<Complex<f32>>,
    frame: AudioFrame,
}

impl Analyser {
    /// The producer belongs to the output callback. This end is read from the UI thread.
    pub fn pair() -> (Producer<f32>, Self) {
        let (producer, consumer) = RingBuffer::new(TAP_CAPACITY);
        (producer, Self::new(consumer))
    }

    fn new(consumer: Consumer<f32>) -> Self {
        let mut planner = RealFftPlanner::<f32>::new();
        let plan = planner.plan_fft_forward(FFT);
        let input = plan.make_input_vec();
        let output = plan.make_output_vec();
        let scratch = vec![Complex::default(); plan.get_scratch_len()];
        let mut window = [0.0; FFT];
        for (index, sample) in window.iter_mut().enumerate() {
            let n = index as f32;
            let size = FFT as f32;
            *sample = 0.42 - 0.5 * (std::f32::consts::TAU * n / size).cos()
                + 0.08 * (2.0 * std::f32::consts::TAU * n / size).cos();
        }
        Self {
            consumer,
            history: [0.0; FFT],
            write: 0,
            filled: 0,
            window,
            smooth: [0.0; BINS],
            plan,
            input,
            output,
            scratch,
            frame: AudioFrame::SILENT,
        }
    }

    /// Drains the tap, then either analyses (`playing`) or writes the idle breath.
    pub fn frame(&mut self, playing: bool, wall_ms: f64) -> &AudioFrame {
        self.drain();
        if !playing {
            self.frame = AudioFrame::breath(wall_ms);
            return &self.frame;
        }
        self.analyse();
        &self.frame
    }

    fn drain(&mut self) {
        let count = self.consumer.slots();
        if count == 0 {
            return;
        }
        let Ok(chunk) = self.consumer.read_chunk(count) else {
            return;
        };
        let (first, second) = chunk.as_slices();
        for &sample in first.iter().chain(second) {
            self.history[self.write] = sample;
            self.write = (self.write + 1) % FFT;
            if self.filled < FFT {
                self.filled += 1;
            }
        }
        chunk.commit_all();
    }

    fn analyse(&mut self) {
        self.window_into_input();
        if self.plan.process_with_scratch(&mut self.input, &mut self.output, &mut self.scratch).is_err() {
            self.frame = AudioFrame { has_spectrum: true, ..AudioFrame::SILENT };
            return;
        }
        let mut spectrum = [0u8; BINS];
        let scale = 1.0 / FFT as f32;
        for (bin, slot) in spectrum.iter_mut().enumerate() {
            let complex = self.output[bin];
            let magnitude = complex.norm() * scale;
            let smoothed = SMOOTHING * self.smooth[bin] + (1.0 - SMOOTHING) * magnitude;
            self.smooth[bin] = smoothed;
            *slot = byte(smoothed);
        }
        let bass = energy(&spectrum, 20.0, 150.0);
        let low_mid = energy(&spectrum, 150.0, 400.0);
        let mid = energy(&spectrum, 400.0, 1200.0);
        let vocal = energy(&spectrum, 1000.0, 3500.0);
        let treble = energy(&spectrum, 3500.0, 12000.0);
        self.frame = AudioFrame {
            power: compress((bass + low_mid) / 2.0, 3.0),
            bass: compress(bass, 1.8),
            low_mid: compress(low_mid, 2.0),
            mid: compress(mid, 2.0),
            vocal: compress(vocal, 1.5),
            treble: compress(treble, 2.0),
            spectrum,
            has_spectrum: true,
        };
    }

    fn window_into_input(&mut self) {
        if self.filled < FFT {
            let pad = FFT - self.filled;
            self.input[..pad].fill(0.0);
            for index in 0..self.filled {
                self.input[pad + index] = self.history[index] * self.window[pad + index];
            }
            return;
        }
        for index in 0..FFT {
            let sample = self.history[(self.write + index) % FFT];
            self.input[index] = sample * self.window[index];
        }
    }
}

fn energy(bins: &[u8; BINS], min_hz: f32, max_hz: f32) -> f32 {
    let start = (min_hz / HZ_PER_BIN).floor() as isize;
    let end = (max_hz / HZ_PER_BIN).floor() as isize;
    if end < start {
        return 0.0;
    }
    let start = start.max(0) as usize;
    let end = (end as usize).min(BINS - 1);
    if start > end || start >= BINS {
        return 0.0;
    }
    let mut sum = 0.0f32;
    let mut count = 0.0f32;
    for bin in &bins[start..=end] {
        sum += f32::from(*bin);
        count += 1.0;
    }
    if count > 0.0 { sum / count } else { 0.0 }
}

fn compress(value: f32, boost: f32) -> f32 {
    (value / 255.0).powf(boost) * 255.0
}

fn byte(magnitude: f32) -> u8 {
    if magnitude <= 0.0 {
        return 0;
    }
    let decibels = 20.0 * f64::from(magnitude).log10();
    let scaled = (255.0 / 70.0) * (decibels + 100.0);
    let floored = scaled.floor();
    if floored <= 0.0 {
        0
    } else if floored >= 255.0 {
        255
    } else {
        floored as u8
    }
}
