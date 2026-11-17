// Validate and sanitize generated theme pairs before they can enter the cache or UI.

use folia_core::{Rgb, WordColor, parse_color};
use folia_store::{DualTheme, StoredTheme};
use serde_json::Value;

use crate::Error;

pub fn theme_prompt(title: &str, lyrics: &str) -> String {
    format!("{}\n\n{}", include_str!("theme_prompt.txt"), source_prompt(title, lyrics))
}

pub(crate) fn source_prompt(title: &str, lyrics: &str) -> String {
    let title_only = lyrics.trim().is_empty();
    let source = if title_only { title } else { lyrics };
    let snippet: String = source
        .chars()
        .scan(0, |length, ch| {
            *length += ch.len_utf16();
            (*length <= 2000).then_some(ch)
        })
        .collect();
    format!(
        "Source: {}\nMusic type: unknown\nSong title: {title}\nSource snippet:\n{snippet}",
        if title_only { "song title, lyrics unavailable" } else { "lyrics" }
    )
}

pub fn theme_json(theme: &DualTheme) -> Result<String, Error> {
    let hex = |color: Rgb| format!("#{:02x}{:02x}{:02x}", color.r, color.g, color.b);
    let side = |theme: &StoredTheme| {
        serde_json::json!({
            "name": theme.name, "description": theme.description,
            "backgroundColor": hex(theme.background), "primaryColor": hex(theme.primary),
            "accentColor": hex(theme.accent), "secondaryColor": hex(theme.secondary),
            "wordColors": theme.word_colors, "lyricsIcons": theme.lyrics_icons,
            "fontStyle": "sans", "animationIntensity": "normal"
        })
    };
    Ok(serde_json::to_string_pretty(&serde_json::json!({"light": side(&theme.light), "dark": side(&theme.dark)}))?)
}

pub fn parse_theme(text: &str) -> Result<DualTheme, Error> {
    let text = text.trim();
    let text = if text.starts_with("```") {
        text.split_once('\n').and_then(|(_, body)| body.strip_suffix("```")).ok_or(Error::InvalidResponse)?.trim()
    } else {
        text
    };
    let value: Value = serde_json::from_str(text).map_err(|_| Error::InvalidResponse)?;
    if !value["light"].is_object() || !value["dark"].is_object() {
        return Err(Error::InvalidResponse);
    }
    let fallback = default_theme();
    Ok(DualTheme { light: sanitize(&value["light"], fallback.light), dark: sanitize(&value["dark"], fallback.dark) })
}

pub fn default_theme() -> DualTheme {
    let light = StoredTheme {
        name: "AI Light".into(),
        background: Rgb::hex(0xffffff),
        primary: Rgb::hex(0x111827),
        accent: Rgb::hex(0x2563eb),
        secondary: Rgb::hex(0x475569),
        ..StoredTheme::default()
    };
    let dark = StoredTheme {
        name: "AI Dark".into(),
        background: Rgb::hex(0x0f172a),
        primary: Rgb::hex(0xf8fafc),
        accent: Rgb::hex(0x7dd3fc),
        secondary: Rgb::hex(0xcbd5e1),
        ..StoredTheme::default()
    };
    DualTheme { light, dark }
}

fn sanitize(value: &Value, mut theme: StoredTheme) -> StoredTheme {
    let color =
        |key: &str, fallback: Rgb| value[key].as_str().and_then(|value| parse_color(value.trim())).unwrap_or(fallback);
    if let Some(name) = value["name"].as_str().map(str::trim).filter(|name| !name.is_empty()) {
        theme.name = name.chars().take(128).collect();
    }
    theme.background = color("backgroundColor", theme.background);
    theme.primary = color("primaryColor", theme.primary);
    theme.accent = color("accentColor", theme.accent);
    theme.secondary = color("secondaryColor", theme.secondary);
    theme.description = value["description"].as_str().map(|text| text.trim().chars().take(1024).collect());
    let accent = format!("#{:02x}{:02x}{:02x}", theme.accent.r, theme.accent.g, theme.accent.b);
    theme.word_colors = value["wordColors"]
        .as_array()
        .into_iter()
        .flatten()
        .take(256)
        .filter_map(|entry| {
            let word = entry["word"].as_str()?.trim();
            if word.is_empty() {
                return None;
            }
            let color = entry["color"].as_str().filter(|value| parse_color(value).is_some()).unwrap_or(&accent);
            Some(WordColor { word: word.chars().take(256).collect(), color: color.to_owned() })
        })
        .collect();
    theme.lyrics_icons = value["lyricsIcons"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .take(12)
        .map(|text| text.chars().take(80).collect())
        .collect();
    theme
}
