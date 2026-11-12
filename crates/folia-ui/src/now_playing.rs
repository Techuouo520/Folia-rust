// Now playing: the lyric canvas fills the window, with a vignette and the translation under it.

use crate::typeface::text;
use folia_i18n::I18n;
use folia_render::{Atmosphere, Program};
use folia_visualizer::{Background, Ground, Scene, background_view, lyric_view};
use iced::widget::{column, container, shader, stack};
use iced::{Alignment, Color, Element, Font, Length, Size};

use crate::overlay::{self, SubtitleRows};

/// Where the subtitle sits, and whether a glass plate needs the picture behind it.
pub struct Foot {
    pub bottom: f32,
    pub glass: bool,
}

/// `subtitle` is the translation and the next line. `foot.bottom` is how far the block sits above the window edge.
/// The shared background behind styles that show one. `vignette` is the geometric edge, not the atmosphere.
#[derive(Clone, Copy)]
pub struct Backdrop {
    pub mode: Background,
    pub shapes: bool,
    pub vignette: bool,
    pub paused: bool,
}

/// Where the picture is and how strong its vignette and bottom subtitle are.
#[derive(Clone, Copy)]
pub struct Screen<'a> {
    pub window: Size,
    pub vignette: f32,
    pub subtitle_opacity: f32,
    pub subtitle_background: bool,
    pub subtitle_blur: bool,
    pub dpi: f32,
    pub opacity: f32,
    pub subtitle_scale: f32,
    pub subtitle_font: Font,
    pub shared: Backdrop,
    pub cover: &'a [folia_core::Rgb],
    pub latent: folia_visualizer::LatentTuning,
    pub daylight: bool,
    pub sora: folia_visualizer::SoraTuning,
    pub monet: folia_visualizer::monet_background::Tuning,
    pub nomand: folia_visualizer::nomand::Tuning,
    pub picture: Option<&'a iced::widget::image::Handle>,
}

pub fn view<'a, M: From<folia_visualizer::styles::monet::Event> + 'a>(
    scene: Scene<'a>,
    i18n: &'a I18n,
    muted: Color,
    background: Color,
    screen: Screen<'a>,
    subtitle: SubtitleRows<'a>,
    foot: Foot,
) -> Element<'a, M> {
    let has_lyrics = !scene.lyrics.lines.is_empty();
    let time = scene.time as f32;
    let glass = foot.glass;
    let bottom = foot.bottom;
    let window = screen.window;
    let subtitle_opacity = screen.subtitle_opacity;
    let subtitle_scale = screen.subtitle_scale;
    let show = scene.style.shows_shared_background();
    let flat = !matches!(scene.style, folia_visualizer::Style::Nebbia | folia_visualizer::Style::Lumiere);
    let ground = Ground {
        mode: screen.shared.mode,
        seed: scene.seed,
        time: scene.time,
        theme: scene.theme,
        audio: scene.audio,
        shapes: screen.shared.shapes,
        vignette: screen.shared.vignette,
        publish: !scene.preview,
        paused: screen.shared.paused,
        player: !scene.preview,
        cover: screen.cover,
        latent: screen.latent,
        daylight: screen.daylight,
        sora: screen.sora,
        monet: screen.monet,
        picture: screen.picture,
        nomand: screen.nomand,
    };
    let scene = Scene { backdrop: scene.backdrop && !show, ..scene };
    let visualizer = lyric_view(scene);
    let show_glow = subtitle.rows.iter().any(Option::is_some);
    let alpha = subtitle_opacity.clamp(0.0, 1.0);
    let ellipse = subtitle_ellipse(&subtitle, window, bottom, subtitle_scale, screen.subtitle_font);
    let atmosphere = shader(Program::new(Atmosphere {
        time,
        vignette: if show { 0.0 } else { screen.vignette.clamp(0.0, 1.0) },
        grain: 0.04,
        glow_center: ellipse.center,
        glow_radius: ellipse.radius,
        glow_rgb: [background.r, background.g, background.b],
        glow_alpha: if show_glow && screen.subtitle_background { alpha } else { 0.0 },
    }))
    .width(Length::Fill)
    .height(Length::Fill);
    let mut layers = stack![];
    // Styles with their own picture publish it. A flat style that hides the shared background still
    // falls back to the theme colour; the shared layer is captured with the lyrics instead.
    if glass && flat && !show {
        let picture = folia_render::BackdropScene {
            window: [window.width, window.height],
            ground: [background.r, background.g, background.b, 1.0],
            shade: [0.0; 4],
            tilt: [0.0; 3],
            quads: Vec::new(),
            images: Vec::new(),
            stamp: u64::from(background.r.to_bits())
                ^ u64::from(background.g.to_bits()) << 16
                ^ u64::from(window.width.to_bits()),
        };
        layers = layers.push(shader(folia_render::Backdrop::new(picture)).width(Length::Fill).height(Length::Fill));
    }
    if show {
        layers = layers.push(background_view(ground));
    }
    let mut layers = layers.push(visualizer).push(atmosphere);
    if show_glow {
        let block = subtitle_block(subtitle, muted, background, &screen, alpha);
        let subtitle = container(block)
            .width(Length::Fill)
            .height(Length::Fill)
            .align_x(Alignment::Center)
            .align_y(Alignment::End)
            .padding(iced::Padding { top: 0.0, right: 16.0, bottom, left: 16.0 });
        layers = layers.push(subtitle);
    } else if !has_lyrics {
        let empty = container(text(i18n.t("ui.noLyricsAvailable")).color(muted)).center(Length::Fill);
        layers = layers.push(empty);
    }
    if screen.opacity < 1.0 {
        layers =
            layers.push(container(iced::widget::space()).width(Length::Fill).height(Length::Fill).style(move |_| {
                container::Style {
                    background: Some(Color { a: 1.0 - screen.opacity.clamp(0.2, 1.0), ..background }.into()),
                    ..Default::default()
                }
            }));
    }
    layers.into()
}

fn subtitle_block<'a, M: 'a>(
    subtitle: SubtitleRows<'a>,
    muted: Color,
    background: Color,
    screen: &Screen<'_>,
    alpha: f32,
) -> Element<'a, M> {
    let translation_size = overlay::translation_px(screen.window.width, screen.subtitle_scale);
    let gap = if subtitle.fallback { 8.0 } else { overlay::row_gap(translation_size) };
    let mut rows = column![].spacing(gap).align_x(Alignment::Center);
    for (index, row) in subtitle.rows.into_iter().flatten().enumerate() {
        let size = subtitle_size(&subtitle, index, screen.window.width, screen.subtitle_scale);
        let alpha = alpha * if index == 0 || subtitle.fallback { 1.0 } else { overlay::SECONDARY_TRACK_ALPHA };
        let color = Color { a: alpha, ..muted };
        let next = row.source == folia_store::RowSource::Next;
        rows = rows.push(crate::subtitle_text::view(
            row.text,
            crate::subtitle_text::Look {
                font: crate::typeface::adapt(screen.subtitle_font),
                size,
                width: (if next { 672.0_f32 } else { 896.0 }).min(screen.window.width - 32.0),
                dpi: screen.dpi,
                color,
                shadow: Color { a: overlay::SHADOW_ALPHA * alpha, ..background },
                blur: screen.subtitle_blur && next,
                single: next,
            },
        ));
    }
    rows.into()
}

fn subtitle_size(rows: &SubtitleRows<'_>, index: usize, width: f32, scale: f32) -> f32 {
    if rows.fallback {
        overlay::upcoming_px(width, scale)
    } else {
        overlay::translation_px(width, scale) * if index == 0 { 1.0 } else { overlay::SECONDARY_TRACK_SIZE }
    }
}

fn subtitle_ellipse(
    rows: &SubtitleRows<'_>,
    window: Size,
    bottom: f32,
    scale: f32,
    font: Font,
) -> overlay::GlowEllipse {
    let mut measured = Size::ZERO;
    for (index, row) in rows.rows.iter().flatten().enumerate() {
        let size = subtitle_size(rows, index, window.width, scale);
        let line =
            crate::typeface::measure(row.text, size, crate::typeface::adapt(font), 896.0_f32.min(window.width - 32.0));
        measured.width = measured.width.max(line.width);
        measured.height += line.height;
        if index > 0 {
            measured.height +=
                if rows.fallback { 8.0 } else { overlay::row_gap(overlay::translation_px(window.width, scale)) };
        }
    }
    overlay::GlowEllipse {
        center: [0.5, (window.height - bottom - (measured.height + 4.0) * 0.5) / window.height.max(1.0)],
        radius: [
            (measured.width + 92.0) * 1.15 * 0.5 / window.width.max(1.0),
            (measured.height + 52.0) * 1.3 * 0.5 / window.height.max(1.0),
        ],
    }
}
