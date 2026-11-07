// Chorus marks for lyrics without them (`chorusDetector.ts`, `chorusEffects.ts`, `chorusResolver.ts`): platform
// ranges first, otherwise the most repeated line texts.

use std::collections::{HashMap, HashSet};

use folia_core::{ChorusRange, INTERLUDE_TEXT, Lyrics};

use crate::is_js_space;

/// Which source marked the chorus (`ResolvedProviderLyricsChorus.mode`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChorusSource {
    /// There were no lyrics.
    None,
    /// The lyrics came with chorus marks.
    Existing,
    /// The platform's chorus ranges.
    Native,
    /// The most repeated line texts.
    Text,
}

/// `detectChorusLines`: the line texts of an LRC-style text repeated most often, when that is more than once. Time
/// tags are removed first; blank, interlude and one-unit lines are not counted.
pub fn detect_chorus_lines(source: &str) -> HashSet<String> {
    let mut counts: HashMap<String, usize> = HashMap::new();
    for line in source.split('\n') {
        let stripped = strip_time_tags(line);
        let text = stripped.trim_matches(is_js_space);
        if text.is_empty() || text == INTERLUDE_TEXT || text.encode_utf16().count() < 2 {
            continue;
        }
        *counts.entry(text.to_owned()).or_default() += 1;
    }
    let most = counts.values().copied().max().unwrap_or(0);
    if most <= 1 {
        return HashSet::new();
    }
    counts.into_iter().filter(|(_, count)| *count == most).map(|(text, _)| text).collect()
}

/// Removes every `[mm:ss.xx]` tag (`/\[(\d{2,3}):(\d{2})[.:](\d{2,3})\]/g`).
fn strip_time_tags(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let mut rest = line;
    while let Some(at) = rest.find('[') {
        out.push_str(&rest[..at]);
        match time_tag_len(&rest[at..]) {
            Some(len) => rest = &rest[at + len..],
            None => {
                out.push('[');
                rest = &rest[at + 1..];
            }
        }
    }
    out.push_str(rest);
    out
}

/// Byte length of a time tag at the start of `text`.
fn time_tag_len(text: &str) -> Option<usize> {
    let bytes = text.as_bytes();
    let digits = |from: usize| bytes[from..].iter().take_while(|byte| byte.is_ascii_digit()).count();
    let minutes = digits(1);
    let mut at = 1 + minutes;
    if !(2..=3).contains(&minutes) || bytes.get(at) != Some(&b':') || digits(at + 1) != 2 {
        return None;
    }
    at += 3;
    if !matches!(bytes.get(at), Some(b'.' | b':')) {
        return None;
    }
    let fraction = digits(at + 1);
    at += 1 + fraction;
    ((2..=3).contains(&fraction) && bytes.get(at) == Some(&b']')).then_some(at + 1)
}

/// `applyDetectedChorusEffects`: marks the lines whose trimmed text is one of `source`'s most repeated lines.
pub fn mark_chorus_text(lyrics: &mut Lyrics, source: &str) {
    let chorus = detect_chorus_lines(source);
    if chorus.is_empty() {
        return;
    }
    for line in &mut lyrics.lines {
        if chorus.contains(line.text.trim_matches(is_js_space)) {
            line.is_chorus = true;
        }
    }
}

/// `applyChorusRangesByTime`: marks every line overlapping a range (touching ends do not overlap).
pub fn mark_chorus_ranges(lyrics: &mut Lyrics, ranges: &[ChorusRange]) {
    for line in &mut lyrics.lines {
        if ranges.iter().any(|range| line.start < range.end && line.end > range.start) {
            line.is_chorus = true;
        }
    }
}

/// The marked lines as ranges, which `createLyricsSetter` keeps for the song.
pub fn chorus_ranges_of(lyrics: &Lyrics) -> Vec<ChorusRange> {
    lyrics
        .lines
        .iter()
        .filter(|line| line.is_chorus)
        .map(|line| ChorusRange { start: line.start, end: line.end })
        .collect()
}

/// `resolveProviderLyricsChorus`: keeps existing marks, else marks by `ranges`, else by the repeated lines of
/// `source` (the platform's raw text) or, when that is blank, of the lines themselves.
pub fn resolve_chorus(lyrics: Option<&mut Lyrics>, ranges: &[ChorusRange], source: Option<&str>) -> ChorusSource {
    let Some(lyrics) = lyrics else { return ChorusSource::None };
    if lyrics.lines.iter().any(|line| line.is_chorus) {
        return ChorusSource::Existing;
    }
    if !ranges.is_empty() {
        mark_chorus_ranges(lyrics, ranges);
        return ChorusSource::Native;
    }
    let source = source.map(|text| text.trim_matches(is_js_space)).filter(|text| !text.is_empty());
    let text = match source {
        Some(text) => text.to_owned(),
        None => lyrics.lines.iter().map(|line| format!("[00:00.00]{}", line.text)).collect::<Vec<_>>().join("\n"),
    };
    mark_chorus_text(lyrics, &text);
    ChorusSource::Text
}
