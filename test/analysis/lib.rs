// Packed model verification and native GPU output against the pinned ONNX reference.
#![cfg(test)]

#[test]
#[cfg(windows)]
fn missing_or_corrupt_native_compiler_reports_its_path() {
    if !folia_render::backend::configured().iter().any(|backends| backends.contains(wgpu::Backends::DX12)) {
        return;
    }
    let temporary = tempfile::tempdir().unwrap();
    for contents in [None, Some(&b"invalid compiler"[..])] {
        if let Some(contents) = contents {
            std::fs::write(temporary.path().join("dxcompiler.dll"), contents).unwrap();
        }
        let error = match pollster::block_on(folia_analysis::gpu::Gpu::from_runtime(temporary.path())) {
            Ok(_) => panic!("an unavailable analysis compiler was accepted"),
            Err(error) => error,
        };
        let folia_analysis::AnalysisError::Runtime { path, source } = error else {
            panic!("missing compiler lost its structured source")
        };
        assert_eq!(path, temporary.path().join("dxcompiler.dll"));
        assert_eq!(
            source.kind(),
            if contents.is_some() { std::io::ErrorKind::InvalidData } else { std::io::ErrorKind::NotFound }
        );
    }
}

use folia_analysis::models::{ModelKind, Weights};

#[test]
fn incomplete_model_exports_are_rejected_before_gpu_upload() {
    let temporary = tempfile::tempdir().unwrap();
    let path = temporary.path().join("truncated.bin");
    std::fs::write(&path, b"FOLIABTW").unwrap();
    for model in [ModelKind::BeatThis, ModelKind::Htdemucs] {
        assert!(Weights::load(&path, model).is_err());
    }
}

#[test]
fn cached_analysis_never_opens_audio_or_models() {
    use folia_analysis::{
        AnalysisData,
        cache::Cache,
        worker::{Event, Needs, Request, Worker, representation_key},
    };
    let temporary = tempfile::tempdir().unwrap();
    let database = temporary.path().join("analysis.db");
    let input: folia_core::AudioInput = temporary.path().join("unavailable-audio.flac").into();
    let key = representation_key(&input, false).unwrap();
    let cache = Cache::at(&database).unwrap();
    let events = vec![folia_core::BeatEvent { time: 1.0, downbeat: true, ..Default::default() }];
    let data = AnalysisData::Beats(std::sync::Arc::new(events.clone()));
    cache.put(&key, &data).unwrap();
    let invalid =
        AnalysisData::Beats(std::sync::Arc::new(vec![folia_core::BeatEvent { time: f64::NAN, ..Default::default() }]));
    assert!(cache.put(&key, &invalid).is_err());
    let (mut worker, updates) = Worker::spawn(temporary.path().join("missing-models"), database).unwrap();
    for (ticket, cached_only) in [(1, true), (2, false)] {
        worker
            .submit(Request {
                ticket,
                input: input.clone(),
                local_file: false,
                needs: Needs { beats: true, stems: false },
                cached_only,
            })
            .unwrap();
        let ready = updates.recv_timeout(std::time::Duration::from_secs(5)).unwrap();
        assert_eq!(ready.ticket, ticket);
        assert!(matches!(ready.event,Event::Ready(AnalysisData::Beats(ref found)) if found.as_ref()==&events));
        assert!(matches!(updates.recv_timeout(std::time::Duration::from_secs(5)).unwrap().event, Event::Finished));
    }
}

#[test]
#[cfg(feature = "live")]
#[ignore = "requires bundled models and a native GPU"]
fn worker_analyses_then_reuses_both_models_without_source_files() {
    use folia_analysis::{
        AnalysisData,
        worker::{Event, Needs, Request, Worker},
    };
    let temporary = tempfile::tempdir().unwrap();
    let audio = temporary.path().join("cached-audio.flac");
    std::fs::write(&audio, include_bytes!("../player/fixtures/seek.flac")).unwrap();
    let input: folia_core::AudioInput = audio.clone().into();
    let database = temporary.path().join("analysis.db");
    let models = std::path::PathBuf::from(std::env::var_os("FOLIA_ANALYSIS_MODELS").unwrap());
    for (ticket, cached_only, models) in [(1, false, models), (2, true, temporary.path().join("missing-models"))] {
        let (mut worker, updates) = Worker::spawn(models, database.clone()).unwrap();
        worker
            .submit(Request {
                ticket,
                input: input.clone(),
                local_file: false,
                needs: Needs { beats: true, stems: true },
                cached_only,
            })
            .unwrap();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(90);
        let (mut beats, mut stems) = (false, false);
        loop {
            assert!(std::time::Instant::now() < deadline, "analysis worker did not complete");
            let update = updates.recv_timeout(std::time::Duration::from_secs(30)).unwrap();
            assert_eq!(update.ticket, ticket);
            match update.event {
                Event::Ready(AnalysisData::Beats(_)) => beats = true,
                Event::Ready(AnalysisData::Stems(data)) => {
                    stems = true;
                    assert_eq!(data.vocal.db.len(), 101);
                }
                Event::Finished => break,
                Event::Failed(error) => panic!("{error}"),
                Event::Analysing(_) => assert!(!cached_only),
                _ => {}
            }
        }
        assert!(beats && stems);
        drop(worker);
        assert!(matches!(
            updates.recv_timeout(std::time::Duration::from_secs(5)),
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected)
        ));
        if !cached_only {
            std::fs::remove_file(&audio).unwrap();
        }
    }
}

#[test]
fn log_mel_matches_the_original_reflection_and_scaling() {
    let cases: serde_json::Value = serde_json::from_str(include_str!("fixtures/mel.json")).unwrap();
    for case in cases.as_array().unwrap() {
        let length = case["length"].as_u64().unwrap() as usize;
        let mono: Vec<_> = (0..length)
            .map(|index| ((index as f64 * 0.031).sin() * 0.2 + ((index % 17) as f64 - 8.0) * 0.001) as f32)
            .collect();
        let actual = folia_analysis::signal::mel_spectrogram(&mono).unwrap();
        assert_eq!(actual.frames, case["frames"].as_u64().unwrap() as usize);
        let expected = case["data"].as_array().unwrap();
        assert_eq!(actual.data.len(), expected.len());
        let error = actual
            .data
            .iter()
            .zip(expected)
            .map(|(a, b)| (f64::from(*a) - b.as_f64().unwrap()).abs())
            .fold(0.0f64, f64::max);
        assert!(error < 0.00001, "{length} samples, mel error {error}");
    }
    assert!(folia_analysis::signal::mel_spectrogram(&[]).unwrap().data.is_empty());
}

#[test]
fn beat_accents_match_original_meter_and_rhythm_gates() {
    let cases: serde_json::Value = serde_json::from_str(include_str!("fixtures/accents.json")).unwrap();
    for case in cases.as_array().unwrap() {
        let bar = case["bar"].as_u64().unwrap() as usize;
        let group = case["group"].as_u64().unwrap() as usize;
        let step = case["step"].as_f64().unwrap();
        let quiet = case["quiet"].as_bool().unwrap();
        let beats: Vec<_> = (0..96).map(|index| 1.0 + index as f64 * step).collect();
        let downbeats =
            beats.iter().enumerate().filter_map(|(index, time)| index.is_multiple_of(bar).then_some(*time)).collect();
        let frames = ((beats.last().unwrap() + 1.0) * 50.0).ceil() as usize;
        let mut data = vec![0.0f32; frames * 128];
        let mut logits = vec![-4.0f32; frames];
        let mut percussive = vec![case["energy"].as_f64().unwrap() as f32; frames];
        for frame in 0..frames {
            let time = frame as f64 / 50.0;
            let index = ((time - 1.0) / step).floor() as i64;
            let local = time - 1.0 - index as f64 * step;
            let strength = if (0..96).contains(&index) {
                if (index as usize).is_multiple_of(group) { 2.0 } else { 1.0 }
            } else {
                0.0
            };
            let pulse = strength * (-local.max(0.0) * 30.0).exp();
            for band in 0..128 {
                data[frame * 128 + band] =
                    (0.5 + band as f64 * 0.001 + pulse * if band < 60 { 1.0 } else { 0.5 }) as f32;
            }
            if quiet && time < 1.0 + 12.0 * step {
                percussive[frame] = 0.01;
            }
        }
        for (index, time) in beats.iter().enumerate() {
            logits[(time * 50.0).round() as usize] = if quiet && index < 12 {
                2.0
            } else if index.is_multiple_of(group) {
                9.0
            } else {
                5.0
            };
        }
        let mel = folia_analysis::signal::MelSpectrogram { data, frames };
        let energy = folia_analysis::percussive::Energy { percussive, total: vec![1.0; frames] };
        let grid = folia_analysis::rhythm::BeatGrid { beats, downbeats };
        let events = folia_analysis::accents::events(&mel, &grid, &logits, Some(&energy)).unwrap();
        let expected = case["events"].as_array().unwrap();
        assert_eq!(events.len(), expected.len(), "bar {bar} group {group}");
        for (actual, expected) in events.iter().zip(expected) {
            assert!((actual.time - expected["time"].as_f64().unwrap()).abs() < 1e-10);
            assert_eq!(actual.downbeat, expected["downbeat"].as_bool().unwrap());
            assert_eq!(actual.rhythmic, expected["rhythmic"].as_bool());
            assert!((f64::from(actual.impact.unwrap()) - expected["impact"].as_f64().unwrap()).abs() < 0.000001);
        }
    }
}

#[test]
fn streaming_audio_resampling_preserves_length_and_cancellation() {
    use folia_analysis::audio::AudioReader;
    let temporary = tempfile::tempdir().unwrap();
    let path = temporary.path().join("seek.flac");
    std::fs::write(&path, include_bytes!("../player/fixtures/seek.flac")).unwrap();
    let input: folia_core::AudioInput = path.into();
    let cancelled = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let mut expected = Vec::new();
    for chunk in [1113, 100000] {
        let mut reader = AudioReader::open(&input, 22050, cancelled.clone()).unwrap();
        let (mut left, mut right) = (Vec::new(), Vec::new());
        while reader.read(chunk, &mut left, &mut right).unwrap() > 0 {}
        assert_eq!(left.len(), 44100);
        assert_eq!(left, right);
        if expected.is_empty() {
            expected = left;
        } else {
            assert_eq!(left, expected);
        }
    }
    cancelled.store(true, std::sync::atomic::Ordering::Release);
    let mut reader = AudioReader::open(&input, 22050, cancelled).unwrap();
    assert!(matches!(reader.read(1, &mut Vec::new(), &mut Vec::new()), Err(folia_analysis::AnalysisError::Cancelled)));
}

#[test]
fn stem_features_are_independent_of_stream_chunk_boundaries() {
    let frames = 44100 * 3 + 127;
    let mut drums = Vec::with_capacity(frames);
    let mut voice = Vec::with_capacity(frames);
    let mut mix = Vec::with_capacity(frames);
    for index in 0..frames {
        let time = index as f64 / 44100.0;
        let pulse = (-((time % 0.5) * 35.0)).exp();
        let drum = (0.4 * pulse * (std::f64::consts::TAU * 60.0 * time).sin()) as f32;
        let vocal = (0.1 * (std::f64::consts::TAU * 220.0 * time).sin() * (time * 5.0).sin().abs()) as f32;
        drums.push(drum);
        voice.push(vocal);
        mix.push(drum + vocal);
    }
    let mut expected = None;
    for chunk in [frames, 147, 9021] {
        let mut features = folia_analysis::signal::StemFeatures::new().unwrap();
        for start in (0..frames).step_by(chunk) {
            let end = (start + chunk).min(frames);
            features.push(&drums[start..end], &voice[start..end], &mix[start..end]).unwrap();
        }
        let actual = features.finish().unwrap();
        assert_eq!(actual.vocal.fps, 50);
        assert_eq!(actual.vocal.db.len(), frames.div_ceil(2) / 441 + 1);
        assert!(!actual.drums.as_ref().unwrap().times.is_empty());
        if let Some(expected) = &expected {
            assert_eq!(&actual, expected);
        } else {
            expected = Some(actual);
        }
    }
}

#[test]
#[cfg(feature = "live")]
#[ignore = "requires pinned Beat This weights, a GPU, and the matching ONNX reference"]
fn beat_this_matches_the_pinned_reference() {
    use folia_analysis::beat_this::{BeatThis, MEL_BINS};
    use folia_analysis::gpu::Gpu;
    let models = std::path::PathBuf::from(std::env::var_os("FOLIA_ANALYSIS_MODELS").unwrap());
    let input = std::fs::read(std::env::var_os("FOLIA_ANALYSIS_MEL").unwrap()).unwrap();
    let reference = std::fs::read(std::env::var_os("FOLIA_ANALYSIS_BEATS").unwrap()).unwrap();
    let input: &[f32] = bytemuck::cast_slice(&input);
    let reference: &[f32] = bytemuck::cast_slice(&reference);
    let frames = input.len() / MEL_BINS;
    assert_eq!(reference.len(), frames * 2);
    let weights = Weights::load(&models.join(ModelKind::BeatThis.file()), ModelKind::BeatThis).unwrap();
    let gpu = pollster::block_on(Gpu::from_runtime(&models.with_file_name("analysis"))).unwrap();
    let mut model = pollster::block_on(BeatThis::new(&gpu, &weights, frames)).unwrap();
    let result = model.run(input, &std::sync::atomic::AtomicBool::new(false)).unwrap();
    for (index, values) in [&result.beat, &result.downbeat].into_iter().enumerate() {
        let expected = &reference[index * frames..(index + 1) * frames];
        let energy: f64 = expected.iter().map(|value| f64::from(*value).powi(2)).sum();
        let error: f64 = values.iter().zip(expected).map(|(a, b)| f64::from(a - b).powi(2)).sum();
        let maximum = values.iter().zip(expected).map(|(a, b)| (a - b).abs()).fold(0.0f32, f32::max);
        let snr = 10.0 * ((energy + 1e-20) / (error + 1e-20)).log10();
        eprintln!("Beat This output {index}: SNR {snr:.2} dB, maximum error {maximum}");
        assert!(snr > 60.0 && maximum < 0.01);
    }
    let mel = folia_analysis::signal::MelSpectrogram { data: input.to_vec(), frames };
    let energy =
        pollster::block_on(folia_analysis::percussive::energy(&gpu, &mel, &std::sync::atomic::AtomicBool::new(false)))
            .unwrap();
    let magnitude = |frame: i64, band: i64| {
        if frame < 0 || frame >= frames as i64 || !(0..128).contains(&band) {
            return 0.0;
        }
        let value = f64::from(input[frame as usize * 128 + band as usize]);
        if value < 1e-3 { (value + 0.5 * value * value).max(0.0) } else { value.exp() - 1.0 }
    };
    for frame in 0..frames {
        let (mut percussive, mut total) = (0.0, 0.0);
        for band in 0..128 {
            let mut time: Vec<_> = (-8..=8).map(|delta| magnitude(frame as i64 + delta, band)).collect();
            let mut bands: Vec<_> = (-8..=8).map(|delta| magnitude(frame as i64, band + delta)).collect();
            time.sort_by(f64::total_cmp);
            bands.sort_by(f64::total_cmp);
            let value = magnitude(frame as i64, band).powi(2);
            let hh = time[8].powi(2);
            let pp = bands[8].powi(2);
            if hh + pp > 0.0 {
                percussive += value * pp / (hh + pp);
            }
            total += value;
        }
        let expected = if total > 0.0 { percussive / total } else { 0.0 };
        let actual = if energy.total[frame] > 0.0 {
            f64::from(energy.percussive[frame]) / f64::from(energy.total[frame])
        } else {
            0.0
        };
        assert!((actual - expected).abs() < 0.00001, "HPSS frame {frame}: {actual} != {expected}");
    }
}

#[test]
#[cfg(feature = "live")]
#[ignore = "requires pinned model weights, a GPU, and an ONNX reference in task-specific files"]
fn htdemucs_matches_the_pinned_reference() {
    use folia_analysis::gpu::Gpu;
    use folia_analysis::htdemucs::{Htdemucs, SEGMENT};
    let models = std::path::PathBuf::from(std::env::var_os("FOLIA_ANALYSIS_MODELS").unwrap());
    let input = std::fs::read(std::env::var_os("FOLIA_ANALYSIS_INPUT").unwrap()).unwrap();
    let reference = std::fs::read(std::env::var_os("FOLIA_ANALYSIS_REFERENCE").unwrap()).unwrap();
    let input: &[f32] = bytemuck::cast_slice(&input);
    let reference: &[f32] = bytemuck::cast_slice(&reference);
    assert_eq!(input.len() % (2 * SEGMENT), 0);
    assert_eq!(reference.len(), input.len() * 4);
    let weights = Weights::load(&models.join(ModelKind::Htdemucs.file()), ModelKind::Htdemucs).unwrap();
    let gpu = pollster::block_on(Gpu::from_runtime(&models.with_file_name("analysis"))).unwrap();
    let started = std::time::Instant::now();
    let mut model = pollster::block_on(Htdemucs::new(&gpu, &weights)).unwrap();
    drop(weights);
    eprintln!("htdemucs init {:?}, GPU buffers {}", started.elapsed(), model.allocated_bytes());
    let cancelled = std::sync::atomic::AtomicBool::new(false);
    for (index, segment) in input.chunks_exact(2 * SEGMENT).enumerate() {
        let started = std::time::Instant::now();
        let stems = model.run(&segment[..SEGMENT], &segment[SEGMENT..], &cancelled).unwrap();
        for (source, stereo) in [(0, stems.drums()), (3, stems.vocals())] {
            for (channel, actual) in [stereo.0, stereo.1].into_iter().enumerate() {
                let from = index * 8 * SEGMENT + source * 2 * SEGMENT + channel * SEGMENT;
                let expected = &reference[from..from + SEGMENT];
                let energy: f64 = expected.iter().map(|value| f64::from(*value).powi(2)).sum();
                let error: f64 = actual.iter().zip(expected).map(|(a, b)| f64::from(a - b).powi(2)).sum();
                let snr = 10.0 * ((energy + 1e-20) / (error + 1e-20)).log10();
                let actual_energy: f64 = actual.iter().map(|value| f64::from(*value).powi(2)).sum();
                let level = 10.0 * ((actual_energy + 1e-20) / (energy + 1e-20)).log10();
                eprintln!("segment {index} source {source} channel {channel}, SNR {snr:.2} dB, level {level:.6} dB");
                assert!(snr > 50.0, "model graph differs from pinned reference, SNR {snr}");
                assert!(level.abs() < 0.01, "stem level changed by {level} dB");
            }
        }
        eprintln!("segment {index} GPU run {:?}", started.elapsed());
    }
    cancelled.store(true, std::sync::atomic::Ordering::Release);
    assert!(matches!(
        model.run(&input[..SEGMENT], &input[SEGMENT..2 * SEGMENT], &cancelled),
        Err(folia_analysis::AnalysisError::Cancelled)
    ));
}
