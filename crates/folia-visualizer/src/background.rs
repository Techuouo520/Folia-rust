// The backgrounds this build can draw. Only the common theme ground exists until the original background modes
// are ported; the registry keeps their original ids and label keys so the settings can be stored and read.

use folia_core::Theme;
use iced::widget::canvas::{Frame, Path};
use iced::{Point, Rectangle};

use crate::color;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Background {
    #[default]
    Common,
}

impl Background {
    pub const ALL: [Background; 1] = [Background::Common];

    /// The original app's mode id, as `visualizerBackgroundMode` stores it.
    pub fn id(self) -> &'static str {
        match self {
            Background::Common => "common",
        }
    }

    pub fn from_id(id: &str) -> Option<Background> {
        Background::ALL.into_iter().find(|mode| mode.id() == id)
    }

    /// `resolveVisualizerBackgroundMode`: the stored mode when this build can draw it, otherwise the default.
    pub fn resolve(stored: Option<&str>) -> Background {
        stored.and_then(Background::from_id).unwrap_or(Background::DEFAULT)
    }

    /// The original app's default when nothing is stored and the stored mode cannot be drawn.
    pub const DEFAULT: Background = Background::Common;

    pub fn label_key(self) -> &'static str {
        match self {
            Background::Common => "options.visualizerBackgroundModeCommon",
        }
    }

    /// The 24×24 line glyph from the original `backgrounds/<mode>/entry.tsx`.
    pub fn glyph_svg(self) -> &'static str {
        match self {
            Background::Common => concat!(
                r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="black" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round">"#,
                r#"<path d="M4 17l5-8 5 8z"/><circle cx="17.5" cy="8.5" r="3" opacity="0.55"/></svg>"#
            ),
        }
    }

    /// The 16:10 picture of what the background lays down, for the mode grid.
    pub fn picture(self, frame: &mut Frame, rect: Rectangle, theme: &Theme) {
        match self {
            Background::Common => common_picture(frame, rect, theme),
        }
    }
}

fn common_picture(frame: &mut Frame, rect: Rectangle, theme: &Theme) {
    frame.fill_rectangle(rect.position(), rect.size(), color(theme.background, 1.0));
    let size = rect.width.min(rect.height) * 0.42;
    let left = rect.center_x() - size * 0.55;
    let bottom = rect.center_y() + size * 0.35;
    let triangle = Path::new(|path| {
        path.move_to(Point::new(left, bottom));
        path.line_to(Point::new(left + size * 0.5, bottom - size));
        path.line_to(Point::new(left + size, bottom));
        path.close();
    });
    frame.fill(&triangle, color(theme.primary, 0.62));
    frame.fill(
        &Path::circle(Point::new(left + size * 1.35, bottom - size * 0.85), size * 0.24),
        color(theme.primary, 0.34),
    );
}
