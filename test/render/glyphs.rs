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
    glyphs.prepare(&device, &queue, &[], [WIDTH as f32, HEIGHT as f32], 1.0);
    assert_eq!(glyphs.stats().size, [0, 0], "an empty stage must not allocate an atlas");
    glyphs.prepare(&device, &queue, &sprites, [WIDTH as f32, HEIGHT as f32], 1.0);
    let populated = glyphs.stats();
    glyphs.prepare(&device, &queue, &[], [WIDTH as f32, HEIGHT as f32], 1.0);
    assert_eq!(glyphs.stats().size, populated.size, "brief blank frames keep the reusable atlas");
    assert_eq!(glyphs.stats().live_marks, 0);
    glyphs.prepare(&device, &queue, &sprites, [WIDTH as f32, HEIGHT as f32], 1.0);
    glyphs.release();
    assert_eq!(glyphs.stats().size, [0, 0]);
    assert_eq!(glyphs.stats().marks, 0);
    assert_eq!(glyphs.stats().peak_size, populated.peak_size);
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

#[test]
#[cfg_attr(feature = "ci", ignore = "requires hardware GPU")]
fn gaussian_glyphs_filter_coverage_and_reuse_sharp_rasters() {
    folia_ui::install_fonts();
    let (adapter, device, queue) = pollster::block_on(folia_render::backend::device()).unwrap();
    assert_ne!(adapter.get_info().device_type, wgpu::DeviceType::Cpu);
    let format = wgpu::TextureFormat::Rgba8Unorm;
    let shape = shape("光", 72.0, iced::Font::DEFAULT);
    let mark =
        |sigma| Mark::BlurredGlyph { glyph: shape.glyphs[0], size: 72.0, outline: 0.0, max_scale: f32::MAX, sigma };
    let sprite = |sigma| Sprite {
        mark: mark(sigma),
        anchor: [128.0, 128.0],
        pen: [-shape.width * 0.5, shape.baseline],
        axes: [1.0, 0.0, 0.0, 1.0],
        color: [1.0; 4],
    };
    let mut glyphs =
        Glyphs::new(&device, format, 1, vec![folia_render::glyphs::Image { width: 1, height: 1, pixels: vec![255] }]);
    glyphs.warm_blur(&device);
    let mut image = GlyphPicture::new(&device);
    let mut sharp = sprite(0.0);
    sharp.mark = Mark::Glyph { glyph: shape.glyphs[0], size: 72.0, outline: 0.0, max_scale: f32::MAX };
    let source = image.draw(&mut glyphs, &device, &queue, &[sharp], 0..1);
    let same = image.draw(&mut glyphs, &device, &queue, &[sprite(0.0)], 0..1);
    assert_eq!(source, same, "zero sigma preserves the original glyph path");
    for sigma in [1.5, 4.0, 8.0, 16.0, 32.0] {
        let filtered = image.draw(&mut glyphs, &device, &queue, &[sprite(sigma)], 0..1);
        let reference = gaussian_reference(&source, 256, sigma);
        let error: f64 =
            filtered.iter().zip(reference).map(|(actual, expected)| (f64::from(*actual) - expected).abs()).sum::<f64>()
                / filtered.len() as f64;
        eprintln!("Gaussian sigma={sigma}, mean={error:.5}/255, stats={:?}", glyphs.blur_stats());
        assert!(error < 0.8, "Gaussian sigma {sigma} differs from a separable reference: {error}");
        assert_eq!(glyphs.stats().marks, 1, "sigma must not change the coverage raster identity");
        assert_eq!(glyphs.blur_stats().jobs, 1);
    }
    let mut tinted = sprite(32.0);
    tinted.color = [0.2, 0.8, 0.3, 0.75];
    let _ = image.draw(&mut glyphs, &device, &queue, &[tinted], 0..1);
    assert_eq!(glyphs.blur_stats().jobs, 0, "tint-only frames reuse filtered coverage");
    glyphs.reserve(&device, 3);
    let solid = Sprite {
        mark: Mark::Image(0),
        anchor: [128.0, 128.0],
        pen: [0.0; 2],
        axes: [8.0, 0.0, 0.0, 8.0],
        color: [1.0; 4],
    };
    let range = image.draw(&mut glyphs, &device, &queue, &[sprite(4.0), sprite(12.0), solid], 2..3);
    assert_eq!(glyphs.stats().live_marks, 2, "normal images coexist with blurred glyphs");
    assert!(range[128 * 256 + 128] > 200);
    assert_eq!(range[64 * 256 + 64], 0, "draw ranges keep their original instance indices");
    let mut retina = sprite(0.0);
    retina.anchor = [64.0, 64.0];
    let sharp_retina = image.draw_density(&mut glyphs, &device, &queue, &[retina], 0..1, 2.0);
    retina.mark = mark(8.0);
    let blurred_retina = image.draw_density(&mut glyphs, &device, &queue, &[retina], 0..1, 2.0);
    let reference = gaussian_reference(&sharp_retina, 256, 16.0);
    let error = blurred_retina
        .iter()
        .zip(reference)
        .map(|(actual, expected)| (f64::from(*actual) - expected).abs())
        .sum::<f64>()
        / blurred_retina.len() as f64;
    assert!(error < 1.0, "DPI scales the physical Gaussian: {error}");
    glyphs.release();
    assert_eq!(glyphs.blur_stats().allocated_bytes, 0);
    assert_eq!(glyphs.stats().size, [0, 0]);
}

struct GlyphPicture {
    texture: wgpu::Texture,
    view: wgpu::TextureView,
    read: wgpu::Buffer,
}

impl GlyphPicture {
    fn new(device: &wgpu::Device) -> Self {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("gaussian-glyph-test"),
            size: wgpu::Extent3d { width: 256, height: 256, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let view = texture.create_view(&Default::default());
        let read = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("gaussian-glyph-test-read"),
            size: 256 * 256 * 4,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        Self { texture, view, read }
    }

    fn draw(
        &mut self,
        glyphs: &mut Glyphs,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        sprites: &[Sprite],
        range: std::ops::Range<u32>,
    ) -> Vec<u8> {
        self.draw_density(glyphs, device, queue, sprites, range, 1.0)
    }

    fn draw_density(
        &mut self,
        glyphs: &mut Glyphs,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        sprites: &[Sprite],
        range: std::ops::Range<u32>,
        density: f32,
    ) -> Vec<u8> {
        glyphs.prepare(device, queue, sprites, [256.0 / density, 256.0 / density], density);
        let mut encoder = device.create_command_encoder(&Default::default());
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("gaussian-glyph-test"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &self.view,
                    resolve_target: None,
                    depth_slice: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                ..Default::default()
            });
            glyphs.draw(&mut pass, range);
        }
        encoder.copy_texture_to_buffer(
            self.texture.as_image_copy(),
            wgpu::TexelCopyBufferInfo {
                buffer: &self.read,
                layout: wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(1024), rows_per_image: Some(256) },
            },
            self.texture.size(),
        );
        queue.submit([encoder.finish()]);
        let (send, receive) = std::sync::mpsc::channel();
        self.read.slice(..).map_async(wgpu::MapMode::Read, move |result| send.send(result).unwrap());
        device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
        receive.recv().unwrap().unwrap();
        let values = self.read.slice(..).get_mapped_range().chunks_exact(4).map(|pixel| pixel[3]).collect();
        self.read.unmap();
        values
    }
}

fn gaussian_reference(source: &[u8], side: usize, sigma: f32) -> Vec<f64> {
    let sigma = f64::from(sigma);
    let radius = (sigma * 3.0).ceil() as isize;
    let weights = (-radius..=radius).map(|offset| (-0.5 * (offset as f64 / sigma).powi(2)).exp()).collect::<Vec<_>>();
    let total: f64 = weights.iter().sum();
    let mut horizontal = vec![0.0; source.len()];
    let mut output = vec![0.0; source.len()];
    for y in 0..side {
        for x in 0..side {
            for (index, weight) in weights.iter().enumerate() {
                let sample = x as isize + index as isize - radius;
                if (0..side as isize).contains(&sample) {
                    horizontal[y * side + x] += f64::from(source[y * side + sample as usize]) * weight / total;
                }
            }
        }
    }
    for y in 0..side {
        for x in 0..side {
            for (index, weight) in weights.iter().enumerate() {
                let sample = y as isize + index as isize - radius;
                if (0..side as isize).contains(&sample) {
                    output[y * side + x] += horizontal[sample as usize * side + x] * weight / total;
                }
            }
        }
    }
    output
}

#[test]
#[ignore = "local release Gaussian cache benchmark"]
fn gaussian_atlas_changing_radius_benchmark() {
    assert!(!cfg!(debug_assertions), "run performance checks with --release");
    folia_ui::install_fonts();
    let (_, device, queue) = pollster::block_on(folia_render::backend::device()).unwrap();
    let shaped = shape("光影回环心海长风", 160.0, iced::Font::DEFAULT);
    for density in [1.0, 2.0, 3.0] {
        let mut glyphs = Glyphs::new(&device, wgpu::TextureFormat::Rgba8Unorm, 64, Vec::new());
        glyphs.warm_blur(&device);
        let mut sprites = (0..64)
            .map(|index| Sprite {
                mark: Mark::BlurredGlyph {
                    glyph: shaped.glyphs[index % shaped.glyphs.len()],
                    size: 160.0,
                    outline: 0.0,
                    max_scale: f32::MAX,
                    sigma: 0.0,
                },
                anchor: [640.0, 360.0],
                pen: [0.0; 2],
                axes: [0.5, 0.0, 0.0, 0.5],
                color: [0.8, 0.9, 1.0, 0.7],
            })
            .collect::<Vec<_>>();
        let mut cpu = Vec::new();
        let mut completed = Vec::new();
        let mut peak_bytes = 0;
        for frame in 0..180 {
            for (index, sprite) in sprites.iter_mut().enumerate() {
                if let Mark::BlurredGlyph { sigma, .. } = &mut sprite.mark {
                    *sigma = 4.0 + (index % 4) as f32 * 3.0 + (frame as f32 * 0.071 + index as f32).sin() * 0.7;
                }
            }
            let started = std::time::Instant::now();
            glyphs.prepare(&device, &queue, &sprites, [1280.0, 720.0], density);
            let prepared = started.elapsed().as_secs_f64() * 1000.0;
            device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
            let done = started.elapsed().as_secs_f64() * 1000.0;
            if frame == 0 {
                eprintln!("density={density} cold prepare={prepared:.3}ms complete={done:.3}ms");
            }
            if frame >= 20 {
                cpu.push(prepared);
                completed.push(done);
            }
            assert_eq!(glyphs.stats().marks, shaped.glyphs.len());
            assert_eq!(glyphs.blur_stats().jobs, 64);
            peak_bytes = peak_bytes.max(glyphs.blur_stats().allocated_bytes);
        }
        cpu.sort_by(f64::total_cmp);
        completed.sort_by(f64::total_cmp);
        eprintln!(
            "density={density} CPU median={:.3} p95={:.3}ms; CPU+GPU completion median={:.3} p95={:.3}ms; peak blur bytes={peak_bytes}; stats={:?}",
            cpu[cpu.len() / 2],
            cpu[cpu.len() * 95 / 100],
            completed[completed.len() / 2],
            completed[completed.len() * 95 / 100],
            glyphs.blur_stats()
        );
        glyphs.release();
        assert_eq!(glyphs.blur_stats().allocated_bytes, 0);
    }
}
