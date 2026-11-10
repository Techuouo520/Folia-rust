// Chorus marks against the original TypeScript (fixtures/chorus.json): detection, ranges, resolve, and the
// paragraphs the marks lead to.

use std::path::Path;

use folia_core::{ChorusRange, Lyrics};
use folia_lyrics::{
    ChorusSource, Format, ParagraphParams, chorus_ranges_of, classify, detect_chorus_lines, draft_paragraphs,
    gap_threshold, mark_chorus_ranges, parse, resolve_chorus, structure_lines,
};
use folia_visualizer::styles::lumiere::Program;
use serde::Deserialize;

#[derive(Deserialize)]
struct Export {
    cases: Vec<Case>,
    existing: Existing,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Case {
    file: String,
    translation: String,
    lines: Vec<ExpectedLine>,
    detected: Vec<String>,
    online: Resolved,
    local: Resolved,
    ranges: Vec<Range>,
    by_ranges: Vec<bool>,
    per_range: Vec<Vec<bool>>,
    native: Resolved,
    plan: Plan,
}

#[derive(Deserialize)]
struct ExpectedLine {
    start: f64,
    end: f64,
    text: String,
    translation: Option<String>,
}

#[derive(Deserialize)]
struct Resolved {
    mode: String,
    chorus: Vec<bool>,
}

#[derive(Deserialize)]
struct Range {
    start: f64,
    end: f64,
}

#[derive(Deserialize)]
struct Plan {
    drafts: Vec<Draft>,
    paragraphs: Vec<Paragraph>,
    shots: Vec<Shot>,
}

#[derive(Deserialize)]
struct Draft {
    lines: Vec<usize>,
    kind: String,
}

#[derive(Deserialize)]
struct Paragraph {
    start: f64,
    end: f64,
    kind: String,
}

#[derive(Deserialize)]
struct Shot {
    kind: String,
    start: f64,
    end: f64,
}

#[derive(Deserialize)]
struct Existing {
    mode: String,
    none: String,
}

fn fixtures() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures").leak()
}

fn read(name: &str) -> String {
    if name.is_empty() { String::new() } else { std::fs::read_to_string(fixtures().join(name)).unwrap() }
}

fn export() -> Export {
    serde_json::from_str(&read("chorus.json")).unwrap()
}

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-6
}

fn mode_id(mode: ChorusSource) -> &'static str {
    match mode {
        ChorusSource::None => "none",
        ChorusSource::Existing => "existing",
        ChorusSource::Native => "native",
        ChorusSource::Text => "text",
    }
}

fn flags(lyrics: &Lyrics) -> Vec<bool> {
    lyrics.lines.iter().map(|line| line.is_chorus).collect()
}

fn ranges(case: &Case) -> Vec<ChorusRange> {
    case.ranges.iter().map(|range| ChorusRange { start: range.start, end: range.end }).collect()
}

fn parsed(case: &Case) -> Lyrics {
    let lyrics = parse(Format::Lrc, &read(&case.file), &read(&case.translation), "");
    assert_eq!(lyrics.lines.len(), case.lines.len(), "{}: line count", case.file);
    for (index, (line, want)) in lyrics.lines.iter().zip(&case.lines).enumerate() {
        assert_eq!(line.text, want.text, "{}: line {index} text", case.file);
        assert!(close(line.start, want.start) && close(line.end, want.end), "{}: line {index} times", case.file);
        assert_eq!(line.translation, want.translation, "{}: line {index} translation", case.file);
    }
    lyrics
}

#[test]
fn chorus_detection_matches_the_original() {
    for case in export().cases {
        let lyrics = parsed(&case);
        let mut detected: Vec<String> = detect_chorus_lines(&read(&case.file)).into_iter().collect();
        detected.sort();
        assert_eq!(detected, case.detected, "{}: detected texts", case.file);

        let mut online = lyrics.clone();
        let mode = resolve_chorus(Some(&mut online), &[], Some(&read(&case.file)));
        assert_eq!(
            (mode_id(mode), flags(&online)),
            (case.online.mode.as_str(), case.online.chorus.clone()),
            "{} online",
            case.file
        );

        let mut local = lyrics.clone();
        let mode = resolve_chorus(Some(&mut local), &[], None);
        assert_eq!(
            (mode_id(mode), flags(&local)),
            (case.local.mode.as_str(), case.local.chorus.clone()),
            "{} local",
            case.file
        );
        let marked = flags(&local).iter().filter(|chorus| **chorus).count();
        eprintln!(
            "{}: {marked} chorus lines (original {})",
            case.file,
            case.local.chorus.iter().filter(|c| **c).count()
        );
    }
}

#[test]
fn chorus_ranges_match_the_original() {
    for case in export().cases {
        let lyrics = parsed(&case);
        let ranges = ranges(&case);
        let mut all = lyrics.clone();
        mark_chorus_ranges(&mut all, &ranges);
        assert_eq!(flags(&all), case.by_ranges, "{}: all ranges", case.file);
        for (index, (range, want)) in ranges.iter().zip(&case.per_range).enumerate() {
            let mut one = lyrics.clone();
            mark_chorus_ranges(&mut one, std::slice::from_ref(range));
            assert_eq!(&flags(&one), want, "{}: range {index}", case.file);
        }
        let mut native = lyrics.clone();
        let mode = resolve_chorus(Some(&mut native), &ranges[..2], Some(&read(&case.file)));
        assert_eq!(
            (mode_id(mode), flags(&native)),
            (case.native.mode.as_str(), case.native.chorus.clone()),
            "{} native",
            case.file
        );
        assert_eq!(chorus_ranges_of(&native).len(), case.native.chorus.iter().filter(|c| **c).count());
    }
    let existing = export().existing;
    let mut ttml = parse(Format::Ttml, &read("lanterns.ttml"), "", "");
    let before = flags(&ttml);
    let mode = resolve_chorus(Some(&mut ttml), &[ChorusRange { start: 0.0, end: 999.0 }], None);
    assert_eq!(mode_id(mode), existing.mode);
    assert_eq!(flags(&ttml), before, "existing marks are kept as they are");
    assert_eq!(mode_id(resolve_chorus(None, &[], None)), existing.none);
}

#[test]
fn chorus_paragraphs_match_the_original() {
    for case in export().cases {
        let mut lyrics = parsed(&case);
        resolve_chorus(Some(&mut lyrics), &[], None);
        let params = ParagraphParams::DEFAULT;
        let structure = structure_lines(&lyrics.lines);
        let drafts = draft_paragraphs(&structure, gap_threshold(&lyrics.lines, &params), &params);
        assert_eq!(drafts.len(), case.plan.drafts.len(), "{}: draft count", case.file);
        for (index, (draft, want)) in drafts.iter().zip(&case.plan.drafts).enumerate() {
            let lines: Vec<usize> = draft.lines.iter().map(|line| line.source_index).collect();
            assert_eq!(lines, want.lines, "{}: draft {index} lines", case.file);
            let kind = format!("{:?}", classify(&draft.lines, index, drafts.len())).to_lowercase();
            assert_eq!(kind, want.kind, "{}: draft {index} kind", case.file);
        }
        let program = Program::compile(&lyrics, "lumiere", false);
        assert_eq!(program.spans.len(), case.plan.paragraphs.len(), "{}: paragraph count", case.file);
        for (index, (span, want)) in program.spans.iter().zip(&case.plan.paragraphs).enumerate() {
            assert!(close(span.start, want.start) && close(span.end, want.end), "{}: paragraph {index}", case.file);
            assert_eq!(format!("{:?}", span.kind).to_lowercase(), want.kind, "{}: paragraph {index} kind", case.file);
        }
        assert_eq!(program.shots.len(), case.plan.shots.len(), "{}: shot count", case.file);
        for (index, (shot, want)) in program.shots.iter().zip(&case.plan.shots).enumerate() {
            assert!(close(shot.start, want.start) && close(shot.end, want.end), "{}: shot {index}", case.file);
            assert_eq!(shot.profile_kind(), want.kind, "{}: shot {index} light position", case.file);
        }
    }
}
