// The interface typeface: the original CSS stack's platform families, plus the weight faces they lack.

use iced::Font;
use iced::advanced::graphics::text::cosmic_text::fontdb;
use iced::advanced::graphics::text::cosmic_text::{Fallback, FontSystem, PlatformFallback};
use iced::font::Weight;
use unicode_script::Script;

pub const BODY_STACK: &[&str] = &[
    "Inter",
    "Noto Sans CJK SC",
    "Source Han Sans SC",
    "PingFang SC",
    "Hiragino Sans GB",
    "Microsoft YaHei",
    "Noto Sans JP",
    "Source Han Sans JP",
    "Arial",
    "Helvetica",
    "DejaVu Sans",
    "Liberation Sans",
];
pub const BODY: Font = Font { family: iced::font::Family::Name("Folia CSS body"), ..Font::DEFAULT };
pub const SYSTEM: Font = Font { family: iced::font::Family::Name("Folia CSS system"), ..Font::DEFAULT };
pub const VI_SYSTEM: Font = Font { family: iced::font::Family::Name("Folia CSS Vietnamese UI"), ..Font::DEFAULT };
pub const VI_BODY: Font = Font { family: iced::font::Family::Name("Folia CSS Vietnamese body"), ..Font::DEFAULT };

/// The original :lang(vi) rule puts Segoe UI before system-ui, preserving explicit custom and monospace faces.
pub fn for_vietnamese(font: Font, vietnamese: bool) -> Font {
    if !vietnamese {
        return font;
    }
    if font.family == SYSTEM.family {
        Font { family: VI_SYSTEM.family, ..font }
    } else if font.family == BODY.family {
        Font { family: VI_BODY.family, ..font }
    } else {
        font
    }
}

fn system_stack(locale: &str) -> &'static [&'static str] {
    if cfg!(target_os = "windows") {
        if locale.starts_with("zh-TW")
            || locale.starts_with("zh-Hant")
            || locale.starts_with("zh-HK")
            || locale.starts_with("zh-MO")
        {
            &["Microsoft JhengHei UI", "Segoe UI"]
        } else if locale.starts_with("zh") {
            &["Microsoft YaHei UI", "Segoe UI"]
        } else if locale.starts_with("ja") {
            &["Yu Gothic UI", "Segoe UI"]
        } else if locale.starts_with("ko") {
            &["Malgun Gothic", "Segoe UI"]
        } else {
            &["Segoe UI"]
        }
    } else if cfg!(target_os = "macos") {
        &[".AppleSystemUIFont", "SF Pro Text", "Helvetica Neue", "Helvetica"]
    } else {
        &["Noto Sans", "DejaVu Sans", "Liberation Sans"]
    }
}
const CJK_STACK: &[&str] = &[
    "Noto Sans CJK SC",
    "Source Han Sans SC",
    "PingFang SC",
    "Hiragino Sans GB",
    "Microsoft YaHei",
    "Noto Sans JP",
    "Source Han Sans JP",
];

// cosmic-text falls back to these for Han text by system locale, and one of them is what Chinese ends up in.
#[cfg(target_os = "windows")]
const CJK: &[&str] = &["Microsoft YaHei UI", "Microsoft JhengHei UI", "Yu Gothic", "Malgun Gothic", "MingLiU_HKSCS"];
#[cfg(target_os = "macos")]
const CJK: &[&str] = &["PingFang SC", "PingFang TC", "PingFang HK", "Hiragino Sans", "Apple SD Gothic Neo"];
#[cfg(not(any(target_os = "windows", target_os = "macos")))]
const CJK: &[&str] =
    &["Noto Sans CJK SC", "Noto Sans CJK TC", "Noto Sans CJK JP", "Noto Sans CJK KR", "Source Han Sans SC"];

#[cfg(target_os = "windows")]
const MONO: &[&str] = &["Consolas", "Courier New"];
#[cfg(target_os = "macos")]
const MONO: &[&str] = &["Menlo", "Monaco", "Courier New"];
#[cfg(not(any(target_os = "windows", target_os = "macos")))]
const MONO: &[&str] = &["Noto Sans Mono", "DejaVu Sans Mono", "Liberation Mono", "FreeMono"];

/// Points iced's shared text system at the original families and gives each of them every weight, so a missing
/// weight resolves inside the family like CSS does instead of falling through to whatever font has that weight.
pub fn install() {
    static INSTALLED: std::sync::Once = std::sync::Once::new();
    INSTALLED.call_once(install_once);
}

fn install_once() {
    let Ok(mut system) = iced::advanced::graphics::text::font_system().write() else {
        tracing::error!("text system lock is poisoned, the interface keeps the default fonts");
        return;
    };
    let fonts = system.raw();
    let system = system_stack(fonts.locale());
    let sans = system.iter().chain(BODY_STACK).find(|family| has_family(fonts.db(), family)).copied();
    let legacy = if cfg!(target_os = "windows") {
        &["Segoe UI", "Noto Sans CJK SC", "Source Han Sans SC", "Microsoft YaHei"][..]
    } else {
        &["Noto Sans CJK SC", "Source Han Sans SC", "PingFang SC", "Hiragino Sans GB"][..]
    };
    if let Some(family) = legacy.iter().find(|family| has_family(fonts.db(), family)) {
        fonts.db_mut().set_sans_serif_family(*family);
    }
    let mono = MONO.iter().find(|family| has_family(fonts.db(), family)).copied();
    if let Some(mono) = mono {
        fonts.db_mut().set_monospace_family(mono);
    }
    for family in system.iter().chain(legacy).chain(BODY_STACK).chain(CJK).chain(MONO) {
        cover_weights(fonts, family);
    }
    cover_weights(fonts, "Segoe UI");
    if let Some(family) = sans {
        alias_family(fonts, family, "Folia CSS system");
    }
    if let Some(body) = BODY_STACK.iter().find(|family| has_family(fonts.db(), family)) {
        alias_family(fonts, body, "Folia CSS body");
    }
    if let Some(vi) = has_family(fonts.db(), "Segoe UI").then_some("Segoe UI").or(sans) {
        alias_family(fonts, vi, "Folia CSS Vietnamese UI");
        let body = if has_family(fonts.db(), "Inter") { "Inter" } else { vi };
        alias_family(fonts, body, "Folia CSS Vietnamese body");
    }
    // Rebuild fallback caches once, before any paragraph is shaped.
    let database = fonts.db().clone();
    let locale = fonts.locale().to_owned();
    *fonts = FontSystem::new_with_locale_and_db_and_fallback(locale, database, CssFallback);
    tracing::info!(?sans, ?mono, "interface fonts");
}

struct CssFallback;

impl Fallback for CssFallback {
    fn common_fallback(&self) -> &[&'static str] {
        PlatformFallback.common_fallback()
    }
    fn forbidden_fallback(&self) -> &[&'static str] {
        PlatformFallback.forbidden_fallback()
    }
    fn script_fallback(&self, script: Script, locale: &str) -> &[&'static str] {
        match script {
            Script::Han | Script::Hiragana | Script::Katakana | Script::Hangul => CJK_STACK,
            _ => PlatformFallback.script_fallback(script, locale),
        }
    }
}

/// The interface face at `wanted`; `install` makes every weight of the families available.
pub const fn font(wanted: Weight) -> Font {
    Font { weight: wanted, ..SYSTEM }
}

/// `base` (another family, italic) at `wanted`.
pub const fn with(base: Font, wanted: Weight) -> Font {
    Font { weight: wanted, ..base }
}

fn has_family(database: &fontdb::Database, family: &str) -> bool {
    database.faces().any(|face| face.families.iter().any(|(name, _)| name == family))
}

fn alias_family(fonts: &mut FontSystem, source: &str, name: &str) {
    let aliases: Vec<_> =
        fonts.db().faces().filter(|face| face.families.iter().any(|(name, _)| name == source)).cloned().collect();
    for mut alias in aliases {
        alias.id = fontdb::ID::dummy();
        alias.families.push((name.to_owned(), fontdb::Language::English_UnitedStates));
        fonts.db_mut().push_face_info(alias);
    }
}

fn cover_weights(system: &mut iced::advanced::graphics::text::cosmic_text::FontSystem, family: &str) {
    let faces: Vec<fontdb::FaceInfo> =
        system.db().faces().filter(|face| face.families.iter().any(|(name, _)| name == family)).cloned().collect();
    if faces.is_empty() {
        return;
    }
    for weight in (100..=900).step_by(100) {
        if faces.iter().any(|face| face.weight.0 == weight && face.style == fontdb::Style::Normal) {
            continue;
        }
        let Some(source) = css_face(&faces, weight) else {
            return;
        };
        let mut alias = source.clone();
        alias.id = fontdb::ID::dummy();
        alias.weight = fontdb::Weight(weight);
        system.db_mut().push_face_info(alias);
    }
}

// CSS Fonts 4 weight matching over the weights a family actually has.
fn css_face(faces: &[fontdb::FaceInfo], target: u16) -> Option<&fontdb::FaceInfo> {
    let weights: Vec<u16> =
        faces.iter().filter(|face| face.style == fontdb::Style::Normal).map(|face| face.weight.0).collect();
    let resolved = css_weight(&weights, target)?;
    faces.iter().find(|face| face.weight.0 == resolved && face.style == fontdb::Style::Normal)
}

/// Resolves a requested CSS weight within one family's available upright faces.
pub fn css_weight(available: &[u16], target: u16) -> Option<u16> {
    let mut weights = available.to_vec();
    weights.sort_unstable();
    weights.dedup();
    if (400..=500).contains(&target) {
        weights
            .iter()
            .copied()
            .find(|weight| *weight >= target && *weight <= 500)
            .or_else(|| weights.iter().copied().rev().find(|weight| *weight < target))
            .or_else(|| weights.iter().copied().find(|weight| *weight > 500))
    } else if target < 400 {
        weights
            .iter()
            .copied()
            .rev()
            .find(|weight| *weight <= target)
            .or_else(|| weights.iter().copied().find(|weight| *weight > target))
    } else {
        weights
            .iter()
            .copied()
            .find(|weight| *weight >= target)
            .or_else(|| weights.iter().copied().rev().find(|weight| *weight < target))
    }
}
