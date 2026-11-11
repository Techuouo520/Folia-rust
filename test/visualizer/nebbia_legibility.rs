// Small Chinese strokes must retain gaps and respond to the public stroke-width setting.

use folia_core::{Line, Lyrics, Theme, Word};
use folia_visualizer::styles::nebbia::{frame, glyphs_settled};
use folia_visualizer::{Item, Style, Values};

use super::nebbia_gpu::{GPU, Offscreen, with_strokes_in_child};

#[test]
#[cfg_attr(feature = "ci", ignore = "needs a GPU; CI runners only have a software renderer")]
fn small_stroke_width_remains_adjustable() {
    if with_strokes_in_child("nebbia_legibility::small_stroke_width_remains_adjustable") {
        return;
    }
    let _guard = GPU.lock().unwrap();
    let manifest = Style::Nebbia.manifest();
    let mut tuning = Values::default();
    for item in manifest.items() {
        if let Item::Member { key, option, .. } = item {
            tuning.toggle_member(
                manifest,
                key,
                option,
                matches!((key, option), ("layouts", "ribbon") | ("appear", "fade") | ("fade", "fog")),
            );
        }
    }
    for (key, value) in
        [("warp", 0.0), ("jitter", 0.0), ("doodle", 0.0), ("rainHit", 0.0), ("rainBig", 0.0), ("busSpeed", 0.0)]
    {
        tuning.set_number(manifest, key, value);
    }
    let size = iced::Size::new(1920.0, 1080.0);
    let mut gpu = Offscreen::new([1920, 1080]);
    let mut all_adjustable = true;
    for ch in ['是', '想', '虽', '然'] {
        let text = ch.to_string();
        let lyrics = Lyrics {
            lines: vec![Line {
                text: text.clone(),
                start: 0.0,
                end: 8.0,
                words: vec![Word { text, start: 0.0, end: 8.0 }],
                ..Line::default()
            }],
            ..Lyrics::default()
        };
        frame(&lyrics, 4.0, size, &Theme::MIDNIGHT, 0.3, &tuning, 0.0);
        let start = std::time::Instant::now();
        while !glyphs_settled(&lyrics) {
            assert!(start.elapsed().as_secs() < 60);
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        let mut images = Vec::new();
        for width in [0.04, 0.09] {
            tuning.set_number(manifest, "strokeWidth", width);
            let mut pass = frame(&lyrics, 4.0, size, &Theme::MIDNIGHT, 0.3, &tuning, 0.0);
            pass.uniforms.sky0 = [0.0; 4];
            pass.uniforms.sky1 = [0.0, 0.0, 0.0, 1.0];
            pass.uniforms.la = [0.0; 4];
            pass.uniforms.lb = [0.0; 4];
            pass.uniforms.lc = [0.0; 4];
            pass.uniforms.spot_i = 0.0;
            pass.uniforms.mist = 1.0;
            for _ in 0..6 {
                gpu.draw(&pass);
            }
            images.push(gpu.pixels());
        }
        let different = images[0].chunks_exact(4).zip(images[1].chunks_exact(4)).filter(|(a, b)| a != b).count();
        println!("small {ch}: changed pixels={different}");
        all_adjustable &= different > 20;
        if ch == '是' {
            let counter = counter_pixels(&glyph_crop(&images[1]));
            println!("small 是: upper counter pixels={counter}");
            assert!(counter >= 2, "the counter in 是 is filled by the stroke");
        }
        if let Ok(dir) = std::env::var("FOLIA_VISUAL_AUDIT") {
            for (i, pixels) in images.iter().enumerate() {
                image::RgbaImage::from_raw(1920, 1080, pixels.clone())
                    .unwrap()
                    .save(std::path::Path::new(&dir).join(format!("small-{ch}-{i}.png")))
                    .unwrap();
                let crop = glyph_crop(pixels);
                let zoom = image::imageops::resize(
                    &crop,
                    crop.width() * 6,
                    crop.height() * 6,
                    image::imageops::FilterType::Nearest,
                );
                zoom.save(std::path::Path::new(&dir).join(format!("crop-{ch}-{i}.png"))).unwrap();
            }
        }
    }
    assert!(all_adjustable, "small glyphs: the eight-pixel floor swallows the width setting");
}

fn glyph_crop(pixels: &[u8]) -> image::RgbaImage {
    let mut shot = image::RgbaImage::from_raw(1920, 1080, pixels.to_vec()).unwrap();
    let (mut left, mut top, mut right, mut bottom) = (1920u32, 1080u32, 0u32, 0u32);
    for (x, y, p) in shot.enumerate_pixels() {
        if p.0[0].max(p.0[1]).max(p.0[2]) < 16 {
            left = left.min(x);
            right = right.max(x);
            top = top.min(y);
            bottom = bottom.max(y);
        }
    }
    assert!(left < right && top < bottom, "missing letter");
    image::imageops::crop(&mut shot, left.saturating_sub(4), top.saturating_sub(4), right - left + 9, bottom - top + 9)
        .to_image()
}

// The upper third of 是 is 日: its rows must retain background between the left and right ink edges.
fn counter_pixels(image: &image::RgbaImage) -> usize {
    let mut gaps = 0;
    for y in 4..4 + (image.height() - 8) / 3 {
        let dark = |x| image.get_pixel(x, y).0[..3].iter().copied().max().unwrap() < 16;
        let left = (0..image.width()).find(|&x| dark(x));
        let right = (0..image.width()).rfind(|&x| dark(x));
        if let (Some(left), Some(right)) = (left, right) {
            gaps += (left..right).filter(|&x| !dark(x)).count();
        }
    }
    gaps
}
