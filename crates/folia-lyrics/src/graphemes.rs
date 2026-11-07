// Grapheme-level lyric data: grapheme ranges, per-grapheme timing and word spans counted in graphemes.

use std::ops::Range;

use folia_core::Line;
use icu_segmenter::GraphemeClusterSegmenter;

use crate::layout::segmenter;

/// Byte ranges of the user-perceived characters of `text` (`splitLyricGraphemes`).
pub fn graphemes(text: &str) -> Vec<Range<usize>> {
    let boundaries: Vec<usize> = GraphemeClusterSegmenter::new().segment_str(text).collect();
    boundaries.windows(2).map(|pair| pair[0]..pair[1]).filter(|range| !range.is_empty()).collect()
}

/// When one grapheme is sung, in seconds.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GraphemeTiming {
    pub start: f64,
    pub end: f64,
}

/// Per-grapheme timing of `line.text` (`buildLineGraphemeTimeline`): each timed word is found in the line and split
/// evenly over its graphemes; graphemes before a word take its start, ones no word covers take the last time reached.
pub fn grapheme_timeline(line: &Line) -> Vec<GraphemeTiming> {
    let ranges = graphemes(&line.text);
    if ranges.is_empty() {
        return Vec::new();
    }
    if line.words.is_empty() {
        return even(ranges.len(), line.start, line.end);
    }
    let texts: Vec<&str> = ranges.iter().map(|range| &line.text[range.clone()]).collect();
    let mut timeline: Vec<Option<GraphemeTiming>> = vec![None; texts.len()];
    let mut cursor = 0;
    let mut last = line.start;
    for word in &line.words {
        let word_ranges = graphemes(&word.text);
        if word_ranges.is_empty() {
            continue;
        }
        let pieces: Vec<&str> = word_ranges.iter().map(|range| &word.text[range.clone()]).collect();
        let start = find(&texts, &pieces, cursor).unwrap_or(cursor);
        let end = (start + pieces.len()).min(texts.len());
        for slot in timeline.iter_mut().take(start).skip(cursor) {
            *slot = Some(GraphemeTiming { start: word.start, end: word.start });
        }
        let timings = even(pieces.len(), word.start, word.end);
        for (offset, timing) in timings.iter().take(end.saturating_sub(start)).enumerate() {
            timeline[start + offset] = Some(*timing);
            last = last.max(timing.end);
        }
        cursor = cursor.max(end);
    }
    timeline.into_iter().map(|timing| timing.unwrap_or(GraphemeTiming { start: last, end: last })).collect()
}

fn even(count: usize, start: f64, end: f64) -> Vec<GraphemeTiming> {
    let unit = (end - start).max(0.0) / count as f64;
    (0..count)
        .map(|index| GraphemeTiming {
            start: start + unit * index as f64,
            end: if index + 1 == count { end } else { start + unit * (index + 1) as f64 },
        })
        .collect()
}

fn find(source: &[&str], target: &[&str], from: usize) -> Option<usize> {
    if target.len() > source.len() {
        return None;
    }
    (from..=source.len() - target.len()).find(|&index| source[index..index + target.len()] == *target)
}

/// One segmenter word of a line, as a grapheme range. `blank` is whitespace, punctuation or symbols.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WordSpan {
    pub start: usize,
    pub end: usize,
    pub blank: bool,
}

/// The line cut into words counted in graphemes (`segmentWords` over `Intl.Segmenter`'s word granularity).
pub fn word_spans(text: &str) -> Vec<WordSpan> {
    let mut spans = Vec::new();
    let mut previous = 0;
    let mut cursor = 0;
    for (boundary, kind) in segmenter().segment_str(text).iter_with_word_type() {
        let segment = &text[previous..boundary];
        previous = boundary;
        let length = graphemes(segment).len();
        if length == 0 {
            continue;
        }
        spans.push(WordSpan { start: cursor, end: cursor + length, blank: !kind.is_word_like() });
        cursor += length;
    }
    spans
}
