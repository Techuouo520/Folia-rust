// Claddagh's settings, the original `claddagh/tuning.ts` ranges.

use crate::tuning::{Group, Item, Manifest, NumberFormat};

const ITEMS: &[Item] = &[
    Item::Slider {
        key: "focusScaleRatio",
        label_key: "options.claddaghFocusScaleRatio",
        hint_key: None,
        min: 0.0,
        max: 1.5,
        step: 0.05,
        default: 0.65,
    },
    Item::Slider {
        key: "radiusScale",
        label_key: "options.claddaghRadiusScale",
        hint_key: None,
        min: 0.5,
        max: 1.5,
        step: 0.05,
        default: 1.0,
    },
    Item::Slider {
        key: "positionX",
        label_key: "options.claddaghPositionX",
        hint_key: None,
        min: -40.0,
        max: 40.0,
        step: 1.0,
        default: 0.0,
    },
    Item::Slider {
        key: "positionY",
        label_key: "options.claddaghPositionY",
        hint_key: None,
        min: -40.0,
        max: 40.0,
        step: 1.0,
        default: 0.0,
    },
    Item::Slider {
        key: "ellipseTiltDeg",
        label_key: "options.claddaghEllipseTiltDeg",
        hint_key: None,
        min: 0.0,
        max: 60.0,
        step: 1.0,
        default: 45.0,
    },
    Item::Switch { key: "showAxisLine", label_key: "options.claddaghShowAxisLine", hint_key: None, default: true },
    Item::Slider {
        key: "letterSpacingOffset",
        label_key: "options.claddaghLetterSpacingOffset",
        hint_key: None,
        min: -5.0,
        max: 20.0,
        step: 0.5,
        default: 0.0,
    },
    Item::Switch {
        key: "showTranslation",
        label_key: "options.claddaghShowTranslation",
        hint_key: None,
        default: false,
    },
    Item::Slider {
        key: "translationScale",
        label_key: "options.claddaghTranslationScale",
        hint_key: None,
        min: 0.5,
        max: 2.0,
        step: 0.05,
        default: 1.0,
    },
    Item::Slider {
        key: "translationLetterSpacingOffset",
        label_key: "options.claddaghTranslationLetterSpacingOffset",
        hint_key: None,
        min: -5.0,
        max: 20.0,
        step: 0.5,
        default: 0.0,
    },
];

const GROUPS: &[Group] =
    &[Group { id: "vis-claddagh", title_key: "options.claddaghSettings", foot_key: None, items: ITEMS }];

pub fn manifest() -> Manifest {
    let mut manifest = Manifest::plain("claddagh_tuning", &[], GROUPS);
    manifest.reset_key = Some("notifications.claddaghReset");
    manifest.hidden_unless =
        &[("translationScale", "showTranslation"), ("translationLetterSpacingOffset", "showTranslation")];
    manifest.indented = &["translationScale", "translationLetterSpacingOffset"];
    manifest.number_formats = &[
        ("focusScaleRatio", NumberFormat::Times { base: 1.0 }),
        ("radiusScale", NumberFormat::Times { base: 0.0 }),
        ("positionX", NumberFormat::Percent { scale: 1.0, signed: true }),
        ("positionY", NumberFormat::Percent { scale: 1.0, signed: true }),
        ("ellipseTiltDeg", NumberFormat::Degrees),
        ("letterSpacingOffset", NumberFormat::Pixels { decimals: 1, signed: true }),
        ("translationScale", NumberFormat::Times { base: 0.0 }),
        ("translationLetterSpacingOffset", NumberFormat::Pixels { decimals: 1, signed: true }),
    ];
    manifest
}
