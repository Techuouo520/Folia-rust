// Original beat accent v10: impact, felt-beat grouping, uncertainty and percussive rhythm gating.
use crate::AnalysisError;
use folia_core::BeatEvent;

use crate::percussive::Energy;
use crate::rhythm::BeatGrid;
use crate::signal::{BANDS, FPS, MelSpectrogram, mel_centres};

pub fn events(
    mel: &MelSpectrogram,
    grid: &BeatGrid,
    logits: &[f32],
    energy: Option<&Energy>,
) -> Result<Vec<BeatEvent>, AnalysisError> {
    let valid_times = |values: &[f64]| {
        values.iter().all(|time| time.is_finite() && *time >= 0.0) && values.windows(2).all(|pair| pair[0] <= pair[1])
    };
    if mel.data.len() != mel.frames * BANDS
        || logits.len() != mel.frames
        || !valid_times(&grid.beats)
        || !valid_times(&grid.downbeats)
        || energy.is_some_and(|value| value.percussive.len() != mel.frames || value.total.len() != mel.frames)
    {
        return Err(AnalysisError::Model("invalid beat accent inputs".to_owned()));
    }
    if grid.beats.is_empty() {
        return Ok(Vec::new());
    }
    let tracks = split_bands(mel);
    let measures: Vec<_> = grid
        .beats
        .iter()
        .map(|time| {
            let frame = (*time * FPS as f64).round() as i64;
            let rises = [rise_at(&tracks[0], frame), rise_at(&tracks[1], frame), rise_at(&tracks[2], frame)];
            (rises[0] * 0.5 + rises[1] * 0.3 + rises[2] * 0.2, loudness_at(&tracks[3], frame))
        })
        .collect();
    let rises: Vec<_> = measures.iter().map(|(rise, _)| *rise).collect();
    let confidence: Vec<_> = grid
        .beats
        .iter()
        .map(|time| {
            let frame = (*time * FPS as f64).round() as usize;
            let peak = logits
                .get(frame.saturating_sub(2)..(frame + 3).min(logits.len()))
                .into_iter()
                .flatten()
                .copied()
                .fold(f32::NEG_INFINITY, f32::max);
            if peak.is_finite() { round3(f64::from(peak)) } else { 0.0 }
        })
        .collect();
    let group = meter_group(&rises, grid, &confidence);
    let mut pulse = pulse_beats(grid, group);
    let unsure = unsure_beats(&confidence, &pulse);
    let mut rhythmic = rhythmic_beats(grid, energy);
    for (rhythmic, unsure) in rhythmic.iter_mut().zip(&unsure) {
        if *unsure {
            *rhythmic = false;
        }
    }
    restore_driven(&mut pulse, group, &confidence, track_share(energy), &unsure);
    let rise_range = normaliser(rises);
    let loud_range = normaliser(measures.iter().map(|(_, loud)| *loud).collect());
    let flat = grid.beats.len() < 8 || (rise_range.is_none() && loud_range.is_none());
    let rise_share = if loud_range.is_none() {
        1.0
    } else if rise_range.is_none() {
        0.0
    } else {
        0.4
    };
    let mut events = Vec::new();
    for (index, time) in grid.beats.iter().enumerate() {
        if !pulse[index] {
            continue;
        }
        let (rise, loud) = measures[index];
        let normal = |value: f64, range: Option<(f64, f64)>| {
            range.map_or(0.0, |(floor, ceiling)| ((value - floor) / (ceiling - floor)).clamp(0.0, 1.0))
        };
        let mixed = rise_share * normal(rise, rise_range) + (1.0 - rise_share) * normal(loud, loud_range);
        let impact = if flat { 1.0 } else { round3(mixed.powf(0.7)) as f32 };
        events.push(BeatEvent {
            time: *time,
            downbeat: is_down(grid, *time),
            impact: Some(impact),
            rhythmic: Some(rhythmic[index]),
        });
    }
    for time in &grid.downbeats {
        if grid.beats.binary_search_by(|value| value.total_cmp(time)).is_err() {
            events.push(BeatEvent { time: *time, downbeat: true, impact: None, rhythmic: None });
        }
    }
    events.sort_by(|a, b| a.time.total_cmp(&b.time));
    tracing::debug!(counted = grid.beats.len(), felt = events.len(), group, "beat rhythm resolved");
    Ok(events)
}

fn split_bands(mel: &MelSpectrogram) -> [Vec<f32>; 4] {
    let bands = mel_centres().map(|hz| {
        if hz < 150.0 {
            0
        } else if hz < 2000.0 {
            1
        } else {
            2
        }
    });
    let counts = [
        bands.iter().filter(|&&band| band == 0).count(),
        bands.iter().filter(|&&band| band == 1).count(),
        bands.iter().filter(|&&band| band == 2).count(),
    ];
    let mut tracks = std::array::from_fn(|_| Vec::with_capacity(mel.frames));
    for row in mel.data.as_chunks::<BANDS>().0 {
        let mut sums = [0.0f64; 3];
        for (index, value) in row.iter().enumerate() {
            sums[bands[index]] += f64::from(*value);
        }
        for index in 0..3 {
            tracks[index].push(if counts[index] > 0 { (sums[index] / counts[index] as f64) as f32 } else { 0.0 });
        }
        tracks[3].push((sums.iter().sum::<f64>() / BANDS as f64) as f32);
    }
    tracks
}

fn at(track: &[f32], frame: i64) -> f64 {
    if track.is_empty() { 0.0 } else { f64::from(track[frame.clamp(0, track.len() as i64 - 1) as usize]) }
}

fn rise_at(track: &[f32], frame: i64) -> f64 {
    let peak = (frame - 1..=frame + 2).map(|index| at(track, index)).fold(f64::NEG_INFINITY, f64::max);
    let base = (frame - 6..=frame - 2).map(|index| at(track, index)).sum::<f64>() / 5.0;
    (peak - base).max(0.0)
}

fn loudness_at(track: &[f32], frame: i64) -> f64 {
    (frame - 12..=frame + 12).map(|index| at(track, index)).sum::<f64>() / 25.0
}

fn percentile(sorted: &[f64], share: f64) -> f64 {
    sorted.get((share * sorted.len().saturating_sub(1) as f64).round() as usize).copied().unwrap_or(0.0)
}

fn median(mut values: Vec<f64>) -> f64 {
    values.sort_by(f64::total_cmp);
    percentile(&values, 0.5)
}

fn normaliser(mut values: Vec<f64>) -> Option<(f64, f64)> {
    values.sort_by(f64::total_cmp);
    let floor = percentile(&values, 0.1);
    let ceiling = percentile(&values, 0.9);
    (ceiling - floor > 1e-6).then_some((floor, ceiling))
}

fn round3(value: f64) -> f64 {
    (value * 1000.0).round() / 1000.0
}

fn is_down(grid: &BeatGrid, time: f64) -> bool {
    grid.downbeats.binary_search_by(|value| value.total_cmp(&time)).is_ok()
}

fn down_starts(grid: &BeatGrid) -> Vec<usize> {
    grid.beats.iter().enumerate().filter_map(|(index, time)| is_down(grid, *time).then_some(index)).collect()
}

fn meter_group(rises: &[f64], grid: &BeatGrid, confidence: &[f64]) -> usize {
    let gap = median(grid.beats.windows(2).map(|pair| pair[1] - pair[0]).collect());
    let bpm = if gap > 0.0 { (60.0 / gap).round() } else { 0.0 };
    let starts = down_starts(grid);
    let mut counts: Vec<(usize, usize)> = Vec::new();
    for pair in starts.windows(2) {
        let length = pair[1] - pair[0];
        if let Some((_, count)) = counts.iter_mut().find(|(found, _)| *found == length) {
            *count += 1;
        } else {
            counts.push((length, 1));
        }
    }
    counts.sort_by_key(|entry| std::cmp::Reverse(entry.1));
    let Some(&(usual, bars)) = counts.first() else { return 1 };
    if bars < 8 {
        return 1;
    }
    let mut positions = vec![0.0; usual];
    let mut logits: Vec<Vec<f64>> = (0..usual).map(|_| Vec::new()).collect();
    for pair in starts.windows(2).filter(|pair| pair[1] - pair[0] == usual) {
        for position in 0..usual {
            positions[position] += rises[pair[0] + position] / bars as f64;
            logits[position].push(confidence[pair[0] + position]);
        }
    }
    let logits: Vec<_> = logits.into_iter().map(median).collect();
    let stands_out = |head: usize, needed: f64| {
        let before = (head + usual - 1) % usual;
        let after = (head + 1) % usual;
        (needed == 0.0 || positions[head] > needed * positions[before].max(positions[after]))
            && logits[head] - logits[before].max(logits[after]) > 0.7
    };
    for (size, ratio, min_bpm) in [(3, 1.2, 120.0), (2, 0.0, 90.0), (3, 1.2, 90.0)] {
        if !usual.is_multiple_of(size) || bpm < min_bpm {
            continue;
        }
        let matches = if usual == size {
            stands_out(0, ratio)
        } else {
            (size..usual).step_by(size).all(|head| stands_out(head, ratio))
        };
        if matches {
            return size;
        }
    }
    1
}

fn pulse_beats(grid: &BeatGrid, group: usize) -> Vec<bool> {
    let mut pulse = vec![true; grid.beats.len()];
    if group > 1 {
        let starts = down_starts(grid);
        if let Some(&first) = starts.first() {
            for (index, value) in pulse.iter_mut().enumerate().take(first) {
                *value = (first - index).is_multiple_of(group);
            }
            for (bar, start) in starts.iter().copied().enumerate() {
                let end = starts.get(bar + 1).copied().unwrap_or(pulse.len());
                for (index, value) in pulse.iter_mut().enumerate().take(end).skip(start) {
                    *value = (index - start).is_multiple_of(group);
                }
            }
        }
    }
    let gap = median(grid.beats.windows(2).map(|pair| pair[1] - pair[0]).collect());
    if gap <= 0.0 || grid.beats.len() < 8 {
        return pulse;
    }
    let spacing = 0.7 * gap * group as f64;
    let mut last: Option<usize> = None;
    for (index, time) in grid.beats.iter().copied().enumerate() {
        if !pulse[index] {
            continue;
        }
        let Some(previous) = last else {
            last = Some(index);
            continue;
        };
        if time - grid.beats[previous] >= spacing {
            last = Some(index);
            continue;
        }
        if is_down(grid, time) && !is_down(grid, grid.beats[previous]) {
            pulse[previous] = false;
            last = Some(index);
        } else {
            pulse[index] = false;
        }
    }
    pulse
}

fn unsure_beats(confidence: &[f64], pulse: &[bool]) -> Vec<bool> {
    let mut unsure = vec![false; pulse.len()];
    let felt: Vec<_> = pulse.iter().enumerate().filter_map(|(index, on)| on.then_some(index)).collect();
    if felt.len() < 8 {
        return unsure;
    }
    let logits: Vec<_> = felt.iter().map(|index| confidence[*index]).collect();
    let mut sorted = logits.clone();
    sorted.sort_by(f64::total_cmp);
    let floor = 0.55 * percentile(&sorted, 0.75);
    let low: Vec<_> = (0..logits.len())
        .map(|index| {
            let around = &logits[index.saturating_sub(4)..(index + 5).min(logits.len())];
            around.iter().sum::<f64>() / (around.len() as f64) < floor
        })
        .collect();
    let mut start = None;
    for index in 0..=low.len() {
        if low.get(index).copied().unwrap_or(false) {
            start.get_or_insert(index);
            continue;
        }
        if let Some(first) = start
            && index - first >= 8
        {
            unsure[felt[first]..=felt[index - 1]].fill(true);
        }
        start = None;
    }
    unsure
}

fn track_share(energy: Option<&Energy>) -> Option<f64> {
    let energy = energy?;
    let all = energy.total.iter().map(|value| f64::from(*value)).sum::<f64>();
    (all > 0.0).then(|| energy.percussive.iter().map(|value| f64::from(*value)).sum::<f64>() / all)
}

fn rhythmic_beats(grid: &BeatGrid, energy: Option<&Energy>) -> Vec<bool> {
    let mut rhythmic = vec![true; grid.beats.len()];
    let Some(energy) = energy else { return rhythmic };
    let starts: Vec<_> = if grid.downbeats.len() >= 2 {
        grid.beats
            .iter()
            .enumerate()
            .filter_map(|(index, time)| (index == 0 || is_down(grid, *time)).then_some(index))
            .collect()
    } else {
        (0..grid.beats.len()).step_by(4).collect()
    };
    if starts.len() < 4 {
        return rhythmic;
    }
    let tail_gap =
        if grid.beats.len() > 1 { grid.beats[grid.beats.len() - 1] - grid.beats[grid.beats.len() - 2] } else { 0.5 };
    let frame = |time: f64| ((time * FPS as f64).floor().max(0.0) as usize).min(energy.total.len());
    let shares: Vec<_> = starts
        .iter()
        .enumerate()
        .map(|(bar, start)| {
            let from = frame(grid.beats[*start]);
            let until = starts
                .get(bar + 1)
                .map(|index| grid.beats[*index])
                .unwrap_or(grid.beats[grid.beats.len() - 1] + tail_gap);
            let end = frame(until);
            let all = energy.total[from..end].iter().map(|value| f64::from(*value)).sum::<f64>();
            if all > 0.0 {
                energy.percussive[from..end].iter().map(|value| f64::from(*value)).sum::<f64>() / all
            } else {
                0.0
            }
        })
        .collect();
    let usual = median(shares.clone());
    if usual < 0.15 {
        return rhythmic;
    }
    for (bar, start) in starts.iter().copied().enumerate() {
        if shares[bar] >= usual * 0.45 {
            continue;
        }
        let end = starts.get(bar + 1).copied().unwrap_or(rhythmic.len());
        rhythmic[start..end].fill(false);
    }
    rhythmic
}

fn restore_driven(pulse: &mut [bool], group: usize, confidence: &[f64], share: Option<f64>, unsure: &[bool]) {
    if group != 2 || share.is_none_or(|value| value < 0.3) {
        return;
    }
    let felt: Vec<_> = pulse.iter().enumerate().filter_map(|(index, on)| on.then_some(index)).collect();
    let groups: Vec<_> = felt.windows(2).filter(|pair| !unsure[pair[0]]).map(|pair| (pair[0], pair[1])).collect();
    let driven: Vec<_> = (0..groups.len())
        .map(|index| {
            let around = &groups[index.saturating_sub(8)..(index + 9).min(groups.len())];
            let heads = around.iter().map(|(head, _)| confidence[*head]).sum::<f64>() / around.len() as f64;
            let mut sum = 0.0;
            let mut count = 0;
            for (head, next) in around {
                for value in &confidence[head + 1..*next] {
                    sum += value;
                    count += 1;
                }
            }
            count > 0 && heads > 0.0 && sum / count as f64 >= 0.6 * heads
        })
        .collect();
    for ((head, next), driven) in groups.into_iter().zip(driven) {
        if driven {
            pulse[head + 1..next].fill(true);
        }
    }
}
