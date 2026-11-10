// Glyph sprites: a filled and a hollow glyph from the same shaped run, drawn offscreen.

use folia_render::glyphs::{Glyphs, Mark, Sprite, shape};

const WIDTH: u32 = 1000;
const HEIGHT: u32 = 600;

#[test]
fn hollow_glyph_is_a_thin_ring_over_a_faint_fill() {
    folia_ui::install_fonts();
    let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::default());
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default())).unwrap();
    let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default())).unwrap();
    let format = wgpu::TextureFormat::Rgba8Unorm;
    let shaped = shape("光", 320.0, iced::Font::DEFAULT);
    assert_eq!(shaped.glyphs.len(), 1);
    let sprite = |x: f32, outline: f32| Sprite {
        mark: Mark::Glyph { glyph: shaped.glyphs[0], size: 320.0, outline, max_scale: f32::MAX },
        anchor: [x, HEIGHT as f32 * 0.5],
        pen: [-shaped.width * 0.5, shaped.baseline],
        axes: [1.0, 0.0, 0.0, 1.0],
        color: [1.0, 1.0, 1.0, 1.0],
    };
    let sprites = [sprite(250.0, 0.0), sprite(750.0, 0.012)];
    let mut glyphs = Glyphs::new(&device, format, 16, Vec::new());
    glyphs.prepare(&device, &queue, &sprites, [WIDTH as f32, HEIGHT as f32], 1.0);
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: None,
        size: wgpu::Extent3d { width: WIDTH, height: HEIGHT, depth_or_array_layers: 1 },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    let row = (WIDTH * 4).div_ceil(256) * 256;
    let read = device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: u64::from(row * HEIGHT),
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
    {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: None,
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color::BLACK), store: wgpu::StoreOp::Store },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });
        glyphs.draw(&mut pass, 0..2);
    }
    encoder.copy_texture_to_buffer(
        texture.as_image_copy(),
        wgpu::TexelCopyBufferInfo {
            buffer: &read,
            layout: wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(row), rows_per_image: Some(HEIGHT) },
        },
        wgpu::Extent3d { width: WIDTH, height: HEIGHT, depth_or_array_layers: 1 },
    );
    queue.submit([encoder.finish()]);
    let slice = read.slice(..);
    slice.map_async(wgpu::MapMode::Read, |result| result.unwrap());
    device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
    let mapped = slice.get_mapped_range();
    let mut image = image::GrayImage::new(WIDTH, HEIGHT);
    for y in 0..HEIGHT {
        for x in 0..WIDTH {
            image.put_pixel(x, y, image::Luma([mapped[(y * row + x * 4) as usize]]));
        }
    }
    drop(mapped);
    let dir = std::env::temp_dir().join("folia-lumiere-text");
    std::fs::create_dir_all(&dir).unwrap();
    image.save(dir.join("glyph-outline.png")).unwrap();

    let ink = |from: u32, to: u32| -> (u64, u64, u8) {
        let (mut sum, mut faint, mut peak) = (0_u64, 0_u64, 0_u8);
        for y in 0..HEIGHT {
            for x in from..to {
                let value = image.get_pixel(x, y)[0];
                sum += u64::from(value);
                faint += u64::from((20..60).contains(&value));
                peak = peak.max(value);
            }
        }
        (sum, faint, peak)
    };
    let (solid, _, solid_peak) = ink(0, WIDTH / 2);
    let (hollow, faint, hollow_peak) = ink(WIDTH / 2, WIDTH);
    eprintln!("solid ink {solid} peak {solid_peak}; hollow ink {hollow} peak {hollow_peak}, faint fill pixels {faint}");
    assert!(solid_peak == 255 && hollow_peak > 200, "strokes must reach full coverage");
    assert!(hollow * 10 < solid * 6 && hollow * 100 > solid * 14, "a ring plus the 0.14 fill: {hollow} vs {solid}");
    assert!(faint > 2000, "the 0.14 fill shows inside the ring");
}
