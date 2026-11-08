// Lumiere tuning defaults and the settings manifest. Values match the original `DEFAULT_LUMIERE_TUNING`.

use crate::tuning::{Choice, Group, Item, Manifest, Values};

/// User-facing lumiere controls, read from the saved `lumiere_tuning` table.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Tuning {
    pub light_intensity: f32,
    pub audio_response: f32,
    pub fog_density: f32,
    pub dark_field: f32,
    pub mote_amount: f32,
    pub bloom: f32,
    pub text_bloom: f32,
    pub unlit_opacity: f32,
    /// 1 = previous + current, 2 = previous + current + next.
    pub window_neighbors: u8,
    pub decay: f32,
    pub echo: f32,
    pub fog_octaves: f32,
    pub line_art: bool,
    pub front_bokeh: bool,
    pub trails: bool,
    pub hide_trails: bool,
    pub seamless: bool,
    pub overlay_frame: bool,
    pub text_only: bool,
    pub keyword_colors: bool,
    pub theme_icons: bool,
    pub theme_color_mix: f32,
    /// `full`, `balanced` or `low`.
    pub render_quality: Quality,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Quality {
    Full,
    Balanced,
    Low,
}

impl Tuning {
    pub const DEFAULT: Tuning = Tuning {
        light_intensity: 1.0,
        audio_response: 1.0,
        fog_density: 1.0,
        dark_field: 0.75,
        mote_amount: 1.0,
        bloom: 1.0,
        text_bloom: 1.0,
        unlit_opacity: 0.22,
        window_neighbors: 2,
        decay: 1.0,
        echo: 1.0,
        fog_octaves: 5.0,
        line_art: true,
        front_bokeh: true,
        trails: true,
        hide_trails: false,
        seamless: true,
        overlay_frame: true,
        text_only: false,
        keyword_colors: true,
        theme_icons: true,
        theme_color_mix: 0.3,
        render_quality: Quality::Full,
    };

    pub fn octaves(self) -> f32 {
        let cap = match self.render_quality {
            Quality::Full => 6.0,
            Quality::Balanced => 4.0,
            Quality::Low => 3.0,
        };
        self.fog_octaves.clamp(2.0, cap)
    }

    /// `LUMIERE_QUALITY_PROFILES`: internal scale of the graphics target.
    pub fn graphics_scale(self) -> f32 {
        match self.render_quality {
            Quality::Full => 1.0,
            Quality::Balanced => 0.7,
            Quality::Low => 0.5,
        }
    }

    /// Mote count multiplier from the same profile. Full and balanced stay at 1.
    pub fn mote_scale(self) -> f32 {
        match self.render_quality {
            Quality::Low => 0.6,
            Quality::Full | Quality::Balanced => 1.0,
        }
    }
}

impl Tuning {
    /// The saved values as the renderer reads them; anything missing keeps its default.
    pub fn read(values: &Values) -> Tuning {
        let m = manifest();
        let number = |key| values.number(m, key);
        let flag = |key| values.flag(m, key);
        Tuning {
            light_intensity: number("lightIntensity"),
            audio_response: number("audioResponse"),
            fog_density: number("fogDensity"),
            dark_field: number("darkField"),
            mote_amount: number("moteAmount"),
            bloom: number("bloom"),
            text_bloom: number("textBloom"),
            unlit_opacity: number("unlitOpacity"),
            window_neighbors: if values.choice(m, "windowNeighbors") == "1" { 1 } else { 2 },
            decay: number("decay"),
            echo: number("echo"),
            fog_octaves: number("fogOctaves").round(),
            line_art: flag("lineArt"),
            front_bokeh: flag("frontBokeh"),
            trails: flag("trails"),
            hide_trails: flag("hideTrails"),
            seamless: flag("seamlessTransitions"),
            overlay_frame: flag("overlayFrame"),
            text_only: flag("textOnly"),
            keyword_colors: flag("keywordColors"),
            theme_icons: flag("themeIcons"),
            theme_color_mix: number("themeColorMix"),
            render_quality: match values.choice(m, "renderQuality") {
                "balanced" => Quality::Balanced,
                "low" => Quality::Low,
                _ => Quality::Full,
            },
        }
    }
}

const fn slider(key: &'static str, label_key: &'static str, range: (f32, f32), step: f32, default: f32) -> Item {
    Item::Slider { key, label_key, hint_key: None, min: range.0, max: range.1, step, default }
}

const fn toggle(key: &'static str, label_key: &'static str, default: bool) -> Item {
    Item::Switch { key, label_key, hint_key: None, default }
}

/// The same row with the original's note under it.
const fn noted(item: Item, hint: &'static str) -> Item {
    match item {
        Item::Slider { key, label_key, min, max, step, default, .. } => {
            Item::Slider { key, label_key, hint_key: Some(hint), min, max, step, default }
        }
        Item::Switch { key, label_key, default, .. } => Item::Switch { key, label_key, hint_key: Some(hint), default },
        Item::Member { .. } | Item::Select { .. } | Item::Picture { .. } | Item::Button { .. } => item,
    }
}

const D: Tuning = Tuning::DEFAULT;

const LIGHT: &[Item] = &[
    slider("lightIntensity", "options.lumiereLightIntensity", (0.3, 2.0), 0.05, D.light_intensity),
    noted(
        slider("audioResponse", "options.lumiereAudioResponse", (0.0, 2.0), 0.05, D.audio_response),
        "options.lumiereAudioResponseHint",
    ),
    slider("bloom", "options.lumiereBloom", (0.0, 2.0), 0.05, D.bloom),
    slider("textBloom", "options.lumiereTextBloom", (0.0, 2.0), 0.05, D.text_bloom),
];

const ATMOSPHERE: &[Item] = &[
    noted(
        slider("darkField", "options.lumiereDarkField", (0.0, 1.0), 0.01, D.dark_field),
        "options.lumiereDarkFieldHint",
    ),
    slider("fogDensity", "options.lumiereFogDensity", (0.0, 2.0), 0.05, D.fog_density),
    slider("fogOctaves", "options.lumiereFogOctaves", (2.0, 6.0), 1.0, D.fog_octaves),
    slider("moteAmount", "options.lumiereMoteAmount", (0.0, 2.0), 0.05, D.mote_amount),
    toggle("frontBokeh", "options.lumiereFrontBokeh", D.front_bokeh),
    toggle("lineArt", "options.lumiereLineArt", D.line_art),
    toggle("overlayFrame", "options.lumiereOverlayFrame", D.overlay_frame),
];

const NEIGHBORS: &[Choice] = &[
    Choice { value: "1", label_key: "options.lumiereWindowNeighborsPrev", requires: None },
    Choice { value: "2", label_key: "options.lumiereWindowNeighborsBoth", requires: None },
];

const TEXT: &[Item] = &[
    noted(toggle("textOnly", "options.lumiereTextOnly", D.text_only), "options.lumiereTextOnlyHint"),
    slider("unlitOpacity", "options.lumiereUnlitOpacity", (0.05, 0.6), 0.01, D.unlit_opacity),
    Item::Select {
        key: "windowNeighbors",
        label_key: "options.lumiereWindowNeighbors",
        hint_key: None,
        options: NEIGHBORS,
        default: "2",
    },
    slider("decay", "options.lumiereDecay", (0.0, 2.0), 0.05, D.decay),
    noted(slider("echo", "options.lumiereEcho", (0.0, 2.0), 0.05, D.echo), "options.lumiereEchoHint"),
    toggle("trails", "options.lumiereTrails", D.trails),
    noted(toggle("hideTrails", "options.lumiereHideTrails", D.hide_trails), "options.lumiereHideTrailsHint"),
    noted(
        toggle("seamlessTransitions", "options.lumiereSeamlessTransitions", D.seamless),
        "options.lumiereSeamlessTransitionsHint",
    ),
];

const THEME: &[Item] = &[
    noted(
        slider("themeColorMix", "options.lumiereThemeColorMix", (0.0, 1.0), 0.01, D.theme_color_mix),
        "options.lumiereThemeColorMixHint",
    ),
    toggle("keywordColors", "options.lumiereKeywordColors", D.keyword_colors),
    toggle("themeIcons", "options.lumiereThemeIcons", D.theme_icons),
];

const QUALITY: &[Choice] = &[
    Choice { value: "full", label_key: "options.lumiereRenderQualityFull", requires: None },
    Choice { value: "balanced", label_key: "options.lumiereRenderQualityBalanced", requires: None },
    Choice { value: "low", label_key: "options.lumiereRenderQualityLow", requires: None },
];

const PERFORMANCE: &[Item] = &[Item::Select {
    key: "renderQuality",
    label_key: "options.lumiereRenderQuality",
    hint_key: Some("options.lumiereRenderQualityHint"),
    options: QUALITY,
    default: "full",
}];

/// Groups and rows in the order of `LumiereSettingsPanel.tsx`.
const GROUPS: &[Group] = &[
    Group { id: "vis-lumiere-light", title_key: "options.lumiereLightSection", foot_key: None, items: LIGHT },
    Group {
        id: "vis-lumiere-atmosphere",
        title_key: "options.lumiereAtmosphereSection",
        foot_key: None,
        items: ATMOSPHERE,
    },
    Group { id: "vis-lumiere-text", title_key: "options.lumiereTextSection", foot_key: None, items: TEXT },
    Group { id: "vis-lumiere-theme", title_key: "options.lumiereThemeSection", foot_key: None, items: THEME },
    Group {
        id: "vis-lumiere-performance",
        title_key: "options.lumierePerformanceSection",
        foot_key: None,
        items: PERFORMANCE,
    },
];

pub fn manifest() -> Manifest {
    let mut manifest = Manifest::plain("lumiere_tuning", &[], GROUPS);
    // Theme word colours and lyric icons are not on the theme yet.
    manifest.locked = &["keywordColors", "themeIcons"];
    manifest.reset_key = Some("notifications.lumiereReset");
    manifest
}
