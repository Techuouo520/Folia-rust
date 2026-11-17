// Embedded upstream preview covers decoded and sampled on the existing background worker.

use folia_core::Rgb;
use iced::widget::image::Handle;
use std::sync::Arc;

pub struct Art {
    pub portrait: Handle,
    pub colors: Vec<Rgb>,
    pub source: Arc<image::RgbaImage>,
}

impl std::fmt::Debug for Art {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.debug_struct("PreviewArt").field("size", &self.source.dimensions()).finish_non_exhaustive()
    }
}

pub fn load(index: usize) -> Result<Arc<Art>, String> {
    let bytes: &[u8] = if index == 0 {
        include_bytes!("../../../assets/preview/placeholder_cover.jpg")
    } else {
        include_bytes!("../../../assets/preview/placeholder_cover-2.jpg")
    };
    let (cover, background) =
        folia_library::cover_and_background(bytes, 512, 1920).map_err(|error| error.to_string())?;
    let colors = folia_visualizer::cover_palette(&background.rgba, background.width, background.height, 6);
    let source = image::RgbaImage::from_raw(background.width, background.height, background.rgba)
        .ok_or_else(|| "invalid preview pixels".to_owned())?;
    Ok(Arc::new(Art {
        portrait: Handle::from_rgba(cover.width, cover.height, cover.rgba),
        colors,
        source: Arc::new(source),
    }))
}

pub fn source_of(cover: Option<&Handle>) -> Option<Arc<image::RgbaImage>> {
    let Handle::Rgba { width, height, pixels, .. } = cover? else {
        return None;
    };
    image::RgbaImage::from_raw(*width, *height, pixels.as_ref().to_vec()).map(Arc::new)
}
