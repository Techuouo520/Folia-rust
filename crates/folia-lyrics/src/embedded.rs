// Separates recognizable bilingual LRC tags before they enter the shared lyric parser.
use folia_core::Lyrics;

use crate::{Format, clock};

/// Parses embedded lyrics without promoting mixed-in translations to sung lines.
pub fn parse_embedded(content: &str, translation: &str, romanization: &str) -> Lyrics {
    let format = Format::detect(content);
    if matches!(format, Format::Lrc | Format::EnhancedLrc)
        && let Some(tracks) = split(content)
    {
        return crate::parse(
            Format::detect(&tracks.main),
            &tracks.main,
            alternate(translation, &tracks.translation),
            alternate(romanization, &tracks.romanization),
        );
    }
    crate::parse(format, content, translation, romanization)
}

struct Tracks {
    main: String,
    translation: String,
    romanization: String,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Script {
    Han,
    Kana,
    Hangul,
    Latin,
    Other,
}

struct Timed<'a> {
    raw: &'a str,
    times: Vec<f64>,
    enhanced: bool,
    blank: bool,
    script: Script,
    romanization: bool,
}

impl<'a> Timed<'a> {
    fn new(raw: &'a str) -> Self {
        let brackets = clock::scan(raw, '[', ']');
        let angles = clock::scan(raw, '<', '>');
        let enhanced = !angles.is_empty() || crate::has_inline_word_tags(raw);
        let times = if brackets.is_empty() { angles } else { brackets }.into_iter().map(|tag| tag.time).collect();
        let text = without_tags(raw);
        let script = script(&text);
        let letters = text.chars().filter(|&c| c.is_ascii_alphabetic() || ('\u{c0}'..='\u{24f}').contains(&c)).count();
        let count = text.chars().filter(|c| !c.is_whitespace()).count().max(1);
        Self {
            raw,
            times,
            enhanced,
            blank: text.trim().is_empty(),
            script,
            romanization: script == Script::Latin && letters >= 2 && letters * 100 >= count * 45,
        }
    }

    fn shares_timeline(&self, other: &Self) -> bool {
        !self.times.is_empty()
            && (self.times == other.times
                || self.times.first() == other.times.first() && (self.enhanced || other.enhanced))
    }
}

fn split(content: &str) -> Option<Tracks> {
    let lines: Vec<_> =
        crate::text_lines(content).map(str::trim).filter(|line| !line.is_empty()).map(Timed::new).collect();
    let (mut main, mut translation, mut romanization) = (Vec::new(), Vec::new(), Vec::new());
    let mut changed = false;
    let mut start = 0;
    while let Some(first) = lines.get(start) {
        let end = start + 1 + lines[start + 1..].iter().take_while(|line| first.shares_timeline(line)).count();
        let group: Vec<_> = lines[start..end].iter().filter(|line| !line.blank).collect();
        if let Some((translated, romanized)) = alternates(&group) {
            main.push(group[0].raw);
            translation.extend(translated.map(|line| line.raw));
            romanization.extend(romanized.map(|line| line.raw));
            changed = true;
        } else {
            main.extend(lines[start..end].iter().map(|line| line.raw));
        }
        start = end;
    }
    changed.then(|| Tracks {
        main: main.join("\n"),
        translation: translation.join("\n"),
        romanization: romanization.join("\n"),
    })
}

// Same-script voices and extra alternatives are ambiguous; retain the entire group in the main track.
fn alternates<'a>(group: &[&'a Timed<'a>]) -> Option<(Option<&'a Timed<'a>>, Option<&'a Timed<'a>>)> {
    let [main, rest @ ..] = group else { return None };
    if rest.is_empty() {
        return None;
    }
    let mut romanized = None;
    let mut translated = None;
    for &line in rest {
        if matches!(main.script, Script::Han | Script::Kana | Script::Hangul) && line.romanization {
            if romanized.replace(line).is_some() {
                return None;
            }
        } else if line.script != main.script {
            if translated.replace(line).is_some() {
                return None;
            }
        } else {
            return None;
        }
    }
    Some((translated, romanized))
}

fn alternate<'a>(explicit: &'a str, inferred: &'a str) -> &'a str {
    if inferred.is_empty() || !clock::scan(explicit, '[', ']').is_empty() || !clock::scan(explicit, '<', '>').is_empty()
    {
        explicit
    } else {
        inferred
    }
}

fn script(text: &str) -> Script {
    if text.chars().any(|c| ('\u{3040}'..='\u{30ff}').contains(&c)) {
        Script::Kana
    } else if text.chars().any(|c| ('\u{ac00}'..='\u{d7af}').contains(&c)) {
        Script::Hangul
    } else if text.chars().any(|c| ('\u{3400}'..='\u{9fff}').contains(&c) || ('\u{f900}'..='\u{faff}').contains(&c)) {
        Script::Han
    } else if text.chars().any(|c| c.is_ascii_alphabetic() || ('\u{c0}'..='\u{24f}').contains(&c)) {
        Script::Latin
    } else {
        Script::Other
    }
}

fn without_tags(mut text: &str) -> String {
    let mut visible = String::new();
    while let Some(ch) = text.chars().next() {
        let close = match ch {
            '[' => Some(']'),
            '<' => Some('>'),
            _ => None,
        };
        if let Some(end) = close.and_then(|close| text.find(close)) {
            text = &text[end + 1..];
        } else {
            visible.push(ch);
            text = &text[ch.len_utf8()..];
        }
    }
    visible
}
