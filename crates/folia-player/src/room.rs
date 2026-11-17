// Zero-latency true-stereo convolution: direct FIR head and preallocated partitioned FFT tail.

use std::sync::Arc;

use folia_core::RoomImpulseResponse;
use realfft::num_complex::Complex32;
use realfft::{ComplexToReal, RealFftPlanner, RealToComplex};

const BLOCK: usize = 128;
const FFT: usize = BLOCK * 2;
const BINS: usize = BLOCK + 1;

#[derive(Debug, thiserror::Error)]
pub enum RoomError {
    #[error("invalid room impulse response")]
    Invalid,
    #[error("room FFT buffers do not match their plan")]
    Fft,
    #[error("room response sample rate does not match the output device")]
    SampleRate,
}

pub struct RoomConvolver {
    sample_rate: u32,
    length: usize,
    gain: f32,
    forward: Arc<dyn RealToComplex<f32>>,
    inverse: Arc<dyn ComplexToReal<f32>>,
    forward_scratch: Vec<Complex32>,
    inverse_scratch: Vec<Complex32>,
    coefficients: [[f32; BLOCK]; 4],
    spectra: [Vec<Complex32>; 4],
    history: [Vec<Complex32>; 2],
    history_at: usize,
    partitions: usize,
    samples: [[f32; BLOCK]; 2],
    recent: [[f32; BLOCK]; 2],
    recent_at: usize,
    offset: usize,
    tail: [[f32; BLOCK]; 2],
    overlap: [[f32; BLOCK]; 2],
    fft_input: Vec<f32>,
    fft_output: Vec<f32>,
    sum: Vec<Complex32>,
}

impl RoomConvolver {
    /// Prepares all FFT plans, spectra and memory on a worker before handing the convolver to audio output.
    pub fn new(impulse: &RoomImpulseResponse) -> Result<Self, RoomError> {
        let length = impulse.channels[0].len();
        if !(8000..=192000).contains(&impulse.sample_rate)
            || length == 0
            || length > impulse.sample_rate as usize * 2
            || !impulse.gain.is_finite()
            || impulse.gain < 0.0
            || impulse
                .channels
                .iter()
                .any(|channel| channel.len() != length || channel.iter().any(|value| !value.is_finite()))
        {
            return Err(RoomError::Invalid);
        }
        let mut planner = RealFftPlanner::new();
        let forward = planner.plan_fft_forward(FFT);
        let inverse = planner.plan_fft_inverse(FFT);
        let partitions = length.div_ceil(BLOCK).saturating_sub(1);
        let mut result = Self {
            sample_rate: impulse.sample_rate,
            length,
            gain: impulse.gain,
            forward_scratch: forward.make_scratch_vec(),
            inverse_scratch: inverse.make_scratch_vec(),
            coefficients: [[0.0; BLOCK]; 4],
            spectra: std::array::from_fn(|_| vec![Complex32::default(); partitions * BINS]),
            history: std::array::from_fn(|_| vec![Complex32::default(); partitions * BINS]),
            history_at: 0,
            partitions,
            samples: [[0.0; BLOCK]; 2],
            recent: [[0.0; BLOCK]; 2],
            recent_at: 0,
            offset: 0,
            tail: [[0.0; BLOCK]; 2],
            overlap: [[0.0; BLOCK]; 2],
            fft_input: forward.make_input_vec(),
            fft_output: inverse.make_output_vec(),
            sum: inverse.make_input_vec(),
            forward,
            inverse,
        };
        for (channel, samples) in impulse.channels.iter().enumerate() {
            result.coefficients[channel][..length.min(BLOCK)].copy_from_slice(&samples[..length.min(BLOCK)]);
            for partition in 0..partitions {
                result.fft_input.fill(0.0);
                let start = (partition + 1) * BLOCK;
                let count = BLOCK.min(length - start);
                result.fft_input[..count].copy_from_slice(&samples[start..start + count]);
                result
                    .forward
                    .process_with_scratch(
                        &mut result.fft_input,
                        &mut result.spectra[channel][partition * BINS..(partition + 1) * BINS],
                        &mut result.forward_scratch,
                    )
                    .map_err(|_| RoomError::Fft)?;
            }
        }
        Ok(result)
    }

    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }
    pub fn length(&self) -> usize {
        self.length
    }

    pub fn reset(&mut self) {
        for history in &mut self.history {
            history.fill(Complex32::default());
        }
        self.samples = [[0.0; BLOCK]; 2];
        self.recent = [[0.0; BLOCK]; 2];
        self.tail = [[0.0; BLOCK]; 2];
        self.overlap = [[0.0; BLOCK]; 2];
        self.offset = 0;
        self.recent_at = 0;
        self.history_at = 0;
    }

    /// Processes one stereo frame without allocating, locking or changing the signal's arrival time.
    pub fn process(&mut self, input: [f32; 2]) -> Result<[f32; 2], RoomError> {
        let mut output = [self.tail[0][self.offset], self.tail[1][self.offset]];
        for (channel, value) in input.into_iter().enumerate() {
            self.recent[channel][self.recent_at] = value;
            self.samples[channel][self.offset] = value;
        }
        for tap in 0..BLOCK.min(self.length) {
            let at = (self.recent_at + BLOCK - tap) % BLOCK;
            let left = self.recent[0][at];
            let right = self.recent[1][at];
            output[0] += left * self.coefficients[0][tap] + right * self.coefficients[2][tap];
            output[1] += left * self.coefficients[1][tap] + right * self.coefficients[3][tap];
        }
        self.recent_at = (self.recent_at + 1) % BLOCK;
        self.offset += 1;
        if self.offset == BLOCK {
            self.offset = 0;
            if self.partitions > 0 {
                self.block()?;
            }
        }
        Ok(output.map(|sample| sample * self.gain))
    }

    fn block(&mut self) -> Result<(), RoomError> {
        for channel in 0..2 {
            self.fft_input.fill(0.0);
            self.fft_input[..BLOCK].copy_from_slice(&self.samples[channel]);
            let start = self.history_at * BINS;
            self.forward
                .process_with_scratch(
                    &mut self.fft_input,
                    &mut self.history[channel][start..start + BINS],
                    &mut self.forward_scratch,
                )
                .map_err(|_| RoomError::Fft)?;
        }
        for output in 0..2 {
            self.sum.fill(Complex32::default());
            for partition in 0..self.partitions {
                let frame = (self.history_at + self.partitions - partition) % self.partitions;
                for (bin, sum) in self.sum.iter_mut().enumerate() {
                    *sum += self.history[0][frame * BINS + bin] * self.spectra[output][partition * BINS + bin]
                        + self.history[1][frame * BINS + bin] * self.spectra[2 + output][partition * BINS + bin];
                }
            }
            self.inverse
                .process_with_scratch(&mut self.sum, &mut self.fft_output, &mut self.inverse_scratch)
                .map_err(|_| RoomError::Fft)?;
            for sample in 0..BLOCK {
                self.tail[output][sample] = self.fft_output[sample] / FFT as f32 + self.overlap[output][sample];
                self.overlap[output][sample] = self.fft_output[sample + BLOCK] / FFT as f32;
            }
        }
        self.history_at = (self.history_at + 1) % self.partitions;
        Ok(())
    }
}
