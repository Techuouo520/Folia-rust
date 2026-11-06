// Classic's settings, the original `classic/tuning.ts` fields this port actually draws.

use crate::tuning::{Group, Item, Manifest};

const ITEMS: &[Item] = &[
    Item::Switch { key: "enableWordRotation", label_key: "options.classicWordRotation", hint_key: None, default: true },
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
    Manifest { storage_key: "classic_tuning", older_keys: &[], groups: GROUPS }
}
