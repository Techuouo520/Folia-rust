// Lucide icons drawn as tinted SVGs and the icon buttons built from them.

use std::sync::LazyLock;

use iced::advanced::widget::Tree;
use iced::advanced::{Layout, Widget, layout, mouse, renderer};
use iced::widget::svg;
use iced::{Color, Element, Length, Radians, Rectangle, Size};
use lucide_static_svg::Icon;

pub fn icon<'a, M: 'a>(icon: Icon, size: f32, color: Color) -> Element<'a, M> {
    tinted(svg::Handle::from_memory(icon.svg_bytes()), size, color)
}

/// Lucide glyphs the original draws with changed attributes, such as `fill="currentColor"`.
#[derive(Debug, Clone, Copy)]
pub enum Variant {
    FilledPlay,
    FilledPause,
    /// The saved star (`.bravais-seam-star.is-on svg { fill: currentColor }`).
    FilledStar,
    /// `strokeWidth={2.4}`.
    BoldChevronLeft,
    BoldChevronRight,
}

static FILLED_PLAY: LazyLock<svg::Handle> =
    LazyLock::new(|| patched(Icon::Play, "fill=\"none\"", "fill=\"currentColor\""));
static FILLED_PAUSE: LazyLock<svg::Handle> =
    LazyLock::new(|| patched(Icon::Pause, "fill=\"none\"", "fill=\"currentColor\""));
static FILLED_STAR: LazyLock<svg::Handle> =
    LazyLock::new(|| patched(Icon::Star, "fill=\"none\"", "fill=\"currentColor\""));
static BOLD_CHEVRON_LEFT: LazyLock<svg::Handle> =
    LazyLock::new(|| patched(Icon::ChevronLeft, "stroke-width=\"2\"", "stroke-width=\"2.4\""));
static BOLD_CHEVRON_RIGHT: LazyLock<svg::Handle> =
    LazyLock::new(|| patched(Icon::ChevronRight, "stroke-width=\"2\"", "stroke-width=\"2.4\""));

pub fn variant<'a, M: 'a>(variant: Variant, size: f32, color: Color) -> Element<'a, M> {
    let handle = match variant {
        Variant::FilledPlay => &FILLED_PLAY,
        Variant::FilledPause => &FILLED_PAUSE,
        Variant::FilledStar => &FILLED_STAR,
        Variant::BoldChevronLeft => &BOLD_CHEVRON_LEFT,
        Variant::BoldChevronRight => &BOLD_CHEVRON_RIGHT,
    };
    tinted(LazyLock::force(handle).clone(), size, color)
}

fn patched(icon: Icon, from: &str, to: &str) -> svg::Handle {
    svg::Handle::from_memory(icon.svg_str().replacen(from, to, 1).into_bytes())
}

fn tinted<'a, M: 'a>(handle: svg::Handle, size: f32, color: Color) -> Element<'a, M> {
    Element::new(Glyph { handle, size, color })
}

// iced rounds image edges half-to-even, so an icon at x.5 shrinks a pixel and loses 1px dots; snap the origin first.
struct Glyph {
    handle: svg::Handle,
    size: f32,
    color: Color,
}

impl<M, Theme, Renderer: iced::advanced::svg::Renderer> Widget<M, Theme, Renderer> for Glyph {
    fn size(&self) -> Size<Length> {
        Size::new(Length::Fixed(self.size), Length::Fixed(self.size))
    }

    fn layout(&mut self, _tree: &mut Tree, _renderer: &Renderer, _limits: &layout::Limits) -> layout::Node {
        layout::Node::new(Size::new(self.size, self.size))
    }

    fn draw(
        &self,
        _tree: &Tree,
        renderer: &mut Renderer,
        _theme: &Theme,
        _style: &renderer::Style,
        layout: Layout<'_>,
        _cursor: mouse::Cursor,
        _viewport: &Rectangle,
    ) {
        let bounds = layout.bounds();
        let snapped = Rectangle { x: bounds.x.round(), y: bounds.y.round(), ..bounds };
        // The SVG tint replaces RGB and keeps the source alpha, so Color.a has to be the opacity.
        let glyph = iced::advanced::svg::Svg {
            handle: self.handle.clone(),
            color: Some(Color { a: 1.0, ..self.color }),
            rotation: Radians(0.0),
            opacity: self.color.a,
        };
        renderer.draw_svg(glyph, snapped, snapped);
    }
}
