// AnimCJK stroke medians, read from `nebbia-strokes.bin.gz`.
// Generated from AnimCJK commit 99dba566b92daf45ae20d05e6c91d3f68d8619f2 the same way as
// folia-major/dev/nebbia/nebbiaStrokesPlugin.mjs: kana, then simplified Chinese, then Japanese
// kanji; only medians; y' = 900 - y; strokes outside the 1024 grid are dropped. The blob is
// little-endian NBST and then gzip. It is separate from the AGPL sources (Arphic Public License).

use std::collections::HashMap;
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};

const NAME: &str = "nebbia-strokes.bin.gz";

pub type Medians = HashMap<char, Vec<Vec<(f32, f32)>>>;

#[derive(Debug)]
pub enum StrokesError {
    Missing,
    Io(std::io::Error),
    Format(&'static str),
}

impl std::fmt::Display for StrokesError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StrokesError::Missing => f.write_str("nebbia stroke file is not present"),
            StrokesError::Io(err) => write!(f, "nebbia stroke file: {err}"),
            StrokesError::Format(detail) => write!(f, "nebbia stroke file: {detail}"),
        }
    }
}

impl std::error::Error for StrokesError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            StrokesError::Io(err) => Some(err),
            _ => None,
        }
    }
}

impl From<std::io::Error> for StrokesError {
    fn from(err: std::io::Error) -> Self {
        StrokesError::Io(err)
    }
}

/// Polylines in em space (0..1, y down) for every character that still has a median after filtering.
pub fn load_default() -> Result<Medians, StrokesError> {
    let Some(path) = locate() else { return Err(StrokesError::Missing) };
    load_file(&path)
}

pub fn load_file(path: &Path) -> Result<Medians, StrokesError> {
    let file = File::open(path)?;
    let mut decoded = Vec::new();
    flate2::read::GzDecoder::new(file).read_to_end(&mut decoded)?;
    decode(&decoded)
}

fn locate() -> Option<PathBuf> {
    if let Ok(value) = std::env::var("FOLIA_NEBBIA_STROKES") {
        let lower = value.to_ascii_lowercase();
        if lower == "off" || lower == "0" || lower == "false" {
            return None;
        }
        return Some(PathBuf::from(value));
    }
    let rel = PathBuf::from("assets").join("nebbia").join(NAME);
    if rel.is_file() {
        return Some(rel);
    }
    let exe = std::env::current_exe().ok()?;
    let beside = exe.parent()?.join("assets").join("nebbia").join(NAME);
    beside.is_file().then_some(beside)
}

fn decode(data: &[u8]) -> Result<Medians, StrokesError> {
    if data.len() < 8 || &data[0..4] != b"NBST" {
        return Err(StrokesError::Format("missing NBST header"));
    }
    let mut at = 4;
    if read_u32(data, &mut at)? != 1 {
        return Err(StrokesError::Format("unsupported version"));
    }
    let count = read_u32(data, &mut at)? as usize;
    let mut out = HashMap::with_capacity(count);
    for _ in 0..count {
        let ch = char::from_u32(read_u32(data, &mut at)?).ok_or(StrokesError::Format("bad codepoint"))?;
        let strokes = read_u16(data, &mut at)? as usize;
        let mut polys = Vec::with_capacity(strokes);
        for _ in 0..strokes {
            let points = read_u16(data, &mut at)? as usize;
            let mut median = Vec::with_capacity(points);
            for _ in 0..points {
                let x = f32::from(read_i16(data, &mut at)?) / 1024.0;
                let y = f32::from(read_i16(data, &mut at)?) / 1024.0;
                median.push((x, y));
            }
            if median.len() > 1 {
                polys.push(catmull(&median, 6));
            }
        }
        if !polys.is_empty() {
            out.insert(ch, polys);
        }
    }
    if at != data.len() {
        return Err(StrokesError::Format("trailing bytes"));
    }
    Ok(out)
}

fn catmull(pts: &[(f32, f32)], n: usize) -> Vec<(f32, f32)> {
    if pts.len() < 3 || n == 0 {
        return pts.to_vec();
    }
    let mut out = Vec::with_capacity((pts.len() - 1) * n + 1);
    for i in 0..pts.len() - 1 {
        let p0 = pts[i.saturating_sub(1)];
        let p1 = pts[i];
        let p2 = pts[i + 1];
        let p3 = pts[(i + 2).min(pts.len() - 1)];
        for k in 0..n {
            let t = k as f32 / n as f32;
            let t2 = t * t;
            let t3 = t2 * t;
            let at = |a: f32, b: f32, c: f32, d: f32| {
                0.5 * (2.0 * b
                    + (-a + c) * t
                    + (2.0 * a - 5.0 * b + 4.0 * c - d) * t2
                    + (-a + 3.0 * b - 3.0 * c + d) * t3)
            };
            out.push((at(p0.0, p1.0, p2.0, p3.0), at(p0.1, p1.1, p2.1, p3.1)));
        }
    }
    out.push(pts[pts.len() - 1]);
    out
}

fn read_u32(data: &[u8], at: &mut usize) -> Result<u32, StrokesError> {
    let bytes = read_n::<4>(data, at)?;
    Ok(u32::from_le_bytes(bytes))
}

fn read_u16(data: &[u8], at: &mut usize) -> Result<u16, StrokesError> {
    let bytes = read_n::<2>(data, at)?;
    Ok(u16::from_le_bytes(bytes))
}

fn read_i16(data: &[u8], at: &mut usize) -> Result<i16, StrokesError> {
    let bytes = read_n::<2>(data, at)?;
    Ok(i16::from_le_bytes(bytes))
}

fn read_n<const N: usize>(data: &[u8], at: &mut usize) -> Result<[u8; N], StrokesError> {
    let end = at.checked_add(N).ok_or(StrokesError::Format("truncated"))?;
    if end > data.len() {
        return Err(StrokesError::Format("truncated"));
    }
    let mut bytes = [0; N];
    bytes.copy_from_slice(&data[*at..end]);
    *at = end;
    Ok(bytes)
}
