// Typefaces for the lyric and subtitle settings: the built-in families, families installed on this machine, and
// the weight mapping the original `resolveThemeFontWeight` uses.

use std::collections::BTreeSet;
use std::sync::Mutex;

use folia_store::{FontSettings, FontStyle};
use iced::Font;
use iced::font::{Family, Weight};

/// Every family the shared text system knows, sorted, without duplicates.
pub fn installed_families() -> Vec<String> {
    let Ok(mut system) = iced::advanced::graphics::text::font_system().write() else {
        tracing::warn!("text system lock is poisoned, the font picker is empty");
        return Vec::new();
    };
    let mut families = BTreeSet::new();
    for face in system.raw().db().faces() {
        for (name, _) in &face.families {
            let name = name.trim();
            if !name.is_empty() {
                families.insert(name.to_owned());
            }
        }
    }
    families.into_iter().collect()
}

/// A family name as `Family::Name` needs it: leaked once per name for the life of the app.
pub fn intern(name: &str) -> &'static str {
    static NAMES: Mutex<Vec<(&'static str, String)>> = Mutex::new(Vec::new());
    let Ok(mut names) = NAMES.lock() else {
        return "";
    };
    if let Some((leaked, _)) = names.iter().find(|(_, held)| held == name) {
        return leaked;
    }
    let leaked: &'static str = Box::leak(name.to_owned().into_boxed_str());
    names.push((leaked, name.to_owned()));
    leaked
}

/// The built-in families the original theme offers (`fontStyle`).
pub fn builtin(style: FontStyle) -> Family {
    match style {
        FontStyle::Sans | FontStyle::Custom => Family::SansSerif,
        FontStyle::Serif => Family::Serif,
        FontStyle::Mono => Family::Monospace,
    }
}

/// A saved face: its built-in family, or the chosen system family, falling back to the first installed fallback.
pub fn resolve(settings: &FontSettings) -> Font {
    let family = match settings.style {
        FontStyle::Custom => settings
            .family
            .as_deref()
            .filter(|family| !family.trim().is_empty())
            .map(intern)
            .map(Family::Name)
            .or_else(|| settings.fallback.first().map(|family| Family::Name(intern(family))))
            .unwrap_or(Family::SansSerif),
        style => builtin(style),
    };
    Font { family, weight: settings.weight.map(weight).unwrap_or(Weight::Normal), ..Font::DEFAULT }
}

/// `resolveThemeFontWeight`: a 100–900 number as the nearest named weight.
pub fn weight(value: u16) -> Weight {
    match value {
        0..=149 => Weight::Thin,
        150..=249 => Weight::ExtraLight,
        250..=349 => Weight::Light,
        350..=449 => Weight::Normal,
        450..=549 => Weight::Medium,
        550..=649 => Weight::Semibold,
        650..=749 => Weight::Bold,
        750..=849 => Weight::ExtraBold,
        _ => Weight::Black,
    }
}

/// The stored weight of a slider value.
pub fn stored_weight(value: f32) -> u16 {
    (value.round().clamp(100.0, 900.0) as u16 / 10) * 10
}
