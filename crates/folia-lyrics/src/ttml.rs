// TTML lyrics (Apple Music / AMLL style): timed syllable spans, inline translation and romanization, head translations.

use std::collections::HashMap;

use folia_core::{BackgroundVocal, Line, Lyrics, Word};
use quick_xml::Reader;
use quick_xml::events::{BytesStart, Event};

use crate::{align, words};

#[derive(Default)]
struct Syllable {
    text: String,
    start: f64,
    end: f64,
    space_after: bool,
}

#[derive(Default)]
struct DraftLine {
    key: Option<String>,
    start: Option<f64>,
    end: Option<f64>,
    syllables: Vec<Syllable>,
    plain: String,
    translation: String,
    romanization: String,
    song_part: Option<String>,
    block_index: Option<u32>,
    agent_id: Option<String>,
    background: Vec<DraftLine>,
}

/// Where text inside the document currently goes.
#[derive(Clone, PartialEq)]
enum Target {
    Ignore,
    Line,
    Syllable,
    Translation,
    Romanization,
    HeadText(HeadKind, String),
    Background(usize, Box<Target>),
}

#[derive(Clone, Copy, PartialEq)]
enum HeadKind {
    Translation,
    Romanization,
}

#[derive(Default)]
struct Document {
    line_timed: bool,
    lines: Vec<DraftLine>,
    head_translations: HashMap<String, String>,
    head_romanizations: HashMap<String, String>,
    /// Stack depth of `<body>` once it opens.
    body_depth: Option<usize>,
    /// `<body>` children seen so far and the song part of the current `<div>`, as AMLL's `parseBody` counts them.
    block: u32,
    part: Option<String>,
}

pub fn parse(content: &str) -> Lyrics {
    match read(content.trim_start_matches('\u{feff}')) {
        Ok(document) => build(document),
        Err(err) => {
            tracing::warn!(error = &err as &dyn std::error::Error, "cannot parse TTML lyrics");
            Lyrics::default()
        }
    }
}

fn read(content: &str) -> Result<Document, quick_xml::Error> {
    let mut reader = Reader::from_str(content);
    let mut document = Document::default();
    let mut stack: Vec<Target> = Vec::new();
    let mut line: Option<DraftLine> = None;
    let mut head_kind = None;

    loop {
        match reader.read_event()? {
            Event::Start(element) => {
                enter_block(&element, &mut document, stack.len());
                let target = open(&element, &mut document, &mut line, &mut head_kind, stack.last());
                stack.push(target);
            }
            Event::Empty(element) => {
                enter_block(&element, &mut document, stack.len());
                open(&element, &mut document, &mut line, &mut head_kind, stack.last());
            }
            Event::End(element) => {
                stack.pop();
                if element.local_name().as_ref() == "p"
                    && let Some(finished) = line.take()
                {
                    document.lines.push(finished);
                }
            }
            Event::Text(text) => push_text(&text.xml10_content(), stack.last(), &mut line, &mut document),
            Event::CData(data) => push_text(&data.xml10_content(), stack.last(), &mut line, &mut document),
            Event::GeneralRef(reference) => {
                let resolved = match reference.resolve_char_ref()? {
                    Some(c) => c.to_string(),
                    None => entity(&reference.xml10_content()).to_owned(),
                };
                push_text(&resolved, stack.last(), &mut line, &mut document);
            }
            Event::Eof => break,
            _ => {}
        }
    }
    Ok(document)
}

/// Counts `<body>` children: each `<div>` or `<p>` directly under it starts a block; a `<div>` carries its song part.
fn enter_block(element: &BytesStart<'_>, document: &mut Document, depth: usize) {
    let name = element.local_name();
    if name.as_ref() == "body" {
        document.body_depth = Some(depth);
        return;
    }
    if document.body_depth.map(|body| body + 1) != Some(depth) {
        return;
    }
    match name.as_ref() {
        "div" => {
            document.block += 1;
            document.part = attribute(element, "song-part").or_else(|| attribute(element, "songPart"));
        }
        "p" => {
            document.block += 1;
            document.part = None;
        }
        _ => {}
    }
}

/// Handles an opening tag and returns where the text inside it goes.
fn open(
    element: &BytesStart<'_>,
    document: &mut Document,
    line: &mut Option<DraftLine>,
    head_kind: &mut Option<HeadKind>,
    parent: Option<&Target>,
) -> Target {
    let attr = |name: &str| attribute(element, name);
    match element.local_name().as_ref() {
        "tt" => {
            document.line_timed = attr("timing").is_some_and(|timing| timing.eq_ignore_ascii_case("line"));
            Target::Ignore
        }
        "translations" | "translation" => {
            *head_kind = Some(HeadKind::Translation);
            Target::Ignore
        }
        "transliterations" | "transliteration" => {
            *head_kind = Some(HeadKind::Romanization);
            Target::Ignore
        }
        "text" => match (*head_kind, attr("for")) {
            (Some(kind), Some(key)) => Target::HeadText(kind, key),
            _ => Target::Ignore,
        },
        "p" => {
            *line = Some(DraftLine {
                key: attr("key"),
                start: attr("begin").and_then(|t| time(&t)),
                end: attr("end").and_then(|t| time(&t)),
                song_part: document.part.clone().filter(|part| !part.is_empty()),
                block_index: document.body_depth.map(|_| document.block),
                agent_id: attr("agent"),
                ..DraftLine::default()
            });
            Target::Line
        }
        "span" => match parent {
            Some(Target::HeadText(kind, key)) => Target::HeadText(*kind, key.clone()),
            _ => line.as_mut().map_or(Target::Ignore, |line| span(element, line, parent)),
        },
        _ => Target::Ignore,
    }
}

fn span(element: &BytesStart<'_>, line: &mut DraftLine, parent: Option<&Target>) -> Target {
    if let Some(Target::Background(index, parent)) = parent {
        return line
            .background
            .get_mut(*index)
            .map_or(Target::Ignore, |vocal| Target::Background(*index, Box::new(span(element, vocal, Some(parent)))));
    }
    let attr = |name: &str| attribute(element, name);
    match (attr("role").as_deref(), parent) {
        (Some("x-bg"), _) => {
            let index = line.background.len();
            line.background.push(DraftLine {
                start: attr("begin").and_then(|t| time(&t)),
                end: attr("end").and_then(|t| time(&t)),
                agent_id: attr("agent"),
                ..DraftLine::default()
            });
            Target::Background(index, Box::new(Target::Line))
        }
        (Some("x-translation"), _) => Target::Translation,
        (Some("x-roman"), _) => Target::Romanization,
        (Some(_), _) => Target::Ignore,
        (None, Some(Target::HeadText(kind, key))) => Target::HeadText(*kind, key.clone()),
        (None, Some(Target::Line)) => {
            let (Some(start), Some(end)) = (attr("begin").and_then(|t| time(&t)), attr("end").and_then(|t| time(&t)))
            else {
                return Target::Line;
            };
            line.syllables.push(Syllable { start, end, ..Syllable::default() });
            Target::Syllable
        }
        (None, Some(other)) => other.clone(),
        (None, None) => Target::Ignore,
    }
}

fn push_text(text: &str, target: Option<&Target>, line: &mut Option<DraftLine>, document: &mut Document) {
    match target {
        Some(Target::HeadText(kind, key)) => {
            let map = match kind {
                HeadKind::Translation => &mut document.head_translations,
                HeadKind::Romanization => &mut document.head_romanizations,
            };
            map.entry(key.clone()).or_default().push_str(text);
        }
        Some(target) => {
            let Some(line) = line else { return };
            push_line_text(text, target, line);
        }
        None => {}
    }
}

fn push_line_text(text: &str, target: &Target, line: &mut DraftLine) {
    match target {
        Target::Background(index, target) => {
            if let Some(vocal) = line.background.get_mut(*index) {
                push_line_text(text, target, vocal);
            }
        }
        Target::Syllable => {
            if let Some(syllable) = line.syllables.last_mut() {
                syllable.text.push_str(text);
            }
        }
        Target::Line if text.trim().is_empty() => {
            if let Some(syllable) = line.syllables.last_mut() {
                syllable.space_after = true;
            }
        }
        Target::Line => line.plain.push_str(text),
        Target::Translation => line.translation.push_str(text),
        Target::Romanization => line.romanization.push_str(text),
        _ => {}
    }
}

fn build(document: Document) -> Lyrics {
    let word_timed = !document.line_timed && document.lines.iter().any(|line| !line.syllables.is_empty());
    let lines = document
        .lines
        .into_iter()
        .filter_map(|draft| {
            let start = draft.start.or_else(|| draft.syllables.first().map(|s| s.start))?;
            let end = draft.end.or_else(|| draft.syllables.last().map(|s| s.end)).unwrap_or(start).max(start);
            let text = if draft.syllables.is_empty() {
                draft.plain.trim().to_owned()
            } else {
                syllable_text(&draft.syllables)
            };
            if text.is_empty() {
                return None;
            }
            let words =
                if draft.syllables.is_empty() { words::spread(&text, start, end) } else { group(&draft.syllables) };
            let from_head = |map: &HashMap<String, String>| draft.key.as_ref().and_then(|key| map.get(key)).cloned();
            let non_empty = |text: String| Some(text.trim().to_owned()).filter(|text| !text.is_empty());
            let is_chorus = draft.song_part.as_ref().is_some_and(|part| part.trim().eq_ignore_ascii_case("chorus"));
            let background_vocals =
                draft.background.into_iter().filter_map(|vocal| background(vocal, start, end)).collect();
            Some(Line {
                words,
                start,
                end,
                text,
                translation: non_empty(draft.translation).or_else(|| from_head(&document.head_translations)),
                romanization: non_empty(draft.romanization).or_else(|| from_head(&document.head_romanizations)),
                song_part: draft.song_part,
                block_index: draft.block_index,
                is_chorus,
                agent_id: draft.agent_id,
                background_vocals,
            })
        })
        .collect();
    Lyrics { lines: align::finish(fold_agents(lines), &[], &[]), word_timed, ..Lyrics::default() }
}

fn background(draft: DraftLine, fallback_start: f64, fallback_end: f64) -> Option<BackgroundVocal> {
    let start = draft.start.or_else(|| draft.syllables.first().map(|word| word.start)).unwrap_or(fallback_start);
    let end = draft.end.or_else(|| draft.syllables.last().map(|word| word.end)).unwrap_or(fallback_end).max(start);
    let text = if draft.syllables.is_empty() { draft.plain.trim().to_owned() } else { syllable_text(&draft.syllables) };
    if text.is_empty() {
        return None;
    }
    let words = if draft.syllables.is_empty() { words::spread(&text, start, end) } else { group(&draft.syllables) };
    let non_empty = |text: String| Some(text.trim().to_owned()).filter(|text| !text.is_empty());
    Some(BackgroundVocal {
        words,
        start,
        end,
        text,
        translation: non_empty(draft.translation),
        romanization: non_empty(draft.romanization),
        agent_id: draft.agent_id,
    })
}

fn fold_agents(lines: Vec<Line>) -> Vec<Line> {
    let mut main: Vec<Line> = Vec::new();
    for line in lines {
        let owner = main.iter_mut().rev().find(|candidate| {
            candidate.agent_id.is_some()
                && line.agent_id.is_some()
                && candidate.agent_id != line.agent_id
                && line.start < candidate.end
                && line.end > candidate.start
                && (line.block_index.is_none()
                    || candidate.block_index.is_none()
                    || line.block_index == candidate.block_index)
        });
        if let Some(owner) = owner {
            owner.background_vocals.push(BackgroundVocal {
                words: line.words,
                start: line.start,
                end: line.end,
                text: line.text,
                translation: line.translation,
                romanization: line.romanization,
                agent_id: line.agent_id,
            });
            owner.background_vocals.extend(line.background_vocals);
        } else {
            main.push(line);
        }
    }
    main
}

fn syllable_text(syllables: &[Syllable]) -> String {
    let mut text = String::new();
    for syllable in syllables {
        text.push_str(&syllable.text);
        if syllable.space_after {
            text.push(' ');
        }
    }
    text.trim().to_owned()
}

/// Joins syllables of one Latin word into a single word, as the original does, so layouts do not break inside words.
fn group(syllables: &[Syllable]) -> Vec<Word> {
    let mut words: Vec<Word> = Vec::new();
    let mut joining = false;
    for (i, syllable) in syllables.iter().enumerate() {
        match words.last_mut() {
            Some(word) if joining => {
                word.text.push_str(&syllable.text);
                word.end = syllable.end;
            }
            _ => words.push(Word { text: syllable.text.clone(), start: syllable.start, end: syllable.end }),
        }
        joining = syllables.get(i + 1).is_some_and(|next| {
            !syllable.space_after && western_joinable(&syllable.text) && western_joinable(&next.text)
        });
    }
    words.retain(|word| !word.text.is_empty());
    words
}

fn western_joinable(text: &str) -> bool {
    let text = text.trim();
    let latin = |c: char| c.is_alphabetic() && (u32::from(c) < 0x250 || ('\u{1e00}'..='\u{1eff}').contains(&c));
    !text.is_empty()
        && !text
            .chars()
            .any(|c| matches!(c, '\u{4e00}'..='\u{9fa5}' | '\u{3040}'..='\u{30ff}' | '\u{ac00}'..='\u{d7af}'))
        && text.chars().any(|c| latin(c) || c.is_numeric() || matches!(c, '\'' | '’' | '`' | '-'))
}

/// `hh:mm:ss.fff`, `mm:ss.fff`, `ss.fff`, or a number with an `s` / `ms` suffix, in seconds.
fn time(value: &str) -> Option<f64> {
    let value = value.trim();
    if let Some(ms) = value.strip_suffix("ms") {
        return ms.parse::<f64>().ok().map(|ms| ms / 1000.0);
    }
    if let Some(seconds) = value.strip_suffix('s') {
        return seconds.parse().ok();
    }
    value.split(':').try_fold(0.0, |total, part| Some(total * 60.0 + part.parse::<f64>().ok()?))
}

fn attribute(element: &BytesStart<'_>, local: &str) -> Option<String> {
    element
        .attributes()
        .flatten()
        .find(|attr| attr.key.local_name().as_ref() == local)
        .and_then(|attr| attr.normalized_value(quick_xml::XmlVersion::Implicit1_0).ok().map(|value| value.into_owned()))
}

fn entity(name: &str) -> &'static str {
    match name {
        "amp" => "&",
        "lt" => "<",
        "gt" => ">",
        "quot" => "\"",
        "apos" => "'",
        _ => "",
    }
}
