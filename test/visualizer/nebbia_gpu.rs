// Reproducible offscreen nebbia frames and whole-renderer GPU timing, without a window or real settings.

use std::sync::Mutex;
use std::time::Instant;

use folia_core::{Lyrics, Theme};
use folia_visualizer::Values;
use folia_visualizer::styles::nebbia::{Pass, Renderer, frame};

pub static GPU: Mutex<()> = Mutex::new(());
const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;

pub(super) struct Offscreen {
    instance: wgpu::Instance,
    device: wgpu::Device,
    queue: wgpu::Queue,
    renderer: Renderer,
    target: wgpu::Texture,
    view: wgpu::TextureView,
    size: [u32; 2],
    queries: Option<wgpu::QuerySet>,
    resolve: wgpu::Buffer,
    read: wgpu::Buffer,
}

impl Offscreen {
    pub(super) fn new(size: [u32; 2]) -> Self {
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::from_env_or_default());
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default())).unwrap();
        let features = adapter.features() & wgpu::Features::TIMESTAMP_QUERY;
        println!("nebbia adapter: {:?}; timestamp={}", adapter.get_info(), !features.is_empty());
        assert_ne!(adapter.get_info().device_type, wgpu::DeviceType::Cpu, "hardware GPU required");
        let (device, queue) = pollster::block_on(
            adapter.request_device(&wgpu::DeviceDescriptor { required_features: features, ..Default::default() }),
        )
        .unwrap();
        let queries = (!features.is_empty()).then(|| {
            device.create_query_set(&wgpu::QuerySetDescriptor {
                label: Some("nebbia-frame-time"),
                ty: wgpu::QueryType::Timestamp,
                count: 2,
            })
        });
        let resolve = device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: 16,
            usage: wgpu::BufferUsages::QUERY_RESOLVE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        let read = device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: 16,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let target = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("nebbia-offscreen"),
            size: wgpu::Extent3d { width: size[0], height: size[1], depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let view = target.create_view(&Default::default());
        let renderer = Renderer::new(&device, &queue, FORMAT);
        Self { instance, device, queue, renderer, target, view, size, queries, resolve, read }
    }

    fn stamp(&self, encoder: &mut wgpu::CommandEncoder, index: u32) {
        let Some(queries) = &self.queries else { return };
        let _pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("nebbia-timestamp-boundary"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &self.view,
                resolve_target: None,
                depth_slice: None,
                ops: wgpu::Operations { load: wgpu::LoadOp::Load, store: wgpu::StoreOp::Store },
            })],
            timestamp_writes: Some(wgpu::RenderPassTimestampWrites {
                query_set: queries,
                beginning_of_pass_write_index: Some(index),
                end_of_pass_write_index: None,
            }),
            ..Default::default()
        });
    }

    pub(super) fn draw(&mut self, pass: &Pass) -> f64 {
        let [w, h] = self.size;
        self.renderer.prepare(&self.device, &self.queue, pass, [0.0, 0.0, w as f32, h as f32], 1.0);
        let mut encoder = self.device.create_command_encoder(&Default::default());
        self.stamp(&mut encoder, 0);
        self.renderer.render(&mut encoder, &self.view, [0, 0, w, h]);
        self.stamp(&mut encoder, 1);
        if let Some(queries) = &self.queries {
            encoder.resolve_query_set(queries, 0..2, &self.resolve, 0);
            encoder.copy_buffer_to_buffer(&self.resolve, 0, &self.read, 0, 16);
        }
        let start = Instant::now();
        self.queue.submit([encoder.finish()]);
        self.device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
        let wall = start.elapsed().as_secs_f64() * 1000.0;
        if self.queries.is_none() {
            return wall;
        }
        let bytes = mapped(&self.device, &self.read);
        let ticks: &[u64] = bytemuck::cast_slice(&bytes);
        assert!(ticks[1] >= ticks[0]);
        (ticks[1] - ticks[0]) as f64 * f64::from(self.queue.get_timestamp_period()) / 1_000_000.0
    }

    pub(super) fn pixels(&self) -> Vec<u8> {
        let [w, h] = self.size;
        let stride = (w * 4).div_ceil(256) * 256;
        let buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: u64::from(stride) * u64::from(h),
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut encoder = self.device.create_command_encoder(&Default::default());
        encoder.copy_texture_to_buffer(
            self.target.as_image_copy(),
            wgpu::TexelCopyBufferInfo {
                buffer: &buffer,
                layout: wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(stride), rows_per_image: Some(h) },
            },
            self.target.size(),
        );
        self.queue.submit([encoder.finish()]);
        mapped(&self.device, &buffer)
            .chunks_exact(stride as usize)
            .flat_map(|row| row[..(w * 4) as usize].iter().copied())
            .collect()
    }
}

fn mapped(device: &wgpu::Device, buffer: &wgpu::Buffer) -> Vec<u8> {
    let (tx, rx) = std::sync::mpsc::channel();
    buffer.slice(..).map_async(wgpu::MapMode::Read, move |result| tx.send(result).unwrap());
    device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
    rx.recv().unwrap().unwrap();
    let bytes = buffer.slice(..).get_mapped_range().to_vec();
    buffer.unmap();
    bytes
}

fn sample(size: [u32; 2], time: f64, bus: f32, theme: &Theme) -> Pass {
    let mut tuning = Values::default();
    tuning.set_number(folia_visualizer::Style::Nebbia.manifest(), "busSpeed", bus);
    let mut pass =
        frame(&Lyrics::default(), time, iced::Size::new(size[0] as f32, size[1] as f32), theme, 1.0, &tuning, 0.9);
    pass.uniforms.spot_i = 0.7;
    pass.uniforms.spot_r = 0.13;
    pass.uniforms.spot_tgt = [0.8, 0.4];
    pass.uniforms.lobes[0] = [size[0] as f32 * 0.4, size[1] as f32 * 0.35, 260.0, 120.0];
    pass.uniforms.lobe_sd[0] = [0.7, 0.4, 0.0, 0.0];
    pass
}

#[test]
#[cfg_attr(feature = "ci", ignore = "needs a GPU; CI runners only have a software renderer")]
fn nebbia_gpu_300_frames() {
    let _guard = GPU.lock().unwrap();
    for size in [[1920, 1080], [2560, 1440]] {
        let mut gpu = Offscreen::new(size);
        for i in 0..12 {
            gpu.draw(&sample(size, 6.0 + f64::from(i) / 60.0, 1.0, &Theme::MIDNIGHT));
        }
        let mut times: Vec<f64> =
            (0..300).map(|i| gpu.draw(&sample(size, 6.2 + f64::from(i) / 60.0, 1.0, &Theme::MIDNIGHT))).collect();
        times.sort_by(f64::total_cmp);
        println!(
            "NEBBIA {}x{} frames=300 metric={} median_ms={:.4} p95_ms={:.4}",
            size[0],
            size[1],
            if gpu.queries.is_some() { "GPU timestamp" } else { "CPU submit-to-completion" },
            (times[149] + times[150]) / 2.0,
            times[284]
        );
    }
}

#[test]
#[cfg_attr(feature = "ci", ignore = "needs a GPU; CI runners only have a software renderer")]
fn nebbia_matches_main_pixels() {
    if with_strokes_in_child("nebbia_gpu::nebbia_matches_main_pixels") {
        return;
    }
    let _guard = GPU.lock().unwrap();
    let size = [1920, 1080];
    let mut gpu = Offscreen::new(size);
    for (name, time) in [("06", 6.0), ("10", 10.5), ("15", 15.0)] {
        let pass = sample(size, time, 1.0, &Theme::MIDNIGHT);
        for _ in 0..6 {
            gpu.draw(&pass);
        }
        let pixels = gpu.pixels();
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("fixtures/nebbia/main-{name}.png"));
        let reference = image::open(path).unwrap().to_rgba8().into_raw();
        compare(&reference, &pixels);
    }
    let lyrics = super::nebbia_song();
    let tuning = Values::default();
    frame(&lyrics, 6.0, iced::Size::new(1920.0, 1080.0), &Theme::MIDNIGHT, 1.0, &tuning, 0.9);
    let started = Instant::now();
    while !folia_visualizer::styles::nebbia::glyphs_settled(&lyrics) {
        assert!(started.elapsed().as_secs() < 60, "glyph worker did not settle");
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    for (name, time) in [("song-06", 6.0), ("song-10", 10.5), ("song-15", 15.0)] {
        let pass = frame(&lyrics, time, iced::Size::new(1920.0, 1080.0), &Theme::MIDNIGHT, 1.0, &tuning, 0.9);
        for _ in 0..6 {
            gpu.draw(&pass);
        }
        let pixels = gpu.pixels();
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("fixtures/nebbia/main-{name}.png"));
        // Legacy writing includes an 8px floor and half-size mask; counters and edges intentionally change.
        // Keep the old images, bound the changed foreground, and separately test small-glyph counters.
        compare_with_limit(&image::open(path).unwrap().to_rgba8().into_raw(), &pixels, 0.75, 0.01);
        if let Ok(dir) = std::env::var("FOLIA_NEBBIA_ACTUAL") {
            image::RgbaImage::from_raw(size[0], size[1], pixels)
                .unwrap()
                .save(std::path::Path::new(&dir).join(format!("optimized-{name}.png")))
                .unwrap();
        }
    }
}

fn compare(reference: &[u8], pixels: &[u8]) {
    compare_with_limit(reference, pixels, 0.5, 0.0025);
}

fn compare_with_limit(reference: &[u8], pixels: &[u8], mean_limit: f64, large_limit: f64) {
    assert_eq!(reference.len(), pixels.len());
    let mut sum = 0u64;
    let mut large = 0usize;
    let mut max = 0u8;
    for (a, b) in reference.chunks_exact(4).zip(pixels.chunks_exact(4)) {
        assert_eq!(b[3], 255);
        for c in 0..3 {
            let d = a[c].abs_diff(b[c]);
            sum += u64::from(d);
            large += usize::from(d > 12);
            max = max.max(d);
        }
    }
    let count = (pixels.len() / 4 * 3) as f64;
    println!(
        "nebbia pixel diff: mean={:.4}/255, >12={:.5}%, max={max}",
        sum as f64 / count,
        large as f64 / count * 100.0
    );
    // Half-size buildings and enlarged fog permit local edge softening, not a scene or palette shift.
    assert!(sum as f64 / count < mean_limit);
    assert!(large as f64 / count < large_limit);
}

// Set the stroke path before the process starts any workers, never mutate the test runner's environment.
pub fn with_strokes_in_child(test: &str) -> bool {
    let path = super::nebbia_strokes_path();
    if std::env::var_os("FOLIA_NEBBIA_STROKES").as_deref() == Some(path.as_os_str()) {
        return false;
    }
    let result = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", test, "--nocapture"])
        .env("FOLIA_NEBBIA_STROKES", path)
        .status()
        .unwrap();
    assert!(result.success(), "isolated nebbia test failed: {test}");
    true
}

#[test]
fn fog_format_is_guaranteed_without_optional_features() {
    let guaranteed = wgpu::TextureFormat::Rgba16Float.guaranteed_format_features(wgpu::Features::empty());
    assert!(
        guaranteed
            .allowed_usages
            .contains(wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING)
    );
    assert!(guaranteed.flags.contains(wgpu::TextureFormatFeatureFlags::FILTERABLE));
}

#[test]
#[cfg_attr(feature = "ci", ignore = "needs a GPU; CI runners only have a software renderer")]
fn nebbia_cache_survives_wrap_seek_palette_and_resize() {
    let _guard = GPU.lock().unwrap();
    let size = [1920, 1080];
    let mut gpu = Offscreen::new(size);
    let mut cold = Renderer::new(&gpu.device, &gpu.queue, FORMAT);
    for (time, bus, theme) in [
        (6.0, 1.0, Theme::MIDNIGHT),
        (6.2, 1.0, Theme::MIDNIGHT),
        (70.0, 1.0, Theme::MIDNIGHT),
        (70.2, 1.0, Theme::MIDNIGHT),
        (70.2, 2.0, Theme::MIDNIGHT),
        (3.0, 0.4, Theme::DAYLIGHT),
        (160.0, 1.0, Theme::MIDNIGHT),
    ] {
        let pass = sample(size, time, bus, &theme);
        // The same timestamp through a new renderer is independent of the cache's travel history.
        std::mem::swap(&mut gpu.renderer, &mut cold);
        for _ in 0..6 {
            gpu.draw(&pass);
        }
        let reference = gpu.pixels();
        std::mem::swap(&mut gpu.renderer, &mut cold);
        println!("nebbia transition time={time} bus={bus}");
        for _ in 0..6 {
            gpu.draw(&pass);
            compare(&reference, &gpu.pixels());
        }
        cold = Renderer::new(&gpu.device, &gpu.queue, FORMAT);
    }
    let mut smaller = Offscreen::new([960, 540]);
    let pass = sample([960, 540], 12.0, 1.0, &Theme::DAYLIGHT);
    for _ in 0..6 {
        smaller.draw(&pass);
    }
    let reference = smaller.pixels();
    // Replace the output target on the same device; keep the existing renderer and its old cache.
    gpu.size = [960, 540];
    gpu.target = gpu.device.create_texture(&wgpu::TextureDescriptor {
        label: None,
        size: wgpu::Extent3d { width: 960, height: 540, depth_or_array_layers: 1 },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: FORMAT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    gpu.view = gpu.target.create_view(&Default::default());
    for _ in 0..6 {
        gpu.draw(&pass);
        compare(&reference, &gpu.pixels());
    }
}

#[test]
#[cfg_attr(feature = "ci", ignore = "needs a GPU; CI runners only have a software renderer")]
fn nebbia_widget_releases_gpu_resources() {
    use iced::widget::shader::{Pipeline, Primitive, Program, Viewport};
    let _guard = GPU.lock().unwrap();
    let gpu = Offscreen::new([640, 360]);
    let lyrics = Lyrics::default();
    let view = folia_visualizer::styles::nebbia::view::<()>(folia_visualizer::Scene {
        style: folia_visualizer::Style::Nebbia,
        lyrics: &lyrics,
        time: 6.0,
        theme: &Theme::MIDNIGHT,
        font_scale: 1.0,
        seed: "nebbia",
        stage: None,
        tuning: Values::default(),
        backdrop: true,
        title: String::new(),
        artist: String::new(),
        album: String::new(),
        portrait: None,
        preview: false,
        audio: &folia_core::AudioFrame::SILENT,
    });
    let state = folia_visualizer::styles::nebbia::ViewState::default();
    let bounds = iced::Rectangle::with_size(iced::Size::new(640.0, 360.0));
    let primitive = view.draw(&state, iced::mouse::Cursor::Unavailable, bounds);
    fn pipeline<P: Primitive>(_: &P, gpu: &Offscreen) -> P::Pipeline {
        P::Pipeline::new(&gpu.device, &gpu.queue, FORMAT)
    }
    let mut pipeline = pipeline(&primitive, &gpu);
    let resources = || {
        let report = gpu.instance.generate_report().unwrap().hub;
        [
            report.textures.num_allocated,
            report.texture_views.num_allocated,
            report.render_pipelines.num_allocated,
            report.bind_groups.num_allocated,
            report.buffers.num_allocated,
        ]
    };
    let before = resources();
    primitive.prepare(
        &mut pipeline,
        &gpu.device,
        &gpu.queue,
        &bounds,
        &Viewport::with_physical_size(iced::Size::new(640, 360), 1.0),
    );
    let allocated = resources();
    assert!(allocated.iter().zip(before).all(|(&live, empty)| live > empty));
    for _ in 0..3 {
        pipeline.trim();
        assert_eq!(resources(), allocated, "another window must not evict this widget");
    }
    drop(state);
    pipeline.trim();
    gpu.queue.submit([]);
    gpu.device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
    assert_eq!(resources(), before, "switching styles must release textures, binds, pipelines and buffers");
}
