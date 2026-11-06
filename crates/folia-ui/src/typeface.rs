// The interface typeface: the original CSS stack's platform families, plus the weight faces they lack.

use iced::Font;
use iced::advanced::graphics::text::cosmic_text::fontdb;
use iced::font::Weight;

#[cfg(target_os = "windows")]
const SANS: &[&str] = &["Segoe UI", "Noto Sans CJK SC", "Source Han Sans SC", "Microsoft YaHei"];
#[cfg(target_os = "macos")]
const SANS: &[&str] = &["Noto Sans CJK SC", "Source Han Sans SC", "PingFang SC", "Hiragino Sans GB"];
#[cfg(not(any(target_os = "windows", target_os = "macos")))]
const SANS: &[&str] = &["Noto Sans CJK SC", "Source Han Sans SC"];

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
    let Ok(mut system) = iced::advanced::graphics::text::font_system().write() else {
        tracing::error!("text system lock is poisoned, the interface keeps the default fonts");
        return;
    };
    let fonts = system.raw();
    let sans = SANS.iter().find(|family| has_family(fonts.db(), family)).copied();
    if let Some(sans) = sans {
        fonts.db_mut().set_sans_serif_family(sans);
    }
    let mono = MONO.iter().find(|family| has_family(fonts.db(), family)).copied();
    if let Some(mono) = mono {
        fonts.db_mut().set_monospace_family(mono);
    }
    for family in SANS.iter().chain(CJK).chain(MONO) {
        cover_weights(fonts, family);
    }
    tracing::info!(?sans, ?mono, "interface fonts");
}

/// The interface face at `wanted`; `install` makes every weight of the families available.
pub const fn font(wanted: Weight) -> Font {
    Font { weight: wanted, ..Font::DEFAULT }
}

/// `base` (another family, italic) at `wanted`.
pub const fn with(base: Font, wanted: Weight) -> Font {
    Font { weight: wanted, ..base }
}

fn has_family(database: &fontdb::Database, family: &str) -> bool {
    database.faces().any(|face| face.families.iter().any(|(name, _)| name == family))
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
    let mut weights: Vec<u16> = faces.iter().map(|face| face.weight.0).collect();
    weights.sort_unstable();
    weights.dedup();
    let resolved = if (400..=500).contains(&target) {
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
    }?;
    faces
        .iter()
        .find(|face| face.weight.0 == resolved && face.style == fontdb::Style::Normal)
        .or_else(|| faces.iter().find(|face| face.weight.0 == resolved))
}
