// Canvas styles feed the glass blur: the quarter source differs from the theme ground, and the screen keeps the
// same pixels as drawing the canvas alone.

use folia_core::{AudioFrame, Line, Lyrics, Theme, Word};
use folia_render::Plate;
use folia_visualizer::styles::monet::Event as MonetEvent;
use folia_visualizer::{Scene, Style, Values, lyric_view};
use iced::advanced::renderer::Style as RenderStyle;
use iced::widget::{canvas, container, row, shader, stack, text};
use iced::{Color, Element, Font, Length, Pixels, Size, Theme as IcedTheme, mouse};
use iced_runtime::user_interface::{Cache, UserInterface};

const W: u32 = 900;
const H: u32 = 600;

#[derive(Debug, Clone)]
enum Msg {
    Monet,
    Timeline,
}

impl From<MonetEvent> for Msg {
    fn from(_: MonetEvent) -> Self {
        Self::Monet
    }
}

fn lyrics() -> Lyrics {
    let text = "Weave that prosaic divine poem";
    let count = text.chars().count() as f64;
    let words = text
        .chars()
        .enumerate()
        .map(|(index, ch)| Word {
            text: ch.to_string(),
            start: 1.0 + index as f64 / count * 4.0,
            end: 1.0 + (index as f64 + 1.0) / count * 4.0,
        })
        .collect();
    Lyrics {
        lines: vec![
            Line { words, start: 1.0, end: 5.0, text: text.to_owned(), ..Line::default() },
            Line {
                words: Vec::new(),
                start: 6.0,
                end: 9.0,
                text: "And leads you to reality".to_owned(),
                ..Line::default()
            },
        ],
        title: Some("Scene".into()),
        artist: Some("Test".into()),
        ..Lyrics::default()
    }
}

fn scene<'a>(lyrics: &'a Lyrics, theme: &'a Theme, style: Style) -> Scene<'a> {
    Scene {
        style,
        lyrics,
        time: 3.0,
        theme,
        font_scale: 1.0,
        seed: "scene",
        stage: None,
        tuning: Values::default(),
        backdrop: true,
        title: "Scene".into(),
        artist: "Test".into(),
        album: String::new(),
        portrait: None,
        preview: false,
        audio: &AudioFrame::SILENT,
    }
}

fn glass<'a>() -> Element<'a, Msg> {
    shader(Plate {
        radius: 0.0,
        fill_origin: [0.0, 0.0],
        fill_size: [0.0, 0.0],
        fill: [0.0, 0.0, 0.0, 0.2],
        shadow: [0.0; 4],
        shadow_blur: 0.0,
        shadow_y: 0.0,
        shadow_spread: 0.0,
        ring: [0.0; 4],
        backdrop: 12.0,
        fade: 1.0,
        saturate: 1.0,
    })
    .width(Length::Fill)
    .height(Length::Fill)
    .into()
}

struct Picture {
    width: u32,
    height: u32,
    logical: (f32, f32),
    device: wgpu::Device,
    queue: wgpu::Queue,
    renderer: iced::Renderer,
    format: wgpu::TextureFormat,
    target: wgpu::Texture,
    view: wgpu::TextureView,
    viewport: iced::widget::shader::Viewport,
    read: wgpu::Buffer,
}

impl Picture {
    fn new() -> Self {
        Self::with_size(W, H)
    }

    fn with_size(width: u32, height: u32) -> Self {
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::default());
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default())).unwrap();
        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default())).unwrap();
        let format = wgpu::TextureFormat::Rgba8Unorm;
        let renderer = iced_wgpu::Renderer::new(
            iced_wgpu::Engine::new(
                &adapter,
                device.clone(),
                queue.clone(),
                format,
                None,
                iced_wgpu::graphics::Shell::headless(),
            ),
            Font::DEFAULT,
            Pixels(16.0),
        );
        Self::from_renderer(width, height, device, queue, format, renderer)
    }

    /// Two windows on one engine, so a present in `other` trims pipelines owned by `main`.
    fn shared(main: (u32, u32), other: (u32, u32)) -> (Self, Self) {
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::default());
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default())).unwrap();
        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default())).unwrap();
        let format = wgpu::TextureFormat::Rgba8Unorm;
        let engine = iced_wgpu::Engine::new(
            &adapter,
            device.clone(),
            queue.clone(),
            format,
            None,
            iced_wgpu::graphics::Shell::headless(),
        );
        let main = Self::from_renderer(
            main.0,
            main.1,
            device.clone(),
            queue.clone(),
            format,
            iced_wgpu::Renderer::new(engine.clone(), Font::DEFAULT, Pixels(16.0)),
        );
        let other = Self::from_renderer(
            other.0,
            other.1,
            device,
            queue,
            format,
            iced_wgpu::Renderer::new(engine, Font::DEFAULT, Pixels(16.0)),
        );
        (main, other)
    }

    fn from_renderer(
        width: u32,
        height: u32,
        device: wgpu::Device,
        queue: wgpu::Queue,
        format: wgpu::TextureFormat,
        renderer: iced::Renderer,
    ) -> Self {
        let target = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("scene-target"),
            size: wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let view = target.create_view(&wgpu::TextureViewDescriptor::default());
        let row = (width * 4).div_ceil(256) * 256;
        let read = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("scene-read"),
            size: u64::from(row) * u64::from(height),
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        Self {
            width,
            height,
            logical: (width as f32, height as f32),
            device,
            queue,
            renderer,
            format,
            target,
            view,
            viewport: iced::widget::shader::Viewport::with_physical_size(Size::new(width, height), 1.0),
            read,
        }
    }

    /// CSS `css_w`×`css_h` at `scale` device pixels per CSS pixel. The framebuffer is the physical size.
    fn at_scale(css_w: u32, css_h: u32, scale: f32) -> Self {
        let width = (css_w as f32 * scale).round().max(1.0) as u32;
        let height = (css_h as f32 * scale).round().max(1.0) as u32;
        let mut picture = Self::with_size(width, height);
        picture.logical = (css_w as f32, css_h as f32);
        picture.viewport = iced::widget::shader::Viewport::with_physical_size(Size::new(width, height), scale);
        picture
    }

    fn frame(&mut self, element: Element<'_, Msg>, cache: Cache) -> Cache {
        let mut ui =
            UserInterface::build(element, Size::new(self.logical.0, self.logical.1), cache, &mut self.renderer);
        ui.draw(
            &mut self.renderer,
            &IcedTheme::Dark,
            &RenderStyle { text_color: Color::BLACK },
            mouse::Cursor::Unavailable,
        );
        self.renderer.present(Some(Color::BLACK), self.format, &self.view, &self.viewport);
        ui.into_cache()
    }

    fn frame_timed(&mut self, element: Element<'_, Msg>, cache: Cache) -> (Cache, f64) {
        let started = std::time::Instant::now();
        let cache = self.frame(element, cache);
        self.device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
        (cache, started.elapsed().as_secs_f64() * 1000.0)
    }

    fn screenshot(&self) -> Vec<u8> {
        let width = self.width;
        let height = self.height;
        let row = (width * 4).div_ceil(256) * 256;
        let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: &self.target,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &self.read,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(row),
                    rows_per_image: Some(height),
                },
            },
            wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
        );
        self.queue.submit([encoder.finish()]);
        let slice = self.read.slice(..);
        let (sender, receiver) = std::sync::mpsc::channel();
        slice.map_async(wgpu::MapMode::Read, move |result| sender.send(result).unwrap());
        self.device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
        receiver.recv().unwrap().unwrap();
        let mapped = slice.get_mapped_range();
        let mut pixels = vec![0_u8; width as usize * height as usize * 4];
        for row_index in 0..height as usize {
            let src = row_index * row as usize;
            let dst = row_index * width as usize * 4;
            pixels[dst..dst + width as usize * 4].copy_from_slice(&mapped[src..src + width as usize * 4]);
        }
        drop(mapped);
        self.read.unmap();
        pixels
    }
}

fn range(rgba: &[u8], width: u32, top: u32, bottom: u32) -> f32 {
    let mut low = 255.0_f32;
    let mut high = 0.0_f32;
    for y in top..bottom {
        for x in 0..width {
            let index = ((y * width + x) * 4) as usize;
            let luma = 0.2126 * f32::from(rgba[index])
                + 0.7152 * f32::from(rgba[index + 1])
                + 0.0722 * f32::from(rgba[index + 2]);
            low = low.min(luma);
            high = high.max(luma);
        }
    }
    high - low
}

fn png(rgba: &[u8], width: u32, height: u32, name: &str) {
    let dir = std::env::temp_dir().join("folia-gpu-lyrics");
    std::fs::create_dir_all(&dir).unwrap();
    let image = image::RgbaImage::from_raw(width, height, rgba.to_vec()).unwrap();
    image.save(dir.join(name)).unwrap();
}

/// The backdrop stage is shared by the process; tests with their own devices take turns.
static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn guarded() -> std::sync::MutexGuard<'static, ()> {
    let guard = LOCK.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    folia_render::release_backdrop();
    guard
}

#[test]
#[cfg_attr(feature = "ci", ignore = "requires hardware GPU")]
fn loading_capsule_keeps_a_visible_rotating_lucide_glyph() {
    use std::time::{Duration, Instant};
    let _guard = guarded();
    folia_ui::install_fonts();
    let temp = tempfile::tempdir().unwrap();
    let omni = folia_omni::Omni::new(temp.path().into(), "unused".into()).unwrap();
    let mut playback = folia_ui::Playback::new(Default::default(), Some(omni));
    let track = folia_core::Track {
        id: -1,
        title: "Loading fixture".into(),
        duration: 120.0,
        online: Some(folia_core::OnlineId { provider: "qq".into(), id: "fixture".into() }),
        ..Default::default()
    };
    let _ = playback.play(vec![track], 0, None);
    let start = Instant::now();
    let mut bar = folia_ui::PlayerBar::new(start);
    bar.set_home(true);
    bar.sync(&playback, W as f32, start);
    let now = start + Duration::from_secs(2);
    bar.sync(&playback, W as f32, now);
    let i18n = folia_i18n::I18n::new(folia_core::Language::En).unwrap();
    let look = || folia_ui::PlayerBarLook {
        i18n: &i18n,
        primary: Color::WHITE,
        secondary: Color::WHITE,
        background: Color::BLACK,
        daylight: false,
        window: Size::new(W as f32, H as f32),
        bottom: 24.0,
        modal: false,
        controls_disabled: false,
    };
    let mut picture = Picture::new();
    picture.frame(bar.view(&playback, look(), now).map(|_| Msg::Monet), Cache::new());
    let first = picture.screenshot();
    picture.frame(bar.view(&playback, look(), now + Duration::from_millis(250)).map(|_| Msg::Monet), Cache::new());
    let second = picture.screenshot();
    let changed: Vec<_> = first
        .chunks_exact(4)
        .zip(second.chunks_exact(4))
        .enumerate()
        .filter(|(_, (a, b))| a.iter().zip(b.iter()).any(|(a, b)| a.abs_diff(*b) > 8))
        .map(|(index, _)| (index as u32 % W, index as u32 / W))
        .collect();
    assert!(changed.len() > 10, "loading must show an animated glyph even on the home page");
    let xmin = changed.iter().map(|p| p.0).min().unwrap();
    let xmax = changed.iter().map(|p| p.0).max().unwrap();
    let ymin = changed.iter().map(|p| p.1).min().unwrap();
    let ymax = changed.iter().map(|p| p.1).max().unwrap();
    assert!(xmax - xmin <= 25 && ymax - ymin <= 25, "only the 20px loader rotates: {xmin},{ymin}..{xmax},{ymax}");
}

#[test]
#[cfg_attr(feature = "ci", ignore = "needs a GPU; CI runners only have a software renderer")]
fn latent_background_enters_the_glass_source() {
    let _guard = guarded();
    folia_ui::install_fonts();
    let lyrics = lyrics();
    let theme = Theme::MIDNIGHT;
    let mut picture = Picture::new();
    let mut cache = Cache::default();
    let mut lyrics_scene = scene(&lyrics, &theme, Style::Classic);
    lyrics_scene.backdrop = false;
    for _ in 0..2 {
        let ground = folia_visualizer::Ground {
            mode: folia_visualizer::Background::Latent,
            seed: "geometry",
            time: 0.0,
            theme: &theme,
            audio: &AudioFrame::SILENT,
            shapes: true,
            vignette: false,
            publish: true,
            paused: false,
            player: true,
            cover: &[],
            latent: folia_visualizer::LatentTuning::DEFAULT,
        };
        let element: Element<'_, Msg> =
            stack![folia_visualizer::background_view(ground), lyric_view::<Msg>(lyrics_scene.clone()), glass()].into();
        cache = picture.frame(element, cache);
    }
    let source = folia_render::read_picture(&picture.device, &picture.queue).expect("latent blur source");
    let contrast = range(&source.rgba, source.width, 0, source.height);
    assert!(contrast > 20.0, "latent background did not reach the glass source: {contrast}");
}

#[test]
fn classic_enters_the_glass_source() {
    let _guard = guarded();
    folia_ui::install_fonts();
    let lyrics = lyrics();
    let theme = Theme::MIDNIGHT;
    let mut picture = Picture::new();
    let mut cache = Cache::default();
    for _ in 0..2 {
        let element: Element<'_, Msg> =
            stack![lyric_view::<Msg>(scene(&lyrics, &theme, Style::Classic)), glass()].into();
        cache = picture.frame(element, cache);
    }
    let source = folia_render::read_picture(&picture.device, &picture.queue).expect("blur source");
    let contrast = range(&source.rgba, source.width, source.height / 3, source.height * 2 / 3);
    eprintln!("classic source {}x{} contrast {contrast:.1}", source.width, source.height);
    assert!(contrast > 30.0, "the blur source is nearly flat: {contrast}");
    let shot = picture.screenshot();
    png(&shot, W, H, "classic-glass.png");
}

#[test]
#[cfg_attr(feature = "ci", ignore = "needs a GPU; CI runners only have a software renderer")]
fn every_canvas_style_enters_the_glass_source() {
    let _guard = guarded();
    folia_ui::install_fonts();
    let lyrics = lyrics();
    let theme = Theme::MIDNIGHT;
    for style in [
        Style::Classic,
        Style::Partita,
        Style::Still,
        Style::Tilt,
        Style::Claddagh,
        Style::Cadenza,
        Style::Monet,
        Style::Nebbia,
    ] {
        folia_render::release_backdrop();
        let mut picture = Picture::new();
        let mut cache = Cache::default();
        for _ in 0..2 {
            let element: Element<'_, Msg> = stack![lyric_view::<Msg>(scene(&lyrics, &theme, style)), glass()].into();
            cache = picture.frame(element, cache);
        }
        let source = folia_render::read_picture(&picture.device, &picture.queue).expect("blur source");
        let contrast = range(&source.rgba, source.width, source.height / 3, source.height * 2 / 3);
        eprintln!("{style:?} source {}x{} contrast {contrast:.1}", source.width, source.height);
        assert!(contrast > 20.0, "{style:?} blur source is nearly flat: {contrast}");
        let shot = picture.screenshot();
        png(&shot, W, H, &format!("{style:?}-glass.png"));
    }
}

#[test]
#[cfg_attr(feature = "ci", ignore = "needs a GPU; CI runners only have a software renderer")]
fn timeline_card_blurs_the_lyrics_behind_it() {
    let _guard = guarded();
    folia_ui::install_fonts();
    let lyrics = lyrics();
    let theme = Theme::MIDNIGHT;
    let i18n = folia_i18n::I18n::new(folia_core::Language::En).unwrap();
    let window = Size::new(W as f32, H as f32);
    let now = std::time::Instant::now();
    let mut timeline = folia_ui::Timeline::default();
    timeline.open(None, Some(&lyrics), 10.0, 3.0, window, now);
    let later = now + std::time::Duration::from_millis(400);
    let mut picture = Picture::new();
    let mut cache = Cache::default();
    for _ in 0..2 {
        let card =
            timeline.view(&i18n, &theme, false, window, Some(&lyrics), later).expect("timeline").map(|_| Msg::Timeline);
        let element: Element<'_, Msg> = stack![lyric_view::<Msg>(scene(&lyrics, &theme, Style::Classic)), card].into();
        cache = picture.frame(element, cache);
    }
    let source = folia_render::read_picture(&picture.device, &picture.queue).expect("blur source");
    let contrast = range(&source.rgba, source.width, source.height / 3, source.height * 2 / 3);
    eprintln!("timeline source {}x{} contrast {contrast:.1}", source.width, source.height);
    assert!(contrast > 30.0, "the card has no blurred lyrics behind it: {contrast}");
    let shot = picture.screenshot();
    let card_contrast = range(&shot, W, H / 4, H * 3 / 4);
    assert!(card_contrast > 20.0, "the card region is flat: {card_contrast}");
    png(&shot, W, H, "timeline-card.png");
}

#[test]
#[ignore]
fn scene_frame_time() {
    let _guard = guarded();
    folia_ui::install_fonts();
    let lyrics = lyrics();
    let theme = Theme::MIDNIGHT;
    for style in [Style::Classic, Style::Monet] {
        for name in ["canvas", "old-glass", "glass-closed", "glass-open"] {
            folia_render::release_backdrop();
            let mut picture = Picture::with_size(1920, 1080);
            let mut cache = Cache::default();
            let mut times = Vec::new();
            for frame in 0..70 {
                let scene = scene(&lyrics, &theme, style);
                let element: Element<'_, Msg> = match name {
                    "canvas" => {
                        canvas(folia_visualizer::Stage::new(scene)).width(Length::Fill).height(Length::Fill).into()
                    }
                    "old-glass" => {
                        let ground = folia_render::BackdropScene {
                            window: [1920.0, 1080.0],
                            ground: [
                                f32::from(theme.background.r) / 255.0,
                                f32::from(theme.background.g) / 255.0,
                                f32::from(theme.background.b) / 255.0,
                                1.0,
                            ],
                            shade: [0.0; 4],
                            tilt: [0.0; 3],
                            quads: Vec::new(),
                            images: Vec::new(),
                            stamp: 7,
                        };
                        stack![
                            shader(folia_render::Backdrop::new(ground)).width(Length::Fill).height(Length::Fill),
                            canvas(folia_visualizer::Stage::new(scene)).width(Length::Fill).height(Length::Fill),
                            glass()
                        ]
                        .into()
                    }
                    "glass-open" => stack![lyric_view::<Msg>(scene), glass()].into(),
                    _ => lyric_view::<Msg>(scene),
                };
                let (next, ms) = picture.frame_timed(element, cache);
                cache = next;
                if frame >= 10 {
                    times.push(ms);
                }
            }
            times.sort_by(|a, b| a.partial_cmp(b).unwrap());
            let avg = times.iter().sum::<f64>() / times.len() as f64;
            let max = times.last().copied().unwrap_or(0.0);
            let p50 = times[times.len() / 2];
            let source = folia_render::read_picture(&picture.device, &picture.queue)
                .map(|shot| (shot.width, shot.height, shot.bytes));
            eprintln!("{style:?} {name}: avg {avg:.3} ms p50 {p50:.3} ms max {max:.3} ms source {source:?}");
        }
    }
}

#[test]
#[cfg_attr(feature = "ci", ignore = "needs a GPU; CI runners only have a software renderer")]
fn lyric_styles_render_small_cjk_and_mixed_text() {
    let _guard = guarded();
    folia_ui::install_fonts();
    let lines = ["虽然我不想轻易逃跑", "还是怕分别", "心跳 Heartbeat 134 雾霞鬱"]
        .into_iter()
        .enumerate()
        .map(|(line, text)| {
            let start = line as f64 * 6.0;
            let step = 5.5 / text.chars().count() as f64;
            Line {
                text: text.into(),
                start,
                end: start + 5.5,
                words: text
                    .chars()
                    .enumerate()
                    .map(|(i, c)| Word {
                        text: c.to_string(),
                        start: start + i as f64 * step,
                        end: start + (i + 1) as f64 * step,
                    })
                    .collect(),
                ..Line::default()
            }
        })
        .collect();
    let lyrics = Lyrics { lines, word_timed: true, ..Lyrics::default() };
    let temporary = tempfile::tempdir().unwrap();
    let dir = std::env::var_os("FOLIA_VISUAL_AUDIT")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| temporary.path().to_owned());
    // Font rasterization and stroke data finish before the first visual audit frame.
    folia_visualizer::styles::nebbia::frame(
        &lyrics,
        3.0,
        Size::new(1920.0, 1080.0),
        &Theme::MIDNIGHT,
        1.0,
        &Values::default(),
        0.0,
    );
    let start = std::time::Instant::now();
    while !folia_visualizer::styles::nebbia::glyphs_settled(&lyrics) {
        assert!(start.elapsed().as_secs() < 60);
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    for (w, h) in [(1200, 800), (1920, 1080)] {
        for (theme_name, theme) in [("dark", Theme::MIDNIGHT), ("light", Theme::DAYLIGHT)] {
            let mut contact = image::RgbaImage::new(1920, 1080);
            let mut mixed_contact = image::RgbaImage::new(1920, 1080);
            for (index, style) in Style::ALL.into_iter().enumerate() {
                folia_render::release_backdrop();
                let mut picture = Picture::with_size(w, h);
                let mut cache = Cache::default();
                for (sample, time, scale) in [("small", 3.0, 0.75), ("mixed", 15.0, 1.0)] {
                    for _ in 0..6 {
                        let scene = Scene { time, font_scale: scale, ..scene(&lyrics, &theme, style) };
                        cache = picture.frame(lyric_view::<Msg>(scene), cache);
                    }
                    let pixels = picture.screenshot();
                    let bright = pixels.chunks_exact(4).filter(|p| p[0].max(p[1]).max(p[2]) > 30).count();
                    assert!(bright > 100, "{} {sample} is blank", style.id());
                    let shot = image::RgbaImage::from_raw(w, h, pixels).unwrap();
                    let thumb = image::imageops::resize(&shot, 640, 360, image::imageops::FilterType::Triangle);
                    let sheet = if sample == "small" { &mut contact } else { &mut mixed_contact };
                    image::imageops::overlay(sheet, &thumb, (index % 3 * 640) as i64, (index / 3 * 360) as i64);
                    shot.save(dir.join(format!("{theme_name}-{w}-{}-{sample}.png", style.id()))).unwrap();
                }
            }
            contact.save(dir.join(format!("contact-{theme_name}-{w}.png"))).unwrap();
            mixed_contact.save(dir.join(format!("contact-mixed-{theme_name}-{w}.png"))).unwrap();
        }
    }
    folia_render::release_backdrop();
}

#[test]
#[cfg_attr(feature = "ci", ignore = "needs a GPU; CI runners only have a software renderer")]
fn claddagh_rotated_axis_stays_a_thin_line() {
    let _guard = guarded();
    folia_ui::install_fonts();
    folia_render::release_backdrop();
    let lyrics = lyrics();
    let theme = Theme::MIDNIGHT;
    let mut picture = Picture::new();
    let mut cache = Cache::default();
    let mut shots = Vec::new();
    for enabled in [false, true] {
        let mut scene = scene(&lyrics, &theme, Style::Claddagh);
        scene.tuning.set_flag(Style::Claddagh.manifest(), "showAxisLine", enabled);
        for _ in 0..2 {
            cache = picture.frame(lyric_view::<Msg>(scene.clone()), cache);
        }
        shots.push(picture.screenshot());
    }
    let changed = shots[0].chunks_exact(4).zip(shots[1].chunks_exact(4)).filter(|(a, b)| a != b).count();
    assert!((300..3000).contains(&changed), "rotated 300x4 axis covers {changed} pixels");
    folia_render::release_backdrop();
}

#[test]
#[cfg_attr(feature = "ci", ignore = "needs a GPU; CI runners only have a software renderer")]
fn screen_pixels_match_the_plain_canvas() {
    let _guard = guarded();
    folia_ui::install_fonts();
    let lyrics = lyrics();
    let theme = Theme::MIDNIGHT;
    for style in
        [Style::Classic, Style::Partita, Style::Still, Style::Tilt, Style::Claddagh, Style::Cadenza, Style::Monet]
    {
        folia_render::release_backdrop();
        let mut picture = Picture::new();
        let mut cache = Cache::default();
        for _ in 0..2 {
            let element: Element<'_, Msg> = lyric_view::<Msg>(scene(&lyrics, &theme, style));
            cache = picture.frame(element, cache);
        }
        let with_scene = picture.screenshot();

        folia_render::release_backdrop();
        let mut picture = Picture::new();
        let mut cache = Cache::default();
        for _ in 0..2 {
            let plain: Element<'_, Msg> = canvas(folia_visualizer::Playing(scene(&lyrics, &theme, style)))
                .width(Length::Fill)
                .height(Length::Fill)
                .into();
            cache = picture.frame(plain, cache);
        }
        let plain = picture.screenshot();
        let differing = with_scene.chunks(4).zip(plain.chunks(4)).filter(|(a, b)| a != b).count();
        eprintln!("{style:?} screen differing pixels {differing}");
        assert_eq!(differing, 0, "{style:?} screen changed");
    }

    // Still used its own canvas cache before this change; a cached picture must equal the live one.
    folia_render::release_backdrop();
    let pictures = folia_visualizer::Pictures::default();
    let mut picture = Picture::new();
    let mut cache = Cache::default();
    for _ in 0..2 {
        let frozen = pictures.frozen(scene(&lyrics, &theme, Style::Still));
        let element: Element<'_, Msg> = canvas(frozen).width(Length::Fill).height(Length::Fill).into();
        cache = picture.frame(element, cache);
    }
    let frozen = picture.screenshot();
    let mut picture = Picture::new();
    let mut cache = Cache::default();
    for _ in 0..2 {
        let element: Element<'_, Msg> = lyric_view::<Msg>(scene(&lyrics, &theme, Style::Still));
        cache = picture.frame(element, cache);
    }
    let live = picture.screenshot();
    let differing = frozen.chunks(4).zip(live.chunks(4)).filter(|(a, b)| a != b).count();
    eprintln!("Still cached vs live differing pixels {differing}");
    assert_eq!(differing, 0);
}

/// Two shader scenes in one iced frame. Prepare runs for every primitive before any of them renders, so a pipeline
/// that keeps one set of textures shows the last scene in both rectangles.
#[test]
#[cfg_attr(feature = "ci", ignore = "needs a GPU; CI runners only have a software renderer")]
fn two_shader_scenes_in_one_frame_keep_their_own_pixels() {
    let _guard = guarded();
    folia_ui::install_fonts();
    let lyrics = lyrics();
    folia_visualizer::styles::nebbia::frame(
        &lyrics,
        6.0,
        Size::new(640.0, 360.0),
        &Theme::MIDNIGHT,
        1.0,
        &Values::default(),
        0.0,
    );
    let started = std::time::Instant::now();
    while !folia_visualizer::styles::nebbia::glyphs_settled(&lyrics) {
        assert!(started.elapsed().as_secs() < 30, "nebbia strokes did not settle");
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    let midnight = Theme::MIDNIGHT;
    let day = Theme::DAYLIGHT;
    for style in [Style::Nebbia, Style::Lumiere] {
        folia_render::release_backdrop();
        let own_night = solo(&lyrics, &midnight, style, 6.0, 640, 360);
        let own_day_same = solo(&lyrics, &day, style, 15.0, 640, 360);
        let own_day = solo(&lyrics, &day, style, 15.0, 400, 360);
        let apart = diff_count(&own_night, &own_day_same);
        assert!(apart > 8_000, "{style:?} controls are too alike to prove isolation: {apart}");

        let forward = ScenePair {
            left: SceneSpec { theme: &midnight, time: 6.0, width: 640.0 },
            right: SceneSpec { theme: &day, time: 15.0, width: 400.0 },
        };
        let backward = ScenePair {
            left: SceneSpec { theme: &day, time: 15.0, width: 400.0 },
            right: SceneSpec { theme: &midnight, time: 6.0, width: 640.0 },
        };
        expect_isolated(&lyrics, style, forward, &own_night, &own_day, 0, 640, "forward");
        expect_isolated(&lyrics, style, backward, &own_day, &own_night, 0, 400, "reversed");
    }
    folia_render::release_backdrop();
}

#[derive(Clone, Copy)]
struct SceneSpec<'a> {
    theme: &'a Theme,
    time: f64,
    width: f32,
}

#[derive(Clone, Copy)]
struct ScenePair<'a> {
    left: SceneSpec<'a>,
    right: SceneSpec<'a>,
}

fn expect_isolated(
    lyrics: &Lyrics,
    style: Style,
    sides: ScenePair<'_>,
    own_left: &[u8],
    own_right: &[u8],
    split: u32,
    left_width: u32,
    label: &str,
) {
    folia_render::release_backdrop();
    let mut picture = Picture::with_size(1040, 360);
    let _ = picture.frame(pair(lyrics, style, sides), Cache::default());
    let shot = picture.screenshot();
    let left = region(&shot, 1040, 0, 0, left_width, 360);
    let right = region(&shot, 1040, split + left_width, 0, 1040 - split - left_width, 360);
    let left_own = diff_count(&left, own_left);
    let right_own = diff_count(&right, own_right);
    eprintln!("{style:?} {label} left own {left_own} right own {right_own}");
    assert_eq!(left_own, 0, "{style:?} {label} left rectangle took on the other scene");
    assert_eq!(right_own, 0, "{style:?} {label} right rectangle took on the other scene");
}

fn solo(lyrics: &Lyrics, theme: &Theme, style: Style, time: f64, width: u32, height: u32) -> Vec<u8> {
    folia_render::release_backdrop();
    let mut picture = Picture::with_size(width, height);
    let scene = Scene { style, time, theme, ..scene(lyrics, theme, style) };
    let cache = picture.frame(lyric_view::<Msg>(scene), Cache::default());
    let _ = cache;
    picture.screenshot()
}

fn pair<'a>(lyrics: &'a Lyrics, style: Style, sides: ScenePair<'a>) -> Element<'a, Msg> {
    let left =
        Scene { style, time: sides.left.time, theme: sides.left.theme, ..scene(lyrics, sides.left.theme, style) };
    let mut right =
        Scene { style, time: sides.right.time, theme: sides.right.theme, ..scene(lyrics, sides.right.theme, style) };
    right.preview = true;
    row![
        container(lyric_view(left)).width(sides.left.width).height(360.0),
        container(lyric_view(right)).width(sides.right.width).height(360.0),
    ]
    .spacing(0)
    .into()
}

fn region(pixels: &[u8], width: u32, x: u32, y: u32, w: u32, h: u32) -> Vec<u8> {
    let mut out = Vec::with_capacity((w * h * 4) as usize);
    for row_index in y..y + h {
        let start = ((row_index * width + x) * 4) as usize;
        out.extend_from_slice(&pixels[start..start + (w * 4) as usize]);
    }
    out
}

fn diff_count(left: &[u8], right: &[u8]) -> usize {
    let count = left.len().min(right.len()) / 4;
    left.chunks_exact(4).zip(right.chunks_exact(4)).take(count).filter(|(a, b)| a != b).count()
}

struct PinnedClock;

impl PinnedClock {
    fn pin() -> Self {
        Self::at(0);
        Self
    }

    fn at(step: u64) {
        folia_visualizer::clock::set_test_clock_ms(Some(1_000_000 + step * 16));
    }
}

impl Drop for PinnedClock {
    fn drop(&mut self) {
        folia_visualizer::clock::set_test_clock_ms(None);
    }
}

fn blank_cell<'a>(width: f32) -> Element<'a, Msg> {
    container(text("")).width(width).height(360.0).into()
}

fn styled<'a>(lyrics: &'a Lyrics, theme: &'a Theme, style: Style, time: f64, preview: bool) -> Scene<'a> {
    Scene { style, time, theme, preview, ..scene(lyrics, theme, style) }
}

fn duo<'a>(left: Element<'a, Msg>, right: Element<'a, Msg>) -> Element<'a, Msg> {
    row![left, right].spacing(0).into()
}

fn lyric_cell<'a>(
    lyrics: &'a Lyrics,
    theme: &'a Theme,
    style: Style,
    time: f64,
    width: f32,
    preview: bool,
) -> Element<'a, Msg> {
    container(lyric_view::<Msg>(styled(lyrics, theme, style, time, preview))).width(width).height(360.0).into()
}

fn draw_at(picture: &mut Picture, cache: Cache, step: u64, element: Element<'_, Msg>) -> (Cache, Vec<u8>) {
    PinnedClock::at(step);
    let cache = picture.frame(element, cache);
    (cache, picture.screenshot())
}

fn solo_for(lyrics: &Lyrics, theme: &Theme, style: Style, time: f64, width: u32, frames: u64) -> Vec<u8> {
    let mut picture = Picture::with_size(width, 360);
    let mut cache = Cache::default();
    let mut shot = Vec::new();
    for step in 0..frames {
        let element = lyric_view::<Msg>(styled(lyrics, theme, style, time, false));
        let drawn = draw_at(&mut picture, cache, step, element);
        cache = drawn.0;
        shot = drawn.1;
    }
    shot
}

fn slots(style: Style) -> (usize, usize) {
    match style {
        Style::Nebbia => (folia_visualizer::styles::nebbia::live_slots(), 0),
        Style::Lumiere => {
            (folia_visualizer::styles::lumiere::stage_slots(), folia_visualizer::styles::lumiere::front_slots())
        }
        _ => (0, 0),
    }
}

/// Close one side, keep the other for several frames, restore it, then remove both and create them again.
#[test]
#[cfg_attr(feature = "ci", ignore = "needs a GPU; CI runners only have a software renderer")]
fn shader_scenes_close_and_reopen_on_one_renderer() {
    let _guard = guarded();
    let _clock = PinnedClock::pin();
    folia_ui::install_fonts();
    let lyrics = lyrics();
    folia_visualizer::styles::nebbia::frame(
        &lyrics,
        6.0,
        Size::new(640.0, 360.0),
        &Theme::MIDNIGHT,
        1.0,
        &Values::default(),
        0.0,
    );
    let started = std::time::Instant::now();
    while !folia_visualizer::styles::nebbia::glyphs_settled(&lyrics) {
        assert!(started.elapsed().as_secs() < 30, "nebbia strokes did not settle");
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    let midnight = Theme::MIDNIGHT;
    let day = Theme::DAYLIGHT;
    for style in [Style::Nebbia, Style::Lumiere] {
        let fresh_left = solo_for(&lyrics, &midnight, style, 6.0, 640, 1);
        let lived_left = solo_for(&lyrics, &midnight, style, 6.0, 640, 6);
        let fresh_right = solo_for(&lyrics, &day, style, 15.0, 400, 1);
        let lived_right = solo_for(&lyrics, &day, style, 15.0, 400, 6);
        assert_eq!(slots(style), (0, 0), "{style:?} oracle renderers leaked slots");

        let both = || {
            duo(
                lyric_cell(&lyrics, &midnight, style, 6.0, 640.0, false),
                lyric_cell(&lyrics, &day, style, 15.0, 400.0, true),
            )
        };
        {
            let mut picture = Picture::with_size(1040, 360);
            let mut cache = Cache::default();
            let drawn = draw_at(&mut picture, cache, 0, both());
            cache = drawn.0;
            assert_eq!(slots(style).0, 2, "{style:?} did not keep both scenes");
            for step in 1..=4 {
                let drawn = draw_at(
                    &mut picture,
                    cache,
                    step,
                    duo(blank_cell(640.0), lyric_cell(&lyrics, &day, style, 15.0, 400.0, true)),
                );
                cache = drawn.0;
            }
            assert_eq!(slots(style).0, 1, "{style:?} kept the closed left scene");
            let drawn = draw_at(&mut picture, cache, 5, both());
            let shot = drawn.1;
            assert_eq!(region(&shot, 1040, 0, 0, 640, 360), fresh_left, "{style:?} restored left");
            assert_eq!(region(&shot, 1040, 640, 0, 400, 360), lived_right, "{style:?} surviving right");
            assert_eq!(slots(style).0, 2, "{style:?} restored left did not allocate a slot");
        }
        assert_eq!(slots(style), (0, 0), "{style:?} first picture leaked slots");

        {
            let mut picture = Picture::with_size(1040, 360);
            let mut cache = Cache::default();
            let drawn = draw_at(&mut picture, cache, 0, both());
            cache = drawn.0;
            for step in 1..=4 {
                let drawn = draw_at(
                    &mut picture,
                    cache,
                    step,
                    duo(lyric_cell(&lyrics, &midnight, style, 6.0, 640.0, false), blank_cell(400.0)),
                );
                cache = drawn.0;
            }
            assert_eq!(slots(style).0, 1, "{style:?} kept the closed right scene");
            let drawn = draw_at(&mut picture, cache, 5, both());
            let shot = drawn.1;
            assert_eq!(region(&shot, 1040, 0, 0, 640, 360), lived_left, "{style:?} surviving left");
            assert_eq!(region(&shot, 1040, 640, 0, 400, 360), fresh_right, "{style:?} restored right");
        }

        {
            let mut picture = Picture::with_size(1040, 360);
            let mut cache = Cache::default();
            let drawn = draw_at(&mut picture, cache, 0, both());
            cache = drawn.0;
            let drawn = draw_at(&mut picture, cache, 1, duo(blank_cell(640.0), blank_cell(400.0)));
            cache = drawn.0;
            assert_eq!(slots(style), (0, 0), "{style:?} scenes stayed after both were removed");
            let drawn = draw_at(&mut picture, cache, 2, both());
            let shot = drawn.1;
            assert_eq!(region(&shot, 1040, 0, 0, 640, 360), fresh_left, "{style:?} recreated left");
            assert_eq!(region(&shot, 1040, 640, 0, 400, 360), fresh_right, "{style:?} recreated right");
            assert_eq!(slots(style).0, 2, "{style:?} recreated scenes");
        }
    }
}

/// Another window presenting the shared engine must not drop a living scene's textures.
#[test]
#[cfg_attr(feature = "ci", ignore = "needs a GPU; CI runners only have a software renderer")]
fn living_lumiere_keeps_textures_while_another_window_presents() {
    let _guard = guarded();
    let _clock = PinnedClock::pin();
    folia_ui::install_fonts();
    let lyrics = lyrics();
    let midnight = Theme::MIDNIGHT;
    let (mut main, mut other) = Picture::shared((640, 360), (64, 64));
    let element = lyric_view::<Msg>(styled(&lyrics, &midnight, Style::Lumiere, 6.0, false));
    let (cache, before) = draw_at(&mut main, Cache::default(), 0, element);
    let slots = folia_visualizer::styles::lumiere::stage_slots();
    let textures = folia_visualizer::styles::lumiere::stage_textures();
    let fronts = folia_visualizer::styles::lumiere::front_slots();
    assert_eq!(slots, 1);
    assert_eq!(fronts, 1);
    assert_eq!(textures, 1, "lumiere did not allocate its pyramid");
    for _ in 0..150 {
        let _ = other.frame(blank_cell(64.0), Cache::default());
    }
    assert_eq!(folia_visualizer::styles::lumiere::stage_slots(), slots, "foreign presents dropped the slot");
    assert_eq!(folia_visualizer::styles::lumiere::front_slots(), fronts);
    assert_eq!(
        folia_visualizer::styles::lumiere::stage_textures(),
        textures,
        "foreign presents released bloom while the owner was alive"
    );
    let element = lyric_view::<Msg>(styled(&lyrics, &midnight, Style::Lumiere, 6.0, false));
    let (cache, after) = draw_at(&mut main, cache, 0, element);
    assert_eq!(after, before, "lumiere changed after foreign presents");
    let _ = main.frame(blank_cell(640.0), cache);
    assert_eq!(folia_visualizer::styles::lumiere::stage_slots(), 0);
    assert_eq!(folia_visualizer::styles::lumiere::front_slots(), 0);
    assert_eq!(folia_visualizer::styles::lumiere::stage_textures(), 0);
}

#[test]
#[cfg_attr(feature = "ci", ignore = "needs a GPU; CI runners only have a software renderer")]
fn preview_does_not_replace_the_playing_blur_source() {
    let _guard = guarded();
    let _clock = PinnedClock::pin();
    folia_ui::install_fonts();
    let lyrics = lyrics();
    folia_visualizer::styles::nebbia::frame(
        &lyrics,
        6.0,
        Size::new(640.0, 360.0),
        &Theme::MIDNIGHT,
        1.0,
        &Values::default(),
        0.0,
    );
    let started = std::time::Instant::now();
    while !folia_visualizer::styles::nebbia::glyphs_settled(&lyrics) {
        assert!(started.elapsed().as_secs() < 30, "nebbia strokes did not settle");
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    let midnight = Theme::MIDNIGHT;
    let day = Theme::DAYLIGHT;
    for style in [Style::Nebbia, Style::Lumiere] {
        let play = blur_source(
            640,
            360,
            stack![lyric_view::<Msg>(styled(&lyrics, &midnight, style, 6.0, false)), glass()].into(),
        );
        let other =
            blur_source(400, 360, stack![lyric_view::<Msg>(styled(&lyrics, &day, style, 15.0, false)), glass()].into());
        assert_ne!(play.2, other.2, "{style:?} blur controls are identical");
        let both = blur_source(
            1040,
            360,
            stack![
                duo(
                    lyric_cell(&lyrics, &midnight, style, 6.0, 640.0, false),
                    lyric_cell(&lyrics, &day, style, 15.0, 400.0, true),
                ),
                glass(),
            ]
            .into(),
        );
        assert_eq!(both.0, play.0, "{style:?} blur width");
        assert_eq!(both.1, play.1, "{style:?} blur height");
        assert_eq!(both.2, play.2, "{style:?} preview replaced the playing blur source");
    }
}

fn common_ground(shapes: bool, vignette: bool) -> Element<'static, Msg> {
    background_ground(folia_visualizer::Background::Common, shapes, vignette)
}

fn latent_ground() -> Element<'static, Msg> {
    tuned_ground(folia_visualizer::LatentTuning::DEFAULT, true)
}

fn tuned_ground(latent: folia_visualizer::LatentTuning, player: bool) -> Element<'static, Msg> {
    let theme = Theme::MIDNIGHT;
    let audio = folia_core::AudioFrame::SILENT;
    let theme = Box::leak(Box::new(theme));
    let audio = Box::leak(Box::new(audio));
    folia_visualizer::background_view(folia_visualizer::Ground {
        mode: folia_visualizer::Background::Latent,
        seed: "geometry",
        time: 0.0,
        theme,
        audio,
        shapes: true,
        vignette: false,
        publish: false,
        paused: false,
        player,
        cover: &[],
        latent,
    })
}

fn background_ground(mode: folia_visualizer::Background, shapes: bool, vignette: bool) -> Element<'static, Msg> {
    let theme = Theme::MIDNIGHT;
    let audio = folia_core::AudioFrame::SILENT;
    // Leak the frame so the view can borrow it for this picture. The test process is short.
    let theme = Box::leak(Box::new(theme));
    let audio = Box::leak(Box::new(audio));
    folia_visualizer::background_view(folia_visualizer::Ground {
        mode,
        seed: "geometry",
        time: 0.0,
        theme,
        audio,
        shapes,
        vignette,
        publish: false,
        paused: false,
        player: true,
        cover: &[],
        latent: folia_visualizer::LatentTuning::DEFAULT,
    })
}

#[test]
fn latent_tuning_round_trips_the_stored_table() {
    let mut tuning = folia_visualizer::LatentTuning::DEFAULT;
    tuning.mesh_speed = 1.2;
    tuning.enhanced = false;
    tuning.layers = folia_visualizer::LatentLayers::Mesh;
    tuning.colors = folia_visualizer::LatentColors::CoverOnly;
    tuning.dynamic_only = false;
    tuning.overlay_on = false;
    let stored = toml::Value::Table(tuning.table());
    let read = folia_visualizer::LatentTuning::read(Some(&stored));
    assert!((read.mesh_speed - 1.2).abs() < 1.0e-4);
    assert!(!read.enhanced);
    assert_eq!(read.layers, folia_visualizer::LatentLayers::Mesh);
    assert_eq!(read.colors, folia_visualizer::LatentColors::CoverOnly);
    assert!(!read.dynamic_only);
    assert!(!read.overlay_on);
    assert!((read.dither_size - folia_visualizer::LatentTuning::DEFAULT.dither_size).abs() < 1.0e-4);
}

#[test]
fn cover_palette_keeps_the_large_region() {
    let mut pixels = vec![0_u8; 8 * 8 * 4];
    for pixel in pixels.chunks_exact_mut(4) {
        pixel.copy_from_slice(&[20, 40, 200, 255]);
    }
    for pixel in pixels.chunks_exact_mut(4).take(4) {
        pixel.copy_from_slice(&[240, 20, 20, 255]);
    }
    let colors = folia_visualizer::cover_colors(&pixels, 2);
    assert_eq!(colors[0], folia_core::Rgb { r: 20, g: 40, b: 200 });
}

#[test]
fn latent_cover_only_keeps_the_cover_inside_the_mesh() {
    let theme = Theme::MIDNIGHT;
    let cover = [folia_core::Rgb { r: 240, g: 20, b: 20 }, folia_core::Rgb { r: 20, g: 40, b: 200 }];
    let mixed = folia_visualizer::resolve_colors(&theme, &cover, folia_visualizer::LatentColors::CoverTheme);
    let only = folia_visualizer::resolve_colors(&theme, &cover, folia_visualizer::LatentColors::CoverOnly);
    let back =
        |rgb: folia_core::Rgb| [f32::from(rgb.r) / 255.0, f32::from(rgb.g) / 255.0, f32::from(rgb.b) / 255.0, 1.0];
    assert_eq!(mixed.dither_back, back(theme.background));
    assert_eq!(only.dither_back, back(cover[1]));
    assert_eq!(only.mesh[0], back(cover[0]));
    assert_eq!(only.mesh[5], back(cover[1]));
    let empty = folia_visualizer::resolve_colors(&theme, &[], folia_visualizer::LatentColors::CoverOnly);
    assert_eq!(empty.dither_back, back(theme.secondary));
}

#[test]
fn cover_split_keeps_red_when_the_channel_ranges_tie() {
    let pixels = [0_u8, 100, 200, 255, 200, 0, 0, 255];
    let colors = folia_visualizer::cover_colors(&pixels, 2);
    assert_eq!(colors[0], folia_core::Rgb { r: 0, g: 100, b: 200 });
}

#[test]
fn cover_palette_resamples_to_fifty_pixels_and_drops_stale_entries() {
    let mut pixels = vec![0_u8; 100 * 100 * 4];
    for (index, pixel) in pixels.chunks_exact_mut(4).enumerate() {
        if index % 100 < 50 {
            pixel.copy_from_slice(&[220, 20, 20, 255]);
        } else {
            pixel.copy_from_slice(&[20, 40, 200, 255]);
        }
    }
    let sampled = folia_visualizer::resample_cover(&pixels, 100, 100);
    assert_eq!(sampled.len(), usize::try_from(folia_visualizer::SAMPLE_SIDE).unwrap().pow(2) * 4);
    assert!(sampled[0] > 180 && sampled[2] < 80, "left sample {:?}", &sampled[..4]);
    let right = (folia_visualizer::SAMPLE_SIDE as usize - 1) * 4;
    assert!(sampled[right] < 80 && sampled[right + 2] > 150, "right sample {:?}", &sampled[right..right + 4]);
    assert!(folia_visualizer::resample_cover(&pixels, 0, 100).is_empty());
    assert!(folia_visualizer::cover_palette(&[], 10, 10, 4).is_empty());
    let palette = folia_visualizer::cover_palette(&pixels, 100, 100, 2);
    assert_eq!(palette.len(), 2);

    let mut store = folia_visualizer::PaletteStore::new();
    for id in 0..i64::from(u32::try_from(folia_visualizer::PALETTE_CAP).unwrap()) + 1 {
        store.insert(id, vec![folia_core::Rgb { r: 1, g: 2, b: 3 }], Some(0));
    }
    assert_eq!(store.len(), folia_visualizer::PALETTE_CAP);
    assert!(store.get(0).is_some(), "the playing song was evicted");
    assert!(store.get(1).is_none(), "an older song stayed past the cap");
    store.remove(7);
    assert!(store.get(7).is_none());
    assert!(!folia_visualizer::accept_art(Some((4, 2)), 4, 3));
    assert!(folia_visualizer::accept_art(Some((4, 2)), 4, 2));
    assert!(!folia_visualizer::accept_art(None, 4, 2));
    assert!(store.get(99).is_none(), "a song with no cover has no palette");
}

#[test]
fn shape_scale_follows_the_audio_spring() {
    let mut current = 0.95;
    let mut velocity = 0.0;
    let target = 1.45;
    for step in 0..180 {
        (current, velocity) = folia_visualizer::follow_spring(current, velocity, target, 1.0 / 60.0);
        assert!(current.is_finite() && velocity.is_finite());
        if step == 1 {
            assert!(current < 1.2, "sprang too far on the second step: {current}");
        }
    }
    assert!((current - target).abs() < 0.02, "spring did not settle: {current}");

    let (jumped, jumped_v) = folia_visualizer::follow_spring(0.95, 0.0, target, 0.1);
    assert!(jumped.is_finite() && jumped_v.is_finite());
    assert!((0.95..1.7).contains(&jumped), "100ms step diverged: {jumped}");
    let mut recovered = jumped;
    let mut recovered_v = jumped_v;
    for _ in 0..180 {
        (recovered, recovered_v) = folia_visualizer::follow_spring(recovered, recovered_v, target, 1.0 / 60.0);
    }
    assert!((recovered - target).abs() < 0.02, "did not recover after a dropped frame: {recovered}");

    let mut stepped = 0.95;
    let mut stepped_v = 0.0;
    for _ in 0..6 {
        (stepped, stepped_v) = folia_visualizer::follow_spring(stepped, stepped_v, target, 0.016);
    }
    let (once, once_v) = folia_visualizer::follow_spring(0.95, 0.0, target, 0.096);
    assert!((stepped - once).abs() < 1.0e-3, "short steps {stepped} left the 96ms step {once}");
    assert!((stepped_v - once_v).abs() < 1.0e-2, "velocity {stepped_v} left {once_v}");
}

#[test]
#[cfg_attr(feature = "ci", ignore = "needs a GPU; CI runners only have a software renderer")]
fn common_geometry_moves_under_an_empty_classic_frame() {
    let _guard = guarded();
    let _clock = PinnedClock::pin();
    let mut picture = Picture::with_size(640, 360);
    let cache = picture.frame(common_ground(true, true), Cache::default());
    let early = picture.screenshot();
    PinnedClock::at(1_250);
    let _ = picture.frame(common_ground(true, true), cache);
    let delta = diff_count(&early, &picture.screenshot());
    assert!(delta > 2_000, "common geometry did not move: {delta}");
}

#[test]
#[cfg_attr(feature = "ci", ignore = "needs a GPU; CI runners only have a software renderer")]
fn geometric_switches_change_the_picture() {
    let _guard = guarded();
    let _clock = PinnedClock::pin();
    let mut shapes = Picture::with_size(640, 360);
    let cache = shapes.frame(common_ground(false, false), Cache::default());
    let bare = shapes.screenshot();
    PinnedClock::at(1_250);
    let _ = shapes.frame(common_ground(false, false), cache);
    assert_eq!(diff_count(&bare, &shapes.screenshot()), 0, "hidden shapes still moved");
    let mut vignette = Picture::with_size(640, 360);
    let _ = vignette.frame(common_ground(false, true), Cache::default());
    let edge = diff_count(&bare, &vignette.screenshot());
    assert!(edge > 1_000, "vignette did not darken the frame: {edge}");
    let center = |pixels: &[u8]| {
        let x = 320u32;
        let y = 180u32;
        let index = ((y * 640 + x) * 4) as usize;
        [pixels[index], pixels[index + 1], pixels[index + 2]]
    };
    assert_eq!(center(&bare), center(&vignette.screenshot()), "vignette reached the middle");
}

#[test]
fn latent_shader_budget_matches_the_paper_cap() {
    let (ratio, width, height) = folia_visualizer::shader_budget(200.0, 120.0, 1.0);
    assert!((ratio - 1.0).abs() < 1.0e-3);
    assert_eq!((width, height), (200.0, 120.0));
    let (ratio, width, height) = folia_visualizer::shader_budget(1920.0, 1080.0, 2.0);
    assert!((width - 1280.0).abs() < 1.0, "width {width}");
    assert!((height - 720.0).abs() < 1.0, "height {height}");
    assert!((ratio - width / 1920.0).abs() < 1.0e-3);
}

#[test]
#[cfg_attr(feature = "ci", ignore = "needs a GPU; CI runners only have a software renderer")]
fn latent_mesh_moves() {
    let _guard = guarded();
    let _clock = PinnedClock::pin();
    let mut picture = Picture::with_size(400, 120);
    let mut cache = Cache::default();
    let mut first = Vec::new();
    let mut middle = Vec::new();
    for step in 0..120 {
        PinnedClock::at(step);
        cache = picture.frame(latent_pair(), cache);
        if step == 0 {
            first = picture.screenshot();
        } else if step == 60 {
            middle = picture.screenshot();
        }
    }
    let last = picture.screenshot();
    assert!(!first.chunks_exact(4).all(|pixel| pixel == &first[..4]), "latent frame is one flat colour");
    let left = region(&last, 400, 0, 0, 200, 120);
    let right = region(&last, 400, 200, 0, 200, 120);
    assert_eq!(diff_count(&left, &right), 0, "same-size latent scenes diverged by position");
    assert!(diff_count(&first, &middle) > 200, "latent did not move across the first second");
    assert!(diff_count(&middle, &last) > 200, "latent stopped after the first second");
    assert_eq!(folia_visualizer::budget_texture_size(), (200, 120));
}

#[test]
#[cfg_attr(feature = "ci", ignore = "needs a GPU; CI runners only have a software renderer")]
fn latent_preview_stays_still_and_layers_change_the_picture() {
    let _guard = guarded();
    let _clock = PinnedClock::pin();
    let mut still = Picture::with_size(200, 120);
    let mut cache = Cache::default();
    cache = still.frame(tuned_ground(folia_visualizer::LatentTuning::DEFAULT, false), cache);
    let first = still.screenshot();
    PinnedClock::at(60);
    let _ = still.frame(tuned_ground(folia_visualizer::LatentTuning::DEFAULT, false), cache);
    assert_eq!(diff_count(&first, &still.screenshot()), 0, "dynamic-only preview moved");

    let mut mesh = Picture::with_size(200, 120);
    let mut dither = Picture::with_size(200, 120);
    let mut mesh_tuning = folia_visualizer::LatentTuning::DEFAULT;
    mesh_tuning.layers = folia_visualizer::LatentLayers::Mesh;
    mesh_tuning.dynamic_only = false;
    let mut dither_tuning = mesh_tuning;
    dither_tuning.layers = folia_visualizer::LatentLayers::Dither;
    PinnedClock::at(0);
    let _ = mesh.frame(tuned_ground(mesh_tuning, true), Cache::default());
    let _ = dither.frame(tuned_ground(dither_tuning, true), Cache::default());
    assert!(diff_count(&mesh.screenshot(), &dither.screenshot()) > 200, "mesh and pixel layers look the same");
}

#[test]
#[cfg_attr(feature = "ci", ignore = "needs a GPU; CI runners only have a software renderer")]
fn latent_budget_texture_is_the_capped_size() {
    let _guard = guarded();
    let _clock = PinnedClock::pin();
    let mut full = Picture::with_size(1920, 1080);
    let _ = full.frame(latent_ground(), Cache::default());
    assert_eq!(folia_visualizer::budget_texture_size(), (1280, 720));
    assert_eq!(full.screenshot().len(), 1920 * 1080 * 4);
    assert!(full.screenshot().chunks_exact(4).any(|pixel| pixel != [0, 0, 0, 255]), "budget blit is blank");

    let mut sharp = Picture::at_scale(800, 450, 1.0);
    let _ = sharp.frame(latent_ground(), Cache::default());
    assert_eq!(folia_visualizer::budget_texture_size(), (800, 450));
    let mut retina = Picture::at_scale(800, 450, 2.0);
    let _ = retina.frame(latent_ground(), Cache::default());
    assert_eq!(folia_visualizer::budget_texture_size(), (1280, 720), "DPI still shaded every physical pixel");
    assert_eq!(retina.screenshot().len(), 1600 * 900 * 4);
    assert!(retina.screenshot().chunks_exact(4).any(|pixel| pixel != [0, 0, 0, 255]), "retina blit is blank");
}

fn latent_pair() -> Element<'static, Msg> {
    row![container(latent_ground()).width(200.0).height(120.0), container(latent_ground()).width(200.0).height(120.0),]
        .spacing(0)
        .into()
}

fn blur_source(width: u32, height: u32, element: Element<'_, Msg>) -> (u32, u32, Vec<u8>) {
    folia_render::release_backdrop();
    PinnedClock::at(0);
    let mut picture = Picture::with_size(width, height);
    let _ = picture.frame(element, Cache::default());
    let shot = folia_render::read_picture(&picture.device, &picture.queue).expect("glass blur source");
    (shot.width, shot.height, shot.rgba)
}
