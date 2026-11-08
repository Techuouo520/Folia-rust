// A shifted or scaled `float` is painted above every layer, including an open scrim.

use iced::{Element, Vector};

/// Draws `element` in the normal stack while `modal` is open. Otherwise the usual float.
pub fn place<'a, Message: 'a>(
    element: impl Into<Element<'a, Message>>,
    modal: bool,
    shift: Vector,
    scale: f32,
) -> Element<'a, Message> {
    let element = element.into();
    if modal {
        return element;
    }
    iced::widget::float(element).translate(move |_, _| shift).scale(scale).into()
}
