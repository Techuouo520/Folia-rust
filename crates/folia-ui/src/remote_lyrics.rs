// Remote lyric selection, duplicate suppression and source fallback rules from remoteLyricText.

use folia_core::{Line, SubtitleContentMode};
use folia_lyrics::{RenderHints, active_line};
use folia_store::{RemoteAppearance, RemoteSource};

#[derive(Default)]
pub(crate) struct Presentation {
    line: Option<Line>,
    next: Option<Line>,
    singing: bool,
    leaving: Option<std::time::Instant>,
    entering: Option<std::time::Instant>,
    pub time: f64,
}

impl Presentation {
    pub fn current(&self) -> (Option<&Line>, Option<&Line>, bool) {
        (self.line.as_ref(), self.next.as_ref(), self.singing)
    }

    pub fn sync(
        &mut self,
        line: Option<&Line>,
        next: Option<&Line>,
        singing: bool,
        time: f64,
        now: std::time::Instant,
    ) {
        let same = self.line.as_ref().map(|line| (line.start, line.text.as_str()))
            == line.map(|line| (line.start, line.text.as_str()));
        if same {
            self.leaving = None;
            self.singing = singing;
            self.time = time;
            if self.next.as_ref() != next {
                self.next = next.cloned();
            }
            if self.line.as_ref() != line {
                self.line = line.cloned();
            }
            return;
        }
        let reduced = crate::motion::reduced(folia_store::MotionSurface::UiMicroMotion);
        if self.line.is_some() && !reduced {
            let started = *self.leaving.get_or_insert(now);
            if now.saturating_duration_since(started).as_millis() < 200 {
                return;
            }
        }
        self.line = line.cloned();
        self.next = next.cloned();
        self.singing = singing;
        self.time = time;
        self.leaving = None;
        self.entering = self.line.as_ref().map(|_| now);
    }

    pub fn animating(&self, now: std::time::Instant) -> bool {
        !crate::motion::reduced(folia_store::MotionSurface::UiMicroMotion)
            && (self.leaving.is_some()
                || self.entering.is_some_and(|at| now.saturating_duration_since(at).as_millis() <= 280))
    }

    pub fn pose(&self, now: std::time::Instant) -> (f32, f32) {
        if crate::motion::reduced(folia_store::MotionSurface::UiMicroMotion) {
            return (1.0, 0.0);
        }
        if let Some(at) = self.leaving {
            let progress = folia_visualizer::tween(
                0.0,
                1.0,
                now.saturating_duration_since(at).as_secs_f64(),
                0.18,
                folia_visualizer::Ease::new(0.42, 0.0, 1.0, 1.0),
            ) as f32;
            return (1.0 - progress, -6.0 * progress);
        }
        let Some(at) = self.entering else {
            return (1.0, 0.0);
        };
        let progress = folia_visualizer::tween(
            0.0,
            1.0,
            now.saturating_duration_since(at).as_secs_f64(),
            0.26,
            folia_visualizer::Ease::new(0.2, 0.8, 0.2, 1.0),
        ) as f32;
        (progress, 6.0 * (1.0 - progress))
    }
}

#[derive(Debug, PartialEq)]
pub struct Rows<'a> {
    pub main: &'a str,
    pub timed: bool,
    pub romanization: Option<&'a str>,
    pub sub: Option<&'a str>,
    pub upcoming: Option<(&'a str, Option<&'a str>)>,
}

pub fn line_at(lines: &[Line], time: f64) -> (Option<&Line>, Option<&Line>, bool) {
    let next = lines.iter().find(|line| line.start > time && !line.is_interlude());
    if let Some(line) = active_line(lines, time).map(|index| &lines[index]).filter(|line| !line.is_interlude()) {
        return (Some(line), next, true);
    }
    let previous = lines
        .iter()
        .filter(|line| line.start <= time && !line.is_interlude())
        .max_by(|left, right| RenderHints::of(left).render_end.total_cmp(&RenderHints::of(right).render_end));
    let singing = previous.zip(next).is_some_and(|(line, next)| next.start - RenderHints::of(line).render_end < 3.0);
    (previous, next, singing)
}

fn source(line: &Line, source: RemoteSource) -> Option<&str> {
    let text = match source {
        RemoteSource::Original => Some(line.text.as_str()),
        RemoteSource::Translation => line.translation.as_deref(),
        RemoteSource::Romanization => line.romanization.as_deref(),
        _ => None,
    }
    .map(str::trim)
    .filter(|text| !text.is_empty());
    text.filter(|text| source == RemoteSource::Original || *text != line.text.trim())
}

pub fn rows<'a>(
    appearance: &RemoteAppearance,
    mode: SubtitleContentMode,
    line: &'a Line,
    next: Option<&'a Line>,
) -> Rows<'a> {
    let [main_source, sub_source, next_main, next_sub] = appearance.lyric_rows;
    let main = source(line, main_source).unwrap_or(line.text.trim());
    let two = appearance.lyric_pairs == 2;
    let fallback = if two { None } else { next.map(|line| line.text.trim()) };
    let mut romanization = None;
    let sub = match sub_source {
        RemoteSource::None => None,
        RemoteSource::Auto => {
            let translation = source(line, RemoteSource::Translation).filter(|text| *text != main);
            let roman = source(line, RemoteSource::Romanization).filter(|text| *text != main);
            match mode {
                SubtitleContentMode::Translation => translation.or(fallback),
                SubtitleContentMode::Romanization => roman.or(fallback),
                SubtitleContentMode::Both => {
                    romanization = roman.filter(|text| Some(*text) != translation);
                    translation.or(if romanization.is_none() { fallback } else { None })
                }
                SubtitleContentMode::None => fallback,
            }
        }
        choice => source(line, choice).filter(|text| *text != main).or(fallback),
    };
    let upcoming = if two {
        next.map(|line| {
            let main = source(line, next_main).unwrap_or(line.text.trim());
            let sub = if next_sub == RemoteSource::Auto {
                match mode {
                    SubtitleContentMode::Translation => source(line, RemoteSource::Translation),
                    SubtitleContentMode::Romanization => source(line, RemoteSource::Romanization),
                    SubtitleContentMode::Both => source(line, RemoteSource::Translation)
                        .filter(|text| *text != main)
                        .or_else(|| source(line, RemoteSource::Romanization)),
                    SubtitleContentMode::None => None,
                }
            } else {
                source(line, next_sub)
            }
            .filter(|text| *text != main);
            (main, sub)
        })
    } else {
        None
    };
    Rows { main, timed: main == line.text.trim(), romanization, sub, upcoming }
}
