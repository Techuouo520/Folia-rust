#![cfg(test)]
// Grain frame index stays inside one wrap of the integer hash.

use folia_render::grain_frame;

#[test]
fn grain_frames_wrap_before_they_grow() {
    assert_eq!(grain_frame(0.0), 0);
    assert_eq!(grain_frame(-3.0), 0);
    assert_eq!(grain_frame(1.0), 48);
    assert_eq!(grain_frame(100.0), 4800 % 4096);
    assert!(grain_frame(10_000.0) < 4096);
    assert_eq!(grain_frame(f32::NAN), 0);
}

mod blur;
mod glyphs;
mod scene;
