// Reuses the playback decoder and channel mapping, with bounded resampling that preserves offline timing.
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use folia_core::AudioInput;
use folia_player::DecodedSource;
use rubato::audioadapter_buffers::direct::InterleavedSlice;
use rubato::{Fft, FixedSync, Indexing, Resampler};

use crate::AnalysisError;

pub struct AudioReader {
    source: DecodedSource,
    cancelled: Arc<AtomicBool>,
    target_rate: u32,
    source_rate: Option<u32>,
    resampler: Option<Resampled>,
    mapped: Vec<f32>,
    ready: Vec<f32>,
    offset: usize,
    finished: bool,
}

impl AudioReader {
    pub fn open(input: &AudioInput, target_rate: u32, cancelled: Arc<AtomicBool>) -> Result<Self, AnalysisError> {
        let source = DecodedSource::open_input(input, cancelled.clone())?;
        Ok(Self {
            source,
            cancelled,
            target_rate,
            source_rate: None,
            resampler: None,
            mapped: Vec::new(),
            ready: Vec::new(),
            offset: 0,
            finished: false,
        })
    }

    /// Appends up to `frames` stereo frames. Returns fewer only at the verified end of the source.
    pub fn read(&mut self, frames: usize, left: &mut Vec<f32>, right: &mut Vec<f32>) -> Result<usize, AnalysisError> {
        let mut read = 0;
        while read < frames {
            if self.cancelled.load(Ordering::Acquire) {
                return Err(AnalysisError::Cancelled);
            }
            if self.offset == self.ready.len() {
                if self.finished {
                    break;
                }
                self.fill()?;
                continue;
            }
            let count = (frames - read).min((self.ready.len() - self.offset) / 2);
            for samples in self.ready[self.offset..self.offset + count * 2].as_chunks::<2>().0 {
                left.push(samples[0]);
                right.push(samples[1]);
            }
            self.offset += count * 2;
            read += count;
        }
        Ok(read)
    }

    fn fill(&mut self) -> Result<(), AnalysisError> {
        self.offset = 0;
        self.ready.clear();
        let Some(block) = self.source.next_packet()? else {
            self.finished = true;
            if let Some(resampler) = &mut self.resampler {
                resampler.finish()?;
                self.ready = resampler.take();
            }
            return Ok(());
        };
        if self.source_rate.is_some_and(|rate| rate != block.rate) {
            return Err(AnalysisError::Model("audio sample rate changed during analysis".to_owned()));
        }
        if self.resampler.is_none() {
            self.resampler = Some(Resampled::new(block.rate, self.target_rate, 2)?);
            self.source_rate = Some(block.rate);
        }
        self.mapped.clear();
        folia_player::map_channels(block.samples, block.channels, 2, &mut self.mapped);
        if let Some(resampler) = &mut self.resampler {
            resampler.push(&self.mapped)?;
            self.ready = resampler.take();
        }
        Ok(())
    }
}

pub(crate) struct Resampled {
    process: Option<Fft<f32>>,
    channels: usize,
    input_rate: u32,
    output_rate: u32,
    input_frames: usize,
    produced: usize,
    skip: usize,
    pending: Vec<f32>,
    scratch: Vec<f32>,
    output: Vec<f32>,
}

impl Resampled {
    pub(crate) fn new(input_rate: u32, output_rate: u32, channels: usize) -> Result<Self, AnalysisError> {
        if input_rate == 0 || output_rate == 0 || channels == 0 {
            return Err(AnalysisError::Resample("invalid sample format".to_owned()));
        }
        let process = if input_rate == output_rate {
            None
        } else {
            Some(
                Fft::new(input_rate as usize, output_rate as usize, 1024, channels, FixedSync::Input)
                    .map_err(resample_error)?,
            )
        };
        let skip = process.as_ref().map_or(0, Resampler::output_delay);
        Ok(Self {
            process,
            channels,
            input_rate,
            output_rate,
            input_frames: 0,
            produced: 0,
            skip,
            pending: Vec::new(),
            scratch: Vec::new(),
            output: Vec::new(),
        })
    }

    pub(crate) fn push(&mut self, input: &[f32]) -> Result<(), AnalysisError> {
        if !input.len().is_multiple_of(self.channels) || input.iter().any(|sample| !sample.is_finite()) {
            return Err(AnalysisError::Resample("invalid PCM block".to_owned()));
        }
        self.input_frames += input.len() / self.channels;
        if self.process.is_none() {
            self.output.extend_from_slice(input);
            self.produced += input.len() / self.channels;
            return Ok(());
        }
        self.pending.extend_from_slice(input);
        while self
            .process
            .as_ref()
            .is_some_and(|process| self.pending.len() / self.channels >= process.input_frames_next())
        {
            self.step(false)?;
        }
        Ok(())
    }

    pub(crate) fn finish(&mut self) -> Result<(), AnalysisError> {
        if self.process.is_none() {
            return Ok(());
        }
        while self.produced < self.expected() {
            self.step(true)?;
        }
        Ok(())
    }

    pub(crate) fn take(&mut self) -> Vec<f32> {
        std::mem::take(&mut self.output)
    }

    fn expected(&self) -> usize {
        (self.input_frames as u128 * u128::from(self.output_rate)).div_ceil(u128::from(self.input_rate)) as usize
    }

    fn step(&mut self, final_chunk: bool) -> Result<(), AnalysisError> {
        let expected = self.expected();
        let Some(process) = &mut self.process else { return Ok(()) };
        let frames = self.pending.len() / self.channels;
        let input = InterleavedSlice::new(&self.pending, self.channels, frames).map_err(resample_error)?;
        let max_out = process.output_frames_max();
        self.scratch.resize(max_out * self.channels, 0.0);
        let mut output =
            InterleavedSlice::new_mut(&mut self.scratch, self.channels, max_out).map_err(resample_error)?;
        let index = Indexing {
            input_offset: 0,
            output_offset: 0,
            partial_len: final_chunk.then_some(frames),
            active_channels_mask: None,
        };
        let (used, written) = process.process_into_buffer(&input, &mut output, Some(&index)).map_err(resample_error)?;
        self.pending.drain(..used.min(frames) * self.channels);
        let skip = self.skip.min(written);
        self.skip -= skip;
        let keep =
            if final_chunk { (written - skip).min(expected.saturating_sub(self.produced)) } else { written - skip };
        self.output.extend_from_slice(&self.scratch[skip * self.channels..(skip + keep) * self.channels]);
        self.produced += keep;
        Ok(())
    }
}

fn resample_error(error: impl std::fmt::Display) -> AnalysisError {
    AnalysisError::Resample(error.to_string())
}
