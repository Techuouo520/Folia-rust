// Native cinema geometry, lens passes and projection contracts through public rendering APIs.

use folia_core::{AudioFrame, CinemaOptions, LyricRenderQuality, Theme};
use folia_visualizer::cinema::{self, Config, Runtime};
use iced::{Rectangle, Size};

#[test]
fn cinema_seats_and_acoustics_share_the_original_hall() {
    let seats = cinema::seats();
    assert_eq!(seats.len(), 364);
    assert_eq!(seats.iter().filter(|seat| seat.occupied).count(), 84);
    let preferred = cinema::selected_seat(None);
    assert!(preferred.preferred && !preferred.occupied);
    assert_eq!(preferred.row, 10);
    assert_eq!(cinema::selected_seat(Some("unknown")).id, preferred.id);
    let listener = cinema::listener(Some(&preferred.id));
    assert_eq!(listener.position, preferred.eye);
    let world = cinema::acoustic_world().unwrap();
    assert_eq!(world.meshes[0].positions.len(), 2288);
    assert_eq!(world.meshes[0].triangles.len(), 1144);
    assert_eq!(world.materials.len(), 7);
    let size = Size::new(960.0, 600.0);
    let content = Size::new(960.0, 960.0 / cinema::SCREEN_ASPECT);
    for seat in [None, Some("R4-13"), Some("R12-24")] {
        let mut head = cinema::Head { yaw: 0.2, pitch: -0.1 };
        let camera = cinema::Camera::new(cinema::selected_seat(seat), &mut head, size);
        for world in [[0.0, 4.885, 0.02], [-3.5, 6.7775, 0.02], [10.0, 4.885, 0.02]] {
            let projected = camera.project(world, size).unwrap();
            let mapped = camera.screen_plane_point(projected, size, content).unwrap();
            assert!((mapped.x - (world[0] / 14.0 + 0.5) * content.width).abs() < 0.001);
            assert!((mapped.y - (0.5 - (world[1] - 4.885) / 7.57) * content.height).abs() < 0.001);
            assert_eq!(camera.screen_point(projected, size, content).is_some(), world[0].abs() <= 7.0);
        }
    }
}

#[test]
#[cfg_attr(feature = "ci", ignore = "needs a GPU")]
fn native_cinema_renders_the_hall_and_live_projection() {
    let (adapter, device, queue) = pollster::block_on(folia_render::backend::device()).unwrap();
    let _registration =
        folia_render::quality::register_device(&adapter, &device, iced_wgpu::graphics::Shell::headless());
    let source_device = folia_render::quality::OffscreenDevice::new(&device, &queue, wgpu::TextureFormat::Rgba8Unorm);
    let mut cinema =
        cinema::Renderer::new(&device, &queue, wgpu::TextureFormat::Rgba8Unorm, source_device.texture_layout())
            .unwrap();
    let source = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("cinema-test-screen"),
        size: wgpu::Extent3d { width: 24, height: 8, depth_or_array_layers: 1 },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::COPY_DST | wgpu::TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    });
    let pixels = (0..192)
        .flat_map(|index| {
            if index % 24 < 8 {
                [180, 35, 70, 255]
            } else if index % 24 < 16 {
                [40, 150, 195, 255]
            } else {
                [235, 190, 90, 255]
            }
        })
        .collect::<Vec<_>>();
    queue.write_texture(
        source.as_image_copy(),
        &pixels,
        wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(96), rows_per_image: Some(8) },
        source.size(),
    );
    let view = source.create_view(&Default::default());
    let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        min_filter: wgpu::FilterMode::Linear,
        mag_filter: wgpu::FilterMode::Linear,
        ..Default::default()
    });
    let picture = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("cinema-test-screen"),
        layout: source_device.texture_layout(),
        entries: &[
            wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&view) },
            wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(&sampler) },
        ],
    });
    let options = CinemaOptions::default();
    let theme = Theme::MIDNIGHT;
    let config = Config {
        generation: 0,
        options: &options,
        render_quality: LyricRenderQuality::default(),
        theme: &theme,
        audio: &AudioFrame::SILENT,
        song_id: None,
        cover: None,
        cover_pending: false,
        paused: false,
        static_mode: false,
        reduced_motion: false,
        active: true,
        queue_ended: false,
        publish_blur: false,
    };
    let size = Size::new(960.0, 600.0);
    let mut frame = Runtime::default().frame(config, size, 0.0);
    frame.time = 6.0;
    frame.dt = 6.0;
    frame.smoke_time = 6.0;
    frame.dust_time = 6.0;
    cinema.prepare(frame, size);
    assert_eq!(cinema.target_size(), Some([1200, 750]));
    let target = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("cinema-test-output"),
        size: wgpu::Extent3d { width: 960, height: 600, depth_or_array_layers: 1 },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let target_view = target.create_view(&Default::default());
    let mut encoder = device.create_command_encoder(&Default::default());
    cinema.render(&mut encoder, &picture);
    cinema.composite(
        &mut encoder,
        &target_view,
        &picture,
        Rectangle::with_size(size),
        Rectangle { x: 0, y: 0, width: 960, height: 600 },
    );
    queue.submit([encoder.finish()]);
    let rgba = read(&device, &queue, &target);
    assert!(rgba.chunks_exact(4).any(|pixel| pixel[0] > 100 && pixel[2] < 100));
    assert!(rgba.chunks_exact(4).any(|pixel| pixel[2] > 150));
    compare(&rgba, "medium-r10-13-t6");
    if std::env::var_os("FOLIA_CINEMA_PERF").is_some() {
        let mut cpu = Vec::new();
        let mut complete = Vec::new();
        frame.dt = 1.0 / 60.0;
        for index in 0..180 {
            frame.time += frame.dt;
            frame.smoke_time += frame.dt;
            frame.dust_time += frame.dt;
            let started = std::time::Instant::now();
            cinema.prepare(frame, size);
            let mut encoder = device.create_command_encoder(&Default::default());
            cinema.render(&mut encoder, &picture);
            cinema.composite(
                &mut encoder,
                &target_view,
                &picture,
                Rectangle::with_size(size),
                Rectangle { x: 0, y: 0, width: 960, height: 600 },
            );
            queue.submit([encoder.finish()]);
            let submitted = started.elapsed().as_secs_f64() * 1000.0;
            device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
            if index >= 20 {
                cpu.push(submitted);
                complete.push(started.elapsed().as_secs_f64() * 1000.0);
            }
        }
        cpu.sort_by(f64::total_cmp);
        complete.sort_by(f64::total_cmp);
        eprintln!(
            "cinema 960x600 medium CPU median={:.3}ms p95={:.3}ms queue-complete median={:.3}ms p95={:.3}ms",
            cpu[80], cpu[152], complete[80], complete[152]
        );
    }
    for (name, seat, lights, paused, still, seconds) in [
        ("day-r4-13-t6", Some("R4-13"), true, false, false, 6),
        ("paused-r10-13-t12", None, false, true, false, 12),
        ("static-r10-13-t6", None, false, false, true, 6),
    ] {
        let options = CinemaOptions { viewpoint: seat.map(str::to_owned), house_lights: lights, ..Default::default() };
        let config = Config { options: &options, paused, static_mode: still, ..config };
        let mut runtime = Runtime::default();
        let mut frame = runtime.frame(config, size, 0.0);
        for index in 1..=seconds * 60 {
            frame = runtime.frame(config, size, f64::from(index) * 1000.0 / 60.0);
        }
        frame.time = seconds as f32;
        frame.smoke_time = seconds as f32;
        frame.dust_time = seconds as f32;
        frame.dt = seconds as f32;
        frame.sample_screen = !still;
        let mut renderer =
            cinema::Renderer::new(&device, &queue, wgpu::TextureFormat::Rgba8Unorm, source_device.texture_layout())
                .unwrap();
        renderer.prepare(frame, size);
        let mut encoder = device.create_command_encoder(&Default::default());
        renderer.render(&mut encoder, &picture);
        renderer.composite(
            &mut encoder,
            &target_view,
            &picture,
            Rectangle::with_size(size),
            Rectangle { x: 0, y: 0, width: 960, height: 600 },
        );
        queue.submit([encoder.finish()]);
        compare(&read(&device, &queue, &target), name);
    }
}

fn compare(rgba: &[u8], name: &str) {
    let reference =
        image::open(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("fixtures/cinema/{name}.png")))
            .unwrap()
            .to_rgba8();
    let mean = rgba
        .chunks_exact(4)
        .zip(reference.pixels())
        .flat_map(|(actual, expected)| (0..3).map(move |index| f64::from(actual[index].abs_diff(expected[index]))))
        .sum::<f64>()
        / (960.0 * 600.0 * 3.0);
    eprintln!("native cinema/upstream {name} mean difference {mean:.6}/255");
    assert!(mean < 1.0, "cinema differs from the upstream {name} reference: {mean}");
}

fn read(device: &wgpu::Device, queue: &wgpu::Queue, texture: &wgpu::Texture) -> Vec<u8> {
    let width = texture.width();
    let height = texture.height();
    let row = (width * 4).div_ceil(256) * 256;
    let output = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("cinema-test-read"),
        size: u64::from(row) * u64::from(height),
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = device.create_command_encoder(&Default::default());
    encoder.copy_texture_to_buffer(
        texture.as_image_copy(),
        wgpu::TexelCopyBufferInfo {
            buffer: &output,
            layout: wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(row), rows_per_image: Some(height) },
        },
        texture.size(),
    );
    queue.submit([encoder.finish()]);
    let (send, receive) = std::sync::mpsc::channel();
    output.slice(..).map_async(wgpu::MapMode::Read, move |result| send.send(result).unwrap());
    device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
    receive.recv().unwrap().unwrap();
    let data = output.slice(..).get_mapped_range();
    data.chunks(row as usize).flat_map(|line| line[..width as usize * 4].iter().copied()).collect()
}
