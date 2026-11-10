// A 17px help icon centred at x.5 through the real iced renderer: the dot fades unless the origin is snapped.

use iced::advanced::renderer::{Headless, Style};
use iced::widget::{container, svg};
use iced::{Color, Element, Font, Length, Pixels, Size, Theme};
use iced_runtime::user_interface::{Cache, UserInterface};
use lucide_static_svg::Icon;

struct Snapped(svg::Handle, Color);

impl<M> iced::advanced::Widget<M, Theme, iced::Renderer> for Snapped {
    fn size(&self) -> Size<Length> {
        Size::new(Length::Fixed(17.0), Length::Fixed(17.0))
    }
    fn layout(
        &mut self,
        _: &mut iced::advanced::widget::Tree,
        _: &iced::Renderer,
        _: &iced::advanced::layout::Limits,
    ) -> iced::advanced::layout::Node {
        iced::advanced::layout::Node::new(Size::new(17.0, 17.0))
    }
    fn draw(
        &self,
        _: &iced::advanced::widget::Tree,
        renderer: &mut iced::Renderer,
        _: &Theme,
        _: &Style,
        layout: iced::advanced::Layout<'_>,
        _: iced::mouse::Cursor,
        _: &iced::Rectangle,
    ) {
        use iced::advanced::svg::Renderer as _;
        let b = layout.bounds();
        let r = iced::Rectangle { x: b.x.round(), y: b.y.round(), ..b };
        let glyph = iced::advanced::svg::Svg {
            handle: self.0.clone(),
            color: Some(self.1),
            rotation: iced::Radians(0.0),
            opacity: 1.0,
        };
        renderer.draw_svg(glyph, r, r);
    }
}

fn shot(offset: f32, snap: bool) -> Vec<u8> {
    let ink = Color::from_rgb8(0x27, 0x27, 0x2a);
    let glyph = svg(svg::Handle::from_memory(Icon::CircleQuestionMark.svg_bytes()))
        .width(Length::Fixed(17.0))
        .height(Length::Fixed(17.0))
        .style(move |_, _| svg::Style { color: Some(ink) });
    let glyph: Element<'_, (), Theme, iced::Renderer> = if snap {
        Element::new(Snapped(svg::Handle::from_memory(Icon::CircleQuestionMark.svg_bytes()), ink))
    } else {
        glyph.into()
    };
    let button = container(glyph).center(Length::Fixed(32.0));
    let root: Element<'_, (), Theme, iced::Renderer> =
        container(button).padding(iced::Padding { top: offset, left: offset, right: 0.0, bottom: 0.0 }).into();
    let mut renderer =
        pollster::block_on(<iced::Renderer as Headless>::new(Font::DEFAULT, Pixels(16.0), None)).unwrap();
    let mut ui = UserInterface::build(root, Size::new(40.0, 40.0), Cache::default(), &mut renderer);
    ui.draw(&mut renderer, &Theme::Light, &Style { text_color: ink }, iced::mouse::Cursor::Unavailable);
    renderer.screenshot(
        &iced::advanced::graphics::Viewport::with_physical_size(Size::new(40, 40), 1.0),
        Color::from_rgb8(0xdc, 0xde, 0xdc),
    )
}

fn dot(rgba: &[u8]) -> u8 {
    (18..21).flat_map(|y| (15..17).map(move |x| (y, x))).map(|(y, x)| rgba[(y * 40 + x) * 4]).min().unwrap()
}

#[test]
#[cfg_attr(feature = "ci", ignore = "needs a GPU; CI runners only have a software renderer")]
fn help_icon_dot_survives_half_pixel_centring() {
    let (plain, snapped, whole) = (dot(&shot(0.0, false)), dot(&shot(0.0, true)), dot(&shot(0.5, false)));
    eprintln!("darkest dot pixel: plain x.5 {plain}, snapped x.5 {snapped}, whole-pixel {whole} (background 220)");
    assert!(plain > 180);
    assert!(snapped < 120);
}
