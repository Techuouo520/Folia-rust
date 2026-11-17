// Nebbia's settings, copied from the original `nebbia/tuning.ts` and `NebbiaSettingsPanel.tsx`.

use crate::tuning::{Choice, Group, Item, Manifest, NumberFormat, Values};

const BEHIND: &[Choice] = &[
    Choice { value: "night", label_key: "options.nebbiaBehind_night", requires: None },
    Choice { value: "aquarium", label_key: "options.nebbiaBehind_aquarium", requires: None },
    Choice { value: "backdrop", label_key: "options.nebbiaBehind_backdrop", requires: None },
];
const RAIN: &[Choice] = &[
    Choice { value: "slant", label_key: "options.nebbiaRain_slant", requires: None },
    Choice { value: "straight", label_key: "options.nebbiaRain_straight", requires: None },
    Choice { value: "off", label_key: "options.nebbiaRain_off", requires: None },
];
const GLASS_STYLES: &[Choice] = &[
    Choice { value: "rain", label_key: "options.nebbiaGlass_rain", requires: None },
    Choice { value: "dew", label_key: "options.nebbiaGlass_dew", requires: None },
    Choice { value: "snow", label_key: "options.nebbiaGlass_snow", requires: None },
];
const SCENERY: &[Item] = &[
    Item::Select {
        key: "behind",
        label_key: "options.nebbiaBehind",
        hint_key: Some("options.nebbiaBehindHint"),
        options: BEHIND,
        default: "night",
    },
    Item::Slider {
        key: "jellies",
        label_key: "options.nebbiaJellies",
        hint_key: None,
        min: 0.0,
        max: 24.0,
        step: 1.0,
        default: 16.0,
    },
    Item::Slider {
        key: "jellySize",
        label_key: "options.nebbiaJellySize",
        hint_key: None,
        min: 0.5,
        max: 1.8,
        step: 0.05,
        default: 1.0,
    },
    Item::Slider {
        key: "bubbles",
        label_key: "options.nebbiaBubbles",
        hint_key: None,
        min: 0.0,
        max: 1.0,
        step: 0.05,
        default: 0.6,
    },
    Item::Member { key: "scenery", option: "spot", label_key: "options.nebbiaScenery_spot", default_on: true },
    Item::Member {
        key: "scenery",
        option: "buildings",
        label_key: "options.nebbiaScenery_buildings",
        default_on: true,
    },
    Item::Member { key: "scenery", option: "lights", label_key: "options.nebbiaScenery_lights", default_on: true },
    Item::Member { key: "scenery", option: "cars", label_key: "options.nebbiaScenery_cars", default_on: true },
    Item::Member { key: "scenery", option: "road", label_key: "options.nebbiaScenery_road", default_on: true },
    Item::Select {
        key: "rain",
        label_key: "options.nebbiaRain",
        hint_key: Some("options.nebbiaRainHint"),
        options: RAIN,
        default: "slant",
    },
];
const GLASS: &[Item] = &[
    Item::Select {
        key: "glass",
        label_key: "options.nebbiaGlassStyle",
        hint_key: Some("options.nebbiaGlassHint"),
        options: GLASS_STYLES,
        default: "rain",
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
        key: "dew",
        label_key: "options.nebbiaDew",
        hint_key: None,
        min: 0.0,
        max: 1.5,
        step: 0.05,
        default: 1.0,
    },
    Item::Slider {
        key: "dewSize",
        label_key: "options.nebbiaDewSize",
        hint_key: None,
        min: 0.5,
        max: 2.0,
        step: 0.05,
        default: 1.0,
    },
    Item::Slider {
        key: "dewGlint",
        label_key: "options.nebbiaDewGlint",
        hint_key: None,
        min: 0.0,
        max: 2.0,
        step: 0.05,
        default: 1.0,
    },
    Item::Slider {
        key: "snow",
        label_key: "options.nebbiaSnow",
        hint_key: None,
        min: 0.0,
        max: 3.0,
        step: 0.05,
        default: 1.0,
    },
    Item::Slider {
        key: "snowSize",
        label_key: "options.nebbiaSnowSize",
        hint_key: None,
        min: 0.5,
        max: 2.0,
        step: 0.05,
        default: 1.0,
    },
    Item::Slider {
        key: "snowMelt",
        label_key: "options.nebbiaSnowMelt",
        hint_key: None,
        min: 0.25,
        max: 3.0,
        step: 0.05,
        default: 1.0,
    },
];

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
];

const GROUPS: &[Group] = &[
    Group {
        id: "vis-nebbia-scenery",
        title_key: "options.nebbiaScenery",
        foot_key: Some("options.nebbiaSceneryHint"),
        items: SCENERY,
    },
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
    Group { id: "vis-nebbia-glass", title_key: "options.nebbiaGlass", foot_key: None, items: GLASS },
];

pub fn manifest() -> Manifest {
    const MULTIPLIER: NumberFormat = NumberFormat::Times { base: 0.0 };
    const PERCENT: NumberFormat = NumberFormat::Percent { scale: 100.0, signed: false };
    let mut manifest = Manifest::plain("nebbia_tuning", &[], GROUPS);
    manifest.reset_key = Some("notifications.nebbiaReset");
    manifest.allow_empty = &["scenery"];
    manifest.number_formats = &[
        ("strokeWidth", PERCENT),
        ("writeSpeed", MULTIPLIER),
        ("jitter", MULTIPLIER),
        ("warp", MULTIPLIER),
        ("doodle", PERCENT),
        ("mist", PERCENT),
        ("hold", NumberFormat::Seconds),
        ("refog", NumberFormat::Seconds),
        ("busSpeed", MULTIPLIER),
        ("neon", MULTIPLIER),
        ("bump", MULTIPLIER),
        ("spot", MULTIPLIER),
        ("spotX", PERCENT),
        ("rainHit", MULTIPLIER),
        ("rainBig", MULTIPLIER),
        ("resolution", MULTIPLIER),
        ("jellySize", MULTIPLIER),
        ("bubbles", PERCENT),
        ("dew", MULTIPLIER),
        ("dewSize", MULTIPLIER),
        ("dewGlint", MULTIPLIER),
        ("snow", MULTIPLIER),
        ("snowSize", MULTIPLIER),
        ("snowMelt", MULTIPLIER),
    ];
    manifest.disabled_when = Some(disabled);
    manifest.choice_defaults = &[("behind", "night", "glass", "rain"), ("behind", "aquarium", "glass", "dew")];
    manifest.choice_labels = &[
        ("options.nebbiaSceneryHint", "behind", "aquarium", "options.nebbiaAquaSceneryHint"),
        ("options.nebbiaScenery_spot", "behind", "aquarium", "options.nebbiaAquaScenery_spot"),
        ("options.nebbiaScenery_buildings", "behind", "aquarium", "options.nebbiaAquaScenery_buildings"),
        ("options.nebbiaScenery_lights", "behind", "aquarium", "options.nebbiaAquaScenery_lights"),
        ("options.nebbiaScenery_cars", "behind", "aquarium", "options.nebbiaAquaScenery_cars"),
        ("options.nebbiaScenery_road", "behind", "aquarium", "options.nebbiaAquaScenery_road"),
        ("options.nebbiaRain", "behind", "aquarium", "options.nebbiaAquaRain"),
        ("options.nebbiaRainHint", "behind", "aquarium", "options.nebbiaAquaRainHint"),
        ("options.nebbiaRain_slant", "behind", "aquarium", "options.nebbiaAquaRain_slant"),
        ("options.nebbiaRain_straight", "behind", "aquarium", "options.nebbiaAquaRain_straight"),
        ("options.nebbiaRain_off", "behind", "aquarium", "options.nebbiaAquaRain_off"),
    ];
    manifest.hidden_choices = &[
        ("rainHit", "glass", "snow"),
        ("rainBig", "glass", "snow"),
        ("dew", "glass", "snow"),
        ("dewSize", "glass", "snow"),
        ("dewGlint", "glass", "snow"),
        ("snow", "glass", "rain"),
        ("snowSize", "glass", "rain"),
        ("snowMelt", "glass", "rain"),
        ("snow", "glass", "dew"),
        ("snowSize", "glass", "dew"),
        ("snowMelt", "glass", "dew"),
        ("jellies", "behind", "night"),
        ("jellySize", "behind", "night"),
        ("bubbles", "behind", "night"),
        ("jellies", "behind", "backdrop"),
        ("jellySize", "behind", "backdrop"),
        ("bubbles", "behind", "backdrop"),
        ("bump", "behind", "aquarium"),
        ("rainHit", "glass", "dew"),
        ("rainBig", "glass", "dew"),
        ("dew", "glass", "rain"),
        ("dewSize", "glass", "rain"),
        ("dewGlint", "glass", "rain"),
    ];
    manifest
}

fn disabled(item: Item, values: &Values) -> bool {
    let manifest = manifest();
    let backdrop = values.choice(manifest, "behind") == "backdrop";
    match item {
        Item::Member { key: "scenery", option, .. } => backdrop && option != "spot",
        Item::Select { key: "rain", .. } => {
            backdrop
                || values.choice(manifest, "behind") != "aquarium"
                    && (values.choice(manifest, "glass") == "snow"
                        || !values.member(manifest, "scenery", "spot")
                        || values.number(manifest, "spot") <= 0.0)
        }
        _ => false,
    }
}

pub(super) fn scenery(values: &Values) -> u32 {
    let manifest = manifest();
    let mut rain = match values.choice(manifest, "rain") {
        "off" => 0,
        "straight" => 16,
        _ => 48,
    };
    if values.choice(manifest, "glass") == "snow" && values.choice(manifest, "behind") != "aquarium" {
        rain = 0;
    }
    ["buildings", "lights", "cars", "road"]
        .into_iter()
        .enumerate()
        .fold(rain, |mask, (bit, part)| mask | if values.member(manifest, "scenery", part) { 1 << bit } else { 0 })
}

/// Early saves used a single `appearMode` / `fadeMode` string. Both become the list the panel stores now.
pub fn normalize(table: Option<&toml::map::Map<String, toml::Value>>, values: &mut Values) {
    let Some(table) = table else { return };
    if !table.contains_key("render") {
        let resolution = table
            .get("resolution")
            .and_then(|value| match value {
                toml::Value::Float(value) => Some(*value as f32),
                toml::Value::Integer(value) => Some(*value as f32),
                toml::Value::String(value) => value.parse::<f32>().ok(),
                _ => None,
            })
            .filter(|value| value.is_finite())
            .unwrap_or(1.0)
            .clamp(0.5, 2.0);
        if resolution != 1.0 {
            let scale = folia_core::LYRIC_RENDER_SCALES
                .into_iter()
                .reduce(|best, next| if (next - resolution).abs() <= (best - resolution).abs() { next } else { best });
            values.set_render_override(folia_core::LyricRenderOverride { scale, ..Default::default() });
        }
    }
    if table.get("glass").is_none() && values.choice(manifest(), "behind") == "aquarium" {
        values.set_choice(manifest(), "glass", "dew");
    }
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
