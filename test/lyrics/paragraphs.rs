// Paragraph structure and the Lumiere paragraph plan against the original TypeScript (fixtures/paragraphs.json).

use std::path::Path;

use folia_core::{Line, Lyrics};
use folia_lyrics::{
    Format, ParagraphBoundary, ParagraphKind, ParagraphParams, classify, draft_paragraphs, gap_threshold, parse,
    structure_lines,
};
use folia_visualizer::styles::lumiere::Program;
use serde::Deserialize;

#[derive(Deserialize)]
struct Case {
    name: String,
    lines: Vec<ExpectedLine>,
    threshold: f64,
    drafts: Vec<Draft>,
    paragraphs: Vec<Paragraph>,
    shots: Vec<Shot>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ExpectedLine {
    start: f64,
    end: f64,
    render_end: f64,
    text: String,
    song_part: Option<String>,
    block_index: Option<u32>,
    is_chorus: bool,
}

#[derive(Deserialize)]
struct Draft {
    lines: Vec<usize>,
    boundary: String,
    kind: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Paragraph {
    start: f64,
    end: f64,
    lyric_end: f64,
    boundary: String,
    kind: String,
    lines: Vec<usize>,
    opening: bool,
}

#[derive(Deserialize)]
struct Shot {
    kind: String,
    start: f64,
    end: f64,
    bridge: bool,
    lines: Vec<usize>,
}

fn fixtures() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures").leak()
}

fn cases() -> Vec<Case> {
    let json = std::fs::read_to_string(fixtures().join("paragraphs.json")).unwrap();
    serde_json::from_str(&json).unwrap()
}

/// The case's lines parsed by folia-lyrics; `#sung` drops the interlude lines, as the export does.
fn lyrics_of(name: &str) -> Lyrics {
    if name == "instrumental" {
        return Lyrics::default();
    }
    let (file, variant) = name.split_once('#').map_or((name, ""), |(file, variant)| (file, variant));
    let format = if file.ends_with(".ttml") { Format::Ttml } else { Format::Lrc };
    let content = std::fs::read_to_string(fixtures().join(file)).unwrap();
    let mut lyrics = parse(format, &content, "", "");
    if variant == "sung" {
        lyrics.lines.retain(|line| !line.is_interlude());
    }
    lyrics
}

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-6
}

fn boundary_id(boundary: ParagraphBoundary) -> &'static str {
    match boundary {
        ParagraphBoundary::SongStart => "song-start",
        ParagraphBoundary::TimeGap => "time-gap",
        ParagraphBoundary::Metadata => "metadata",
        ParagraphBoundary::DurationCap => "duration-cap",
        ParagraphBoundary::LineCap => "line-cap",
        ParagraphBoundary::Intro => "intro",
        ParagraphBoundary::Instrumental => "instrumental",
    }
}

fn kind_id(kind: ParagraphKind) -> &'static str {
    match kind {
        ParagraphKind::Breath => "breath",
        ParagraphKind::Verse => "verse",
        ParagraphKind::Lift => "lift",
        ParagraphKind::Chorus => "chorus",
        ParagraphKind::Break => "break",
        ParagraphKind::Outro => "outro",
    }
}

fn check_lines(name: &str, lines: &[Line], expected: &[ExpectedLine]) {
    assert_eq!(lines.len(), expected.len(), "{name}: line count");
    for (index, (line, structure)) in lines.iter().zip(structure_lines(lines)).enumerate() {
        let want = &expected[index];
        assert_eq!(line.text, want.text, "{name}: line {index} text");
        assert!(close(line.start, want.start) && close(line.end, want.end), "{name}: line {index} times");
        assert!(close(structure.render_end, want.render_end), "{name}: line {index} render end");
        assert_eq!(line.song_part, want.song_part, "{name}: line {index} song part");
        assert_eq!(line.block_index, want.block_index, "{name}: line {index} block index");
        assert_eq!(line.is_chorus, want.is_chorus, "{name}: line {index} chorus");
    }
}

#[test]
fn paragraph_structure_matches_the_original() {
    let cases = cases();
    assert!(cases.len() >= 7);
    for case in &cases {
        let lyrics = lyrics_of(&case.name);
        check_lines(&case.name, &lyrics.lines, &case.lines);
        let params = ParagraphParams::DEFAULT;
        let threshold = gap_threshold(&lyrics.lines, &params);
        assert!(close(threshold, case.threshold), "{}: threshold {threshold} vs {}", case.name, case.threshold);
        let structure = structure_lines(&lyrics.lines);
        let drafts = draft_paragraphs(&structure, threshold, &params);
        assert_eq!(drafts.len(), case.drafts.len(), "{}: draft count", case.name);
        for (index, (draft, want)) in drafts.iter().zip(&case.drafts).enumerate() {
            let lines: Vec<usize> = draft.lines.iter().map(|line| line.source_index).collect();
            assert_eq!(lines, want.lines, "{}: draft {index} lines", case.name);
            assert_eq!(boundary_id(draft.boundary), want.boundary, "{}: draft {index} boundary", case.name);
            let kind = classify(&draft.lines, index, drafts.len());
            assert_eq!(kind_id(kind), want.kind, "{}: draft {index} kind", case.name);
        }
    }
}

#[test]
fn lumiere_plan_matches_the_original() {
    for case in cases() {
        let lyrics = lyrics_of(&case.name);
        let program = Program::compile(&lyrics, "lumiere", false);
        let name = &case.name;
        assert_eq!(program.spans.len(), case.paragraphs.len(), "{name}: paragraph count");
        for (index, (span, want)) in program.spans.iter().zip(&case.paragraphs).enumerate() {
            assert!(close(span.start, want.start) && close(span.end, want.end), "{name}: paragraph {index} range");
            assert!(close(span.lyric_end, want.lyric_end), "{name}: paragraph {index} lyric end");
            assert_eq!(boundary_id(span.boundary), want.boundary, "{name}: paragraph {index} boundary");
            assert_eq!(kind_id(span.kind), want.kind, "{name}: paragraph {index} kind");
            assert_eq!(span.opening, want.opening, "{name}: paragraph {index} opening");
            let lines: Vec<usize> = (span.lines[0]..span.lines[1]).collect();
            assert_eq!(lines, want.lines, "{name}: paragraph {index} lines");
        }
        assert_eq!(program.shots.len(), case.shots.len(), "{name}: shot count");
        for (index, (shot, want)) in program.shots.iter().zip(&case.shots).enumerate() {
            assert!(close(shot.start, want.start) && close(shot.end, want.end), "{name}: shot {index} range");
            assert_eq!(shot.bridge, want.bridge, "{name}: shot {index} bridge");
            let lines = &shot.lines[..usize::from(shot.line_count)];
            assert_eq!(lines, want.lines.as_slice(), "{name}: shot {index} lines");
            assert_eq!(shot.profile_kind(), want.kind, "{name}: shot {index} light position");
        }
    }
}
