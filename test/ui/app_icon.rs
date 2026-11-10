// Checks the original PNG and every prebuilt Windows icon frame.

use image::{GenericImageView, ImageFormat};

#[test]
fn the_original_png_becomes_a_transparent_window_icon() {
    let png = include_bytes!("../../assets/icon.png");
    assert_eq!(png[24], 16);
    let image = image::load_from_memory_with_format(png, ImageFormat::Png).unwrap().into_rgba8();
    assert_eq!(image.dimensions(), (512, 512));
    assert_eq!(image.get_pixel(0, 0)[3], 0);
    assert!(image.pixels().any(|pixel| pixel[3] == 255));
    let rgba = image.into_raw();
    let icon = iced::window::icon::from_rgba(rgba.clone(), 512, 512).unwrap();
    let (pixels, size) = icon.into_raw();
    assert_eq!(pixels, rgba);
    assert_eq!((size.width, size.height), (512, 512));
}

#[test]
fn the_executable_icon_has_four_decodable_rgba_sizes() {
    let ico = include_bytes!("../../assets/icon.ico");
    assert_eq!(&ico[..6], &[0, 0, 1, 0, 4, 0]);
    for (entry, size) in ico[6..70].chunks_exact(16).zip([16, 32, 48, 256]) {
        let dimension = if entry[0] == 0 { 256 } else { u32::from(entry[0]) };
        assert_eq!(dimension, size);
        assert_eq!(entry[0], entry[1]);
        assert_eq!(u16::from_le_bytes(entry[6..8].try_into().unwrap()), 32);
        let length = u32::from_le_bytes(entry[8..12].try_into().unwrap()) as usize;
        let offset = u32::from_le_bytes(entry[12..16].try_into().unwrap()) as usize;
        let image = image::load_from_memory_with_format(&ico[offset..offset + length], ImageFormat::Png).unwrap();
        assert_eq!(image.dimensions(), (size, size));
        assert_eq!(image.color(), image::ColorType::Rgba8);
        assert_eq!(image.into_rgba8().get_pixel(0, 0)[3], 0);
    }
}
