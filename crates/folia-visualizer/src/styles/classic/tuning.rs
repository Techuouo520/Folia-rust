// Classic's settings, the original `classic/tuning.ts` fields this port actually draws.

use crate::tuning::{Choice, Group, Item, Manifest};

const LAYOUT: &[Choice] = &[
    Choice { value: "false", label_key: "options.classicLegacyLayoutOff", requires: None },
    Choice { value: "true", label_key: "options.classicLegacyLayoutOn", requires: None },
];

const ITEMS: &[Item] = &[
    Item::Switch { key: "enableWordRotation", label_key: "options.classicWordRotation", hint_key: None, default: true },
    Item::Select {
        key: "useLegacyLayout",
        label_key: "options.classicLegacyLayout",
        hint_key: None,
        options: LAYOUT,
        default: "false",
    },
    Item::Slider {
        key: "breathingFloatMultiplier",
        label_key: "options.classicBreathingFloatMultiplier",
        hint_key: None,
        min: 0.0,
        max: 2.0,
        step: 0.05,
        default: 1.0,
    },
    Item::Slider {
        key: "wordSpacing",
        label_key: "options.classicWordSpacing",
        hint_key: None,
        min: 0.0,
        max: 2.0,
        step: 0.05,
        default: 0.7,
    },
];

const GROUPS: &[Group] =
    &[Group { id: "vis-classic", title_key: "options.classicSettings", foot_key: None, items: ITEMS }];

pub fn manifest() -> Manifest {
    let mut manifest = Manifest::plain("classic_tuning", &[], GROUPS);
    manifest.hidden_when = &[("wordSpacing", "useLegacyLayout")];
    manifest.reset_key = Some("notifications.classicReset");
    manifest
}
