// Finds a track's lyrics: sidecar files next to it first, then lyrics embedded in its tags.

use std::fs;
use std::path::Path;

use encoding_rs::{Encoding, GB18030};
use folia_core::Lyrics;
use lofty::prelude::*;
use lofty::tag::TagItem;

/// Sidecar formats in the order the original app prefers them; `<stem>.t.<ext>` holds a translation.
const SIDECAR_EXTENSIONS: [&str; 6] = ["lrc", "vtt", "ttml", "qrc", "yrc", "krc"];

pub fn lyrics_for(track_path: &Path) -> Option<Lyrics> {
    sidecar(track_path).or_else(|| embedded(track_path))
}

fn sidecar(track_path: &Path) -> Option<Lyrics> {
    SIDECAR_EXTENSIONS.iter().find_map(|&extension| {
        let content = read_text(&track_path.with_extension(extension))?;
        let translation = read_text(&track_path.with_extension(format!("t.{extension}"))).unwrap_or_default();
        Some(folia_lyrics::parse_file(extension, &content, &translation)).filter(|lyrics| !lyrics.lines.is_empty())
    })
}

fn embedded(track_path: &Path) -> Option<Lyrics> {
    let file = lofty::read_from_path(track_path)
        .inspect_err(|error| tracing::debug!(error = error as &dyn std::error::Error, "cannot read embedded lyrics"))
        .ok()?;
    let items = file.primary_tag().into_iter().chain(file.tags()).find_map(|tag| {
        let items: Vec<_> = tag.items().filter(|item| lyric_text(item).is_some()).collect();
        (!items.is_empty()).then_some(items)
    })?;
    let main = items
        .iter()
        .position(|item| original(item))
        .or_else(|| items.iter().position(|item| !translated(item) && !romanized(item)))
        .or_else(|| items.iter().position(|item| !translation_label(item) && !romanized(item)))
        .or_else(|| items.iter().position(|item| !romanized(item)))
        .unwrap_or(0);
    let translation = items
        .iter()
        .enumerate()
        .find(|(index, item)| *index != main && translation_label(item) && !romanized(item))
        .or_else(|| {
            items.iter().enumerate().find(|(index, item)| *index != main && translated(item) && !romanized(item))
        })
        .or_else(|| items.iter().enumerate().find(|(index, item)| *index != main && !romanized(item)))
        .map(|(index, _)| index);
    let romanization =
        items.iter().enumerate().find(|(index, item)| *index != main && romanized(item)).map(|(index, _)| index);
    let text = |index: Option<usize>| {
        index.and_then(|index| items.get(index)).and_then(|item| lyric_text(item)).unwrap_or_default()
    };
    let lyrics = folia_lyrics::parse_embedded(text(Some(main)), text(translation), text(romanization));
    (!lyrics.lines.is_empty()).then_some(lyrics)
}

fn lyric_text(item: &TagItem) -> Option<&str> {
    matches!(item.key(), ItemKey::Lyrics | ItemKey::UnsyncLyrics)
        .then(|| item.value().text())
        .flatten()
        .filter(|text| !text.trim().is_empty())
}

fn original(item: &TagItem) -> bool {
    let description = item.description().to_lowercase();
    description.contains("original") || description.contains("原文")
}

fn translated(item: &TagItem) -> bool {
    translation_label(item) || item.lang().eq_ignore_ascii_case(b"chi") || item.lang().eq_ignore_ascii_case(b"zho")
}

fn translation_label(item: &TagItem) -> bool {
    let description = item.description().to_lowercase();
    description.contains("trans") || description.contains("翻译") || description.contains("译文")
}

fn romanized(item: &TagItem) -> bool {
    let description = item.description().to_lowercase();
    ["roman", "romaji", "pinyin", "音译", "罗马"].iter().any(|name| description.contains(name))
}

/// Reads a text file in UTF-8, UTF-16 with a BOM, or GB18030 as the fallback for legacy Chinese files.
pub fn read_text(path: &Path) -> Option<String> {
    let bytes = fs::read(path).ok()?;
    Some(decode_text(&bytes))
}

pub fn decode_text(bytes: &[u8]) -> String {
    if let Some((encoding, bom_len)) = Encoding::for_bom(bytes) {
        return encoding.decode_without_bom_handling(&bytes[bom_len..]).0.into_owned();
    }
    match std::str::from_utf8(bytes) {
        Ok(text) => text.to_owned(),
        Err(_) => GB18030.decode_without_bom_handling(bytes).0.into_owned(),
    }
}
