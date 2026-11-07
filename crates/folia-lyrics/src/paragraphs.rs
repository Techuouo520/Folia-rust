// Lyric paragraphs shared by the styles that cut a song into scenes (`paragraphStructure.ts`): gap and metadata
// splits, size caps and paragraph kinds.

use folia_core::Line;

use crate::{RenderHints, word_spans};

/// What a paragraph is, from its song part, length, word count and punctuation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ParagraphKind {
    Breath,
    Verse,
    Lift,
    Chorus,
    Break,
    Outro,
}

/// Where a paragraph was cut. `Intro` and `Instrumental` are the interlude paragraphs a style adds itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ParagraphBoundary {
    SongStart,
    TimeGap,
    Metadata,
    DurationCap,
    LineCap,
    Intro,
    Instrumental,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ParagraphParams {
    /// The split threshold is the median line gap times this, clamped to `[gap_min, gap_max]` seconds.
    pub gap_multiplier: f64,
    pub gap_min: f64,
    pub gap_max: f64,
    /// Paragraphs with more lines or a longer span are cut at their widest gap.
    pub max_lines: usize,
    pub max_duration: f64,
    /// The head of an oversized paragraph is cut again too.
    pub recursive_split: bool,
}

impl ParagraphParams {
    pub const DEFAULT: ParagraphParams = ParagraphParams {
        gap_multiplier: 2.5,
        gap_min: 1.25,
        gap_max: 3.5,
        max_lines: 6,
        max_duration: 18.0,
        recursive_split: true,
    };
}

/// A song line with its index in the song and its render end, which never passes the next line's start.
#[derive(Debug, Clone, Copy)]
pub struct StructureLine<'a> {
    pub source_index: usize,
    pub line: &'a Line,
    pub render_end: f64,
}

#[derive(Debug, Clone)]
pub struct ParagraphDraft<'a> {
    pub lines: Vec<StructureLine<'a>>,
    pub boundary: ParagraphBoundary,
}

/// From the first line's start to the last line's render end.
fn span(lines: &[StructureLine<'_>]) -> f64 {
    match (lines.first(), lines.last()) {
        (Some(first), Some(last)) => last.render_end - first.line.start,
        _ => 0.0,
    }
}

pub fn structure_lines(lines: &[Line]) -> Vec<StructureLine<'_>> {
    lines
        .iter()
        .enumerate()
        .map(|(source_index, line)| {
            let next = lines.get(source_index + 1).map_or(f64::INFINITY, |next| next.start);
            let render_end = line.start.max(RenderHints::of(line).render_end.min(next));
            StructureLine { source_index, line, render_end }
        })
        .collect()
}

/// The median of the positive gaps between lines (0.5 when there are none) times the multiplier, clamped.
pub fn gap_threshold(lines: &[Line], params: &ParagraphParams) -> f64 {
    let mut gaps: Vec<f64> = lines
        .windows(2)
        .map(|pair| pair[1].start - RenderHints::of(&pair[0]).render_end.min(pair[1].start))
        .filter(|gap| *gap > 0.0)
        .collect();
    gaps.sort_by(f64::total_cmp);
    let middle = gaps.len() / 2;
    let median = match gaps.len() {
        0 => 0.5,
        len if len.is_multiple_of(2) => (gaps[middle - 1] + gaps[middle]) / 2.0,
        _ => gaps[middle],
    };
    (median * params.gap_multiplier).clamp(params.gap_min, params.gap_max)
}

/// Both lines carry a block index or a song part and they differ.
pub fn metadata_changed(previous: &Line, next: &Line) -> bool {
    let block = matches!((previous.block_index, next.block_index), (Some(a), Some(b)) if a != b);
    let part = matches!((&previous.song_part, &next.song_part), (Some(a), Some(b)) if a != b);
    block || part
}

/// Cuts a paragraph over the line or duration cap at its widest gap; the head is cut again when recursive.
pub fn split_oversized<'a>(draft: ParagraphDraft<'a>, params: &ParagraphParams) -> Vec<ParagraphDraft<'a>> {
    let mut output = Vec::new();
    let mut remaining = draft;
    while remaining.lines.len() > params.max_lines
        || (remaining.lines.len() > 1 && span(&remaining.lines) > params.max_duration)
    {
        let at = split_index(&remaining.lines);
        let tail = remaining.lines.split_off(at);
        if params.recursive_split {
            output.extend(split_oversized(remaining, params));
        } else {
            output.push(remaining);
        }
        let capped = output.last().is_some_and(|head: &ParagraphDraft<'_>| head.lines.len() >= params.max_lines);
        let boundary = if capped { ParagraphBoundary::LineCap } else { ParagraphBoundary::DurationCap };
        remaining = ParagraphDraft { lines: tail, boundary };
    }
    output.push(remaining);
    output
}

/// The widest gap among lines `2..=len-2` (the first on ties), or line `min(4, len-1)` when there are none.
fn split_index(lines: &[StructureLine<'_>]) -> usize {
    let mut best: Option<(usize, f64)> = None;
    for index in 2..lines.len().saturating_sub(1) {
        let gap = lines[index].line.start - lines[index - 1].render_end;
        if gap.is_nan() {
            continue;
        }
        if best.is_none_or(|(_, widest)| gap > widest) {
            best = Some((index, gap));
        }
    }
    best.map_or(4.min(lines.len() - 1), |(index, _)| index).max(1)
}

/// Cuts where the metadata changes or a gap reaches the threshold, then splits oversized paragraphs.
pub fn draft_paragraphs<'a>(
    lines: &[StructureLine<'a>],
    threshold: f64,
    params: &ParagraphParams,
) -> Vec<ParagraphDraft<'a>> {
    let mut drafts = Vec::new();
    let mut current = ParagraphDraft { lines: Vec::new(), boundary: ParagraphBoundary::SongStart };
    for (index, line) in lines.iter().enumerate() {
        let boundary = index.checked_sub(1).map(|previous| &lines[previous]).and_then(|previous| {
            if metadata_changed(previous.line, line.line) {
                Some(ParagraphBoundary::Metadata)
            } else if line.line.start - previous.render_end >= threshold {
                Some(ParagraphBoundary::TimeGap)
            } else {
                None
            }
        });
        if let Some(boundary) = boundary
            && !current.lines.is_empty()
        {
            let finished = std::mem::replace(&mut current, ParagraphDraft { lines: Vec::new(), boundary });
            drafts.extend(split_oversized(finished, params));
        }
        current.lines.push(*line);
    }
    if !current.lines.is_empty() {
        drafts.extend(split_oversized(current, params));
    }
    drafts
}

/// Word-like segments of a line (`segmentLyricWords(line).filter(isWordLike)`).
pub fn count_word_like(line: &Line) -> usize {
    word_spans(&line.text).iter().filter(|span| !span.blank).count()
}

/// Chorus mark, then break mark, then the last paragraph is the outro, short or few words is a breath, many
/// exclamations or dense words is a lift, everything else a verse.
pub fn classify(lines: &[StructureLine<'_>], index: usize, total: usize) -> ParagraphKind {
    let part = |line: &StructureLine<'_>| line.line.song_part.as_deref().unwrap_or_default().to_lowercase();
    let marked = |words: &[&str]| lines.iter().any(|line| words.iter().any(|word| part(line).contains(word)));
    if lines.iter().any(|line| line.line.is_chorus) || marked(&["chorus", "副歌"]) {
        return ParagraphKind::Chorus;
    }
    if marked(&["bridge", "break", "間奏", "ブリッジ"]) {
        return ParagraphKind::Break;
    }
    if index + 1 == total {
        return ParagraphKind::Outro;
    }
    let duration = span(lines);
    let segments: usize = lines.iter().map(|line| count_word_like(line.line)).sum();
    let marks: usize = lines
        .iter()
        .map(|line| line.line.text.chars().filter(|c| matches!(c, '!' | '?' | '！' | '？' | '…')).count())
        .sum();
    if duration <= 3.5 || segments <= 3 {
        ParagraphKind::Breath
    } else if marks >= 2 || segments as f64 / duration.max(1.0) > 2.5 {
        ParagraphKind::Lift
    } else {
        ParagraphKind::Verse
    }
}
