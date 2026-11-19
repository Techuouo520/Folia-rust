// CPU cover variants are prepared off the UI thread and retained only for visible radio identities.

use iced::advanced::image::Handle;

#[derive(Clone, Debug)]
pub struct Artwork {
    pub(crate) source: iced::advanced::image::Id,
    pub(crate) veil: Handle,
}

pub(crate) fn prepare(source: Handle) -> Option<Artwork> {
    let key = source.id();
    let Handle::Rgba { width, height, pixels, .. } = source else { return None };
    let image = image::RgbaImage::from_raw(width, height, pixels.to_vec())?;
    let veil = image::imageops::resize(&image, 18, 18, image::imageops::FilterType::Triangle);
    Some(Artwork { source: key, veil: Handle::from_rgba(18, 18, veil.into_raw()) })
}
