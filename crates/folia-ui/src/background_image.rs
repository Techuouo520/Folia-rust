// Coalesces image processing requests and keeps only the newest completed background poster.

use folia_core::Theme;
use folia_visualizer::monet_background::{self, Tuning};
use iced::widget::image::Handle;
use std::sync::Arc;

#[derive(Clone, PartialEq)]
struct Key {
    source: usize,
    colors: [folia_core::Rgb; 3],
    tuning: Tuning,
    raw: bool,
}

#[derive(Default)]
pub struct Poster {
    desired: Option<Key>,
    generation: u64,
    running: bool,
    image: Option<Handle>,
}

impl Poster {
    pub fn clear(&mut self) {
        if self.desired.take().is_some() {
            self.generation = self.generation.wrapping_add(1);
        }
        self.image = None;
    }

    pub fn image(&self) -> Option<&Handle> {
        self.image.as_ref()
    }

    pub fn request(
        &mut self,
        source: Option<&Arc<image::RgbaImage>>,
        theme: &Theme,
        mut tuning: Tuning,
        raw: bool,
    ) -> Option<Job> {
        // Layout and drift happen on the GPU and do not invalidate the processed bitmap.
        tuning.half = false;
        tuning.offset = 0.0;
        tuning.drift = false;
        tuning.drift_strength = 0.0;
        tuning.uploaded = false;
        if raw {
            tuning = Tuning::DEFAULT;
        }
        let key = source.map(|source| Key {
            source: Arc::as_ptr(source) as usize,
            colors: if raw { [folia_core::Rgb::hex(0); 3] } else { [theme.background, theme.primary, theme.accent] },
            tuning,
            raw,
        });
        if key != self.desired {
            self.desired = key;
            self.generation = self.generation.wrapping_add(1);
            self.image = None;
        }
        if self.running || self.image.is_some() {
            return None;
        }
        let source = source?.clone();
        self.running = true;
        Some(Job { generation: self.generation, source, theme: theme.clone(), tuning, raw })
    }

    pub fn finish(&mut self, generation: u64, image: Option<Handle>) {
        self.running = false;
        if self.generation == generation {
            self.image = image;
        }
    }
}

pub struct Job {
    pub generation: u64,
    source: Arc<image::RgbaImage>,
    theme: Theme,
    tuning: Tuning,
    raw: bool,
}

struct SharedPixels(Arc<image::RgbaImage>);

impl AsRef<[u8]> for SharedPixels {
    fn as_ref(&self) -> &[u8] {
        self.0.as_raw()
    }
}

impl Job {
    pub fn paint(self) -> Handle {
        if self.raw {
            Handle::from_rgba(
                self.source.width(),
                self.source.height(),
                iced::advanced::graphics::core::Bytes::from_owner(SharedPixels(self.source)),
            )
        } else {
            monet_background::paint(&self.source, &self.theme, self.tuning)
        }
    }
}

/// Decodes a user asset on a worker, keeping its full aspect ratio.
pub fn load(path: &std::path::Path) -> Result<Arc<image::RgbaImage>, String> {
    let bytes = std::fs::read(path).map_err(|err| err.to_string())?;
    let cover = folia_library::cover_uncropped(&bytes, 1920).map_err(|err| err.to_string())?;
    image::RgbaImage::from_raw(cover.width, cover.height, cover.rgba)
        .map(Arc::new)
        .ok_or_else(|| "invalid background pixels".to_owned())
}

pub fn import(source: &std::path::Path, destination: &std::path::Path) -> Result<Arc<image::RgbaImage>, String> {
    let image = load(source)?;
    let parent = destination.parent().ok_or_else(|| "background destination has no parent".to_owned())?;
    std::fs::create_dir_all(parent).map_err(|err| err.to_string())?;
    image.save_with_format(destination, image::ImageFormat::Png).map_err(|err| err.to_string())?;
    Ok(image)
}
