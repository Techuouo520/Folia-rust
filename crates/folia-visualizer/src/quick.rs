// The style-specific quick controls the player panel shows under the lyric style row (`entry.tsx`'s
// `renderQuickControls`), read from and written to each style's saved values.

use crate::registry::Style;
use crate::tuning::Values;

/// How a control looks and acts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuickKind {
    /// A lit icon button that flips one switch.
    Toggle { on: bool },
    /// A chip that cycles a small list and names the value it lands on.
    Cycle,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QuickControl {
    /// Stable id the panel sends back when it is pressed.
    pub key: &'static str,
    /// The words under or in the control.
    pub label_key: &'static str,
    /// The hover tip's first half.
    pub tip_key: &'static str,
    pub glyph: &'static str,
    pub kind: QuickKind,
}

/// `appear` presets and their labels (`APPEAR_PRESETS`).
const NEBBIA_APPEAR: [(&[&str], &str); 4] = [
    (&["write", "fade"], "options.nebbiaAppearAuto"),
    (&["write"], "options.nebbiaAppearWrite"),
    (&["fade"], "options.nebbiaAppearFade"),
    (&["fade", "breath"], "options.nebbiaAppearBreath"),
];
const NEBBIA_FADE: [(&[&str], &str); 3] = [
    (&["fog"], "options.nebbiaFadeFog"),
    (&["fingers"], "options.nebbiaFadeFingers"),
    (&["fog", "fingers"], "options.nebbiaFadeMix"),
];

/// The controls of one style, in the order the original renders them.
pub fn quick_controls(style: Style, values: &Values) -> Vec<QuickControl> {
    let manifest = style.manifest();
    let toggle = |key: &'static str, label_key: &'static str, glyph: &'static str| QuickControl {
        key,
        label_key,
        tip_key: label_key,
        glyph,
        kind: QuickKind::Toggle { on: values.flag(manifest, key) },
    };
    match style {
        Style::Classic => vec![toggle("enableWordRotation", "options.classicWordRotation", "rotate")],
        Style::Partita => vec![toggle("showGuideLines", "options.partitaGuideLines", "spline")],
        Style::Tilt => {
            let current = values.choice(manifest, "colorScheme");
            let label = manifest
                .items()
                .find_map(|item| match item {
                    crate::tuning::Item::Select { key: "colorScheme", options, .. } => {
                        options.iter().find(|choice| choice.value == current).map(|choice| choice.label_key)
                    }
                    _ => None,
                })
                .unwrap_or("options.tiltColorScheme");
            vec![QuickControl {
                key: "colorScheme",
                label_key: label,
                tip_key: "options.tiltColorScheme",
                glyph: "palette",
                kind: QuickKind::Cycle,
            }]
        }
        Style::Claddagh => vec![toggle("showAxisLine", "options.claddaghShowAxisLine", "separator")],
        Style::Lumiere => vec![
            toggle("overlayFrame", "options.lumiereOverlayFrame", "frame"),
            toggle("hideTrails", "options.lumiereHideTrails", "eye-off"),
        ],
        Style::Monet => vec![
            toggle("showDescription", "options.monetShowDescription", "align-left"),
            toggle("showPortraitDragHanger", "options.monetPortraitDragHanger", "move"),
            toggle("showAudioVisualization", "options.monetShowAudioVisualization", "audio"),
        ],
        Style::Nebbia => vec![
            QuickControl {
                key: "appear",
                label_key: nebbia_label(&NEBBIA_APPEAR, values, "appear"),
                tip_key: "options.nebbiaAppear",
                glyph: "text",
                kind: QuickKind::Cycle,
            },
            QuickControl {
                key: "fade",
                label_key: nebbia_label(&NEBBIA_FADE, values, "fade"),
                tip_key: "options.nebbiaFade",
                glyph: "fog",
                kind: QuickKind::Cycle,
            },
        ],
        Style::Cadenza | Style::Still => Vec::new(),
    }
}

fn nebbia_label(
    presets: &'static [(&'static [&'static str], &'static str)],
    values: &Values,
    key: &str,
) -> &'static str {
    let manifest = Style::Nebbia.manifest();
    let on: Vec<&str> = presets
        .iter()
        .flat_map(|(entries, _)| entries.iter().copied())
        .filter(|entry| values.member(manifest, key, entry))
        .collect();
    presets
        .iter()
        .find(|(entries, _)| entries.iter().copied().eq(on.iter().copied()))
        .map_or("options.nebbiaCustom", |(_, label)| *label)
}

/// Applies a press to the style's values; returns false where the key is not one of its controls.
pub fn apply_quick(style: Style, values: &mut Values, key: &str) -> bool {
    let manifest = style.manifest();
    match (style, key) {
        (Style::Tilt, "colorScheme") => {
            let options: Vec<&str> = manifest
                .items()
                .find_map(|item| match item {
                    crate::tuning::Item::Select { key: "colorScheme", options, .. } => {
                        Some(options.iter().map(|choice| choice.value).collect())
                    }
                    _ => None,
                })
                .unwrap_or_default();
            let current = values.choice(manifest, "colorScheme");
            let next =
                options.iter().position(|option| *option == current).map_or(0, |index| (index + 1) % options.len());
            if let Some(option) = options.get(next) {
                values.set_choice(manifest, "colorScheme", option);
            }
            true
        }
        (Style::Nebbia, "appear") => {
            cycle_members(values, &NEBBIA_APPEAR, "appear");
            true
        }
        (Style::Nebbia, "fade") => {
            cycle_members(values, &NEBBIA_FADE, "fade");
            true
        }
        (_, key)
            if matches!(manifest.items().find(|item| item.key() == key), Some(crate::tuning::Item::Switch { .. })) =>
        {
            let on = values.flag(manifest, key);
            values.set_flag(manifest, key, !on);
            true
        }
        _ => false,
    }
}

fn cycle_members(values: &mut Values, presets: &'static [(&'static [&'static str], &'static str)], key: &str) {
    let manifest = Style::Nebbia.manifest();
    let on: Vec<&str> = presets
        .iter()
        .flat_map(|(entries, _)| entries.iter().copied())
        .filter(|entry| values.member(manifest, key, entry))
        .collect();
    let next = presets
        .iter()
        .position(|(entries, _)| entries.iter().copied().eq(on.iter().copied()))
        .map_or(0, |index| (index + 1) % presets.len());
    values.set_members(manifest, key, presets[next].0);
}
