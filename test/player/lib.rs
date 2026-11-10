// Checks channel mapping and real playback of a silent WAV on the default device.

use std::fs;
use std::path::Path;
use std::thread::sleep;
use std::time::{Duration, Instant};

use folia_player::{Analyser, Player, PlayerState, TAP_CAPACITY, map_channels, push_samples};

mod equalizer;

#[test]
fn maps_channels() {
    let mut out = Vec::new();
    map_channels(&[0.1, 0.2], 1, 2, &mut out);
    assert_eq!(out, [0.1, 0.1, 0.2, 0.2]);
    out.clear();
    map_channels(&[1.0, 2.0, 3.0, 4.0, 5.0, 6.0], 3, 2, &mut out);
    assert_eq!(out, [1.0, 2.0, 4.0, 5.0]);
    out.clear();
    map_channels(&[1.0, 2.0], 2, 2, &mut out);
    assert_eq!(out, [1.0, 2.0]);
}

fn write_wav(path: &Path, rate: u32, seconds: f32) {
    let frames = (rate as f32 * seconds) as u32;
    let data_len = frames * 2;
    let mut bytes = Vec::new();
    for (tag, value) in [(b"RIFF", 36 + data_len)] {
        bytes.extend_from_slice(tag);
        bytes.extend_from_slice(&value.to_le_bytes());
    }
    bytes.extend_from_slice(b"WAVEfmt ");
    bytes.extend_from_slice(&16u32.to_le_bytes());
    bytes.extend_from_slice(&1u16.to_le_bytes());
    bytes.extend_from_slice(&1u16.to_le_bytes());
    bytes.extend_from_slice(&rate.to_le_bytes());
    bytes.extend_from_slice(&(rate * 2).to_le_bytes());
    bytes.extend_from_slice(&2u16.to_le_bytes());
    bytes.extend_from_slice(&16u16.to_le_bytes());
    bytes.extend_from_slice(b"data");
    bytes.extend_from_slice(&data_len.to_le_bytes());
    bytes.resize(bytes.len() + data_len as usize, 0);
    fs::write(path, bytes).unwrap();
}

fn wait_for(player: &Player, timeout: Duration, done: impl Fn(&folia_player::Status) -> bool) -> folia_player::Status {
    let start = Instant::now();
    loop {
        let status = player.status();
        if done(&status) || start.elapsed() > timeout {
            return status;
        }
        sleep(Duration::from_millis(20));
    }
}

#[test]
fn plays_seeks_and_ends() {
    let Ok(player) = Player::new() else {
        eprintln!("no audio device, skipping");
        return;
    };
    player.set_volume(0.0);
    let dir = tempfile::tempdir().unwrap();
    let wav = dir.path().join("silence.wav");
    write_wav(&wav, 22_050, 1.5);

    player.load(&wav);
    let status = wait_for(&player, Duration::from_secs(2), |s| s.position > 0.3);
    assert_eq!(status.state, PlayerState::Playing);
    assert!((status.duration - 1.5).abs() < 0.01, "{status:?}");

    player.pause();
    let paused_at = player.status().position;
    sleep(Duration::from_millis(200));
    assert!((player.status().position - paused_at).abs() < 0.05, "position holds while paused");

    player.seek(1.0);
    sleep(Duration::from_millis(100));
    let after_seek = player.status();
    assert_eq!(after_seek.state, PlayerState::Paused);
    assert!((after_seek.position - 1.0).abs() < 0.05, "{after_seek:?}");

    player.resume();
    let ended = wait_for(&player, Duration::from_secs(3), |s| s.state == PlayerState::Ended);
    assert_eq!(ended.state, PlayerState::Ended);
    assert!(ended.position > 1.4, "{ended:?}");

    player.stop();
    sleep(Duration::from_millis(50));
    assert_eq!(player.status().state, PlayerState::Idle);
}

#[test]
fn missing_file_fails() {
    let Ok(player) = Player::new() else { return };
    player.load(Path::new("does/not/exist.flac"));
    let status = wait_for(&player, Duration::from_secs(1), |s| s.state == PlayerState::Failed);
    assert_eq!(status.state, PlayerState::Failed);
}

const FFT: usize = 2048;
const RATE: f64 = 44_100.0;

fn sine(frequency: f64, amplitude: f32) -> Vec<f32> {
    (0..FFT).map(|index| amplitude * (std::f64::consts::TAU * frequency * index as f64 / RATE).sin() as f32).collect()
}

fn feed(samples: &[f32]) -> folia_player::AudioFrame {
    let (mut producer, mut analyser) = Analyser::pair();
    push_samples(&mut producer, samples);
    *analyser.frame(true, 0.0)
}

/// Naive real DFT of the windowed frame, then one smoothing step from zero. Same constants as the analyser.
fn reference_bytes(samples: &[f32]) -> [u8; 1024] {
    let mut windowed = [0.0f64; FFT];
    for (index, sample) in samples.iter().enumerate() {
        let n = index as f64;
        let size = FFT as f64;
        let window = 0.42 - 0.5 * (std::f64::consts::TAU * n / size).cos()
            + 0.08 * (2.0 * std::f64::consts::TAU * n / size).cos();
        windowed[index] = f64::from(*sample) * window;
    }
    let mut bytes = [0u8; 1024];
    for bin in 0..1024 {
        let mut real = 0.0f64;
        let mut imag = 0.0f64;
        for (index, sample) in windowed.iter().enumerate() {
            let angle = -std::f64::consts::TAU * bin as f64 * index as f64 / FFT as f64;
            real += sample * angle.cos();
            imag += sample * angle.sin();
        }
        let magnitude = (real * real + imag * imag).sqrt() / FFT as f64;
        let smoothed = 0.4 * magnitude;
        bytes[bin] = reference_byte(smoothed);
    }
    bytes
}

fn reference_byte(magnitude: f64) -> u8 {
    if magnitude <= 0.0 {
        return 0;
    }
    let scaled = (255.0 / 70.0) * (20.0 * magnitude.log10() + 100.0);
    let floored = scaled.floor();
    if floored <= 0.0 {
        0
    } else if floored >= 255.0 {
        255
    } else {
        floored as u8
    }
}

fn reference_mix(first: &[f32], second: &[f32]) -> [u8; 1024] {
    let mut bytes = [0u8; 1024];
    for bin in 0..1024 {
        let magnitude = |samples: &[f32]| {
            let size = FFT as f64;
            let mut real = 0.0f64;
            let mut imag = 0.0f64;
            for (index, sample) in samples.iter().enumerate() {
                let n = index as f64;
                let window = 0.42 - 0.5 * (std::f64::consts::TAU * n / size).cos()
                    + 0.08 * (2.0 * std::f64::consts::TAU * n / size).cos();
                let value = f64::from(*sample) * window;
                let angle = -std::f64::consts::TAU * bin as f64 * n / size;
                real += value * angle.cos();
                imag += value * angle.sin();
            }
            (real * real + imag * imag).sqrt() / size
        };
        let mixed = 0.6 * (0.4 * magnitude(first)) + 0.4 * magnitude(second);
        bytes[bin] = reference_byte(mixed);
    }
    bytes
}

#[test]
fn sine_matches_naive_dft() {
    let samples = sine(1_000.0, 0.5);
    let frame = feed(&samples);
    let reference = reference_bytes(&samples);
    assert!(frame.has_spectrum);
    for bin in 0..1024 {
        let delta = i32::from(frame.spectrum[bin]) - i32::from(reference[bin]);
        assert!(delta.abs() <= 1, "bin {bin}: got {} reference {}", frame.spectrum[bin], reference[bin]);
    }
}

#[test]
fn silence_is_zero() {
    let frame = feed(&[0.0; FFT]);
    assert!(frame.has_spectrum);
    assert!(frame.spectrum.iter().all(|bin| *bin == 0));
    assert_eq!(
        (frame.power, frame.bass, frame.low_mid, frame.mid, frame.vocal, frame.treble),
        (0.0, 0.0, 0.0, 0.0, 0.0, 0.0)
    );
}

#[test]
fn smoothing_mixes_the_previous_frame() {
    let first = sine(1_000.0, 0.5);
    let second = sine(3_000.0, 0.25);
    let (mut producer, mut analyser) = Analyser::pair();
    push_samples(&mut producer, &first);
    let _ = analyser.frame(true, 0.0);
    push_samples(&mut producer, &second);
    let frame = analyser.frame(true, 0.0);
    let reference = reference_mix(&first, &second);
    for bin in 0..1024 {
        let delta = i32::from(frame.spectrum[bin]) - i32::from(reference[bin]);
        assert!(delta.abs() <= 1, "bin {bin}: got {} reference {}", frame.spectrum[bin], reference[bin]);
    }
}

#[test]
fn hundred_hertz_lifts_bass_and_power() {
    let frame = feed(&sine(100.0, 0.5));
    assert!(frame.bass > 20.0, "bass {}", frame.bass);
    assert!(frame.power > 1.0, "power {}", frame.power);
    assert!(frame.low_mid < frame.bass, "low mid {} bass {}", frame.low_mid, frame.bass);
    assert!(frame.mid < 1.0, "mid {}", frame.mid);
    assert!(frame.vocal < 1.0, "vocal {}", frame.vocal);
    assert!(frame.treble < 1.0, "treble {}", frame.treble);
}

#[test]
fn breath_at_zero_is_twenty() {
    let (_producer, mut analyser) = Analyser::pair();
    let frame = analyser.frame(false, 0.0);
    assert!(!frame.has_spectrum);
    assert!(frame.spectrum.iter().all(|bin| *bin == 0));
    assert_eq!(frame.power, 20.0);
    assert_eq!(frame.bass, 20.0);
    assert_eq!(frame.low_mid, 20.0);
    assert_eq!(frame.mid, 20.0);
    assert_eq!(frame.vocal, 20.0);
    assert_eq!(frame.treble, 20.0);
}

#[test]
fn full_tap_drops_without_blocking() {
    let (mut producer, mut analyser) = Analyser::pair();
    push_samples(&mut producer, &[0.0; TAP_CAPACITY]);
    assert_eq!(producer.slots(), 0);
    let started = Instant::now();
    push_samples(&mut producer, &[1.0; 4096]);
    assert!(started.elapsed() < Duration::from_millis(50));
    assert_eq!(producer.slots(), 0);
    let frame = analyser.frame(true, 0.0);
    assert!(frame.spectrum.iter().all(|bin| *bin == 0));
}
