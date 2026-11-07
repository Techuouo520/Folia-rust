// Claddagh's settings, the original `claddagh/tuning.ts` ranges.

use crate::tuning::{Group, Item, Manifest};

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
];

const GROUPS: &[Group] =
    &[Group { id: "vis-claddagh", title_key: "options.claddaghSettings", foot_key: None, items: ITEMS }];

pub fn manifest() -> Manifest {
    let mut manifest = Manifest::plain("claddagh_tuning", &[], GROUPS);
    manifest.reset_key = Some("notifications.claddaghReset");
    manifest
}
