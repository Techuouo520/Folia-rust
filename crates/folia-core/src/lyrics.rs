// Timed lyrics shared by the parsers, the player UI and the visualizers; all times are in seconds.

use serde::{Deserialize, Serialize};

/// Alternate tracks displayed inside lyric styles. The shared overlay has its own row layout.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SubtitleContentMode {
    #[default]
    Translation,
    Romanization,
    Both,
    None,
}

impl SubtitleContentMode {
    pub fn single(self, line: &Line) -> Option<&str> {
        match self {
            Self::Translation | Self::Both => line.translation.as_deref(),
            Self::Romanization => line.romanization.as_deref(),
            Self::None => None,
        }
        .map(str::trim)
        .filter(|text| !text.is_empty())
    }
}

/// Text of the interlude placeholder line inserted into long instrumental gaps.
pub const INTERLUDE_TEXT: &str = "......";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Word {
    pub text: String,
    pub start: f64,
    pub end: f64,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Line {
    pub words: Vec<Word>,
    pub start: f64,
    pub end: f64,
    pub text: String,
    pub translation: Option<String>,
    pub romanization: Option<String>,
    /// The TTML `itunes:song-part` of the line's `<div>`, such as `Verse` or `Chorus`.
    #[serde(default)]
    pub song_part: Option<String>,
    /// 1-based index of the TTML `<body>` child (`<div>` or `<p>`) holding the line.
    #[serde(default)]
    pub block_index: Option<u32>,
    /// The song part is `chorus`.
    #[serde(default)]
    pub is_chorus: bool,
}

impl Line {
    pub fn is_interlude(&self) -> bool {
        self.text == INTERLUDE_TEXT
    }
}

/// A stretch of the song a platform marks as chorus, in seconds.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ChorusRange {
    pub start: f64,
    pub end: f64,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Lyrics {
    pub lines: Vec<Line>,
    pub title: Option<String>,
    pub artist: Option<String>,
    /// True when the source carried real per-word timing rather than estimated word times.
    pub word_timed: bool,
}

impl Lyrics {
    /// Index of the line sung at `time`: the last line that has started, if any.
    pub fn line_at(&self, time: f64) -> Option<usize> {
        self.lines.partition_point(|line| line.start <= time).checked_sub(1)
    }
}
