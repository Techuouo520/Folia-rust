#![cfg(test)]
// Offscreen lumiere frames through the app's own primitives: both groups with their bloom, front bokeh, frame.

use std::path::PathBuf;

use folia_core::{Line, Lyrics, Theme, Word};
use folia_visualizer::Style;
use folia_visualizer::styles::lumiere::{self, Tuning};
use folia_visualizer::{Item, Values};
use iced::widget::canvas::Program as CanvasProgram;
use iced::widget::shader::Program;
use iced::{Rectangle, mouse};
use iced_wgpu::core::Renderer as CoreRenderer;
use iced_wgpu::graphics::geometry::Renderer as _;
use iced_wgpu::graphics::{Shell, Viewport};
use iced_wgpu::primitive::Renderer as ShaderDraw;
use iced_wgpu::wgpu;

const WIDTH: u32 = 1920;
const HEIGHT: u32 = 1080;

/// A line timed character by character.
#[cfg(all(feature = "stress", not(feature = "ci")))]
fn timed(start: f64, end: f64, text: &str) -> Line {
    let count = text.chars().count() as f64;
    let span = (end - start) / count;
    let words = text
        .chars()
        .enumerate()
        .map(|(index, c)| Word {
            text: c.to_string(),
            start: start + span * index as f64,
            end: start + span * (index as f64 + 1.0),
        })
        .collect();
    Line { words, start, end, text: text.to_owned(), ..Line::default() }
}

/// A whole song for the atlas: 24 lines, a new paragraph after every sixth.
#[cfg(all(feature = "stress", not(feature = "ci")))]
fn song() -> Lyrics {
    let texts = [
        "夜色慢慢落进城市的灯",
        "我在街角等一阵风",
        "风里有你走过的声音",
        "像旧唱片转到最后一圈",
        "光从云的缝隙里漏下来",
        "照亮了没说完的那句话",
        "后来我们各自走远",
        "把名字写在雨水里",
        "雨停的时候字也散了",
        "只剩一盏路灯还醒着",
        "它记得每一次回头",
        "也记得你笑的样子",
        "如果明天天气很好！？",
        "我就把窗户全部打开！！",
        "让海的气味进来！！",
        "让蓝色铺满整个房间",
        "你会不会也在某处",
        "听见同一首歌",
        "当最后一个音符停下",
        "世界安静得像刚醒来",
        "我把心事折成纸船",
        "放进银河慢慢漂远",
        "每一颗星都是答案",
        "每一束光都是你",
    ];
    let mut lines = Vec::new();
    let mut at = 0.6;
    for (index, text) in texts.iter().enumerate() {
        let length = 3.0 + text.chars().count() as f64 * 0.18;
        lines.push(timed(at, at + length, text));
        at += length + if index % 6 == 5 { 4.5 } else { 0.25 };
    }
    Lyrics { lines, title: Some("长歌".to_owned()), artist: Some("folia".to_owned()), word_timed: true }
}

fn line(start: f64, end: f64, text: &str) -> Line {
    let words: Vec<Word> = text
        .split_whitespace()
        .scan(start, |cursor, part| {
            let span = (end - start) / text.split_whitespace().count() as f64;
            let word = Word { text: part.to_owned(), start: *cursor, end: *cursor + span };
            *cursor += span;
            Some(word)
        })
        .collect();
    Line { words, start, end, text: text.to_owned(), ..Line::default() }
}

fn sample() -> Lyrics {
    Lyrics {
        lines: vec![
            line(0.4, 3.6, "光落在这一行"),
            line(3.8, 7.2, "像安静的雨"),
            line(7.4, 11.0, "字被照亮"),
            line(14.0, 18.0, "另一段灯又亮了"),
        ],
        title: Some("绘光".to_owned()),
        artist: Some("folia".to_owned()),
        word_timed: true,
    }
}

#[test]
fn defaults_match_the_original_panel() {
    let tuning = Tuning::DEFAULT;
    assert_eq!(tuning.light_intensity, 1.0);
    assert_eq!(tuning.dark_field, 0.75);
    assert_eq!(tuning.unlit_opacity, 0.22);
    assert_eq!(tuning.window_neighbors, 2);
    assert_eq!(tuning.fog_octaves, 5.0);
    assert!(tuning.line_art && tuning.overlay_frame && tuning.seamless);
    let manifest = Style::Lumiere.manifest();
    assert_eq!(manifest.storage_key, "lumiere_tuning");
    assert_eq!(manifest.groups.len(), 5);
    assert_eq!(manifest.items().count(), 23);
    assert!(manifest.items().any(|item| matches!(item, Item::Select { key: "renderQuality", hint_key: Some(_), .. })));
    assert_eq!(Tuning::read(&Values::default()), Tuning::DEFAULT);
    let mut values = Values::default();
    values.set_number(manifest, "fogOctaves", 3.4);
    values.set_choice(manifest, "windowNeighbors", "1");
    values.set_choice(manifest, "renderQuality", "low");
    values.set_flag(manifest, "seamlessTransitions", false);
    let loaded = Tuning::read(&Values::from_table(manifest, Some(&values.to_table(manifest))));
    assert_eq!(loaded.fog_octaves, 3.0);
    assert_eq!(loaded.window_neighbors, 1);
    assert_eq!(loaded.render_quality, lumiere::Quality::Low);
    assert!(!loaded.seamless);
    assert_eq!(Style::from_id("lumiere"), Some(Style::Lumiere));
    assert_eq!(Style::Lumiere.label_key(), "ui.visualizerLumiere");
}

/// The stage pass of one frame, drawn the way the app's shader widget draws it.
#[cfg(all(feature = "stress", not(feature = "ci")))]
fn stage<'a>(
    lyrics: &'a Lyrics,
    time: f64,
    seed: &'a str,
    state: &lumiere::LiveState,
) -> <lumiere::Field<'a, ()> as Program<()>>::Primitive {
    let theme = Theme::MIDNIGHT;
    let scene = folia_visualizer::Scene {
        style: Style::Lumiere,
        lyrics,
        time,
        theme: &theme,
        font_scale: 1.0,
        seed,
        stage: None,
        tuning: Values::default(),
        backdrop: false,
        title: String::new(),
        artist: String::new(),
        album: String::new(),
        portrait: None,
        typography: folia_visualizer::Typography::default(),
        preview: false,
        audio: &folia_core::AudioFrame::SILENT,
    };
    let bounds = Rectangle { x: 0.0, y: 0.0, width: WIDTH as f32, height: HEIGHT as f32 };
    Program::draw(&lumiere::field_view::<()>(scene), state, mouse::Cursor::Unavailable, bounds)
}

/// Plays `song()` at 30 frames a second through a sprite pipeline of its own and reports the atlas.
#[test]
#[cfg(all(feature = "stress", not(feature = "ci")))]
#[ignore = "explicit release GPU stress check"]
fn atlas_over_a_whole_song() {
    folia_ui::install_fonts();
    let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::default());
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default())).unwrap();
    let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default())).unwrap();
    let mut glyphs =
        folia_render::glyphs::Glyphs::new(&device, wgpu::TextureFormat::Rgba8Unorm, 4096, lumiere::sprite_images());
    let lyrics = song();
    let end = lyrics.lines.last().unwrap().end + 8.0;
    let state = lumiere::LiveState::default();
    let (mut most_sprites, mut most_echo, mut frames) = (0, 0, 0);
    let mut side_at = Vec::new();
    let mut time = 0.0;
    while time < end {
        let pass = stage(&lyrics, time, "lumiere", &state);
        let sprites = pass.glyphs.lock();
        most_sprites = most_sprites.max(sprites.len());
        most_echo = most_echo.max(pass.echo);
        glyphs.prepare(&device, &queue, &sprites, [WIDTH as f32, HEIGHT as f32], 1.0);
        drop(sprites);
        let stats = glyphs.stats();
        if side_at.last().is_none_or(|(_, size)| *size != stats.size) {
            side_at.push((time, stats.size));
        }
        frames += 1;
        time += 1.0 / 30.0;
        if frames % 30 == 0 {
            device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
        }
    }
    let stats = glyphs.stats();
    eprintln!(
        "atlas over {end:.1}s at 1920x1080 x1: peak {:?}, most marks drawn in one frame {} ({} texels); final {:?} holding {}; most sprites {most_sprites}, most echo sprites {most_echo}; size changes {side_at:?}",
        stats.peak_size, stats.peak_live_marks, stats.peak_live_texels, stats.size, stats.marks
    );
    assert!(stats.peak_size[0] <= 4096 && stats.peak_size[1] <= 4096);
    assert!(most_sprites < 4096, "sprite buffer overflow: {most_sprites}");
}

#[test]
#[cfg_attr(feature = "ci", ignore = "needs a hardware GPU")]
fn lyric_frames_keep_their_glyphs_and_line_art() {
    let short = sample();
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../lyrics/fixtures/lanterns.ttml");
    let paragraphs = folia_lyrics::parse(folia_lyrics::Format::Ttml, &std::fs::read_to_string(path).unwrap(), "", "");
    let mut picture = Picture::new();
    for (lyrics, times, check_art) in [(&short, [5.0, 9.0, 16.0], true), (&paragraphs, [12.0, 31.0, 80.0], false)] {
        for time in times {
            let (pixels, _, lines, sprites) =
                picture.render(lyrics, time, &Theme::MIDNIGHT, "lumiere", Values::default());
            assert!(sprites.1 > sprites.0 as usize, "frame {time} has no lyric glyphs");
            if check_art {
                assert_eq!(pixels.len(), (WIDTH * HEIGHT * 4) as usize);
                let (mut low, mut high) = (255u8, 0u8);
                for pixel in pixels.chunks_exact(4) {
                    let channel = pixel[0].max(pixel[1]).max(pixel[2]);
                    low = low.min(channel);
                    high = high.max(channel);
                }
                assert!(high > 24, "frame {time} max channel {high}");
                assert!(high - low > 20, "frame {time} is flat: {low}..{high}");
                assert!(lines > 64, "frame {time} has no line art");
            }
        }
    }
}

struct Picture {
    device: wgpu::Device,
    queue: wgpu::Queue,
    renderer: iced::Renderer,
    format: wgpu::TextureFormat,
    frame: wgpu::Texture,
    frame_view: wgpu::TextureView,
    viewport: Viewport,
    read: wgpu::Buffer,
}

impl Picture {
    fn new() -> Self {
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::default());
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default())).unwrap();
        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default())).unwrap();
        let format = wgpu::TextureFormat::Rgba8Unorm;
        let renderer = iced_wgpu::Renderer::new(
            iced_wgpu::Engine::new(&adapter, device.clone(), queue.clone(), format, None, Shell::headless()),
            iced::Font::DEFAULT,
            iced::Pixels(16.0),
        );
        folia_ui::install_fonts();
        let (frame, frame_view) =
            target(&device, format, wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC);
        let row = (WIDTH * 4).div_ceil(256) * 256;
        let read = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("lumiere-read"),
            size: u64::from(row) * u64::from(HEIGHT),
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        Self {
            device,
            queue,
            renderer,
            format,
            frame,
            frame_view,
            viewport: Viewport::with_physical_size(iced::Size::new(WIDTH, HEIGHT), 1.0),
            read,
        }
    }

    /// The stack `lyric_view` builds: stage pass, front bokeh, then the clear canvas with the frame.
    fn render(
        &mut self,
        lyrics: &Lyrics,
        time: f64,
        theme: &Theme,
        seed: &str,
        tuning: Values,
    ) -> (Vec<u8>, &'static str, usize, (u32, usize)) {
        let bounds = Rectangle { x: 0.0, y: 0.0, width: WIDTH as f32, height: HEIGHT as f32 };
        let scene = || folia_visualizer::Scene {
            style: Style::Lumiere,
            lyrics,
            time,
            theme,
            font_scale: 1.0,
            seed,
            stage: None,
            tuning: tuning.clone(),
            backdrop: false,
            title: String::new(),
            artist: String::new(),
            album: String::new(),
            portrait: None,
            typography: folia_visualizer::Typography::default(),
            preview: false,
            audio: &folia_core::AudioFrame::SILENT,
        };
        let state = lumiere::LiveState::default();
        let field = lumiere::field_view::<()>(scene());
        let pass = Program::draw(&field, &state, mouse::Cursor::Unavailable, bounds);
        let scale = f32::from_le_bytes(pass.bytes[824..828].try_into().unwrap());
        assert!(scale > 0.5, "camera scale {scale}");
        let line_bytes = pass.lines.as_ref().map(|bytes| bytes.len()).unwrap_or(0);
        let sprites = (pass.echo, pass.glyphs.lock().len());
        let kind = pass.kind;
        self.reset();
        ShaderDraw::draw_primitive(&mut self.renderer, bounds, pass);
        present(&mut self.renderer, self.format, Some(iced::Color::BLACK), &self.frame_view, &self.viewport);

        self.reset();
        let front_state = lumiere::LiveState::default();
        let front = lumiere::front_view::<()>(scene());
        let over = Program::draw(&front, &front_state, mouse::Cursor::Unavailable, bounds);
        ShaderDraw::draw_primitive(&mut self.renderer, bounds, over);
        present(&mut self.renderer, self.format, None, &self.frame_view, &self.viewport);

        self.reset();
        let canvas_state = folia_visualizer::SceneState::default();
        let clear = folia_visualizer::Stage::new(scene());
        let geometries = CanvasProgram::<folia_visualizer::styles::monet::Event>::draw(
            &clear,
            &canvas_state,
            &self.renderer,
            &iced::Theme::Dark,
            bounds,
            mouse::Cursor::Unavailable,
        );
        for geometry in geometries {
            self.renderer.draw_geometry(geometry);
        }
        present(&mut self.renderer, self.format, None, &self.frame_view, &self.viewport);

        let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
        let row = (WIDTH * 4).div_ceil(256) * 256;
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: &self.frame,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &self.read,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(row),
                    rows_per_image: Some(HEIGHT),
                },
            },
            wgpu::Extent3d { width: WIDTH, height: HEIGHT, depth_or_array_layers: 1 },
        );
        self.queue.submit([encoder.finish()]);
        let slice = self.read.slice(..);
        let (sender, receiver) = std::sync::mpsc::channel();
        slice.map_async(wgpu::MapMode::Read, move |result| sender.send(result).unwrap());
        self.device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
        receiver.recv().unwrap().unwrap();
        let mapped = slice.get_mapped_range();
        let mut pixels = vec![0_u8; WIDTH as usize * HEIGHT as usize * 4];
        for row_index in 0..HEIGHT as usize {
            let src = row_index * row as usize;
            let dst = row_index * WIDTH as usize * 4;
            pixels[dst..dst + WIDTH as usize * 4].copy_from_slice(&mapped[src..src + WIDTH as usize * 4]);
        }
        drop(mapped);
        self.read.unmap();
        (pixels, kind, line_bytes, sprites)
    }

    fn reset(&mut self) {
        let bounds = Rectangle { x: 0.0, y: 0.0, width: WIDTH as f32, height: HEIGHT as f32 };
        CoreRenderer::reset(&mut self.renderer, bounds);
    }
}

fn present(
    renderer: &mut iced::Renderer,
    format: wgpu::TextureFormat,
    clear: Option<iced::Color>,
    view: &wgpu::TextureView,
    viewport: &Viewport,
) {
    let started = folia_render::gpu_builds().1;
    renderer.present(clear, format, view, viewport);
    if folia_render::gpu_builds().1 != started {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
        while folia_render::gpu_builds().0 > 0 {
            assert!(std::time::Instant::now() < deadline, "GPU initialization timed out");
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        renderer.present(clear, format, view, viewport);
    }
}

fn target(
    device: &wgpu::Device,
    format: wgpu::TextureFormat,
    usage: wgpu::TextureUsages,
) -> (wgpu::Texture, wgpu::TextureView) {
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("lumiere-target"),
        size: wgpu::Extent3d { width: WIDTH, height: HEIGHT, depth_or_array_layers: 1 },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage,
        view_formats: &[],
    });
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    (texture, view)
}
