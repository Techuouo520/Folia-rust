// The original seam title click rules, including diagonal drags and keyboard activation.

use folia_ui::seam_title_activates;
use iced::Point;

#[test]
fn seam_title_pointer_clicks_use_the_original_four_pixel_radius() {
    let down = Some(Point::new(20.0, 30.0));
    assert!(seam_title_activates(false, down, Point::new(24.0, 30.0), false));
    assert!(seam_title_activates(false, down, Point::new(20.0, 34.0), false));
    assert!(!seam_title_activates(false, down, Point::new(24.01, 30.0), false));
    assert!(!seam_title_activates(false, down, Point::new(23.0, 33.0), false));
}

#[test]
fn seam_title_selection_blocks_pointer_activation_even_without_a_recorded_press() {
    assert!(!seam_title_activates(false, Some(Point::ORIGIN), Point::ORIGIN, true));
    assert!(!seam_title_activates(false, None, Point::ORIGIN, true));
    assert!(seam_title_activates(false, None, Point::new(500.0, 500.0), false));
}

#[test]
fn seam_title_keyboard_activation_ignores_pointer_history_and_selection() {
    assert!(seam_title_activates(true, Some(Point::ORIGIN), Point::new(500.0, 500.0), true));
    assert!(seam_title_activates(true, None, Point::ORIGIN, false));
}
