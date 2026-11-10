// Checks the equalizer's coefficients against the RBJ cookbook and that a processed frame allocates nothing.

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use std::sync::atomic::{AtomicUsize, Ordering};

use folia_core::{AudioEffects, BandShape, EQUALIZER_FREQUENCIES, EQUALIZER_MAX_GAIN_DB, EQUALIZER_MIN_GAIN_DB};
use folia_player::{Coefficients, Parameters, Processor};

thread_local! {
    /// Only the thread running the allocation test counts; the harness runs other tests at the same time.
    static COUNTING: Cell<bool> = const { Cell::new(false) };
    static ALLOCATIONS: Cell<usize> = const { Cell::new(0) };
}

struct Counting;

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if COUNTING.try_with(Cell::get).unwrap_or(false) {
            ALLOCATIONS.with(|count| count.set(count.get() + 1));
        }
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        unsafe { System.dealloc(pointer, layout) }
    }
}

#[global_allocator]
static ALLOCATOR: Counting = Counting;

/// The cookbook's peaking formula, written out again so the port is checked against the text, not itself.
fn peaking(frequency: f32, q: f32, gain_db: f32, rate: f32) -> [f32; 5] {
    let a = 10f32.powf(gain_db / 40.0);
    let w = 2.0 * std::f32::consts::PI * frequency / rate;
    let alpha = w.sin() / (2.0 * q);
    let a0 = 1.0 + alpha / a;
    [
        (1.0 + alpha * a) / a0,
        (-2.0 * w.cos()) / a0,
        (1.0 - alpha * a) / a0,
        (-2.0 * w.cos()) / a0,
        (1.0 - alpha / a) / a0,
    ]
}

fn low_shelf(frequency: f32, gain_db: f32, rate: f32) -> [f32; 5] {
    let a = 10f32.powf(gain_db / 40.0);
    let w = 2.0 * std::f32::consts::PI * frequency / rate;
    let (sin, cos) = w.sin_cos();
    let alpha = sin / 2.0 * 2f32.sqrt();
    let two = 2.0 * a.sqrt() * alpha;
    let a0 = (a + 1.0) + (a - 1.0) * cos + two;
    [
        a * ((a + 1.0) - (a - 1.0) * cos + two) / a0,
        2.0 * a * ((a - 1.0) - (a + 1.0) * cos) / a0,
        a * ((a + 1.0) - (a - 1.0) * cos - two) / a0,
        -2.0 * ((a - 1.0) + (a + 1.0) * cos) / a0,
        ((a + 1.0) + (a - 1.0) * cos - two) / a0,
    ]
}

fn high_shelf(frequency: f32, gain_db: f32, rate: f32) -> [f32; 5] {
    let a = 10f32.powf(gain_db / 40.0);
    let w = 2.0 * std::f32::consts::PI * frequency / rate;
    let (sin, cos) = w.sin_cos();
    let alpha = sin / 2.0 * 2f32.sqrt();
    let two = 2.0 * a.sqrt() * alpha;
    let a0 = (a + 1.0) - (a - 1.0) * cos + two;
    [
        a * ((a + 1.0) + (a - 1.0) * cos + two) / a0,
        -2.0 * a * ((a - 1.0) + (a + 1.0) * cos) / a0,
        a * ((a + 1.0) + (a - 1.0) * cos - two) / a0,
        2.0 * ((a - 1.0) - (a + 1.0) * cos) / a0,
        ((a + 1.0) - (a - 1.0) * cos - two) / a0,
    ]
}

#[test]
fn coefficients_match_the_rbj_cookbook() {
    let rate = 48_000.0;
    for (index, frequency) in EQUALIZER_FREQUENCIES.iter().enumerate() {
        for gain in [6.0, -9.0, EQUALIZER_MAX_GAIN_DB, EQUALIZER_MIN_GAIN_DB] {
            let shape = BandShape::of(index);
            let port = Coefficients::new(shape, *frequency, 1.4, gain, rate);
            let text = match shape {
                BandShape::Peaking => peaking(*frequency, 1.4, gain, rate),
                BandShape::LowShelf => low_shelf(*frequency, gain, rate),
                BandShape::HighShelf => high_shelf(*frequency, gain, rate),
            };
            for (got, want) in [port.b0, port.b1, port.b2, port.a1, port.a2].iter().zip(text) {
                assert!((got - want).abs() < 1e-5, "band {index} gain {gain}: {got} vs {want}");
            }
        }
    }
}

#[test]
fn processing_a_frame_allocates_nothing() {
    let mut processor = Processor::new(48_000.0);
    processor.apply(Parameters {
        enabled: true,
        gains: [7.0, -3.0, 2.0, 0.0, -6.0, 4.0, 1.0, -2.0, 5.0, -8.0],
        effects: AudioEffects {
            highpass: 60.0,
            lowpass: 12_000.0,
            drive: 0.35,
            crush: 0.2,
            wow: 0.3,
            noise: 0.25,
            width: 1.4,
            space: 0.5,
            punch: 0.4,
        },
    });
    let (mut left, mut right) = (0.1f32, -0.1f32);
    COUNTING.with(|counting| counting.set(true));
    ALLOCATIONS.with(|count| count.set(0));
    for index in 0..10_000 {
        left = ((index as f32) * 0.01).sin() * 0.2;
        right = ((index as f32) * 0.013).cos() * 0.2;
        processor.process(&mut left, &mut right);
    }
    COUNTING.with(|counting| counting.set(false));
    assert_eq!(ALLOCATIONS.with(Cell::get), 0, "a processed frame allocated");
    assert!(left.is_finite() && right.is_finite());
}
