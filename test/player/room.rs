// Native room convolution agrees with direct FIR and warms, swaps and releases through its real audio handoff.

use folia_core::RoomImpulseResponse;
use folia_player::{RoomControl, RoomConvolver};

#[test]
fn room_partitioned_convolution_preserves_true_stereo_timing() {
    for length in [1, 127, 128, 129, 257, 1200] {
        let channels = std::array::from_fn(|channel| {
            (0..length).map(|index| (((index * 7 + channel * 13) % 29) as f32 - 14.0) * 0.0008).collect::<Vec<_>>()
        });
        let impulse = RoomImpulseResponse { sample_rate: 48000, channels, gain: 0.75 };
        let mut convolver = RoomConvolver::new(&impulse).unwrap();
        let input: Vec<[f32; 2]> = (0..2400)
            .map(|index| [((index % 23) as f32 - 11.0) * 0.01, ((index * 3 % 31) as f32 - 15.0) * 0.012])
            .collect();
        for (time, frame) in input.iter().copied().enumerate() {
            let actual = convolver.process(frame).unwrap();
            let mut expected = [0.0f64; 2];
            for tap in 0..length.min(time + 1) {
                for (ear, sum) in expected.iter_mut().enumerate() {
                    *sum += f64::from(input[time - tap][0]) * f64::from(impulse.channels[ear][tap])
                        + f64::from(input[time - tap][1]) * f64::from(impulse.channels[ear + 2][tap]);
                }
            }
            for ear in 0..2 {
                assert!(
                    (f64::from(actual[ear]) - expected[ear] * 0.75).abs() < 2e-7,
                    "length={length},time={time},ear={ear}"
                );
            }
        }
        convolver.reset();
        assert_eq!(convolver.process([0.0, 0.0]).unwrap(), [0.0, 0.0]);
    }
}

#[test]
fn room_handoff_warms_crossfades_and_returns_to_dry() {
    let response = |gain| {
        let mut channels: [Vec<f32>; 4] = std::array::from_fn(|_| vec![0.0; 256]);
        channels[0][0] = 1.0;
        channels[3][0] = 1.0;
        Box::new(RoomConvolver::new(&RoomImpulseResponse { sample_rate: 48000, channels, gain }).unwrap())
    };
    let (mut control, mut mixer) = RoomControl::pair(48000);
    control.set(Some(response(0.5))).unwrap();
    mixer.drain();
    for sample in 0..30000 {
        let (mut left, mut right) = (0.2, 0.1);
        mixer.process(&mut left, &mut right);
        assert!(left.is_finite() && right.is_finite());
        if sample < 256 {
            assert_eq!((left, right), (0.2, 0.1));
        }
        if sample > 28000 {
            assert!((left - 0.1).abs() < 1e-5 && (right - 0.05).abs() < 1e-5);
        }
        if sample % 128 == 0 {
            control.collect();
            mixer.drain();
        }
    }
    control.set(Some(response(0.75))).unwrap();
    mixer.drain();
    let mut previous = 0.1;
    for sample in 0..7000 {
        let (mut left, mut right) = (0.2, 0.1);
        mixer.process(&mut left, &mut right);
        assert!((left - previous).abs() < 0.001);
        previous = left;
        if sample % 128 == 0 {
            control.collect();
            mixer.drain();
        }
    }
    assert!((previous - 0.15).abs() < 1e-5);
    control.set(None).unwrap();
    mixer.drain();
    for sample in 0..30000 {
        let (mut left, mut right) = (0.2, 0.1);
        mixer.process(&mut left, &mut right);
        if sample > 28000 {
            assert_eq!((left, right), (0.2, 0.1));
        }
        if sample % 128 == 0 {
            control.collect();
            mixer.drain();
        }
    }
    control.collect();
    assert!(control.released());
}

#[test]
fn room_release_acknowledges_the_latest_fade_and_backpressure() {
    let response = || {
        Box::new(
            RoomConvolver::new(&RoomImpulseResponse {
                sample_rate: 48000,
                channels: std::array::from_fn(|_| vec![0.0; 256]),
                gain: 1.0,
            })
            .unwrap(),
        )
    };
    let (mut control, mut mixer) = RoomControl::pair(48000);
    control.set(None).unwrap();
    assert!(control.released());
    control.set(Some(response())).unwrap();
    mixer.drain();
    control.set(None).unwrap();
    mixer.drain();
    mixer.silent(48000 / 5);
    assert!(!control.released());
    control.set(Some(response())).unwrap();
    control.set(None).unwrap();
    mixer.drain();
    mixer.silent(48000 * 36 / 100);
    assert!(!control.released(), "an earlier release acknowledged the latest fade");
    mixer.silent(48000 / 5);
    control.collect();
    assert!(control.released());
    for _ in 0..12 {
        control.set(Some(response())).unwrap();
    }
    control.set(None).unwrap();
    for _ in 0..12 {
        control.collect();
        mixer.drain();
        mixer.silent(48000);
    }
    control.collect();
    assert!(control.released(), "a saturated command queue retained its last room");
}
