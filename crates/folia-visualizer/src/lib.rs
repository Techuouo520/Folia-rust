// Lyric animation styles drawn on an iced canvas. A frame is playback time plus the current audio frame.

mod anim;
mod preview;
mod registry;
pub mod styles;
mod tuning;

pub use anim::{Ease, keyframes, spring, tween};
pub use preview::{
    Frozen, POSTER_TIME, PREVIEW_LOOP, Pictures, Playing, RESERVED_LOOP, STAGE_HEIGHT, STAGE_WIDTH, preview_lyrics,
    preview_reserved,
};
pub use registry::Style;
pub use tuning::{Group, Item, Manifest, Values};

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use folia_core::{AudioFrame, Line, Lyrics, Rgb, Theme};
use iced::advanced::text::{self as core_text, Paragraph as _};
use iced::widget::canvas::{self, Frame, Geometry};
use iced::widget::image::Handle;
use iced::widget::{shader, stack};
use iced::{Color, Element, Font, Length, Point, Rectangle, Renderer, Size, mouse};

pub(crate) const BOLD: Font = Font { weight: iced::font::Weight::Bold, ..Font::DEFAULT };

/// Everything a style needs to draw one frame.
#[derive(Clone)]
pub struct Scene<'a> {
    pub style: Style,
    pub lyrics: &'a Lyrics,
    /// Playback position in seconds.
    pub time: f64,
    pub theme: &'a Theme,
    pub font_scale: f32,
    /// Song id. Lumiere casts light positions from this string, matching the original program seed.
    pub seed: &'a str,
    /// Draws as on a stage this large, scaled down to cover the widget; `None` draws at the widget size.
    pub stage: Option<Size>,
    /// The style's saved tuning. An empty value reads as that style's defaults.
    pub tuning: Values,
    /// Fills the theme background before the style. A shader behind the canvas turns this off.
    pub backdrop: bool,
    /// Song title, artist and album. Empty strings fall back to "Monet" inside that style.
    pub title: String,
    pub artist: String,
    pub album: String,
    /// Cover or uploaded portrait, already decoded.
    pub portrait: Option<Handle>,
    /// Style-picker and workbench pictures use Monet's static spectrum. Playback reads `audio`.
    pub preview: bool,
    /// Loudness for this frame. Bands are 0–255, the same scale as the original byte analyser.
    pub audio: &'a AudioFrame,
}

/// Draws between fresh Monet text atlases: one minute at 60 fps.
const MONET_ATLAS_FRAMES: u32 = 3600;

/// Per-widget state: measured line layouts survive between frames.
#[derive(Default)]
pub struct SceneState {
    cadenza: RefCell<styles::cadenza::Cache>,
    claddagh: RefCell<styles::claddagh::Cache>,
    classic: RefCell<LayoutCache<styles::classic::LineLayout>>,
    partita: RefCell<LayoutCache<styles::partita::PartitaLayout>>,
    tilt: RefCell<LayoutCache<styles::tilt::TiltLayout>>,
    lumiere: RefCell<styles::lumiere::Cache>,
    monet: RefCell<styles::monet::Cache>,
    monet_ink: RefCell<canvas::Cache>,
    monet_draws: Cell<u32>,
    still: styles::still::StillCache,
}

/// Monet publishes the dragged portrait offset, so the message type must take one.
impl<Message: From<styles::monet::PortraitOffset>> canvas::Program<Message> for Scene<'_> {
    type State = SceneState;

    fn draw(
        &self,
        state: &SceneState,
        renderer: &Renderer,
        _theme: &iced::Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<Geometry> {
        self.render(state, renderer, bounds.size())
    }

    fn update(
        &self,
        state: &mut Self::State,
        event: &canvas::Event,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Option<canvas::Action<Message>> {
        if self.style != Style::Monet || self.stage.is_some() {
            return None;
        }
        let (size, place) = self.placement(bounds.size());
        let at = cursor.position_in(bounds).map(|point| place.to_stage(point));
        styles::monet::update(self, &mut state.monet.borrow_mut(), event, size, at)
    }

    fn mouse_interaction(&self, state: &SceneState, bounds: Rectangle, cursor: mouse::Cursor) -> mouse::Interaction {
        if self.style != Style::Monet || self.stage.is_some() {
            return mouse::Interaction::None;
        }
        let (size, place) = self.placement(bounds.size());
        let at = cursor.position_in(bounds).map(|point| place.to_stage(point));
        styles::monet::interaction(self, &state.monet.borrow(), size, at)
    }
}

impl Scene<'_> {
    /// The stage this scene lays out on: its own `stage`, or the widget size.
    fn stage_size(&self, target: Size) -> Size {
        match self.stage {
            Some(stage) if stage.width > 0.0 && stage.height > 0.0 => stage,
            _ => target,
        }
    }

    /// The stage size and how it maps onto a widget of `target`: scaled to cover it, centred.
    fn placement(&self, target: Size) -> (Size, Place) {
        let size = self.stage_size(target);
        let scale = (target.width / size.width).max(target.height / size.height);
        let offset =
            iced::Vector::new((target.width - size.width * scale) / 2.0, (target.height - size.height * scale) / 2.0);
        (size, Place { offset, scale })
    }

    /// Monet's springing text sizes churn glyphs. Its own cache keeps them in a text atlas apart from the UI's, and a
    /// fresh cache every `MONET_ATLAS_FRAMES` draws drops that atlas before it fragments up to the size limit.
    fn monet_geometry(&self, state: &SceneState, renderer: &Renderer, target: Size) -> Geometry {
        let draws = state.monet_draws.get().wrapping_add(1);
        state.monet_draws.set(draws);
        let mut ink = state.monet_ink.borrow_mut();
        if draws.is_multiple_of(MONET_ATLAS_FRAMES) {
            *ink = canvas::Cache::new();
        } else {
            ink.clear();
        }
        ink.draw(renderer, target, |frame| self.paint(state, frame, target))
    }

    pub(crate) fn render(&self, state: &SceneState, renderer: &Renderer, target: Size) -> Vec<Geometry> {
        if self.style == Style::Still {
            return vec![styles::still::draw(self, state, renderer, target)];
        }
        if self.style == Style::Monet {
            return vec![self.monet_geometry(state, renderer, target)];
        }
        let mut frame = Frame::new(renderer, target);
        self.paint(state, &mut frame, target);
        vec![frame.into_geometry()]
    }

    /// Fills `target` with the theme background, then draws the stage scaled down to cover it.
    pub(crate) fn paint(&self, state: &SceneState, frame: &mut Frame, target: Size) {
        let (size, place) = self.placement(target);
        if self.backdrop {
            frame.fill_rectangle(iced::Point::ORIGIN, target, color(self.theme.background, 1.0));
        }
        if (place.scale - 1.0).abs() > f32::EPSILON {
            frame.translate(place.offset);
            frame.scale(place.scale);
        }
        match self.style {
            Style::Partita => styles::partita::draw(self, &mut state.partita.borrow_mut(), frame, size),
            Style::Tilt => styles::tilt::draw(self, &mut state.tilt.borrow_mut(), frame, size),
            Style::Claddagh => styles::claddagh::draw(self, &mut state.claddagh.borrow_mut(), frame, size),
            Style::Cadenza => styles::cadenza::draw(self, &mut state.cadenza.borrow_mut(), frame, size),
            Style::Lumiere => styles::lumiere::draw(self, &mut state.lumiere.borrow_mut(), frame, size),
            Style::Still => styles::still::draw_stage(self, frame, size),
            Style::Classic => styles::classic::draw(self, &mut state.classic.borrow_mut(), frame, size),
            Style::Nebbia => styles::nebbia::draw(self, frame, size),
            Style::Monet => styles::monet::draw(self, &mut state.monet.borrow_mut(), frame, size, place),
        }
    }
}

/// Stage-to-widget transform of a scene's frame. `Frame::with_clip` drafts start untransformed, so a style that clips
/// maps its regions through this and applies it again inside.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Place {
    pub offset: iced::Vector,
    pub scale: f32,
}

impl Place {
    pub fn to_widget(self, rect: Rectangle) -> Rectangle {
        Rectangle {
            x: self.offset.x + rect.x * self.scale,
            y: self.offset.y + rect.y * self.scale,
            width: rect.width * self.scale,
            height: rect.height * self.scale,
        }
    }

    pub fn to_stage(self, point: Point) -> Point {
        Point::new((point.x - self.offset.x) / self.scale, (point.y - self.offset.y) / self.scale)
    }

    /// Draws `draw` in stage coordinates, clipped to the stage rectangle `region`.
    pub fn clip(self, frame: &mut Frame, region: Rectangle, draw: impl FnOnce(&mut Frame)) {
        frame.with_clip(self.to_widget(region), |inner| {
            inner.translate(self.offset);
            inner.scale(self.scale);
            draw(inner);
        });
    }
}

/// Keeps the layouts of the few lines on screen, keyed by line start, canvas size and font scale.
pub struct LayoutCache<T> {
    entries: Vec<(LayoutKey, Rc<T>)>,
}

#[derive(Clone, Copy, PartialEq)]
struct LayoutKey {
    start: u64,
    width: u32,
    height: u32,
    font_scale: u32,
    salt: u64,
}

impl<T> Default for LayoutCache<T> {
    fn default() -> Self {
        Self { entries: Vec::new() }
    }
}

impl<T> LayoutCache<T> {
    const CAPACITY: usize = 8;

    pub fn get_or_insert(
        &mut self,
        line: &Line,
        size: Size,
        font_scale: f32,
        salt: u64,
        build: impl FnOnce() -> Rc<T>,
    ) -> Rc<T> {
        let key = LayoutKey {
            start: line.start.to_bits(),
            width: size.width.to_bits(),
            height: size.height.to_bits(),
            font_scale: font_scale.to_bits(),
            salt,
        };
        if let Some((_, layout)) = self.entries.iter().find(|(k, _)| *k == key) {
            return layout.clone();
        }
        let layout = build();
        if self.entries.len() == Self::CAPACITY {
            self.entries.remove(0);
        }
        self.entries.push((key, layout.clone()));
        layout
    }
}

/// The playing picture. Canvas styles draw alone. Nebbia puts its shader under a clear lyric canvas; lumiere draws its
/// lyrics in its own pass, then the front bokeh, then a clear canvas with the corner frame.
pub fn lyric_view<'a, Message: From<styles::monet::PortraitOffset> + 'a>(scene: Scene<'a>) -> Element<'a, Message> {
    if !matches!(scene.style, Style::Nebbia | Style::Lumiere) {
        return iced::widget::canvas(scene).width(Length::Fill).height(Length::Fill).into();
    }
    let clear =
        iced::widget::canvas(Scene { backdrop: false, ..scene.clone() }).width(Length::Fill).height(Length::Fill);
    if scene.style == Style::Nebbia {
        return stack![shader(styles::nebbia::view(scene)).width(Length::Fill).height(Length::Fill), clear].into();
    }
    let front = shader(styles::lumiere::front_view(scene.clone())).width(Length::Fill).height(Length::Fill);
    stack![shader(styles::lumiere::field_view(scene)).width(Length::Fill).height(Length::Fill), front, clear].into()
}

pub fn color(rgb: Rgb, alpha: f32) -> Color {
    Color::from_rgba8(rgb.r, rgb.g, rgb.b, alpha.clamp(0.0, 1.0))
}

/// Size of `content` set in `font` at `size` pixels, wrapping at word boundaries past `max_width`.
pub fn measure(content: &str, size: f32, font: Font, max_width: f32) -> Size {
    let paragraph = <Renderer as core_text::Renderer>::Paragraph::with_text(core_text::Text {
        content,
        bounds: Size::new(max_width, f32::INFINITY),
        size: size.into(),
        line_height: core_text::LineHeight::Relative(1.22),
        font,
        align_x: core_text::Alignment::Default,
        align_y: iced::alignment::Vertical::Top,
        shaping: core_text::Shaping::Advanced,
        wrapping: core_text::Wrapping::Word,
    });
    paragraph.min_bounds()
}

/// `content` cut with an ellipsis so it wraps to at most `lines` lines of `width`, like `-webkit-line-clamp`.
pub fn clamp_lines(content: &str, size: f32, font: Font, width: f32, lines: usize) -> String {
    let fits = |text: &str| {
        let bounds = measure(text, size, font, width);
        bounds.width <= width + 0.5 && (bounds.height / (size * 1.22)).round() as usize <= lines
    };
    if fits(content) {
        return content.to_owned();
    }
    let ends: Vec<usize> = content.char_indices().map(|(at, _)| at).collect();
    let cut = |count: usize| format!("{}\u{2026}", content[..ends[count]].trim_end());
    let (mut fitting, mut overflowing) = (0, ends.len());
    while overflowing - fitting > 1 {
        let middle = (fitting + overflowing) / 2;
        if fits(&cut(middle)) {
            fitting = middle;
        } else {
            overflowing = middle;
        }
    }
    cut(fitting)
}
