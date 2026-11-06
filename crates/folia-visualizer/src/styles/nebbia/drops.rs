// Condensed drops on the fog and the raindrop sprite. Drops are drawn on the GPU as sprites, as the
// original nebbiaGl.ts does with raindrop-fx's normal-map raindrop (MIT, assets/nebbia).

use std::sync::{Mutex, OnceLock};

use super::Rng;
use super::place::Drop;

/// Condensed drops per 2200 square pixels (`NEBBIA_FIXED.fieldDrops`).
const FIELD: f32 = 0.5;

/// The sprite and its mip chain, RGBA8, largest first.
pub struct Sprite {
    pub levels: Vec<(u32, u32, Vec<u8>)>,
}

/// Condensed drops for this CSS size, slightly more and bigger towards the bottom (the original `buildField`).
/// `read` sees them only when the size's key differs from `known`; returns the key.
pub fn field(width: f32, height: f32, known: u64, read: impl FnOnce(&[Drop])) -> u64 {
    static FIELD_DROPS: Mutex<(u64, Vec<Drop>)> = Mutex::new((0, Vec::new()));
    let key = (u64::from(width.to_bits()) << 32) | u64::from(height.to_bits());
    if key == known {
        return key;
    }
    let mut held = FIELD_DROPS.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    if held.0 != key {
        held.1.clear();
        let mut rnd = Rng(4242);
        let count = (width * height / 2200.0 * FIELD).round() as usize;
        for _ in 0..count {
            let big = rnd.unit() < 0.12;
            let w = if big { 40.0 + rnd.unit() * 34.0 } else { 14.0 + rnd.unit().powf(1.4) * 22.0 };
            let sp = rnd.unit() * 0.25;
            let x = rnd.unit() * width;
            let y = height * (1.0 - rnd.unit().powf(1.25));
            let dw = w * (1.0 + sp * 0.5);
            held.1.push([x, y, dw, w * (1.0 + sp), dw / 100.0, 1.0]);
        }
        held.0 = key;
    }
    read(&held.1);
    key
}

pub fn sprite() -> Option<&'static Sprite> {
    static SPRITE: OnceLock<Option<Sprite>> = OnceLock::new();
    SPRITE
        .get_or_init(|| {
            let image =
                image::load_from_memory(include_bytes!("../../../../../assets/nebbia/nebbiaRaindrop.png")).ok()?;
            let mut level = image.to_rgba8();
            let mut levels = Vec::new();
            loop {
                let (w, h) = level.dimensions();
                let next = (w > 1 || h > 1).then(|| {
                    image::imageops::resize(
                        &level,
                        (w / 2).max(1),
                        (h / 2).max(1),
                        image::imageops::FilterType::Triangle,
                    )
                });
                levels.push((w, h, level.into_raw()));
                match next {
                    Some(smaller) => level = smaller,
                    None => break,
                }
            }
            Some(Sprite { levels })
        })
        .as_ref()
}
