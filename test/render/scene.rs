// Canvas styles feed the glass blur: the quarter source differs from the theme ground, and the screen keeps the
// same pixels as drawing the canvas alone.

use folia_core::{AudioFrame, Line, Lyrics, Theme, Word};
use folia_render::Plate;
use folia_visualizer::styles::monet::Event as MonetEvent;
use folia_visualizer::{Scene, Style, Values, lyric_view};
use iced::advanced::renderer::Style as RenderStyle;
use iced::widget::{canvas, shader, stack};
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

    fn frame(&mut self, element: Element<'_, Msg>, cache: Cache) -> Cache {
        let mut ui =
            UserInterface::build(element, Size::new(self.width as f32, self.height as f32), cache, &mut self.renderer);
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
