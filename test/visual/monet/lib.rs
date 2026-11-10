#![cfg(test)]
// Offscreen Monet at 1920×1032 in the daylight theme: poster boxes against the original CSS, glyph coverage of the
// Chinese text, and PNGs of three moments of the same song as the reference screenshot.

use std::time::Instant;

use folia_core::{Line, Lyrics, Theme, Word};
use folia_visualizer::styles::monet::{self, Event as MonetEvent};
use folia_visualizer::{Scene, SceneState, Style, Values};
use iced::widget::canvas::Program as CanvasProgram;
use iced::{Rectangle, Size, Theme as IcedTheme, mouse};
use iced_wgpu::core::Renderer as CoreRenderer;
use iced_wgpu::graphics::geometry::Renderer as _;
use iced_wgpu::graphics::{Shell, Viewport};

const WIDTH: u32 = 1920;
const HEIGHT: u32 = 1032;
const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures");

fn timed(text: &str, start: f64, end: f64, translation: Option<&str>) -> Line {
    let count = text.chars().count().max(1) as f64;
    let step = (end - start) / count;
    let words = text
        .chars()
        .enumerate()
        .map(|(index, ch)| Word {
            text: ch.to_string(),
            start: start + index as f64 * step,
            end: start + (index as f64 + 1.0) * step,
        })
        .collect();
    Line { words, start, end, text: text.to_owned(), translation: translation.map(str::to_owned), ..Line::default() }
}

/// 《我好像在哪见过你》 as the platform serves it: a "title - artist" line, the intro dots, then the verse.
fn song() -> Lyrics {
    let head = Line {
        words: vec![
            Word { text: "我好像在哪见过你".into(), start: 0.0, end: 1.0 },
            Word { text: " - ".into(), start: 1.0, end: 1.2 },
            Word { text: "薛之谦".into(), start: 1.2, end: 4.0 },
        ],
        start: 0.0,
        end: 4.0,
        text: "我好像在哪见过你 - 薛之谦".into(),
        ..Line::default()
    };
    let lines = vec![
        head,
        Line { start: 4.0, end: 5.4, text: folia_core::INTERLUDE_TEXT.into(), ..Line::default() },
        timed("和你有关观后无感", 5.4, 8.6, Some("Nothing about you moves me after the show")),
        timed("若是真的敢问作者何来罪恶", 8.6, 11.6, None),
        timed("我好像在哪见过你", 11.6, 14.8, Some("I seem to have seen you somewhere")),
        timed("你明明说不出", 14.8, 17.2, None),
        timed("我们之间没有谁会难过", 17.2, 20.4, None),
    ];
    Lyrics {
        lines,
        title: Some("我好像在哪见过你".into()),
        artist: Some("薛之谦".into()),
        word_timed: true,
        ..Lyrics::default()
    }
}

/// The cover cut out of the reference screenshot, standing in for the decoded album art.
fn cover() -> iced::widget::image::Handle {
    let shot = image::open(format!("{FIXTURES}/user-original.webp")).unwrap().to_rgba8();
    let art = image::imageops::crop_imm(&shot, 1200, 250, 530, 530).to_image();
    iced::widget::image::Handle::from_rgba(art.width(), art.height(), art.into_raw())
}

fn scene<'a>(
    lyrics: &'a Lyrics,
    theme: &'a Theme,
    time: f64,
    portrait: Option<iced::widget::image::Handle>,
) -> Scene<'a> {
    scene_audio(lyrics, theme, time, portrait, &folia_core::AudioFrame::SILENT)
}

fn scene_audio<'a>(
    lyrics: &'a Lyrics,
    theme: &'a Theme,
    time: f64,
    portrait: Option<iced::widget::image::Handle>,
    audio: &'a folia_core::AudioFrame,
) -> Scene<'a> {
    Scene {
        style: Style::Monet,
        lyrics,
        time,
        theme,
        font_scale: 1.0,
        seed: "monet",
        stage: None,
        tuning: Values::default(),
        backdrop: true,
        title: lyrics.title.clone().unwrap_or_default(),
        artist: lyrics.artist.clone().unwrap_or_default(),
        album: "初学者".into(),
        portrait,
        preview: false,
        audio,
    }
}

#[test]
#[cfg_attr(feature = "ci", ignore = "needs a GPU; CI runners only have a software renderer")]
fn audio_power_changes_the_poster() {
    let lyrics = song();
    let theme = Theme::MIDNIGHT;
    let loud = folia_core::AudioFrame { power: 200.0, has_spectrum: true, ..folia_core::AudioFrame::SILENT };
    let mut picture = Picture::with_size(1200, 800);
    let state = SceneState::default();
    let quiet = scene_audio(&lyrics, &theme, 6.0, None, &folia_core::AudioFrame::SILENT);
    let driven = scene_audio(&lyrics, &theme, 6.0, None, &loud);
    let (quiet_png, _) = picture.render(&quiet, &state);
    let (loud_png, _) = picture.render(&driven, &state);
    let dir = std::env::temp_dir().join("folia-bands");
    std::fs::create_dir_all(&dir).unwrap();
    let quiet_path = dir.join("monet-power0.png");
    let loud_path = dir.join("monet-power200.png");
    std::fs::write(&quiet_path, &quiet_png).unwrap();
    std::fs::write(&loud_path, &loud_png).unwrap();
    assert_ne!(quiet_png, loud_png, "power 200 did not change the monet frame");
    let _ = std::fs::remove_file(&quiet_path);
    let _ = std::fs::remove_file(&loud_path);
}

struct Css {
    heading: Rectangle,
    rail: Rectangle,
    portrait: Rectangle,
    capsule_top: f32,
    rule_top: f32,
}

/// `VisualizerMonet.tsx` worked by hand at 1920×1032: default tuning (fontScale 1.2, square, no offset, description),
/// Tailwind 4 (`2xl:px-20`, `lg:py-8`, `xl:pr-12`, `space-y-*` as margin-block-end), a one-row title.
fn css() -> Css {
    let (w, h) = (1920.0_f32, 1032.0_f32);
    let large = 1.0 + 0.16 * (w - 1536.0) / (2200.0 - 1536.0);
    let lyric = (2.28 * 16.0_f32).min(w * 0.0275).max(1.34 * 16.0) * 1.2 * large;
    let glow = (lyric * 1.2).round();
    let row = (1520.0 * large).round();
    let row_x = (w - row) / 2.0;
    let column = (w * 0.28).min((430.0 * large).round()).max(220.0);
    let content_x = row_x + 80.0;
    let content = row - column - 160.0;
    let inner_max = (380.0 * large).round();
    let inner = (w * 0.26).min(inner_max).max(210.0);
    let header = (content - (0.35135 * inner - 48.0)).max(192.0);
    let artist_px = (w * 0.018).min(((1.8 * large * 1000.0).round() / 1000.0) * 16.0).max(16.0);
    let title_px = (w * 0.033).min(((2.8 * large * 1000.0).round() / 1000.0) * 16.0).max(23.2);
    let artist = artist_px * 1.5;
    let title = title_px * 1.06 + title_px * (0.25 * 2.0 - 0.25 - 0.12);
    let rail_h = (0.52 * h).min((520.0 * large).max(lyric * 1.18 * 7.0).round()).max(280.0);
    let total = 32.0 + (artist + 6.0 + 56.0 + 12.0) + (title + 20.0 + 24.0) + rail_h + (16.0 + 34.0) + 32.0;
    let top = (h - total) / 2.0 + 32.0;
    let rail_y = top + artist + 6.0 + 56.0 + 12.0 + title + 20.0 + 24.0;
    let column_x = row_x + row - column;
    let wrapper = inner.min(column - 12.0 - 48.0);
    let wrapper_x = column_x + column - 48.0 - wrapper;
    let side = wrapper * 1.35135;
    Css {
        heading: Rectangle { x: content_x, y: top, width: header, height: rail_y - 24.0 - top },
        rail: Rectangle {
            x: content_x - glow,
            y: rail_y,
            width: (780.0 * large).round().min(content + glow * 2.0),
            height: rail_h,
        },
        portrait: Rectangle { x: wrapper_x - wrapper * 0.35135, y: (h - side) / 2.0, width: side, height: side },
        capsule_top: rail_y + rail_h + 16.0,
        rule_top: top + artist + 6.0,
    }
}

fn near(name: &str, got: Rectangle, want: Rectangle) {
    let pairs = [(got.x, want.x), (got.y, want.y), (got.width, want.width), (got.height, want.height)];
    assert!(pairs.iter().all(|(a, b)| (a - b).abs() <= 2.0), "{name}: rust {got:?} css {want:?}");
    eprintln!(
        "{name}: rust ({:.2}, {:.2}, {:.2}, {:.2}) css ({:.2}, {:.2}, {:.2}, {:.2})",
        got.x, got.y, got.width, got.height, want.x, want.y, want.width, want.height
    );
}

#[test]
fn layout_matches_original_css() {
    folia_ui::install_fonts();
    let lyrics = song();
    let theme = Theme::DAYLIGHT;
    let scene = scene(&lyrics, &theme, 0.5, None);
    let layout = monet::layout(&scene, Size::new(WIDTH as f32, HEIGHT as f32), 0.0);
    let want = css();
    near("heading", layout.heading, want.heading);
    near("rail", layout.rail, want.rail);
    near("portrait", layout.portrait.unwrap(), want.portrait);
    let capsule = layout.capsule.unwrap();
    assert!((capsule.y - want.capsule_top).abs() <= 2.0 && (capsule.height - 34.0).abs() < 0.01);
    assert!((layout.rule.y - want.rule_top).abs() <= 2.0);
    assert_eq!(layout.title_rows, 1);
    // Edges read off the reference screenshot (same layout, AI theme): portrait 1188..1742 × 239..793, rule from 164,
    // capsule border 887..921, capsule and dot left edges 210 / 227.
    let portrait = layout.portrait.unwrap();
    for (got, want) in [
        (portrait.x, 1188.0),
        (portrait.x + portrait.width, 1742.0),
        (portrait.y, 239.0),
        (portrait.y + portrait.height, 793.0),
        (layout.rule.y, 164.0),
        (capsule.y, 887.0),
        (capsule.y + capsule.height, 921.0),
        (capsule.x, 210.0),
    ] {
        assert!((got - want).abs() <= 2.0, "screenshot edge {want}, rust {got}");
    }
    let rect =
        monet::layout(&Scene { tuning: rectangular(), ..scene.clone() }, Size::new(WIDTH as f32, HEIGHT as f32), 0.0);
    let frame = rect.portrait.unwrap();
    assert!(
        (frame.width - 410.0).abs() < 0.5
            && (frame.height - 410.0 / 0.74).abs() < 0.5
            && (frame.x - 1332.5).abs() < 0.5
    );
    eprintln!("rectangular portrait ({:.2}, {:.2}, {:.2}, {:.2})", frame.x, frame.y, frame.width, frame.height);
}

fn rectangular() -> Values {
    let mut values = Values::default();
    values.set_choice(monet::manifest(), "portraitStyle", "rectangular");
    values
}

#[test]
fn chinese_text_has_glyphs() {
    folia_ui::install_fonts();
    let italic = iced::Font { style: iced::font::Style::Italic, ..iced::Font::DEFAULT };
    let semibold = iced::Font { weight: iced::font::Weight::Semibold, ..iced::Font::DEFAULT };
    for (text, px, font) in [
        ("薛之谦", 31.472, italic),
        ("我好像在哪见过你", 48.944, semibold),
        ("初学者", 14.0, iced::Font::DEFAULT),
        ("我好像在哪见过你 - 薛之谦", 47.83, semibold),
        ("和你有关观后无感", 47.83, semibold),
        ("若是真的敢问作者何来罪恶", 47.83, semibold),
    ] {
        let shaped = monet::shape(text, px, font);
        assert_eq!(shaped.missing, 0, "{text} has .notdef glyphs");
        assert_eq!(shaped.glyphs.len(), text.chars().count(), "{text} glyph count");
    }
}

#[test]
#[cfg_attr(feature = "ci", ignore = "needs a GPU; CI runners only have a software renderer")]
fn offscreen_frames_write_pngs() {
    let lyrics = song();
    let theme = Theme::DAYLIGHT;
    let dir = std::env::temp_dir().join("folia-monet2");
    std::fs::create_dir_all(&dir).unwrap();
    let mut picture = Picture::new();
    let art = cover();
    let mut times = Vec::new();
    let state = SceneState::default();
    for time in [0.5_f64, 6.0, 12.0] {
        let scene = scene(&lyrics, &theme, time, Some(art.clone()));
        let (png, ms) = picture.render(&scene, &state);
        times.push(ms);
        let path = dir.join(format!("monet-{time:.1}.png"));
        std::fs::write(&path, &png).unwrap();
        let image = image::load_from_memory(&png).unwrap().to_rgba8();
        assert_eq!((image.width(), image.height()), (WIDTH, HEIGHT));
        // The ground is the theme colour, not a painted wash.
        let corner = image.get_pixel(4, 4);
        assert_eq!((corner[0], corner[1], corner[2]), (0xf5, 0xf5, 0xf4), "frame {time} corner");
        // The portrait's corner is cut: just inside the frame's corner the ground shows, at its middle the cover.
        let layout = monet::layout(&scene, Size::new(WIDTH as f32, HEIGHT as f32), 0.0);
        let frame = layout.portrait.unwrap();
        let inside_corner = image.get_pixel(frame.x as u32 + 3, frame.y as u32 + 3);
        let middle = image.get_pixel((frame.x + frame.width / 2.0) as u32, (frame.y + frame.height / 2.0) as u32);
        if time > 2.0 {
            assert!(
                inside_corner[0] > 200 && inside_corner[1] > 200,
                "frame {time} corner not clipped {inside_corner:?}"
            );
            assert!(middle[0] < 200 || middle[2] < 200, "frame {time} cover missing {middle:?}");
        }
        eprintln!("monet t={time} wrote {}", path.display());
    }
    // Same frames again: the measurements are warm, which is what playback sees.
    let mut warm = Vec::new();
    for time in [0.5_f64, 6.0, 12.0] {
        let scene = scene(&lyrics, &theme, time + 0.016, Some(art.clone()));
        warm.push(picture.render(&scene, &state).1);
    }
    eprintln!("monet frame ms cold {times:?} warm {warm:?}");
}

struct Picture {
    width: u32,
    height: u32,
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
        Self::with_size(WIDTH, HEIGHT)
    }

    fn with_size(width: u32, height: u32) -> Self {
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
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("monet-target"),
            size: wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let frame_view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let row = (width * 4).div_ceil(256) * 256;
        let read = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("monet-read"),
            size: u64::from(row) * u64::from(height),
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        Self {
            width,
            height,
            device,
            queue,
            renderer,
            format,
            frame: texture,
            frame_view,
            viewport: Viewport::with_physical_size(iced::Size::new(width, height), 1.0),
            read,
        }
    }

    /// The frame as PNG, and the milliseconds from drawing to pixels read back (encoding excluded).
    fn render(&mut self, scene: &Scene<'_>, state: &SceneState) -> (Vec<u8>, f64) {
        let started = Instant::now();
        let bounds = Rectangle { x: 0.0, y: 0.0, width: self.width as f32, height: self.height as f32 };
        CoreRenderer::reset(&mut self.renderer, bounds);
        let playing = folia_visualizer::Stage::new(scene.clone());
        let geometries = CanvasProgram::<MonetEvent>::draw(
            &playing,
            state,
            &self.renderer,
            &IcedTheme::Light,
            bounds,
            mouse::Cursor::Unavailable,
        );
        for geometry in geometries {
            self.renderer.draw_geometry(geometry);
        }
        self.renderer.present(Some(iced::Color::BLACK), self.format, &self.frame_view, &self.viewport);
        let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
        let row = (self.width * 4).div_ceil(256) * 256;
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
                    rows_per_image: Some(self.height),
                },
            },
            wgpu::Extent3d { width: self.width, height: self.height, depth_or_array_layers: 1 },
        );
        self.queue.submit([encoder.finish()]);
        let slice = self.read.slice(..);
        let (sender, receiver) = std::sync::mpsc::channel();
        slice.map_async(wgpu::MapMode::Read, move |result| sender.send(result).unwrap());
        self.device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
        receiver.recv().unwrap().unwrap();
        let mapped = slice.get_mapped_range();
        let mut pixels = vec![0_u8; self.width as usize * self.height as usize * 4];
        for row_index in 0..self.height as usize {
            let src = row_index * row as usize;
            let dst = row_index * self.width as usize * 4;
            pixels[dst..dst + self.width as usize * 4].copy_from_slice(&mapped[src..src + self.width as usize * 4]);
        }
        drop(mapped);
        self.read.unmap();
        let elapsed = started.elapsed().as_secs_f64() * 1000.0;
        let image = image::RgbaImage::from_raw(self.width, self.height, pixels).unwrap();
        let mut png = Vec::new();
        image.write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png).unwrap();
        (png, elapsed)
    }
}
