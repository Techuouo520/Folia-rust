// Long Monet playback with workbench pictures on one renderer: panel labels drawn beside it must stay pixel-identical.

use folia_core::Theme as FoliaTheme;
use folia_visualizer::styles::monet::Event as MonetEvent;
use folia_visualizer::{Scene, Style, Values};
use iced::advanced::renderer::{Headless, Style as RenderStyle};
use iced::widget::{Canvas, column, container, row, text};
use iced::{Color, Element, Length, Pixels, Size, Theme};
use iced_runtime::user_interface::{Cache, UserInterface};

const W: u32 = 1920;
const H: u32 = 1080;
const PANEL: u32 = 420;

#[derive(Debug, Clone)]
struct Msg;

impl From<MonetEvent> for Msg {
    fn from(_: MonetEvent) -> Self {
        Msg
    }
}

const LABELS: [&str; 12] = [
    "歌词样式",
    "播放器面板 Player panel",
    "防止息屏",
    "页面 Pages",
    "均衡器 Equalizer",
    "单曲循环 Repeat one",
    "秋至的春信",
    "昆仑 / 在与东",
    "主题配色 Theme colors",
    "自动混音 Automix 1.0×",
    "音量 Volume 72%",
    "日间 / 夜间",
];

fn lrc() -> String {
    let mut out = String::new();
    for line in 0..90u32 {
        let start = line * 3;
        let chars: String =
            (0..10 + line % 6).filter_map(|i| char::from_u32(0x4E00 + (line * 37 + i * 11) % 6000)).collect();
        out.push_str(&format!("[{:02}:{:02}.00]{chars} word{line} Lyric\n", start / 60, start % 60));
    }
    out
}

fn panel<'a>() -> Element<'a, Msg, Theme, iced::Renderer> {
    let labels = LABELS.iter().enumerate().map(|(i, label)| text(*label).size(12.0 + (i % 3) as f32).into());
    container(column(labels).spacing(10).padding(16))
        .width(Length::Fixed(PANEL as f32))
        .height(Length::Fill)
        .style(|_| container::background(Color::from_rgb8(0xf4, 0xf4, 0xf5)))
        .into()
}

fn picture<'a>(scene: Scene<'a>, width: f32, height: f32) -> Element<'a, Msg, Theme, iced::Renderer> {
    Canvas::new(folia_visualizer::Stage::new(scene)).width(Length::Fixed(width)).height(Length::Fixed(height)).into()
}

fn layout<'a>(scene: Scene<'a>, workbench: bool) -> Element<'a, Msg, Theme, iced::Renderer> {
    let main_w = (W - PANEL) as f32;
    let stage: Element<'a, Msg, Theme, iced::Renderer> = if workbench {
        let preview = Scene { stage: Some(Size::new(W as f32, H as f32)), preview: true, ..scene.clone() };
        let wobble = (scene.time * 60.0 % 240.0) as f32;
        let thumbs = [400.0 + wobble, 300.0 - wobble * 0.5, 220.0 + wobble * 0.3, 160.0].into_iter().map(|w| {
            picture(Scene { stage: Some(Size::new(W as f32, H as f32)), preview: true, ..scene.clone() }, w, w * 0.5625)
        });
        column![picture(preview, main_w, 700.0), row(thumbs).spacing(8)].into()
    } else {
        picture(scene, main_w, H as f32)
    };
    row![stage, panel()].into()
}

fn panel_pixels(shot: &[u8]) -> Vec<u8> {
    (0..H as usize)
        .flat_map(|y| shot[(y * W as usize + (W - PANEL) as usize) * 4..(y + 1) * W as usize * 4].to_vec())
        .collect()
}

fn save(name: &str, shot: &[u8]) {
    let output = tempfile::tempdir().unwrap();
    let dir = output.path();
    let _ = std::fs::create_dir_all(&dir);
    let _ = image::RgbaImage::from_raw(W, H, shot.to_vec()).map(|img| img.save(dir.join(name)));
}

fn time_at(frame: u32) -> f64 {
    // 60 fps playback with a seek every 15 s, as if the lyrics were scrolled by hand.
    (f64::from(frame) / 60.0 + f64::from(frame / 900) * 37.0) % 270.0
}

fn draw(
    renderer: &mut iced::Renderer,
    cache: Cache,
    lyrics: &folia_core::Lyrics,
    frame: u32,
    workbench: bool,
) -> (Vec<u8>, Cache) {
    let theme = FoliaTheme::DAYLIGHT;
    let audio = folia_core::AudioFrame::SILENT;
    let scene = Scene {
        style: Style::Monet,
        lyrics,
        time: time_at(frame),
        theme: &theme,
        font_scale: 1.0,
        seed: "monet",
        stage: None,
        tuning: Values::default(),
        backdrop: true,
        title: "秋至的春信".into(),
        artist: "昆仑 / 在与东".into(),
        album: String::new(),
        portrait: None,
        preview: false,
        audio: &audio,
    };
    let mut ui = UserInterface::build(layout(scene, workbench), Size::new(W as f32, H as f32), cache, renderer);
    ui.draw(renderer, &Theme::Light, &RenderStyle { text_color: Color::BLACK }, iced::mouse::Cursor::Unavailable);
    let cache = ui.into_cache();
    (
        renderer
            .screenshot(&iced::advanced::graphics::Viewport::with_physical_size(Size::new(W, H), 1.0), Color::WHITE),
        cache,
    )
}

fn renderer() -> iced::Renderer {
    pollster::block_on(<iced::Renderer as Headless>::new(folia_ui::INTERFACE_FONT, Pixels(16.0), None)).unwrap()
}

/// Frames to play, from `FOLIA_ATLAS_FRAMES`. Before the fix the shared atlas filled up at frame 21298.
fn frames() -> u32 {
    std::env::var("FOLIA_ATLAS_FRAMES").ok().and_then(|v| v.parse().ok()).unwrap_or(1200)
}

#[test]
#[cfg_attr(feature = "ci", ignore = "needs a GPU; CI runners only have a software renderer")]
fn panel_text_survives_long_monet_playback() {
    let lyrics = folia_lyrics::parse_file("lrc", &lrc(), "");
    let mut long = renderer();
    let mut cache = Cache::default();
    let mut baseline: Option<Vec<u8>> = None;
    let started = std::time::Instant::now();
    let total = frames();
    for frame in 0..total {
        let workbench = (frame / 300) % 3 == 1;
        let (shot, next) = draw(&mut long, cache, &lyrics, frame, workbench);
        cache = next;
        let pixels = panel_pixels(&shot);
        match &baseline {
            None => baseline = Some(pixels),
            Some(base) if *base != pixels => {
                let differing = base.chunks(4).zip(pixels.chunks(4)).filter(|(a, b)| a != b).count();
                save(&format!("broken-{frame}.png"), &shot);
                panic!("panel text changed at frame {frame}: {differing} pixels differ");
            }
            Some(_) => {}
        }
        // The whole frame, Monet's own text included, must match a renderer that has drawn nothing else.
        if frame % 1500 == 1499 && !workbench {
            let (fresh, _) = draw(&mut renderer(), Cache::default(), &lyrics, frame, false);
            let differing = fresh.chunks(4).zip(shot.chunks(4)).filter(|(a, b)| a != b).count();
            if differing > 0 {
                save(&format!("long-{frame}.png"), &shot);
                save(&format!("fresh-{frame}.png"), &fresh);
                panic!("frame {frame} differs from a fresh renderer: {differing} pixels");
            }
        }
    }
    eprintln!("{total} frames, {:.2} ms/frame", started.elapsed().as_secs_f64() * 1000.0 / f64::from(total));
}
