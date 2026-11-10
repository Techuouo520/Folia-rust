// Backdrop blur: CSS radius to pyramid steps, release, and one offscreen wall with the tools panel open.

use folia_render::{Pyramid, bench_glass, levels_for_radius, preview_wall, pyramid_bytes};

#[test]
fn css_blur_maps_onto_pyramid_steps() {
    assert_eq!(levels_for_radius(0.0), 0);
    assert_eq!(levels_for_radius(-4.0), 0);
    assert_eq!(levels_for_radius(f32::NAN), 0);
    // 8px is the back button and the title bar (`backdrop-blur-sm` in Tailwind 4).
    // 12px is the handle, the tools button and the player back button. 14px is the toast.
    // 24px is the tools panel, the island, the capsule and the search veil. 40px is the player panel.
    assert_eq!(levels_for_radius(8.0), 2);
    assert_eq!(levels_for_radius(12.0), 3);
    assert_eq!(levels_for_radius(14.0), 3);
    assert_eq!(levels_for_radius(24.0), 4);
    assert_eq!(levels_for_radius(40.0), 5);
    assert!(pyramid_bytes(3840, 2160, 5) < 20 * 1024 * 1024);
}

#[test]
fn closing_the_pyramid_drops_its_textures() {
    let (device, queue) = device();
    let mut pyramid = Pyramid::new(&device, wgpu::TextureFormat::Rgba8Unorm);
    assert_eq!(pyramid.resident_bytes(), 0);
    pyramid.ensure(&device, &queue, 1200, 800, 4);
    assert!(pyramid.resident_bytes() > 0);
    pyramid.release();
    assert_eq!(pyramid.resident_bytes(), 0);
    folia_render::compile_plate(&device, &queue, wgpu::TextureFormat::Bgra8UnormSrgb);
}

#[test]
fn wall_with_an_open_tools_panel_keeps_text_contrast() {
    let (device, queue) = device();
    let shot = preview_wall(&device, &queue, 960, 640).expect("preview");
    let dir = std::env::temp_dir().join("folia-blur");
    std::fs::create_dir_all(&dir).expect("temp dir");
    let path = dir.join("wall-tools.png");
    save_png(&path, shot.width, shot.height, &shot.rgba);
    // Red poster, above the glass. White title bar of the right-hand poster, under the glass.
    let outside = pixel(&shot.rgba, shot.width, 80, 320);
    let inside = pixel(&shot.rgba, shot.width, 700, 418);
    assert!(outside[0] > 140 && outside[0] > outside[1] * 2, "poster outside the panel stays red: {outside:?}");
    assert!(inside.into_iter().max().unwrap_or(0) > 20, "glass keeps the poster colour, not a black plate: {inside:?}");
    assert!(
        outside[0] > inside[0] + 80,
        "the 40% black fill darkens the poster: outside {outside:?} inside {inside:?}"
    );
    assert!(shot.bytes < 30 * 1024 * 1024, "open glass stays under 30 MB, got {}", shot.bytes);
    eprintln!("wall-tools {}", path.display());
}

#[test]
fn glass_pass_frame_time() {
    let (device, queue) = device();
    for (name, level) in [("title-8", 2u32), ("handle-12", 3), ("home-24", 4), ("player-40", 5)] {
        let bench = bench_glass(&device, &queue, 1200, 800, level, 8).expect(name);
        let line = format!("{name} avg_ms={:.3} max_ms={:.3} bytes={}", bench.avg_ms, bench.max_ms, bench.bytes);
        eprintln!("{line}");
        let _ = std::fs::write(std::env::temp_dir().join("folia-blur").join(format!("{name}.txt")), &line);
        assert!(bench.bytes < 30 * 1024 * 1024);
    }
}

fn pixel(rgba: &[u8], width: u32, x: u32, y: u32) -> [u8; 3] {
    let index = ((y * width + x) * 4) as usize;
    [rgba[index], rgba[index + 1], rgba[index + 2]]
}

fn save_png(path: &std::path::Path, width: u32, height: u32, rgba: &[u8]) {
    let image = image::RgbaImage::from_raw(width, height, rgba.to_vec()).expect("png buffer");
    image.save(path).expect("write png");
}

fn device() -> (wgpu::Device, wgpu::Queue) {
    let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::default());
    let adapter =
        pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default())).expect("adapter");
    pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default())).expect("device")
}
