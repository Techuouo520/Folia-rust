// Line wrapping for the lyric window (lineWrap.ts): the frame band, four layouts per line, fitting and wrapping.

use folia_lyrics::WordSpan;
use folia_render::glyphs::shape;

use super::glyph_line::LYRIC_FONT;

const FRAME_CLEARANCE_X: f32 = 0.04;
const FRAME_CLEARANCE_TOP: f32 = 0.035;
const SUBTITLE_CLEARANCE: f32 = 0.2;
const SINGLE_MIN_FIT: f32 = 0.8;
const MAX_LINES: usize = 2;
const ROW_PITCH: f32 = 1.6;
const COLUMN_PITCH: f32 = 1.6;
const OPENING: &[char] = &['(', '[', '{', '（', '【', '《', '「', '『', '〈', '〔', '［', '“', '‘'];

/// The area inside the frame's corner brackets, in height units (x from the left, y from the top).
#[derive(Clone, Copy)]
pub struct Band {
    pub left: f32,
    pub right: f32,
    pub top: f32,
    pub bottom: f32,
}

/// `frameInsets`: the corner brackets' margins in logical pixels.
pub fn frame_insets(width: f32, height: f32) -> (f32, f32) {
    ((width * 0.065).max(30.0), (height * 0.085).max(30.0))
}

pub fn frame_band(width: f32, height: f32) -> Band {
    let (pad_x, pad_y) = frame_insets(width, height);
    let left = pad_x / height + FRAME_CLEARANCE_X;
    let top = pad_y / height + FRAME_CLEARANCE_TOP;
    Band { left, right: width / height - left, top, bottom: (top + 0.2).max(1.0 - SUBTITLE_CLEARANCE) }
}

/// One layout of a line: glyph centers from the block center, the spot path, and the block size (logical pixels).
pub struct Variant {
    pub points: Vec<[f32; 2]>,
    pub spots: Vec<[f32; 2]>,
    pub along: f32,
    pub across: f32,
    pub lines: usize,
    pub ink_width: f32,
    pub ink_height: f32,
}

/// `[orientation 0 across / 1 down][0 one line / 1 wrapped]`.
pub type Flow = [[Variant; 2]; 2];

pub struct FlowGlyph<'a> {
    pub text: &'a str,
    pub scale: f32,
    pub advance: f32,
    pub upright: bool,
    pub jag: f32,
}

/// Width of `text` at `px` with letter spacing after every grapheme, the last included (pretext's natural width).
pub fn natural(text: &str, px: f32, spacing: f32) -> f32 {
    shape(text, px, LYRIC_FONT).width + spacing * px * folia_lyrics::graphemes(text).len() as f32
}

#[derive(Clone, Copy, PartialEq)]
enum Kind {
    Word,
    Punct,
    Space,
}

#[derive(Clone, Copy)]
struct Span {
    start: usize,
    end: usize,
    kind: Kind,
}

#[derive(Clone, Copy)]
enum Piece {
    Unit { start: usize, end: usize },
    Space { start: usize },
}

fn blank(glyphs: &[FlowGlyph<'_>], index: usize) -> bool {
    glyphs[index].text.trim().is_empty()
}

/// `spansOf`: words, the characters no word covers, and whitespace at word ends as spans of their own.
fn spans_of(glyphs: &[FlowGlyph<'_>], words: &[WordSpan]) -> Vec<Span> {
    let mut ranges = Vec::new();
    let mut cursor = 0;
    for word in words {
        let start = cursor.max(word.start.min(glyphs.len()));
        let end = word.end.min(glyphs.len());
        ranges.extend((cursor..start).map(|index| (index, index + 1, false)));
        if end > start {
            ranges.push((start, end, !word.blank));
        }
        cursor = cursor.max(end);
    }
    ranges.extend((cursor..glyphs.len()).map(|index| (index, index + 1, false)));
    let mut spans = Vec::new();
    for (mut start, mut end, word) in ranges {
        let lead_from = start;
        while start < end && blank(glyphs, start) {
            start += 1;
        }
        spans.extend((lead_from..start).map(|index| Span { start: index, end: index + 1, kind: Kind::Space }));
        let tail_to = end;
        while end > start && blank(glyphs, end - 1) {
            end -= 1;
        }
        if end > start {
            spans.push(Span { start, end, kind: if word { Kind::Word } else { Kind::Punct } });
        }
        spans.extend((end..tail_to).map(|index| Span { start: index, end: index + 1, kind: Kind::Space }));
    }
    spans
}

fn text_of(glyphs: &[FlowGlyph<'_>], start: usize, end: usize) -> String {
    glyphs[start..end].iter().map(|glyph| glyph.text).collect()
}

/// `unitsOf`: words with the punctuation that touches them; opening marks stick to the next word.
fn units_of(glyphs: &[FlowGlyph<'_>], spans: &[Span]) -> Vec<Piece> {
    let mut pieces: Vec<Piece> = Vec::new();
    let mut open: Option<(usize, usize)> = None;
    for (index, span) in spans.iter().enumerate() {
        if span.kind == Kind::Space {
            if let Some((start, end)) = open.take() {
                pieces.push(Piece::Unit { start, end });
            }
            pieces.push(Piece::Space { start: span.start });
            continue;
        }
        let next = spans.get(index + 1);
        if span.kind == Kind::Punct {
            let text = text_of(glyphs, span.start, span.end);
            if text.chars().all(|c| OPENING.contains(&c)) && next.is_some_and(|next| next.kind != Kind::Space) {
                open = Some(open.map_or((span.start, span.end), |(start, _)| (start, span.end)));
                continue;
            }
            if open.is_none()
                && let Some(Piece::Unit { end, .. }) = pieces.last_mut()
                && *end == span.start
            {
                *end = span.end;
                continue;
            }
        }
        let start = open.take().map_or(span.start, |(start, _)| start);
        pieces.push(Piece::Unit { start, end: span.end });
    }
    if let Some((start, end)) = open {
        pieces.push(Piece::Unit { start, end });
    }
    pieces
}

/// `splitLongUnits`: a unit longer than `limit` falls apart into characters, punctuation kept on the one before.
fn split_long(glyphs: &[FlowGlyph<'_>], pieces: &[Piece], advances: &[f32], limit: f32) -> Vec<Piece> {
    let mut out = Vec::with_capacity(pieces.len());
    for piece in pieces {
        let Piece::Unit { start, end, .. } = *piece else {
            out.push(*piece);
            continue;
        };
        let extent: f32 = advances[start..end].iter().sum();
        if extent <= limit || end - start < 2 {
            out.push(*piece);
            continue;
        }
        let first = out.len();
        for (index, glyph) in glyphs.iter().enumerate().take(end).skip(start) {
            let letter = glyph.text.chars().any(char::is_alphanumeric);
            if !letter
                && out.len() > first
                && let Some(Piece::Unit { end, .. }) = out.last_mut()
            {
                *end = index + 1;
                continue;
            }
            out.push(Piece::Unit { start: index, end: index + 1 });
        }
    }
    out
}

struct Row {
    /// `(unit start, unit end, offset of the unit's left edge)`.
    units: Vec<(usize, usize, f32)>,
    width: f32,
}

/// Greedy line breaking over atomic units with collapsed spaces between them (pretext's rich-inline walk).
fn walk(units: &[(usize, usize, f32, f32)], max_width: f32) -> Vec<Row> {
    let mut rows: Vec<Row> = Vec::new();
    for &(start, end, gap, extent) in units {
        match rows.last_mut() {
            Some(row) if row.width + gap + extent <= max_width + 1e-3 => {
                row.units.push((start, end, row.width + gap));
                row.width += gap + extent;
            }
            _ => rows.push(Row { units: vec![(start, end, 0.0)], width: extent }),
        }
    }
    rows
}

/// `breakLines`: each glyph's row and its center along the row, and each row's length.
fn break_lines(
    glyphs: &[FlowGlyph<'_>],
    pieces: &[Piece],
    advances: &[f32],
    hero_px: f32,
    spacing: f32,
    lines: usize,
) -> (Vec<usize>, Vec<f32>, Vec<f32>) {
    let mut units = Vec::new();
    let mut gap = 0.0;
    let mut widest: f32 = 0.0;
    let mut widest_gap: f32 = 0.0;
    for piece in pieces {
        match *piece {
            Piece::Space { start } => {
                let px = hero_px * glyphs[start].scale;
                gap = natural("a a", px, spacing) - natural("aa", px, spacing);
                widest_gap = widest_gap.max(gap);
            }
            Piece::Unit { start, end, .. } => {
                let extent: f32 = advances[start..end].iter().sum();
                units.push((start, end, gap, extent));
                widest = widest.max(extent);
                gap = 0.0;
            }
        }
    }
    let mut rows = walk(&units, f32::INFINITY);
    if lines > 1 && rows.len() == 1 && rows[0].units.len() > 1 {
        let total = rows[0].width;
        let mut target = (total + widest) / 2.0 + widest_gap;
        if walk(&units, target).len() > lines {
            target = total / 2.0 + widest + widest_gap;
        }
        rows = walk(&units, target);
        if rows.len() > lines {
            let rest = rows.split_off(lines - 1);
            let mut merged = Row { units: Vec::new(), width: 0.0 };
            for (index, row) in rest.into_iter().enumerate() {
                let shift = if index > 0 { merged.width + widest_gap } else { 0.0 };
                merged.units.extend(row.units.iter().map(|&(start, end, offset)| (start, end, offset + shift)));
                merged.width = shift + row.width;
            }
            rows.push(merged);
        }
    }
    let mut row = vec![usize::MAX; glyphs.len()];
    let mut offset = vec![0.0; glyphs.len()];
    for (r, line) in rows.iter().enumerate() {
        let mut previous_end = 0.0;
        for &(start, end, left) in &line.units {
            let mut before = start;
            while before > 0 && row[before - 1] == usize::MAX && blank(glyphs, before - 1) {
                before -= 1;
                row[before] = r;
                offset[before] = (previous_end + left) / 2.0;
            }
            let mut inner = left;
            for index in start..end {
                row[index] = r;
                offset[index] = inner + advances[index] / 2.0;
                inner += advances[index];
            }
            previous_end = inner;
        }
    }
    for index in 1..glyphs.len() {
        if row[index] == usize::MAX && row[index - 1] != usize::MAX {
            row[index] = row[index - 1];
            offset[index] = offset[index - 1] + advances[index - 1] / 2.0;
        }
    }
    for index in (0..glyphs.len()).rev() {
        if row[index] != usize::MAX {
            continue;
        }
        let next = (index + 1 < glyphs.len()).then(|| (row[index + 1], offset[index + 1] - advances[index + 1] / 2.0));
        let (r, at) = next.filter(|(r, _)| *r != usize::MAX).unwrap_or((0, 0.0));
        row[index] = r;
        offset[index] = at.max(0.0);
    }
    let mut lengths: Vec<f32> = rows.iter().map(|line| line.width).collect();
    if lengths.is_empty() {
        lengths.push(0.0);
    }
    (row, offset, lengths)
}

/// `flowLine`: the four layouts of a line. `limits` (logical pixels) only decide which words must split into letters.
pub fn flow_line(glyphs: &[FlowGlyph<'_>], words: &[WordSpan], hero_px: f32, spacing: f32, limits: [f32; 2]) -> Flow {
    let spans = spans_of(glyphs, words);
    let base = units_of(glyphs, &spans);
    let mut horizontal = vec![0.0; glyphs.len()];
    for span in &spans {
        let px = hero_px * glyphs[span.start].scale;
        let width = natural(&text_of(glyphs, span.start, span.end), px, spacing);
        let canvas: f32 = glyphs[span.start..span.end].iter().map(|glyph| glyph.advance).sum();
        for index in span.start..span.end {
            horizontal[index] = if canvas > 0.0 {
                width * glyphs[index].advance / canvas
            } else {
                width / (span.end - span.start) as f32
            };
        }
    }
    let vertical: Vec<f32> = glyphs
        .iter()
        .enumerate()
        .map(|(index, glyph)| if glyph.upright { hero_px * glyph.scale * (1.0 + spacing) } else { horizontal[index] })
        .collect();
    let variant = |orient: usize, lines: usize| {
        let advances = if orient == 0 { &horizontal } else { &vertical };
        let pieces = split_long(glyphs, &base, advances, limits[orient]);
        let (row, offset, lengths) = break_lines(glyphs, &pieces, advances, hero_px, spacing, lines);
        place(glyphs, advances, &row, &offset, &lengths, orient, hero_px)
    };
    [[variant(0, 1), variant(0, MAX_LINES)], [variant(1, 1), variant(1, MAX_LINES)]]
}

fn place(
    glyphs: &[FlowGlyph<'_>],
    advances: &[f32],
    row: &[usize],
    offset: &[f32],
    lengths: &[f32],
    orient: usize,
    hero_px: f32,
) -> Variant {
    let count = lengths.len();
    let along = lengths.iter().copied().fold(0.0, f32::max);
    let pitch = if orient == 0 { ROW_PITCH } else { COLUMN_PITCH } * hero_px;
    let mut points = Vec::with_capacity(glyphs.len());
    let mut spots = Vec::with_capacity(glyphs.len());
    let (mut ink_x, mut ink_y): (f32, f32) = (0.0, 0.0);
    for (index, glyph) in glyphs.iter().enumerate() {
        let r = row[index].min(count - 1);
        let size = hero_px * glyph.scale;
        let shown = !glyph.text.trim().is_empty();
        if orient == 0 {
            let x = offset[index] - lengths[r] / 2.0;
            let row_y = (r as f32 - (count as f32 - 1.0) / 2.0) * pitch;
            let point = [x, row_y + (1.0 - glyph.scale) * hero_px * 0.32 + glyph.jag];
            spots.push([x, row_y]);
            if shown {
                ink_x = ink_x.max(point[0].abs() + advances[index] / 2.0);
                ink_y = ink_y.max(point[1].abs() + size / 2.0);
            }
            points.push(point);
        } else {
            let column_x = ((count as f32 - 1.0) / 2.0 - r as f32) * pitch;
            let y = offset[index] - lengths[r] / 2.0;
            let point = [column_x + glyph.jag * 0.8, y];
            spots.push([column_x, y]);
            if shown {
                ink_x = ink_x.max(point[0].abs() + size / 2.0);
                ink_y = ink_y.max(point[1].abs() + advances[index] / 2.0);
            }
            points.push(point);
        }
    }
    Variant {
        points,
        spots,
        along,
        across: (count as f32 - 1.0) * pitch + hero_px,
        lines: count,
        ink_width: ink_x * 2.0,
        ink_height: ink_y * 2.0,
    }
}

/// How much more a line `along` long must shrink at `scale` to fit `budget` (never grows).
pub fn fit_scale(along: f32, budget: f32, scale: f32) -> f32 {
    (budget / (scale * along).max(1e-6)).min(1.0)
}

/// Wrap when one line at `SINGLE_MIN_FIT` still does not fit and the line can wrap.
pub fn should_wrap(flow: &[Variant; 2], budget: f32, scale: f32) -> bool {
    flow[1].lines > 1 && scale * flow[0].along * SINGLE_MIN_FIT > budget
}

/// `center` moved into `[lo, hi]` for a block `extent` long; centered when it cannot fit.
pub fn clamp_into(center: f32, extent: f32, lo: f32, hi: f32) -> f32 {
    if extent >= hi - lo {
        return (lo + hi) / 2.0;
    }
    center.max(lo + extent / 2.0).min(hi - extent / 2.0)
}
