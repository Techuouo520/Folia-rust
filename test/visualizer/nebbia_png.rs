// Headless readback of one nebbia frame through the crate's own renderer (mask, drops and window passes).

use std::sync::mpsc;

use folia_visualizer::styles::nebbia::{Renderer, frame, glyphs_settled};

pub fn rgba(width: u32, height: u32, lyrics: &folia_core::Lyrics, time: f64, theme: &folia_core::Theme) -> Vec<u8> {
    rgba_energy(width, height, lyrics, time, theme, 0.0)
}

pub fn rgba_energy(
    width: u32,
    height: u32,
    lyrics: &folia_core::Lyrics,
    time: f64,
    theme: &folia_core::Theme,
    energy: f32,
) -> Vec<u8> {
    let size = iced::Size::new(width as f32, height as f32);
    let tuning = folia_visualizer::Values::default();
    let started = std::time::Instant::now();
    frame(lyrics, time, size, theme, 1.0, &tuning, energy);
    while !glyphs_settled(lyrics) && started.elapsed().as_secs() < 60 {
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::default());
    let adapter =
        pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default())).expect("gpu adapter");
    let (device, queue) =
        pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default())).expect("gpu device");
    let format = wgpu::TextureFormat::Rgba8Unorm;
    let mut renderer = Renderer::new(&device, &queue, format);
    let pass = frame(lyrics, time, size, theme, 1.0, &tuning, energy);
    renderer.prepare(&device, &queue, &pass, [0.0, 0.0, width as f32, height as f32], 1.0);
    let target = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("folia-nebbia-frame"),
        size: wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let view = target.create_view(&wgpu::TextureViewDescriptor::default());
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
    renderer.render(&mut encoder, &view, [0, 0, width, height]);
    let padded = width.div_ceil(64) * 64;
    let buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("folia-nebbia-read"),
        size: u64::from(padded) * u64::from(height) * 4,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    encoder.copy_texture_to_buffer(
        target.as_image_copy(),
        wgpu::TexelCopyBufferInfo {
            buffer: &buffer,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(padded * 4),
                rows_per_image: Some(height),
            },
        },
        wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
    );
    queue.submit([encoder.finish()]);
    let (sender, receiver) = mpsc::channel();
    buffer.slice(..).map_async(wgpu::MapMode::Read, move |result| {
        let _ = sender.send(result);
    });
    device.poll(wgpu::PollType::wait_indefinitely()).expect("gpu poll");
    receiver.recv().expect("gpu map").expect("gpu map failed");
    let mapped = buffer.slice(..).get_mapped_range();
    let mut rgba = vec![0u8; (width * height * 4) as usize];
    for y in 0..height {
        let src = (y * padded * 4) as usize;
        let dst = (y * width * 4) as usize;
        rgba[dst..dst + (width * 4) as usize].copy_from_slice(&mapped[src..src + (width * 4) as usize]);
    }
    drop(mapped);
    buffer.unmap();
    rgba
}
