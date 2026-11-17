// Original Beat This log-mel and drum/vocal measurements, streamed without retaining a track's PCM.
use std::sync::Arc;

use folia_core::{DrumHits, StemAnalysis, VocalEnvelope};
use realfft::{RealFftPlanner, RealToComplex, num_complex::Complex};

use crate::AnalysisError;
use crate::audio::Resampled;

pub const FPS: usize = 50;
pub const BANDS: usize = 128;
const FFT: usize = 1024;
const HOP: usize = 441;
const VOICE_HOP: usize = 882;

pub struct MelSpectrogram {
    pub data: Vec<f32>,
    pub frames: usize,
}

pub fn mel_spectrogram(mono: &[f32]) -> Result<MelSpectrogram, AnalysisError> {
    let mut stream = MelStream::new();
    stream.push(mono)?;
    stream.finish()?;
    Ok(MelSpectrogram { frames: stream.frame, data: stream.take() })
}

fn hz_to_mel(hz: f64) -> f64 {
    if hz < 1000.0 { hz / (200.0 / 3.0) } else { 15.0 + (hz / 1000.0).ln() / (6.4f64.ln() / 27.0) }
}

fn mel_to_hz(mel: f64) -> f64 {
    if mel < 15.0 { (200.0 / 3.0) * mel } else { 1000.0 * ((6.4f64.ln() / 27.0) * (mel - 15.0)).exp() }
}

pub fn mel_centres() -> [f64; BANDS] {
    let low = hz_to_mel(30.0);
    let high = hz_to_mel(11000.0);
    std::array::from_fn(|index| mel_to_hz(low + (high - low) * (index + 1) as f64 / (BANDS + 1) as f64))
}

pub(crate) struct MelStream {
    plan: Arc<dyn RealToComplex<f64>>,
    input: Vec<f64>,
    spectrum: Vec<Complex<f64>>,
    scratch: Vec<Complex<f64>>,
    window: [f64; FFT],
    filters: [Vec<(usize, f64)>; BANDS],
    raw: Vec<f32>,
    from: usize,
    total: usize,
    frame: usize,
    rows: Vec<f32>,
}

impl MelStream {
    pub(crate) fn new() -> Self {
        let plan = RealFftPlanner::<f64>::new().plan_fft_forward(FFT);
        let input = plan.make_input_vec();
        let spectrum = plan.make_output_vec();
        let scratch = vec![Complex::default(); plan.get_scratch_len()];
        let window =
            std::array::from_fn(|index| 0.5 * (1.0 - (std::f64::consts::TAU * index as f64 / FFT as f64).cos()));
        let low = hz_to_mel(30.0);
        let high = hz_to_mel(11000.0);
        let corner = |index: usize| mel_to_hz(low + (high - low) * index as f64 / (BANDS + 1) as f64);
        let filters = std::array::from_fn(|band| {
            let (left, center, right) = (corner(band), corner(band + 1), corner(band + 2));
            (0..=FFT / 2)
                .filter_map(|bin| {
                    let hz = bin as f64 * 22050.0 / FFT as f64;
                    let weight = ((hz - left) / (center - left)).min((right - hz) / (right - center)).max(0.0);
                    (weight > 0.0).then_some((bin, weight))
                })
                .collect()
        });
        Self {
            plan,
            input,
            spectrum,
            scratch,
            window,
            filters,
            raw: Vec::new(),
            from: 0,
            total: 0,
            frame: 0,
            rows: Vec::new(),
        }
    }

    pub(crate) fn push(&mut self, mono: &[f32]) -> Result<(), AnalysisError> {
        if mono.iter().any(|value| !value.is_finite()) {
            return Err(AnalysisError::Model("non-finite PCM cannot be analysed".to_owned()));
        }
        self.raw.extend_from_slice(mono);
        self.total += mono.len();
        self.compute(false)
    }

    fn finish(&mut self) -> Result<(), AnalysisError> {
        self.compute(true)
    }

    fn take(&mut self) -> Vec<f32> {
        std::mem::take(&mut self.rows)
    }

    pub(crate) fn into_spectrogram(mut self) -> Result<MelSpectrogram, AnalysisError> {
        self.finish()?;
        Ok(MelSpectrogram { frames: self.frame, data: self.take() })
    }

    fn compute(&mut self, finish: bool) -> Result<(), AnalysisError> {
        if self.total == 0 {
            return Ok(());
        }
        let frames = if finish {
            self.total / HOP + 1
        } else if self.total <= FFT / 2 {
            0
        } else {
            (self.total - FFT / 2) / HOP + 1
        };
        while self.frame < frames {
            let center = (self.frame * HOP) as i64;
            for index in 0..FFT {
                let at = center + index as i64 - (FFT / 2) as i64;
                let at = if at < 0 {
                    (-at).min(self.total as i64 - 1)
                } else if at >= self.total as i64 {
                    (2 * self.total as i64 - at - 2).max(0)
                } else {
                    at
                } as usize;
                self.input[index] = f64::from(self.raw[at - self.from]) * self.window[index];
            }
            self.plan
                .process_with_scratch(&mut self.input, &mut self.spectrum, &mut self.scratch)
                .map_err(|error| AnalysisError::Model(error.to_string()))?;
            for filter in &self.filters {
                let energy: f64 =
                    filter.iter().map(|(bin, weight)| self.spectrum[*bin].norm() / (FFT as f64).sqrt() * weight).sum();
                self.rows.push((1000.0 * energy.max(1e-10)).ln_1p() as f32);
            }
            self.frame += 1;
        }
        let keep_from = (self.frame * HOP).saturating_sub(FFT / 2).min(self.total);
        self.raw.drain(..keep_from - self.from);
        self.from = keep_from;
        Ok(())
    }
}

#[derive(Default)]
struct Levels {
    raw: Vec<f32>,
    from: usize,
    total: usize,
    db: Vec<f32>,
}

impl Levels {
    fn push(&mut self, samples: &[f32]) {
        self.raw.extend_from_slice(samples);
        self.total += samples.len();
        let frames = self.total.saturating_sub(VOICE_HOP) / VOICE_HOP + usize::from(self.total >= VOICE_HOP);
        self.compute(frames);
    }

    fn compute(&mut self, frames: usize) {
        while self.db.len() < frames {
            let center = self.db.len() * VOICE_HOP;
            let start = center.saturating_sub(VOICE_HOP);
            let end = (center + VOICE_HOP).min(self.total);
            let sum: f64 =
                self.raw[start - self.from..end - self.from].iter().map(|sample| f64::from(*sample).powi(2)).sum();
            self.db.push((10.0 * (sum / (2 * VOICE_HOP) as f64 + 1e-12).log10()) as f32);
        }
        let keep_from = (self.db.len() * VOICE_HOP).saturating_sub(VOICE_HOP).min(self.total);
        self.raw.drain(..keep_from - self.from);
        self.from = keep_from;
    }
}

pub struct StemFeatures {
    drums: Resampled,
    voice: Resampled,
    drum_mel: MelStream,
    voice_mel: MelStream,
    levels: Levels,
    previous: Option<[f32; BANDS]>,
    flux: Vec<f32>,
    bands: [Vec<f32>; 3],
    band_indices: [Vec<usize>; 3],
    drum_energy: Vec<f32>,
    mix_energy: Vec<f32>,
    samples: usize,
}

impl StemFeatures {
    pub fn new() -> Result<Self, AnalysisError> {
        let centers = mel_centres();
        let ranges = [(0.0, 150.0), (150.0, 4000.0), (6000.0, f64::INFINITY)];
        let band_indices = ranges.map(|(from, to)| {
            centers.iter().enumerate().filter_map(|(index, hz)| (*hz >= from && *hz < to).then_some(index)).collect()
        });
        Ok(Self {
            drums: Resampled::new(44100, 22050, 1)?,
            voice: Resampled::new(44100, 22050, 1)?,
            drum_mel: MelStream::new(),
            voice_mel: MelStream::new(),
            levels: Levels::default(),
            previous: None,
            flux: Vec::new(),
            bands: std::array::from_fn(|_| Vec::new()),
            band_indices,
            drum_energy: Vec::new(),
            mix_energy: Vec::new(),
            samples: 0,
        })
    }

    /// Accepts finalized overlap-add samples at 44.1kHz, in whole-track order.
    pub fn push(&mut self, drums: &[f32], voice: &[f32], mix: &[f32]) -> Result<(), AnalysisError> {
        if drums.len() != voice.len() || voice.len() != mix.len() {
            return Err(AnalysisError::Model("stem feature blocks have different lengths".to_owned()));
        }
        self.drums.push(drums)?;
        self.voice.push(voice)?;
        self.levels.push(voice);
        for (index, (drum, mix)) in drums.iter().zip(mix).enumerate() {
            let frame = (self.samples + index) / HOP;
            if self.drum_energy.len() <= frame {
                self.drum_energy.push(0.0);
                self.mix_energy.push(0.0);
            }
            self.drum_energy[frame] = (f64::from(self.drum_energy[frame]) + f64::from(*drum).powi(2)) as f32;
            self.mix_energy[frame] = (f64::from(self.mix_energy[frame]) + f64::from(*mix).powi(2)) as f32;
        }
        self.samples += drums.len();
        self.read_mel(false)
    }

    pub fn finish(mut self) -> Result<StemAnalysis, AnalysisError> {
        self.drums.finish()?;
        self.voice.finish()?;
        self.read_mel(true)?;
        self.levels.compute(self.voice_mel.frame);
        let top = quantile(&self.levels.db, 0.97);
        let onsets = vocal_onsets(&self.flux, &self.levels.db, top);
        let vocal = VocalEnvelope { fps: FPS as u32, db: self.levels.db, top, onsets, from: 0 };
        let drums = (drum_coverage(&self.drum_energy, &self.mix_energy) >= 0.08).then(|| pick_drums(&self.bands));
        Ok(StemAnalysis { drums, vocal })
    }

    pub fn live_drums(&self) -> Option<DrumHits> {
        if self.samples < 441_000 || drum_coverage(&self.drum_energy, &self.mix_energy) < 0.08 {
            return None;
        }
        let frames = self.bands[0].len().saturating_sub(10);
        let tracks = self.bands.each_ref().map(|track| track[..frames].to_vec());
        Some(pick_drums(&tracks))
    }

    fn read_mel(&mut self, finish: bool) -> Result<(), AnalysisError> {
        self.drum_mel.push(&self.drums.take())?;
        self.voice_mel.push(&self.voice.take())?;
        if finish {
            self.drum_mel.finish()?;
            self.voice_mel.finish()?;
        }
        for row in self.drum_mel.take().as_chunks::<BANDS>().0 {
            for (band, indices) in self.band_indices.iter().enumerate() {
                let sum: f64 = indices.iter().map(|index| f64::from(row[*index])).sum();
                self.bands[band].push(if indices.is_empty() { 0.0 } else { (sum / indices.len() as f64) as f32 });
            }
        }
        for row in self.voice_mel.take().as_chunks::<BANDS>().0 {
            let flux = self.previous.as_ref().map_or(0.0, |previous| {
                row.iter().zip(previous).map(|(a, b)| (f64::from(*a) - f64::from(*b)).max(0.0)).sum::<f64>()
                    / BANDS as f64
            });
            self.flux.push(flux as f32);
            self.previous = Some(*row);
        }
        Ok(())
    }
}

fn quantile(values: &[f32], share: f64) -> f32 {
    let mut sorted = values.to_vec();
    sorted.sort_by(f32::total_cmp);
    sorted.get(((share * sorted.len() as f64) as usize).min(sorted.len().saturating_sub(1))).copied().unwrap_or(-120.0)
}

fn vocal_onsets(flux: &[f32], db: &[f32], top: f32) -> Vec<u32> {
    let n = flux.len().min(db.len());
    let count = n.max(1) as f64;
    let mean = flux[..n].iter().map(|value| f64::from(*value)).sum::<f64>() / count;
    let delta = 0.7 * (flux[..n].iter().map(|value| (f64::from(*value) - mean).powi(2)).sum::<f64>() / count).sqrt();
    let mut onsets = Vec::new();
    for frame in 3..n.saturating_sub(3) {
        if db[frame] <= top - 28.0
            || onsets.last().is_some_and(|last| frame - (*last as usize) < 3)
            || db[frame + 1] < db[frame - 1] - 3.0
        {
            continue;
        }
        if flux[frame - 3..=frame + 3].iter().any(|value| *value > flux[frame]) {
            continue;
        }
        let from = frame.saturating_sub(10);
        let local =
            flux[from..=frame + 3].iter().map(|value| f64::from(*value)).sum::<f64>() / (frame + 4 - from) as f64;
        if f64::from(flux[frame]) >= local + delta {
            onsets.push(frame as u32);
        }
    }
    onsets
}

fn rise_shares(track: &[f32]) -> Vec<f32> {
    let mut rise = Vec::with_capacity(track.len());
    for frame in 0..track.len() {
        let peak = track[frame.saturating_sub(1)..(frame + 3).min(track.len())].iter().copied().fold(0.0f32, f32::max);
        let from = frame.saturating_sub(6);
        let to = frame.saturating_sub(1);
        let base = if to > from {
            track[from..to].iter().map(|value| f64::from(*value)).sum::<f64>() / (to - from) as f64
        } else {
            0.0
        };
        rise.push((f64::from(peak) - base).max(0.0) as f32);
    }
    let positive: Vec<_> = rise.iter().copied().filter(|value| *value > 0.0).collect();
    let top = if positive.is_empty() { 1.0 } else { quantile(&positive, 0.99).max(f32::MIN_POSITIVE) };
    rise.into_iter().map(|value| (value / top).min(1.0)).collect()
}

fn pick_drums(tracks: &[Vec<f32>; 3]) -> DrumHits {
    let frames = tracks[0].len();
    let bands = tracks.each_ref().map(|track| rise_shares(track));
    let mut top = vec![0.0f32; frames];
    let mut kinds = vec![0u8; frames];
    for frame in 0..frames {
        for (kind, band) in bands.iter().enumerate() {
            if band[frame] > top[frame] {
                top[frame] = band[frame];
                kinds[frame] = kind as u8;
            }
        }
    }
    let mut hits = DrumHits::default();
    for frame in 0..frames {
        let strength = top[frame];
        if strength < 0.15
            || (frame.saturating_sub(3)..=(frame + 3).min(frames - 1))
                .any(|at| top[at] > strength || (top[at] == strength && at < frame))
        {
            continue;
        }
        let time = frame as f64 / FPS as f64;
        if let Some(last) = hits.times.len().checked_sub(1)
            && time - hits.times[last] < 0.09
        {
            if strength > hits.strengths[last] {
                hits.times[last] = time;
                hits.strengths[last] = strength;
                hits.kinds[last] = kinds[frame];
            }
            continue;
        }
        hits.times.push(time);
        hits.strengths.push(strength);
        hits.kinds.push(kinds[frame]);
    }
    hits
}

fn drum_coverage(drums: &[f32], mix: &[f32]) -> f64 {
    let windows: Vec<_> = (0..mix.len())
        .step_by(25)
        .map(|from| {
            let to = (from + 25).min(mix.len());
            (
                drums[from..to].iter().map(|value| f64::from(*value)).sum::<f64>(),
                mix[from..to].iter().map(|value| f64::from(*value)).sum::<f64>(),
            )
        })
        .collect();
    let mut levels: Vec<_> = windows.iter().map(|(_, mix)| *mix).collect();
    levels.sort_by(f64::total_cmp);
    let floor = levels.get(levels.len() / 2).copied().unwrap_or(0.0) * 0.05;
    let audible: Vec<_> = windows.into_iter().filter(|(_, mix)| *mix > floor && *mix > 0.0).collect();
    if audible.is_empty() {
        return 0.0;
    }
    audible.iter().filter(|(drum, mix)| *drum >= 0.03 * mix).count() as f64 / audible.len() as f64
}
