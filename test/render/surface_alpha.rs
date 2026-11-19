// Read native surface bytes: screenshot format conversion can hide invalid premultiplied pixels.
use iced::advanced::Renderer as _;
use iced::advanced::graphics::geometry::Renderer as _;
use iced::widget::canvas::{Frame, Path, gradient};
use iced::{Color, Point, Rectangle, Size};
#[test]
#[cfg_attr(feature = "ci", ignore = "requires a hardware GPU for surface alpha readback")]
fn transparent_surface_retains_fades_and_premultiplied_edges() {
    let instance = wgpu::Instance::default();
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default())).unwrap();
    let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default())).unwrap();
    for (label, format) in [
        (
            "rgba8",
            folia_render::backend::surface_format(
                &[wgpu::TextureFormat::Rgb10a2Unorm, wgpu::TextureFormat::Rgba8Unorm],
                false,
            )
            .unwrap(),
        ),
        ("rgb10a2", wgpu::TextureFormat::Rgb10a2Unorm),
    ] {
        let engine = iced_wgpu::Engine::new(
            &adapter,
            device.clone(),
            queue.clone(),
            format,
            None,
            iced_wgpu::graphics::Shell::headless(),
        );
        let mut renderer = iced::Renderer::new(engine, iced::Font::DEFAULT, iced::Pixels(16.));
        let mut frame = Frame::new(&renderer, Size::new(512., 64.));
        let gradient = gradient::Linear::new(Point::ORIGIN, Point::new(512., 0.))
            .add_stop(0., Color::from_rgba(1., 1., 1., 0.))
            .add_stop(1., Color::WHITE);
        frame.fill(&Path::rectangle(Point::ORIGIN, Size::new(512., 64.)), gradient);
        renderer.reset(Rectangle::with_size(Size::new(512., 64.)));
        renderer.draw_geometry(frame.into_geometry());
        let size = wgpu::Extent3d { width: 512, height: 64, depth_or_array_layers: 1 };
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: None,
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        renderer.present(
            Some(Color::TRANSPARENT),
            format,
            &texture.create_view(&Default::default()),
            &iced::advanced::graphics::Viewport::with_physical_size(Size::new(512, 64), 1.),
        );
        let buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: 512 * 64 * 4,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let mut encoder = device.create_command_encoder(&Default::default());
        encoder.copy_texture_to_buffer(
            texture.as_image_copy(),
            wgpu::TexelCopyBufferInfo {
                buffer: &buffer,
                layout: wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(512 * 4), rows_per_image: None },
            },
            size,
        );
        queue.submit([encoder.finish()]);
        buffer.slice(..).map_async(wgpu::MapMode::Read, |r| r.unwrap());
        device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
        let bytes = buffer.slice(..).get_mapped_range();
        let pixels: Vec<u8> = if label == "rgba8" {
            bytes.to_vec()
        } else {
            bytes
                .chunks_exact(4)
                .flat_map(|p| {
                    let n = u32::from_le_bytes(p.try_into().unwrap());
                    [
                        ((n & 1023) * 255 / 1023) as u8,
                        (((n >> 10) & 1023) * 255 / 1023) as u8,
                        (((n >> 20) & 1023) * 255 / 1023) as u8,
                        ((n >> 30) * 85) as u8,
                    ]
                })
                .collect()
        };
        let alpha: std::collections::BTreeSet<_> = pixels.chunks_exact(4).map(|p| p[3]).collect();
        let overshoot = pixels.chunks_exact(4).map(|p| p[0].saturating_sub(p[3])).max().unwrap();
        let ghost = pixels.chunks_exact(4).filter(|p| p[3] == 0 && p[0] > 1).count();
        println!(
            "{label}: alpha_levels={} maximum_rgb_above_alpha={overshoot} colored_pixels_with_zero_alpha={ghost}",
            alpha.len()
        );
        if label == "rgba8" {
            assert!(alpha.len() > 200);
            assert!(overshoot <= 1);
            assert_eq!(ghost, 0);
        }
    }
}
