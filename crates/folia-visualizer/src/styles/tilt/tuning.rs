// Tilt's settings, the original `tilt/tuning.ts` fields this port actually draws.

use folia_core::Theme;
use iced::widget::canvas::{Frame, Path};
use iced::{Point, Rectangle, border::Radius};

use crate::color;
use crate::tuning::{Group, Item, Manifest, PictureChoice};

fn scheme_bars(frame: &mut Frame, rect: Rectangle, theme: &Theme, upright: iced::Color, slanted: iced::Color) {
    frame.fill_rectangle(rect.position(), rect.size(), color(theme.background, 1.0));
    let bar =
        Rectangle { x: rect.x + rect.width * 0.24, y: rect.center_y() - 4.0, width: rect.width * 0.26, height: 8.0 };
    frame.fill(&Path::rounded_rectangle(bar.position(), bar.size(), Radius::from(2.0)), upright);
    let skew = 8.0;
    let slanted_bar =
        Rectangle { x: rect.x + rect.width * 0.5, y: rect.center_y() - 4.0, width: rect.width * 0.26, height: 8.0 };
    let path = Path::new(|path| {
        path.move_to(Point::new(slanted_bar.x + skew, slanted_bar.y));
        path.line_to(Point::new(slanted_bar.x + slanted_bar.width + skew, slanted_bar.y));
        path.line_to(Point::new(slanted_bar.x + slanted_bar.width, slanted_bar.y + slanted_bar.height));
        path.line_to(Point::new(slanted_bar.x, slanted_bar.y + slanted_bar.height));
        path.close();
    });
    frame.fill(&path, slanted);
}

fn default_scheme(frame: &mut Frame, rect: Rectangle, theme: &Theme) {
    scheme_bars(frame, rect, theme, color(theme.primary, 1.0), color(theme.accent, 1.0));
}

fn swap_scheme(frame: &mut Frame, rect: Rectangle, theme: &Theme) {
    scheme_bars(frame, rect, theme, color(theme.accent, 1.0), color(theme.primary, 1.0));
}

fn accent_all_scheme(frame: &mut Frame, rect: Rectangle, theme: &Theme) {
    scheme_bars(frame, rect, theme, color(theme.accent, 1.0), color(theme.accent, 1.0));
}

fn primary_all_scheme(frame: &mut Frame, rect: Rectangle, theme: &Theme) {
    scheme_bars(frame, rect, theme, color(theme.primary, 1.0), color(theme.primary, 1.0));
}

const SCHEME_PICTURES: &[PictureChoice] = &[
    PictureChoice {
        value: "default",
        label_key: "options.tiltColorSchemeDefault",
        requires: None,
        picture: default_scheme,
    },
    PictureChoice { value: "swap", label_key: "options.tiltColorSchemeSwap", requires: None, picture: swap_scheme },
    PictureChoice {
        value: "accentAll",
        label_key: "options.tiltColorSchemeAccentAll",
        requires: None,
        picture: accent_all_scheme,
    },
    PictureChoice {
        value: "primaryAll",
        label_key: "options.tiltColorSchemePrimaryAll",
        requires: None,
        picture: primary_all_scheme,
    },
];

const ITEMS: &[Item] = &[
    Item::Picture {
        key: "colorScheme",
        label_key: "options.tiltColorScheme",
        hint_key: None,
        options: SCHEME_PICTURES,
        default: "default",
    },
    Item::Slider {
        key: "splitProbability",
        label_key: "options.tiltSplitProbability",
        hint_key: None,
        min: 0.0,
        max: 1.0,
        step: 0.05,
        default: 0.75,
    },
    Item::Slider {
        key: "tiltStyleProbability",
        label_key: "options.tiltStyleProbability",
        hint_key: None,
        min: 0.0,
        max: 1.0,
        step: 0.05,
        default: 0.35,
    },
];

const GROUPS: &[Group] = &[Group { id: "vis-tilt", title_key: "options.tiltSettings", foot_key: None, items: ITEMS }];

pub fn manifest() -> Manifest {
    let mut manifest = Manifest::plain("tilt_tuning", &[], GROUPS);
    manifest.reset_key = Some("notifications.tiltReset");
    manifest
}
