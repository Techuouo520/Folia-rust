// Glyph skeletons for the fog writing: AnimCJK stroke medians when the data file has the character,
// otherwise the original fallback (rasterize with the lyric font stack at weight 600, thin with
// Zhang-Suen and trace). Both load on background threads against iced's shared font system.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock, mpsc};

use iced::advanced::graphics::text::cosmic_text::{
    Align, Attrs, Buffer, Family, Metrics, Shaping, SwashCache, SwashContent, SwashImage, Weight,
};
use iced::advanced::graphics::text::font_system;

/// Lyric stack from the original `BUILTIN_FONT_STACKS.sans`. Built-in themes have no custom family,
/// so `resolveThemeFontStack` is this list and `resolveThemeFontWeight(theme, 600)` is 600.
const LYRIC_STACK: &[&str] = &[
    "Inter",
    "Noto Sans CJK SC",
    "Noto Sans JP",
    "Source Han Sans SC",
    "PingFang SC",
    "Hiragino Sans GB",
    "Microsoft YaHei",
    "Helvetica Neue",
    "Arial",
];
const LYRIC_WEIGHT: Weight = Weight(600);

const SK: usize = 96;

/// One stroke's centre line in em units (0..1, y down) with its running arc length.
#[derive(Clone)]
pub struct Stroke {
    pub pts: Vec<(f32, f32)>,
    pub cum: Vec<f32>,
    pub len: f32,
}

#[derive(Clone)]
pub struct Ink {
    pub strokes: Vec<Stroke>,
    pub total: f32,
}

impl Stroke {
    /// The point and its normal at arc length `s`, as the original `pointAt`.
    pub fn point_at(&self, s: f32) -> [f32; 4] {
        let mut i = 1;
        while i < self.pts.len() - 1 && self.cum[i] < s {
            i += 1;
        }
        let a = self.pts[i - 1];
        let b = self.pts[i];
        let len = (self.cum[i] - self.cum[i - 1]).max(1e-6);
        let k = ((s - self.cum[i - 1]) / len).clamp(0.0, 1.0);
        let dx = (b.0 - a.0) / len;
        let dy = (b.1 - a.1) / len;
        [a.0 + (b.0 - a.0) * k, a.1 + (b.1 - a.1) * k, -dy, dx]
    }
}

/// Queue skeletons for every character in the song. Later calls for the same text do nothing.
/// Stroke medians, when the data file is present, are loaded on their own thread.
pub fn warm(lyrics: &folia_core::Lyrics) {
    start_stroke_load();
    let id = text_id(lyrics);
    let mut seen = warmed().lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    if !seen.insert(id) {
        return;
    }
    drop(seen);
    let mut chars = Vec::new();
    for line in &lyrics.lines {
        for ch in line.text.chars() {
            if !ch.is_whitespace() && !chars.contains(&ch) {
                chars.push(ch);
            }
        }
    }
    if let Some(title) = lyrics.title.as_deref() {
        for ch in title.chars() {
            if !ch.is_whitespace() && !chars.contains(&ch) {
                chars.push(ch);
            }
        }
    }
    if let Some(artist) = lyrics.artist.as_deref() {
        for ch in artist.chars() {
            if !ch.is_whitespace() && !chars.contains(&ch) {
                chars.push(ch);
            }
        }
    }
    if let Err(err) = worker().send(chars) {
        tracing::error!(
            error = &err as &dyn std::error::Error,
            "nebbia skeleton thread is gone; characters stay unwritten"
        );
    }
}

/// True once the stroke-order data has been tried and every character of this song stored, ready or not.
pub fn glyphs_settled(lyrics: &folia_core::Lyrics) -> bool {
    if !STROKES_TRIED.load(Ordering::Acquire) {
        return false;
    }
    let map = glyphs().lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    lyrics
        .lines
        .iter()
        .flat_map(|line| line.text.chars())
        .filter(|ch| !ch.is_whitespace())
        .all(|ch| map.contains_key(&ch))
}

pub fn epoch() -> u64 {
    EPOCH.load(Ordering::Relaxed)
}

/// The skeleton of `ch`, once its background build has finished.
pub fn glyph(ch: char) -> Option<std::sync::Arc<Ink>> {
    glyphs().lock().unwrap_or_else(std::sync::PoisonError::into_inner).get(&ch).cloned().flatten()
}

static EPOCH: AtomicU64 = AtomicU64::new(1);
static STROKES_EPOCH: AtomicU64 = AtomicU64::new(0);
static STROKES_TRIED: AtomicBool = AtomicBool::new(false);

/// Changes once, when the stroke-order data arrives: layouts that size characters by stroke count redo.
pub fn strokes_epoch() -> u64 {
    STROKES_EPOCH.load(Ordering::Relaxed)
}

/// Strokes of `ch` in the stroke-order data, `None` when it has none or the data has not loaded.
pub fn stroke_count(ch: char) -> Option<usize> {
    stroke_ink(ch).map(|ink| ink.strokes.len())
}

fn warmed() -> &'static Mutex<std::collections::HashSet<u64>> {
    static SEEN: OnceLock<Mutex<std::collections::HashSet<u64>>> = OnceLock::new();
    SEEN.get_or_init(|| Mutex::new(std::collections::HashSet::new()))
}

fn text_id(lyrics: &folia_core::Lyrics) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    for line in &lyrics.lines {
        for unit in line.text.encode_utf16() {
            hash = hash.wrapping_mul(0x0000_0100_0000_01b3).wrapping_add(u64::from(unit));
        }
    }
    hash
}

fn worker() -> &'static mpsc::Sender<Vec<char>> {
    static TX: OnceLock<mpsc::Sender<Vec<char>>> = OnceLock::new();
    TX.get_or_init(|| {
        let (tx, rx) = mpsc::channel::<Vec<char>>();
        let spawned = std::thread::Builder::new().name("folia-nebbia-ink".to_owned()).spawn(move || {
            while let Ok(chars) = rx.recv() {
                for ch in chars {
                    if glyphs().lock().unwrap_or_else(std::sync::PoisonError::into_inner).contains_key(&ch) {
                        continue;
                    }
                    let built = build_ink(ch).map(std::sync::Arc::new);
                    let mut slot = glyphs().lock().unwrap_or_else(std::sync::PoisonError::into_inner);
                    if slot.contains_key(&ch) {
                        continue;
                    }
                    let built = stroke_ink(ch).or(built);
                    slot.insert(ch, built);
                    EPOCH.fetch_add(1, Ordering::Relaxed);
                }
            }
        });
        if let Err(err) = spawned {
            tracing::error!(error = &err as &dyn std::error::Error, "nebbia skeleton thread did not start");
        }
        tx
    })
}

fn build_ink(ch: char) -> Option<Ink> {
    stroke_ink(ch).map(|ink| (*ink).clone()).or_else(|| skeleton(ch))
}

type StrokeInks = HashMap<char, std::sync::Arc<Ink>>;

fn stroke_inks() -> &'static Mutex<Option<StrokeInks>> {
    static INKS: std::sync::LazyLock<Mutex<Option<StrokeInks>>> = std::sync::LazyLock::new(|| Mutex::new(None));
    &INKS
}

fn stroke_ink(ch: char) -> Option<std::sync::Arc<Ink>> {
    stroke_inks().lock().unwrap_or_else(std::sync::PoisonError::into_inner).as_ref()?.get(&ch).cloned()
}

fn start_stroke_load() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        let spawned = std::thread::Builder::new().name("folia-nebbia-strokes".to_owned()).spawn(|| {
            match super::strokes::load_default() {
                Ok(map) => adopt_strokes(map),
                Err(err) => tracing::warn!(
                    error = &err as &dyn std::error::Error,
                    "nebbia stroke medians stay on the font skeleton"
                ),
            }
            STROKES_TRIED.store(true, Ordering::Release);
        });
        if let Err(err) = spawned {
            tracing::error!(error = &err as &dyn std::error::Error, "nebbia stroke loader did not start");
            STROKES_TRIED.store(true, Ordering::Release);
        }
    });
}

fn adopt_strokes(map: super::strokes::Medians) {
    let inks: StrokeInks = map
        .into_iter()
        .filter_map(|(ch, polys)| ink_from_polylines(polys).map(|ink| (ch, std::sync::Arc::new(ink))))
        .collect();
    {
        let mut slot = stroke_inks().lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        *slot = Some(inks.clone());
    }
    STROKES_EPOCH.fetch_add(1, Ordering::Relaxed);
    let mut glyphs = glyphs().lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    let mut changed = false;
    for (ch, entry) in glyphs.iter_mut() {
        if let Some(ink) = inks.get(ch) {
            *entry = Some(std::sync::Arc::clone(ink));
            changed = true;
        }
    }
    if changed {
        EPOCH.fetch_add(1, Ordering::Relaxed);
    }
}

pub fn ink_from_polylines(polys: Vec<Vec<(f32, f32)>>) -> Option<Ink> {
    let strokes: Vec<Stroke> = polys
        .into_iter()
        .filter(|poly| poly.len() > 1)
        .map(measure_stroke)
        .filter(|stroke| stroke.len > 0.005)
        .collect();
    let total = strokes.iter().map(|stroke| stroke.len).sum();
    if strokes.is_empty() { None } else { Some(Ink { strokes, total }) }
}

fn glyphs() -> &'static Mutex<HashMap<char, Option<std::sync::Arc<Ink>>>> {
    static GLYPHS: std::sync::LazyLock<Mutex<HashMap<char, Option<std::sync::Arc<Ink>>>>> =
        std::sync::LazyLock::new(|| Mutex::new(HashMap::new()));
    &GLYPHS
}

fn skeleton(ch: char) -> Option<Ink> {
    let dots = raster(ch)?;
    let mut img = dots;
    thin(&mut img);
    let polys = trace(&img);
    let strokes: Vec<Stroke> = polys
        .into_iter()
        .map(|poly| {
            let simplified = chaikin(&rdp(&poly, 1.2), 2);
            let pts: Vec<(f32, f32)> = simplified.into_iter().map(|(x, y)| (x / SK as f32, y / SK as f32)).collect();
            measure_stroke(pts)
        })
        .filter(|stroke| stroke.len > 0.005)
        .collect();
    let total = strokes.iter().map(|stroke| stroke.len).sum();
    if strokes.is_empty() { None } else { Some(Ink { strokes, total }) }
}

fn measure_stroke(pts: Vec<(f32, f32)>) -> Stroke {
    let mut cum = Vec::with_capacity(pts.len());
    cum.push(0.0);
    for i in 1..pts.len() {
        let dx = pts[i].0 - pts[i - 1].0;
        let dy = pts[i].1 - pts[i - 1].1;
        cum.push(cum[i - 1] + (dx * dx + dy * dy).sqrt());
    }
    let len = cum.last().copied().unwrap_or(0.0);
    Stroke { pts, cum, len }
}

fn raster(ch: char) -> Option<Vec<u8>> {
    thread_local! {
        static CACHE: std::cell::RefCell<SwashCache> = std::cell::RefCell::new(SwashCache::new());
    }
    let mut fonts = font_system().write().unwrap_or_else(std::sync::PoisonError::into_inner);
    let system = fonts.raw();
    let metrics = Metrics::new(SK as f32 * 0.78, SK as f32);
    let mut buffer = Buffer::new(system, metrics);
    let family = lyric_family(system, ch);
    let attrs = Attrs::new().weight(LYRIC_WEIGHT).family(family);
    let placed = {
        let mut view = buffer.borrow_with(system);
        view.set_size(Some(SK as f32), Some(SK as f32));
        let text = ch.to_string();
        view.set_text(&text, &attrs, Shaping::Advanced, Some(Align::Center));
        view.shape_until_scroll(true);
        view.layout_runs()
            .flat_map(|run| run.glyphs.iter().map(|glyph| glyph.physical((0.0, run.line_y), 1.0)).collect::<Vec<_>>())
            .collect::<Vec<_>>()
    };
    CACHE.with(|slot| {
        let mut cache = slot.borrow_mut();
        let mut pixels = vec![0u8; SK * SK];
        for physical in placed {
            let Some(image) = cache.get_image(system, physical.cache_key).as_ref() else { continue };
            blit_mask(&mut pixels, physical.x, physical.y, image);
        }
        if pixels.iter().any(|v| *v > 110) { Some(pixels) } else { None }
    })
}

fn lyric_family(system: &mut iced::advanced::graphics::text::cosmic_text::FontSystem, ch: char) -> Family<'static> {
    for name in LYRIC_STACK {
        if face_has(system, name, ch) {
            return Family::Name(name);
        }
    }
    Family::SansSerif
}

fn face_has(system: &mut iced::advanced::graphics::text::cosmic_text::FontSystem, name: &str, ch: char) -> bool {
    use iced::advanced::graphics::text::cosmic_text::fontdb::{Family as DbFamily, Query, Stretch, Style};
    let id = system.db().query(&Query {
        families: &[DbFamily::Name(name)],
        weight: LYRIC_WEIGHT,
        stretch: Stretch::Normal,
        style: Style::Normal,
    });
    let Some(id) = id else { return false };
    let Some(font) = system.get_font(id, LYRIC_WEIGHT) else { return false };
    font.as_swash().charmap().map(ch) != 0
}

fn blit_mask(pixels: &mut [u8], x: i32, y: i32, image: &SwashImage) {
    let place = image.placement;
    let w = place.width as i32;
    let h = place.height as i32;
    for row in 0..h {
        for col in 0..w {
            let dx = x + place.left + col;
            let dy = y - place.top + row;
            if dx < 0 || dy < 0 || dx >= SK as i32 || dy >= SK as i32 {
                continue;
            }
            let src = (row * w + col) as usize;
            let coverage = match image.content {
                SwashContent::Mask => image.data.get(src).copied().unwrap_or(0),
                SwashContent::SubpixelMask => {
                    let at = src * 3;
                    let r = image.data.get(at).copied().unwrap_or(0) as u16;
                    let g = image.data.get(at + 1).copied().unwrap_or(0) as u16;
                    let b = image.data.get(at + 2).copied().unwrap_or(0) as u16;
                    ((r + g + b) / 3) as u8
                }
                SwashContent::Color => image.data.get(src * 4).copied().unwrap_or(0),
            };
            let at = dy as usize * SK + dx as usize;
            if coverage > pixels[at] {
                pixels[at] = coverage;
            }
        }
    }
}

fn thin(img: &mut [u8]) {
    let at = |img: &[u8], x: i32, y: i32| -> u8 {
        if x < 0 || y < 0 || x >= SK as i32 || y >= SK as i32 || img[(y as usize) * SK + x as usize] <= 110 {
            0
        } else {
            1
        }
    };
    let mut changed = true;
    while changed {
        changed = false;
        for pass in 0..2 {
            let mut del = Vec::new();
            for y in 0..SK as i32 {
                for x in 0..SK as i32 {
                    if at(img, x, y) == 0 {
                        continue;
                    }
                    let n = [
                        at(img, x, y - 1),
                        at(img, x + 1, y - 1),
                        at(img, x + 1, y),
                        at(img, x + 1, y + 1),
                        at(img, x, y + 1),
                        at(img, x - 1, y + 1),
                        at(img, x - 1, y),
                        at(img, x - 1, y - 1),
                    ];
                    let b: u8 = n.iter().sum();
                    if !(2..=6).contains(&b) {
                        continue;
                    }
                    let mut a = 0;
                    for i in 0..8 {
                        if n[i] == 0 && n[(i + 1) % 8] == 1 {
                            a += 1;
                        }
                    }
                    if a != 1 {
                        continue;
                    }
                    let hit = if pass == 0 {
                        n[0] * n[2] * n[4] + n[2] * n[4] * n[6] > 0
                    } else {
                        n[0] * n[2] * n[6] + n[0] * n[4] * n[6] > 0
                    };
                    if hit {
                        continue;
                    }
                    del.push(y as usize * SK + x as usize);
                }
            }
            if !del.is_empty() {
                changed = true;
            }
            for i in del {
                img[i] = 0;
            }
        }
    }
}

fn trace(img: &[u8]) -> Vec<Vec<(f32, f32)>> {
    let on = |x: i32, y: i32| {
        x >= 0 && y >= 0 && x < SK as i32 && y < SK as i32 && img[(y as usize) * SK + x as usize] > 110
    };
    let neighbors: [(i32, i32); 8] = [(1, 0), (0, 1), (-1, 0), (0, -1), (1, 1), (-1, 1), (-1, -1), (1, -1)];
    let deg = |x: i32, y: i32| neighbors.iter().filter(|(dx, dy)| on(x + dx, y + dy)).count();
    let mut seen = vec![false; SK * SK];
    let mut polys = Vec::new();
    let walk = |sx: i32, sy: i32, seen: &mut [bool], polys: &mut Vec<Vec<(f32, f32)>>| {
        let mut x = sx;
        let mut y = sy;
        let mut poly = vec![(x as f32, y as f32)];
        seen[(y as usize) * SK + x as usize] = true;
        let mut dir = (0i32, 0i32);
        loop {
            let mut best = None;
            let mut best_score = -9.0;
            for (dx, dy) in neighbors {
                let nx = x + dx;
                let ny = y + dy;
                if !on(nx, ny) || seen[(ny as usize) * SK + nx as usize] {
                    continue;
                }
                let score = if dir == (0, 0) {
                    0.0
                } else {
                    let len = ((dx * dx + dy * dy) as f32).sqrt();
                    (dx * dir.0 + dy * dir.1) as f32 / len
                };
                if score > best_score {
                    best_score = score;
                    best = Some((nx, ny, dx, dy));
                }
            }
            let Some((nx, ny, dx, dy)) = best else { break };
            x = nx;
            y = ny;
            dir = (dx, dy);
            seen[(y as usize) * SK + x as usize] = true;
            poly.push((x as f32, y as f32));
        }
        if poly.len() >= 4 {
            polys.push(poly);
        }
    };
    for y in 0..SK as i32 {
        for x in 0..SK as i32 {
            if on(x, y) && deg(x, y) == 1 && !seen[(y as usize) * SK + x as usize] {
                walk(x, y, &mut seen, &mut polys);
            }
        }
    }
    for y in 0..SK as i32 {
        for x in 0..SK as i32 {
            if on(x, y) && !seen[(y as usize) * SK + x as usize] {
                walk(x, y, &mut seen, &mut polys);
            }
        }
    }
    for poly in &mut polys {
        let a = poly[0].0 + poly[0].1 * 0.6;
        let b = poly[poly.len() - 1].0 + poly[poly.len() - 1].1 * 0.6;
        if a > b {
            poly.reverse();
        }
    }
    polys.sort_by(|a, b| {
        let min = |poly: &[(f32, f32)], axis: usize| {
            poly.iter().map(|p| if axis == 0 { p.0 } else { p.1 }).fold(f32::INFINITY, f32::min)
        };
        ((min(a, 1) - min(b, 1)) * 0.6 + (min(a, 0) - min(b, 0))).total_cmp(&0.0)
    });
    polys
}

fn rdp(pts: &[(f32, f32)], eps: f32) -> Vec<(f32, f32)> {
    if pts.len() < 3 {
        return pts.to_vec();
    }
    let a = pts[0];
    let b = pts[pts.len() - 1];
    let len = ((b.0 - a.0).powi(2) + (b.1 - a.1).powi(2)).sqrt().max(1.0);
    let mut idx = 0;
    let mut dmax = 0.0;
    for (i, p) in pts.iter().enumerate().skip(1).take(pts.len().saturating_sub(2)) {
        let d = ((b.0 - a.0) * (a.1 - p.1) - (a.0 - p.0) * (b.1 - a.1)).abs() / len;
        if d > dmax {
            dmax = d;
            idx = i;
        }
    }
    if dmax > eps {
        let mut left = rdp(&pts[..=idx], eps);
        left.pop();
        left.extend(rdp(&pts[idx..], eps));
        left
    } else {
        vec![a, b]
    }
}

fn chaikin(input: &[(f32, f32)], iterations: usize) -> Vec<(f32, f32)> {
    let mut pts = input.to_vec();
    for _ in 0..iterations {
        if pts.len() < 3 {
            break;
        }
        let mut next = vec![pts[0]];
        for i in 0..pts.len() - 1 {
            let p = pts[i];
            let q = pts[i + 1];
            next.push((p.0 * 0.75 + q.0 * 0.25, p.1 * 0.75 + q.1 * 0.25));
            next.push((p.0 * 0.25 + q.0 * 0.75, p.1 * 0.25 + q.1 * 0.75));
        }
        next.push(*pts.last().unwrap_or(&pts[0]));
        pts = next;
    }
    pts
}
