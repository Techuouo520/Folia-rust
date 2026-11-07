// Tilt's settings, the original `tilt/tuning.ts` fields this port actually draws.

use crate::tuning::{Choice, Group, Item, Manifest};

const SCHEMES: &[Choice] = &[
    Choice { value: "default", label_key: "options.tiltColorSchemeDefault", requires: None },
    Choice { value: "swap", label_key: "options.tiltColorSchemeSwap", requires: None },
    Choice { value: "accentAll", label_key: "options.tiltColorSchemeAccentAll", requires: None },
    Choice { value: "primaryAll", label_key: "options.tiltColorSchemePrimaryAll", requires: None },
];

const ITEMS: &[Item] = &[
    Item::Select {
        key: "colorScheme",
        label_key: "options.tiltColorScheme",
        hint_key: None,
        options: SCHEMES,
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
