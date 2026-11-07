// Nebbia's settings, copied from the original `nebbia/tuning.ts` and `NebbiaSettingsPanel.tsx`.

use crate::tuning::{Group, Item, Manifest, Values};

const WRITING: &[Item] = &[
    Item::Slider {
        key: "strokeWidth",
        label_key: "options.nebbiaStrokeWidth",
        hint_key: None,
        min: 0.04,
        max: 0.16,
        step: 0.005,
        default: 0.09,
    },
    Item::Slider {
        key: "writeSpeed",
        label_key: "options.nebbiaWriteSpeed",
        hint_key: None,
        min: 0.4,
        max: 2.5,
        step: 0.05,
        default: 1.0,
    },
    Item::Slider {
        key: "jitter",
        label_key: "options.nebbiaJitter",
        hint_key: None,
        min: 0.0,
        max: 1.5,
        step: 0.05,
        default: 0.6,
    },
    Item::Slider {
        key: "warp",
        label_key: "options.nebbiaWarp",
        hint_key: None,
        min: 0.0,
        max: 1.5,
        step: 0.05,
        default: 0.6,
    },
    Item::Slider {
        key: "doodle",
        label_key: "options.nebbiaDoodle",
        hint_key: None,
        min: 0.0,
        max: 1.0,
        step: 0.05,
        default: 0.3,
    },
    Item::Slider {
        key: "mist",
        label_key: "options.nebbiaMist",
        hint_key: None,
        min: 0.2,
        max: 1.0,
        step: 0.01,
        default: 0.75,
    },
    Item::Slider {
        key: "hold",
        label_key: "options.nebbiaHold",
        hint_key: None,
        min: 0.0,
        max: 6.0,
        step: 0.1,
        default: 1.2,
    },
    Item::Slider {
        key: "refog",
        label_key: "options.nebbiaRefog",
        hint_key: None,
        min: 0.5,
        max: 12.0,
        step: 0.5,
        default: 4.0,
    },
    Item::Switch {
        key: "credits",
        label_key: "options.nebbiaCredits",
        hint_key: Some("options.nebbiaCreditsHint"),
        default: true,
    },
];

const APPEAR: &[Item] = &[
    Item::Member { key: "appear", option: "write", label_key: "options.nebbiaAppearWrite", default_on: true },
    Item::Member { key: "appear", option: "fade", label_key: "options.nebbiaAppearFade", default_on: true },
    Item::Member { key: "appear", option: "breath", label_key: "options.nebbiaAppearBreath", default_on: false },
];

const FADE: &[Item] = &[
    Item::Member { key: "fade", option: "fog", label_key: "options.nebbiaFadeFog", default_on: true },
    Item::Member { key: "fade", option: "fingers", label_key: "options.nebbiaFadeFingers", default_on: false },
];

const LAYOUTS: &[Item] = &[
    Item::Member { key: "layouts", option: "ribbon", label_key: "options.nebbiaLayout_ribbon", default_on: true },
    Item::Member { key: "layouts", option: "lines2", label_key: "options.nebbiaLayout_lines2", default_on: true },
    Item::Member { key: "layouts", option: "lines3", label_key: "options.nebbiaLayout_lines3", default_on: true },
    Item::Member { key: "layouts", option: "cross", label_key: "options.nebbiaLayout_cross", default_on: true },
    Item::Member { key: "layouts", option: "pillar", label_key: "options.nebbiaLayout_pillar", default_on: true },
    Item::Member { key: "layouts", option: "header", label_key: "options.nebbiaLayout_header", default_on: true },
    Item::Member { key: "layouts", option: "badge", label_key: "options.nebbiaLayout_badge", default_on: true },
    Item::Member { key: "layouts", option: "columns", label_key: "options.nebbiaLayout_columns", default_on: true },
    Item::Member { key: "layouts", option: "orbit", label_key: "options.nebbiaLayout_orbit", default_on: true },
];

const WINDOW: &[Item] = &[
    Item::Slider {
        key: "busSpeed",
        label_key: "options.nebbiaBusSpeed",
        hint_key: None,
        min: 0.0,
        max: 3.0,
        step: 0.05,
        default: 1.0,
    },
    Item::Slider {
        key: "neon",
        label_key: "options.nebbiaNeon",
        hint_key: None,
        min: 0.0,
        max: 2.0,
        step: 0.05,
        default: 1.0,
    },
    Item::Slider {
        key: "bump",
        label_key: "options.nebbiaBump",
        hint_key: None,
        min: 0.0,
        max: 2.5,
        step: 0.05,
        default: 1.0,
    },
    Item::Slider {
        key: "spot",
        label_key: "options.nebbiaSpot",
        hint_key: None,
        min: 0.0,
        max: 2.0,
        step: 0.05,
        default: 0.5,
    },
    Item::Slider {
        key: "spotX",
        label_key: "options.nebbiaSpotX",
        hint_key: None,
        min: 0.0,
        max: 1.0,
        step: 0.01,
        default: 0.8,
    },
    Item::Slider {
        key: "rainHit",
        label_key: "options.nebbiaRainHit",
        hint_key: None,
        min: 0.0,
        max: 3.0,
        step: 0.05,
        default: 1.0,
    },
    Item::Slider {
        key: "rainBig",
        label_key: "options.nebbiaRainBig",
        hint_key: None,
        min: 0.0,
        max: 2.0,
        step: 0.05,
        default: 1.0,
    },
    Item::Slider {
        key: "resolution",
        label_key: "options.nebbiaResolution",
        hint_key: None,
        min: 0.5,
        max: 2.0,
        step: 0.25,
        default: 1.0,
    },
];

const GROUPS: &[Group] = &[
    Group { id: "vis-nebbia-writing", title_key: "options.nebbiaWriting", foot_key: None, items: WRITING },
    Group {
        id: "vis-nebbia-appear",
        title_key: "options.nebbiaAppear",
        foot_key: Some("options.nebbiaAppearHint"),
        items: APPEAR,
    },
    Group {
        id: "vis-nebbia-fade",
        title_key: "options.nebbiaFade",
        foot_key: Some("options.nebbiaFadeHint"),
        items: FADE,
    },
    Group {
        id: "vis-nebbia-layouts",
        title_key: "options.nebbiaLayouts",
        foot_key: Some("options.nebbiaLayoutsHint"),
        items: LAYOUTS,
    },
    Group { id: "vis-nebbia-window", title_key: "options.nebbiaWindow", foot_key: None, items: WINDOW },
];

pub fn manifest() -> Manifest {
    let mut manifest = Manifest::plain("nebbia_tuning", &[], GROUPS);
    manifest.reset_key = Some("notifications.nebbiaReset");
    manifest
}

/// Early saves used a single `appearMode` / `fadeMode` string. Both become the list the panel stores now.
pub fn normalize(table: Option<&toml::map::Map<String, toml::Value>>, values: &mut Values) {
    let Some(table) = table else { return };
    if table.get("appear").is_none() {
        let legacy = match table.get("appearMode").and_then(toml::Value::as_str) {
            Some("write") => &["write"][..],
            Some("fade") => &["fade"][..],
            Some("auto") => &["write", "fade"][..],
            _ => &[],
        };
        force(values, "appear", legacy);
    }
    if table.get("fade").is_none() {
        let legacy = match table.get("fadeMode").and_then(toml::Value::as_str) {
            Some("fog") => &["fog"][..],
            Some("fingers") => &["fingers"][..],
            Some("mix") => &["fog", "fingers"][..],
            _ => &[],
        };
        force(values, "fade", legacy);
    }
}

fn force(values: &mut Values, key: &str, on: &[&str]) {
    if on.is_empty() {
        return;
    }
    let options: Vec<&str> = manifest()
        .items()
        .filter_map(|item| match item {
            crate::tuning::Item::Member { key: item_key, option, .. } if item_key == key => Some(option),
            _ => None,
        })
        .collect();
    for option in &options {
        if on.contains(option) && !values.member(manifest(), key, option) {
            let _ = values.toggle_member(manifest(), key, option, true);
        }
    }
    for option in &options {
        if !on.contains(option) && values.member(manifest(), key, option) {
            let _ = values.toggle_member(manifest(), key, option, false);
        }
    }
}
