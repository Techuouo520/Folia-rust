// Monet's settings, the original `monet/tuning.ts` and `MonetSettingsPanel.tsx`.

use crate::tuning::{Choice, Group, Item, Manifest};

const SOURCE: &[Choice] = &[
    Choice { value: "cover", label_key: "options.monetPortraitSourceCover", requires: None },
    Choice { value: "custom", label_key: "options.monetPortraitSourceCustom", requires: Some("portraitPath") },
];

const SHAPE: &[Choice] = &[
    Choice { value: "rectangular", label_key: "options.monetPortraitStyleRectangular", requires: None },
    Choice { value: "square", label_key: "options.monetPortraitStyleSquare", requires: None },
];

const AUDIO: &[Choice] = &[
    Choice { value: "bar", label_key: "options.monetAudioStyleBar", requires: None },
    Choice { value: "line", label_key: "options.monetAudioStyleLine", requires: None },
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
    Item::Select {
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
    Item::Select {
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
    manifest.paths = &["portraitPath"];
    manifest.concealed = &["portraitOffsetX"];
    // Theme has no `wordColors`, so the switch cannot change a colour.
    manifest.locked = &["keywordColoringEnabled"];
    manifest.reset_key = Some("notifications.monetReset");
    manifest
}

/// Saved horizontal portrait shift, clamped the way `resolveStoredMonetTuning` clamps it.
pub fn offset(values: &crate::tuning::Values) -> f32 {
    values.number(manifest(), "portraitOffsetX").clamp(-150.0, 0.0)
}
