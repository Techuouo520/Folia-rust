// Shared backgrounds this build can draw. `url` stays out. A mode is listed only once its picture is real.

use folia_core::Theme;
use iced::widget::canvas::{Frame, Path};
use iced::{Point, Rectangle};

use crate::color;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Background {
    Common,
    /// Original default. Cover-coloured mesh gradient and dither.
    #[default]
    Latent,
    /// Starfield. Blank mode is a solid black or white frame.
    Sora,
    Monet,
    Nomand,
}

impl Background {
    pub const ALL: [Background; 5] =
        [Background::Common, Background::Monet, Background::Nomand, Background::Latent, Background::Sora];

    /// The original app's mode id, as `visualizerBackgroundMode` stores it.
    pub fn id(self) -> &'static str {
        match self {
            Background::Common => "common",
            Background::Latent => "latent",
            Background::Sora => "sora",
            Background::Monet => "monet",
            Background::Nomand => "nomand",
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
    pub const DEFAULT: Background = Background::Latent;

    pub fn label_key(self) -> &'static str {
        match self {
            Background::Common => "options.visualizerBackgroundModeCommon",
            Background::Latent => "options.visualizerBackgroundModeLatent",
            Background::Sora => "options.visualizerBackgroundModeSora",
            Background::Monet => "options.visualizerBackgroundModeMonet",
            Background::Nomand => "options.visualizerBackgroundModeNomand",
        }
    }

    /// The 24×24 line glyph from the original `backgrounds/<mode>/entry.tsx`.
    pub fn glyph_svg(self) -> &'static str {
        match self {
            Background::Nomand => {
                r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="black" stroke-width="1.6"><rect x="3" y="4" width="18" height="16" rx="2"/><path d="M7 4v16m5-16v16m5-16v16"/></svg>"#
            }
            Background::Monet => {
                r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="black" stroke-width="1.6"><rect x="3.5" y="4.5" width="17" height="15" rx="2"/><path d="M20.5 8.5L8 19.5" opacity="0.55"/></svg>"#
            }
            Background::Common => concat!(
                r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="black" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round">"#,
                r#"<path d="M4 17l5-8 5 8z"/><circle cx="17.5" cy="8.5" r="3" opacity="0.55"/></svg>"#
            ),
            Background::Latent => concat!(
                r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="black" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round">"#,
                r#"<path d="M4.5 7.5h.01M9 7.5h.01M13.5 7.5h.01M18 7.5h.01" stroke-width="2.2" opacity="0.55"/>"#,
                r#"<path d="M3 15c3-3.5 6-3.5 9 0s6 3.5 9 0"/></svg>"#
            ),
            Background::Sora => concat!(
                r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="black" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round">"#,
                r#"<path d="M12 5.5l1.2 2.9 2.9 1.2-2.9 1.2L12 13.7l-1.2-2.9-2.9-1.2 2.9-1.2z"/>"#,
                r#"<path d="M5.5 16.5h.01M18 15.5h.01M7 7h.01" stroke-width="2.2" opacity="0.5"/></svg>"#
            ),
        }
    }

    /// The 16:10 picture of what the background lays down, for the mode grid.
    pub fn picture(self, frame: &mut Frame, rect: Rectangle, theme: &Theme) {
        match self {
            Background::Common => common_picture(frame, rect, theme),
            Background::Latent => latent_picture(frame, rect, theme),
            Background::Sora => sora_picture(frame, rect, theme),
            Background::Monet => latent_picture(frame, rect, theme),
            Background::Nomand => common_picture(frame, rect, theme),
        }
    }
}

fn sora_picture(frame: &mut Frame, rect: Rectangle, theme: &Theme) {
    frame.fill_rectangle(rect.position(), rect.size(), color(theme.background, 1.0));
    let radius = rect.width.min(rect.height) * 0.035;
    for (dx, dy, rgb) in [(-0.22, -0.18, theme.primary), (0.16, -0.08, theme.accent), (0.02, 0.2, theme.primary)] {
        frame.fill(
            &Path::circle(Point::new(rect.center_x() + rect.width * dx, rect.center_y() + rect.height * dy), radius),
            color(rgb, 0.9),
        );
    }
}

fn latent_picture(frame: &mut Frame, rect: Rectangle, theme: &Theme) {
    frame.fill_rectangle(rect.position(), rect.size(), color(theme.background, 1.0));
    let radius = rect.width.min(rect.height) * 0.28;
    for (dx, dy, rgb, alpha) in
        [(-0.2, -0.05, theme.secondary, 0.85), (0.15, 0.1, theme.accent, 0.7), (0.0, 0.18, theme.primary, 0.45)]
    {
        frame.fill(
            &Path::circle(Point::new(rect.center_x() + rect.width * dx, rect.center_y() + rect.height * dy), radius),
            color(rgb, alpha),
        );
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
