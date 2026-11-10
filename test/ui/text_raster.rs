// Interface Chinese rasterized the way iced draws it (cosmic-text + swash coverage), blended web-style and
// linear, saved next to a Chromium reference for comparison.

use iced::advanced::graphics::text::cosmic_text::{
    Attrs, Buffer, Family, Metrics, Shaping, SwashCache, SwashContent, Weight,
};
use iced::advanced::graphics::text::font_system;

const BG: f32 = 0xe6 as f32 / 255.0;
const INK: f32 = 0x1f as f32 / 255.0;

fn linear(c: f32) -> f32 {
    if c <= 0.04045 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) }
}

fn encode(c: f32) -> f32 {
    if c <= 0.0031308 { c * 12.92 } else { 1.055 * c.powf(1.0 / 2.4) - 0.055 }
}

/// Coverage of `text` at `size` px and `weight`, laid out on a `w`x`h` grid.
fn coverage(text: &str, size: f32, weight: u16, w: usize, h: usize) -> Vec<f32> {
    let mut fonts = font_system().write().unwrap();
    let system = fonts.raw();
    let mut buffer = Buffer::new(system, Metrics::new(size, 19.0));
    let attrs = Attrs::new().family(Family::Name("Microsoft YaHei UI")).weight(Weight(weight));
    let mut cache = SwashCache::new();
    let mut cov = vec![0.0f32; w * h];
    {
        let mut view = buffer.borrow_with(system);
        view.set_size(Some(w as f32), Some(h as f32));
        view.set_text(text, &attrs, Shaping::Advanced, None);
        view.shape_until_scroll(true);
    }
    let glyphs: Vec<_> = buffer
        .layout_runs()
        .flat_map(|run| run.glyphs.iter().map(|g| g.physical((4.0, run.line_y), 1.0)).collect::<Vec<_>>())
        .collect();
    for glyph in glyphs {
        let Some(image) = cache.get_image(system, glyph.cache_key).as_ref() else { continue };
        let p = image.placement;
        for row in 0..p.height as i32 {
            for col in 0..p.width as i32 {
                let x = glyph.x + p.left + col;
                let y = glyph.y - p.top + row;
                if x < 0 || y < 0 || x as usize >= w || y as usize >= h {
                    continue;
                }
                let src = (row * p.width as i32 + col) as usize;
                let a = match image.content {
                    SwashContent::Mask => image.data[src],
                    SwashContent::SubpixelMask => image.data[src * 3 + 1],
                    SwashContent::Color => image.data[src * 4 + 3],
                };
                let at = y as usize * w + x as usize;
                cov[at] = cov[at].max(f32::from(a) / 255.0);
            }
        }
    }
    cov
}

#[test]
fn chinese_interface_text_raster() {
    let (w, h) = (320usize, 22usize);
    let rows: [(&str, u16); 3] = [("600 web", 600), ("700 web", 700), ("600 linear", 600)];
    let mut image = image::RgbaImage::new(w as u32, (h * rows.len()) as u32);
    let mut ink = Vec::new();
    for (i, (label, weight)) in rows.iter().enumerate() {
        let cov = coverage("哪有什么不合适 呓语", 13.0, *weight, w, h);
        let mut sum = 0.0;
        for y in 0..h {
            for x in 0..w {
                let a = cov[y * w + x];
                let v = if label.ends_with("linear") {
                    encode(linear(BG) + (linear(INK) - linear(BG)) * a)
                } else {
                    BG + (INK - BG) * a
                };
                sum += BG - v;
                let byte = (v * 255.0).round() as u8;
                image.put_pixel(x as u32, (i * h + y) as u32, image::Rgba([byte, byte, byte, 255]));
            }
        }
        ink.push((*label, sum));
    }
    let dir = std::path::PathBuf::from(std::env::var("TEMP").unwrap()).join("folia-nebbia-fix");
    std::fs::create_dir_all(&dir).unwrap();
    image.save(dir.join("iced-raster.png")).unwrap();
    std::fs::write(dir.join("iced-raster.txt"), format!("{ink:?}")).unwrap();
    assert!(ink.iter().all(|(_, sum)| *sum > 1.0));
}

/// The same line through cryoglyph itself (iced's text renderer, web color mode), onto an Rgba8Unorm target.
#[test]
#[cfg_attr(feature = "ci", ignore = "needs a GPU; CI runners only have a software renderer")]
fn chinese_interface_text_cryoglyph() {
    use cryoglyph::{Cache, ColorMode, Resolution, TextArea, TextAtlas, TextBounds, TextRenderer, Viewport};
    let (w, h) = (320u32, 22u32);
    let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::default());
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default())).unwrap();
    let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default())).unwrap();
    let format = wgpu::TextureFormat::Rgba8Unorm;
    let mut fonts = cryoglyph::FontSystem::new();
    let mut swash = cryoglyph::SwashCache::new();
    let cache = Cache::new(&device);
    let mut atlas = TextAtlas::with_color_mode(&device, &queue, &cache, format, ColorMode::Web);
    let mut renderer = TextRenderer::new(&mut atlas, &device, wgpu::MultisampleState::default(), None);
    let mut viewport = Viewport::new(&device, &cache);
    viewport.update(&queue, Resolution { width: w, height: h });
    let mut buffer = cryoglyph::Buffer::new(&mut fonts, cryoglyph::Metrics::new(13.0, 19.0));
    let attrs = cryoglyph::Attrs::new()
        .family(cryoglyph::Family::Name("Microsoft YaHei UI"))
        .weight(cryoglyph::cosmic_text::Weight(600));
    buffer.set_size(&mut fonts, Some(w as f32), Some(h as f32));
    buffer.set_text(&mut fonts, "哪有什么不合适 呓语", &attrs, cryoglyph::cosmic_text::Shaping::Advanced, None);
    buffer.shape_until_scroll(&mut fonts, true);
    let target = device.create_texture(&wgpu::TextureDescriptor {
        label: None,
        size: wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let view = target.create_view(&wgpu::TextureViewDescriptor::default());
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
    let area = TextArea {
        buffer: &buffer,
        left: 4.0,
        top: 0.0,
        scale: 1.0,
        bounds: TextBounds { left: 0, top: 0, right: w as i32, bottom: h as i32 },
        default_color: cryoglyph::Color::rgb(0x1f, 0x1f, 0x1f),
    };
    renderer.prepare(&device, &queue, &mut encoder, &mut fonts, &mut atlas, &viewport, [area], &mut swash).unwrap();
    {
        let bg = f64::from(0xe6u8) / 255.0;
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: None,
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color { r: bg, g: bg, b: bg, a: 1.0 }),
                    store: wgpu::StoreOp::Store,
                },
                depth_slice: None,
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });
        renderer.render(&atlas, &viewport, &mut pass).unwrap();
    }
    let padded = w.div_ceil(64) * 64;
    let read = device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: u64::from(padded * h * 4),
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    encoder.copy_texture_to_buffer(
        target.as_image_copy(),
        wgpu::TexelCopyBufferInfo {
            buffer: &read,
            layout: wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(padded * 4), rows_per_image: Some(h) },
        },
        wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
    );
    queue.submit([encoder.finish()]);
    read.slice(..).map_async(wgpu::MapMode::Read, |_| {});
    device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
    let mapped = read.slice(..).get_mapped_range();
    let mut image = image::RgbaImage::new(w, h);
    let mut ink = 0u64;
    for y in 0..h {
        for x in 0..w {
            let at = ((y * padded + x) * 4) as usize;
            let px = [mapped[at], mapped[at + 1], mapped[at + 2], 255];
            ink += u64::from(230u8.saturating_sub(px[0]));
            image.put_pixel(x, y, image::Rgba(px));
        }
    }
    let dir = std::path::PathBuf::from(std::env::var("TEMP").unwrap()).join("folia-nebbia-fix");
    std::fs::create_dir_all(&dir).unwrap();
    image.save(dir.join("cryoglyph-patched.png")).unwrap();
    std::fs::write(dir.join("cryoglyph-patched.txt"), ink.to_string()).unwrap();
    assert!(ink > 1000);
}
