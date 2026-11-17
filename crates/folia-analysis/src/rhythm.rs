// Beat This chunk overlap, keep-first stitching and seven-frame peak picking from the original analysis contract.
use std::sync::atomic::{AtomicBool, Ordering};

use crate::AnalysisError;
use crate::beat_this::{BeatThis, MAX_FRAMES, MEL_BINS};
use crate::signal::{FPS, MelSpectrogram};

pub struct BeatGrid {
    pub beats: Vec<f64>,
    pub downbeats: Vec<f64>,
}

pub(crate) struct Predicted {
    pub grid: BeatGrid,
    pub logits: Vec<f32>,
}

pub fn beats_from_logits(beat: &[f32], downbeat: &[f32]) -> BeatGrid {
    let beats = peaks(beat);
    if beats.is_empty() {
        return BeatGrid { beats, downbeats: Vec::new() };
    }
    let mut downbeats = Vec::new();
    for time in peaks(downbeat) {
        let next = beats.partition_point(|beat| *beat < time);
        let before = beats[next.saturating_sub(1)];
        let after = beats[next.min(beats.len() - 1)];
        let snapped = if (after - time).abs() < (before - time).abs() { after } else { before };
        if downbeats.last() != Some(&snapped) {
            downbeats.push(snapped);
        }
    }
    BeatGrid { beats, downbeats }
}

fn peaks(logits: &[f32]) -> Vec<f64> {
    let mut result = Vec::new();
    let mut group: Option<(f64, usize)> = None;
    for (index, value) in logits.iter().enumerate() {
        if *value <= 0.0
            || !value.is_finite()
            || logits[index.saturating_sub(3)..(index + 4).min(logits.len())].iter().any(|other| other > value)
        {
            continue;
        }
        if let Some((mean, count)) = &mut group
            && index as f64 - *mean <= 1.0
        {
            *count += 1;
            *mean += (index as f64 - *mean) / (*count as f64);
            continue;
        }
        if let Some((mean, _)) = group {
            result.push(mean.round() / FPS as f64);
        }
        group = Some((index as f64, 1));
    }
    if let Some((mean, _)) = group {
        result.push(mean.round() / FPS as f64);
    }
    result
}

pub(crate) fn predict(
    model: &mut BeatThis,
    mel: &MelSpectrogram,
    cancelled: &AtomicBool,
    progress: impl Fn(f64) -> bool,
) -> Result<Predicted, AnalysisError> {
    if mel.data.len() != mel.frames * MEL_BINS {
        return Err(AnalysisError::Model("invalid mel spectrogram shape".to_owned()));
    }
    if mel.frames == 0 {
        return Ok(Predicted { grid: BeatGrid { beats: Vec::new(), downbeats: Vec::new() }, logits: Vec::new() });
    }
    let stride = MAX_FRAMES - 12;
    let mut starts = Vec::new();
    let mut start = -6i64;
    while start < mel.frames as i64 - 6 {
        starts.push(start);
        start += stride as i64;
    }
    if mel.frames > stride
        && let Some(last) = starts.last_mut()
    {
        *last = mel.frames as i64 - (MAX_FRAMES - 6) as i64;
    }
    let frames = (mel.frames + 12).min(MAX_FRAMES);
    let mut chunk = vec![0.0; frames * MEL_BINS];
    let mut beat = vec![-1000.0; mel.frames];
    let mut downbeat = vec![-1000.0; mel.frames];
    let mut covered = vec![false; mel.frames];
    for start in starts {
        if cancelled.load(Ordering::Acquire) {
            return Err(AnalysisError::Cancelled);
        }
        chunk.fill(0.0);
        let from = start.max(0) as usize;
        let to = (start + MAX_FRAMES as i64).clamp(0, mel.frames as i64) as usize;
        let left_pad = (-start).max(0) as usize;
        chunk[left_pad * MEL_BINS..(left_pad + to - from) * MEL_BINS]
            .copy_from_slice(&mel.data[from * MEL_BINS..to * MEL_BINS]);
        let predicted = model.run(&chunk, cancelled)?;
        for offset in 6..frames - 6 {
            let target = start + offset as i64;
            if target < 0 || target >= mel.frames as i64 || covered[target as usize] {
                continue;
            }
            let target = target as usize;
            covered[target] = true;
            beat[target] = predicted.beat[offset];
            downbeat[target] = predicted.downbeat[offset];
        }
        let completed = (start + frames as i64 - 6).clamp(0, mel.frames as i64) as f64 / FPS as f64;
        if !progress(completed) {
            return Err(AnalysisError::Cancelled);
        }
    }
    let grid = beats_from_logits(&beat, &downbeat);
    Ok(Predicted { grid, logits: beat })
}
