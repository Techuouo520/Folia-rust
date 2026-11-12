// Monet's settings, the original `monet/tuning.ts` and `MonetSettingsPanel.tsx`.

use folia_core::Theme;
use iced::widget::canvas::{Frame, LineCap, Path, Stroke};
use iced::{Point, Rectangle, border::Radius};

use crate::color;
use crate::tuning::{Choice, Group, Item, Manifest, PictureChoice};

const SOURCE: &[Choice] = &[
    Choice { value: "cover", label_key: "options.monetPortraitSourceCover", requires: None },
    Choice { value: "custom", label_key: "options.monetPortraitSourceCustom", requires: Some("portraitPath") },
];

fn square_portrait(frame: &mut Frame, rect: Rectangle, theme: &Theme) {
    let side = rect.height * 0.56;
    plate(frame, rect, side, side, theme);
}

fn tall_portrait(frame: &mut Frame, rect: Rectangle, theme: &Theme) {
    let height = rect.height * 0.76;
    plate(frame, rect, height * 0.75, height, theme);
}

fn plate(frame: &mut Frame, rect: Rectangle, width: f32, height: f32, theme: &Theme) {
    let box_ = Rectangle { x: rect.center_x() - width / 2.0, y: rect.center_y() - height / 2.0, width, height };
    let path = Path::rounded_rectangle(box_.position(), box_.size(), Radius::from(2.0));
    frame.fill(&path, color(theme.primary, 0.62));
}

const SHAPE: &[PictureChoice] = &[
    PictureChoice {
        value: "rectangular",
        label_key: "options.monetPortraitStyleRectangular",
        requires: None,
        picture: tall_portrait,
    },
    PictureChoice {
        value: "square",
        label_key: "options.monetPortraitStyleSquare",
        requires: None,
        picture: square_portrait,
    },
];

/// The spectrum at the bottom: bars, or one line (`MonetSettingsPanel.tsx`).
fn bar_spectrum(frame: &mut Frame, rect: Rectangle, theme: &Theme) {
    let at = |x: f32, y: f32| Point::new(rect.x + x / 64.0 * rect.width, rect.y + y / 40.0 * rect.height);
    let width = (3.0 / 64.0 * rect.width).max(1.0);
    for (x, top) in [(14.0, 22.0), (22.0, 14.0), (30.0, 19.0), (38.0, 10.0), (46.0, 17.0), (54.0, 24.0)] {
        let from = at(x, top);
        let to = at(x, 30.0);
        frame.fill_rectangle(
            Point::new(from.x - width / 2.0, from.y),
            iced::Size::new(width, to.y - from.y),
            color(theme.primary, 0.62),
        );
    }
}

fn line_spectrum(frame: &mut Frame, rect: Rectangle, theme: &Theme) {
    let at = |x: f32, y: f32| Point::new(rect.x + x / 64.0 * rect.width, rect.y + y / 40.0 * rect.height);
    let path = Path::new(|path| {
        path.move_to(at(10.0, 26.0));
        for (x, y) in [(18.0, 21.0), (26.0, 24.0), (34.0, 12.0), (42.0, 19.0), (50.0, 15.0), (56.0, 23.0)] {
            path.line_to(at(x, y));
        }
    });
    let stroke = Stroke::default()
        .with_width((2.0 / 64.0 * rect.width).max(1.0))
        .with_color(color(theme.primary, 0.62))
        .with_line_cap(LineCap::Round);
    frame.stroke(&path, stroke);
}

const AUDIO: &[PictureChoice] = &[
    PictureChoice { value: "bar", label_key: "options.monetAudioStyleBar", requires: None, picture: bar_spectrum },
    PictureChoice { value: "line", label_key: "options.monetAudioStyleLine", requires: None, picture: line_spectrum },
];

const LYRICS: &[Item] = &[
    Item::Slider {
        key: "fontScale",
        label_key: "options.monetFontScale",
        hint_key: None,
        min: 0.7,
        max: 1.5,
        step: 0.05,
        default: 1.2,
    },
    Item::Switch {
        key: "keywordColoringEnabled",
        label_key: "options.monetKeywordColoring",
        hint_key: None,
        default: true,
    },
    Item::Switch { key: "showDescription", label_key: "options.monetShowDescription", hint_key: None, default: true },
];

const PORTRAIT: &[Item] = &[
    Item::Select {
        key: "portraitSource",
        label_key: "options.monetPortraitSource",
        hint_key: None,
        options: SOURCE,
        default: "cover",
    },
    Item::Button { key: "uploadPortrait", label_key: "options.monetUploadPortrait", path_key: Some("portraitPath") },
    Item::Button { key: "clearPortrait", label_key: "options.monetClearPortrait", path_key: None },
    Item::Picture {
        key: "portraitStyle",
        label_key: "options.monetPortraitStyle",
        hint_key: None,
        options: SHAPE,
        default: "square",
    },
    Item::Switch {
        key: "showPortraitDragHanger",
        label_key: "options.monetPortraitDragHanger",
        hint_key: None,
        default: true,
    },
    Item::Slider {
        key: "portraitOffsetX",
        label_key: "options.monetPortraitDragHanger",
        hint_key: None,
        min: -150.0,
        max: 0.0,
        step: 1.0,
        default: 0.0,
    },
];

const SOUND: &[Item] = &[
    Item::Switch {
        key: "showAudioVisualization",
        label_key: "options.monetShowAudioVisualization",
        hint_key: None,
        default: true,
    },
    Item::Picture {
        key: "audioStyle",
        label_key: "options.monetAudioStyle",
        hint_key: None,
        options: AUDIO,
        default: "bar",
    },
];

const GROUPS: &[Group] = &[
    Group { id: "vis-monet-lyrics", title_key: "options.lyricsStyleSettings", foot_key: None, items: LYRICS },
    Group { id: "vis-monet-portrait", title_key: "options.cappellaAvatarSource", foot_key: None, items: PORTRAIT },
    Group { id: "vis-monet-audio", title_key: "options.monetAudioStyle", foot_key: None, items: SOUND },
];

pub fn manifest() -> Manifest {
    let mut manifest = Manifest::plain("monet_tuning", &[], GROUPS);
    // `MonetSettingsPanel.tsx` reveals the audio style only while the spectrum is on.
    manifest.hidden_unless = &[("audioStyle", "showAudioVisualization")];
    manifest.paths = &["portraitPath"];
    manifest.concealed = &["portraitOffsetX"];
    manifest.reset_key = Some("notifications.monetReset");
    manifest
}

/// Saved horizontal portrait shift, clamped the way `resolveStoredMonetTuning` clamps it.
pub fn offset(values: &crate::tuning::Values) -> f32 {
    values.number(manifest(), "portraitOffsetX").clamp(-150.0, 0.0)
}
