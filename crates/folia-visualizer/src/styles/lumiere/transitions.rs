// Resolves the original paragraph transitions from the compiled song and playback time.
// Adapted from Folia lumiereSceneFrames/lumiereTransitions, copyright (c) 2026 chthollyphile, AGPL-3.0-only.

use super::program::Program;

#[derive(Clone, Copy, PartialEq)]
enum Kind {
    LightsOut,
    FlareCut,
    FocusPull,
}

pub(super) struct Transition {
    kind: Kind,
    start: f64,
    end: f64,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct Frame {
    pub index: usize,
    pub alpha: f32,
    pub scale: f32,
    pub blur: f32,
    pub light: f32,
}

impl Frame {
    fn identity(index: usize) -> Self {
        Self { index, alpha: 1.0, scale: 1.0, blur: 0.0, light: 1.0 }
    }
}

pub(super) fn compile(program: &Program) -> Vec<Transition> {
    let mut previous = None;
    program
        .spans
        .windows(2)
        .enumerate()
        .map(|(index, spans)| {
            let mut choices = [Kind::LightsOut, Kind::FlareCut, Kind::FocusPull];
            let count = if let Some(previous) = previous {
                let mut slot = 0;
                for kind in [Kind::LightsOut, Kind::FlareCut, Kind::FocusPull] {
                    if kind != previous {
                        choices[slot] = kind;
                        slot += 1;
                    }
                }
                slot
            } else {
                choices.len()
            };
            let hash = format!("{}:{index}:transition", program.seed).chars().fold(0_i32, |hash, character| {
                let mut units = [0; 2];
                hash.wrapping_mul(31).wrapping_add(i32::from(character.encode_utf16(&mut units)[0]))
            });
            let kind = choices[hash.unsigned_abs() as usize % count];
            previous = Some(kind);
            let gap = spans[1].start - spans[0].lyric_end;
            let duration = if kind == Kind::FocusPull {
                (if gap > 0.0 { gap * 0.5 } else { 0.45 }).clamp(0.35, 0.6)
            } else {
                (if gap > 0.0 { gap * 0.6 } else { 0.8 }).clamp(0.5, 1.1)
            };
            Transition { kind, start: spans[0].start.max(spans[0].end - duration), end: spans[0].end }
        })
        .collect()
}

fn smooth(value: f64) -> f32 {
    let t = value.clamp(0.0, 1.0);
    (t * t * (3.0 - 2.0 * t)) as f32
}

fn optical(kind: Kind, near: f32, index: usize) -> Frame {
    match kind {
        Kind::LightsOut => Frame { alpha: 1.0 - near, ..Frame::identity(index) },
        Kind::FlareCut => Frame {
            alpha: 1.0 - 0.35 * near * near,
            scale: 1.0 + 0.03 * near,
            blur: 7.0 * near * near,
            ..Frame::identity(index)
        },
        Kind::FocusPull => Frame { scale: 1.0 + 0.015 * near, blur: 12.0 * near, ..Frame::identity(index) },
    }
}

fn exit(transitions: &[Transition], index: usize, time: f64) -> Frame {
    let Some(out) = transitions.get(index).filter(|out| time >= out.start) else {
        return Frame::identity(index);
    };
    let near = smooth((time - out.start) / (out.end - out.start).max(0.001));
    let mut frame = optical(out.kind, near, index);
    if out.kind == Kind::LightsOut {
        frame.alpha = 1.0;
        frame.light = 1.0 - smooth((time - out.start) / (out.end - out.start).max(0.05));
    }
    frame
}

pub(super) fn frames(program: &Program, transitions: &[Transition], time: f64, enabled: bool) -> [Option<Frame>; 2] {
    if program.spans.is_empty() {
        return [None, None];
    }
    let index = if program.seamless { 0 } else { program.paragraph(time) };
    if program.seamless {
        return [None, Some(Frame::identity(index))];
    }
    let current = exit(transitions, index, time);
    if !enabled {
        return [None, Some(Frame { light: current.light, ..Frame::identity(index) })];
    }
    let Some(previous) = index.checked_sub(1).and_then(|index| transitions.get(index)) else {
        return [None, Some(current)];
    };
    let duration = if previous.kind == Kind::FocusPull {
        (previous.end - previous.start).clamp(0.3, 0.6)
    } else {
        (previous.end - previous.start).clamp(0.4, 0.9)
    };
    let progress = (time - program.spans[index].start) / duration;
    if !(0.0..1.0).contains(&progress) {
        return [None, Some(current)];
    }
    let weight = smooth(progress);
    let enter = optical(previous.kind, 1.0 - weight, index);
    let mut old = exit(transitions, index - 1, previous.end);
    old.alpha *= 1.0 - weight;
    old.light = exit(transitions, index - 1, time).light;
    let current = Frame {
        alpha: enter.alpha.min(weight) * current.alpha,
        scale: enter.scale * current.scale,
        blur: enter.blur.max(current.blur),
        ..current
    };
    [(old.alpha > 0.002).then_some(old), Some(current)]
}
