// Lumiere line art. Diagrams are the original `lineart/` recipes, rebuilt when the shot or aspect changes.

use super::profiles::{Profile, beams_for};

const TAU: f32 = std::f32::consts::TAU;
const PI: f32 = std::f32::consts::PI;

pub struct Mesh {
    pub pts: Vec<[f32; 2]>,
    pub strokes: Vec<Stroke>,
    pub nodes: Vec<Node>,
}

#[derive(Clone, Copy)]
pub struct Stroke {
    pub start: u32,
    pub count: u16,
    pub width: f32,
    pub alpha: f32,
    pub delay: f32,
    pub span: f32,
    pub dash_on: f32,
    pub dash_off: f32,
}

#[derive(Clone, Copy)]
pub struct Node {
    pub x: f32,
    pub y: f32,
    pub size: f32,
    pub delay: f32,
    pub phase: f32,
}

#[derive(Clone, Copy)]
struct Pen {
    width: f32,
    alpha: f32,
    delay: f32,
    span: f32,
    dash_on: f32,
    dash_off: f32,
}

impl Pen {
    fn new(width: f32, alpha: f32) -> Self {
        Self { width, alpha, delay: 0.0, span: 0.4, dash_on: 0.0, dash_off: 0.0 }
    }

    fn at(self, delay: f32) -> Self {
        Self { delay, ..self }
    }
}

impl Mesh {
    pub fn new() -> Self {
        Self { pts: Vec::new(), strokes: Vec::new(), nodes: Vec::new() }
    }

    pub fn clear(&mut self) {
        self.pts.clear();
        self.strokes.clear();
        self.nodes.clear();
    }

    fn path(&mut self, points: &[[f32; 2]], pen: Pen) {
        if points.len() < 2 {
            return;
        }
        let start = self.pts.len() as u32;
        self.pts.extend_from_slice(points);
        self.strokes.push(Stroke {
            start,
            count: points.len() as u16,
            width: pen.width,
            alpha: pen.alpha,
            delay: pen.delay,
            span: pen.span,
            dash_on: pen.dash_on,
            dash_off: pen.dash_off,
        });
    }

    fn seg(&mut self, a: [f32; 2], b: [f32; 2], pen: Pen) {
        self.path(&[a, b], pen);
    }

    fn node(&mut self, at: [f32; 2], size: f32, delay: f32, phase: f32) {
        self.nodes.push(Node { x: at[0], y: at[1], size, delay, phase });
    }

    fn arc(&mut self, spec: [f32; 5], steps: usize, pen: Pen) {
        let [cx, cy, r, from, to] = spec;
        let mut points = Vec::with_capacity(steps + 1);
        for i in 0..=steps {
            let a = from + (to - from) * i as f32 / steps as f32;
            points.push([cx + a.cos() * r, cy + a.sin() * r]);
        }
        self.path(&points, pen);
    }

    fn ellipse(&mut self, spec: [f32; 7], steps: usize, pen: Pen) {
        let [cx, cy, rx, ry, rotation, from, to] = spec;
        let (co, si) = (rotation.cos(), rotation.sin());
        let mut points = Vec::with_capacity(steps + 1);
        for i in 0..=steps {
            let a = from + (to - from) * i as f32 / steps as f32;
            let (x, y) = (a.cos(), a.sin());
            points.push([cx + x * rx * co - y * ry * si, cy + x * rx * si + y * ry * co]);
        }
        self.path(&points, pen);
    }

    fn cubic(&mut self, p0: [f32; 2], p1: [f32; 2], p2: [f32; 2], p3: [f32; 2], steps: usize, pen: Pen) {
        let mut points = Vec::with_capacity(steps + 1);
        for i in 0..=steps {
            let t = i as f32 / steps as f32;
            let u = 1.0 - t;
            let (a, b, c, d) = (u * u * u, 3.0 * u * u * t, 3.0 * u * t * t, t * t * t);
            points.push([a * p0[0] + b * p1[0] + c * p2[0] + d * p3[0], a * p0[1] + b * p1[1] + c * p2[1] + d * p3[1]]);
        }
        self.path(&points, pen);
    }
}

impl Default for Mesh {
    fn default() -> Self {
        Self::new()
    }
}

pub struct Rng(pub u32);

impl Rng {
    pub fn next(&mut self) -> f32 {
        self.0 = self.0.wrapping_add(0x6d2b_79f5);
        let state = self.0;
        let mut mixed = (state ^ (state >> 15)).wrapping_mul(state | 1);
        mixed = mixed.wrapping_add((mixed ^ (mixed >> 7)).wrapping_mul(mixed | 61)) ^ mixed;
        ((mixed ^ (mixed >> 14)) as f64 / 4_294_967_296.0) as f32
    }
}

pub fn art_seed(program: &super::program::Program, index: usize, kind: &str) -> u32 {
    super::program::shot_stream(program, index, kind, b":art")
}

/// The diagram for this shot. `seed` is the art stream, separate from the light stream.
pub fn build(profile: &Profile, aspect: f32, seed: u32, mesh: &mut Mesh) {
    mesh.clear();
    let mut rng = Rng(seed);
    match profile.kind {
        "zenith-halo" => {
            halo(aspect * 0.5, 0.03, 0.42, 0.7, 0.0, mesh);
            halo(aspect * 0.5, 0.03, 0.24, 0.4, 0.2, mesh);
            sparks(aspect, 20, &mut rng, 0.3, None, mesh);
        }
        "zenith-twin" => {
            frame(aspect, [0.07, 0.21, 0.93, 0.8], 0.0, 0.32, mesh);
            sparks(aspect, 18, &mut rng, 0.2, None, mesh);
        }
        "zenith-well" => {
            mesh.ellipse([aspect * 0.5, 0.8, 0.26, 0.05, 0.0, 0.0, TAU], 64, Pen::new(0.0018, 0.7).at(0.1));
            mesh.ellipse(
                [aspect * 0.5, 0.8, 0.17, 0.032, 0.0, 0.0, TAU],
                48,
                dash(Pen::new(0.0012, 0.45).at(0.2), 0.004, 0.008),
            );
            mesh.node([aspect * 0.5, 0.8], 0.03, 0.3, 0.0);
        }
        "lattice-blinds" | "lattice-sweep" => {
            blinds(aspect * 0.03, 0.02, aspect * 0.16, 0.26, if profile.kind.ends_with("sweep") { 8 } else { 9 }, mesh);
            sparks(aspect, 14, &mut rng, 0.3, None, mesh);
        }
        "lattice-cross" => {
            cross_window(aspect * 0.8, 0.01, aspect * 0.16, 0.26, mesh);
            sparks(aspect, 14, &mut rng, 0.3, Some([0.05, 0.3, 0.7, 0.9]), mesh);
        }
        "lattice-arch" => {
            arch(aspect * 0.08, -0.06, aspect * 0.12, 0.32, mesh);
            sparks(aspect, 14, &mut rng, 0.3, None, mesh);
        }
        "lattice-rose" => {
            rose(aspect * 0.5, 0.08, 0.12, mesh);
            sparks(aspect, 16, &mut rng, 0.3, None, mesh);
        }
        "lattice-slit" => {
            door(aspect * 0.1, 0.08, 0.86, 0.012, mesh);
            sparks(aspect, 10, &mut rng, 0.3, Some([0.2, 0.2, 0.95, 0.9]), mesh);
        }
        "lattice-shoji" => {
            grid([aspect * 0.02, 0.06], [aspect * 0.18, 0.4], 3, 5, false, mesh);
            sparks(aspect, 10, &mut rng, 0.3, None, mesh);
        }
        "lattice-grille" => {
            grid([aspect * 0.02, 0.02], [aspect * 0.16, 0.26], 4, 4, true, mesh);
            sparks(aspect, 14, &mut rng, 0.3, None, mesh);
        }
        "lattice-komorebi" => {
            canopy(aspect, &mut rng, mesh);
            sparks(aspect, 12, &mut rng, 0.3, None, mesh);
        }
        "lattice-nave" => {
            tall(aspect * 0.05, aspect * 0.95, -0.08, 0.26, 4, mesh);
            sparks(aspect, 18, &mut rng, 0.3, None, mesh);
        }
        "prism-split" => {
            prism(aspect * 0.42, 0.42, 0.2, 0.8, mesh);
            sparks(aspect, 16, &mut rng, 0.3, None, mesh);
        }
        "prism-shard" => {
            shards(aspect, &mut rng, 8, mesh);
            sparks(aspect, 18, &mut rng, 0.3, None, mesh);
        }
        "prism-lines" => {
            spectrum(aspect * 0.12, aspect * 0.88, 0.84, 0.16, &mut rng, 18, mesh);
            sparks(aspect, 10, &mut rng, 0.3, None, mesh);
        }
        "prism-merge" => {
            prism(aspect * 0.34, 0.46, 0.12, 0.6, mesh);
            sparks(aspect, 16, &mut rng, 0.3, None, mesh);
        }
        "prism-cube" => {
            cube(aspect * 0.42, 0.34, 0.08, mesh);
            sparks(aspect, 12, &mut rng, 0.3, None, mesh);
        }
        "prism-wheel" => {
            prism(aspect * 0.5, 0.1, 0.1, 0.6, mesh);
            sparks(aspect, 16, &mut rng, 0.3, None, mesh);
        }
        "prism-droplet" => {
            rainbow(aspect * 0.5, 1.02, 0.62, mesh);
            sparks(aspect, 12, &mut rng, 0.3, None, mesh);
        }
        "caustic-pool" => {
            mesh.ellipse([aspect * 0.5, 0.82, aspect * 0.42, 0.14, 0.0, 0.0, TAU], 64, Pen::new(0.0014, 0.4).at(0.1));
            sparks(aspect, 10, &mut rng, 0.3, None, mesh);
        }
        "caustic-cup" => {
            cup(aspect * 0.24, 0.52, 0.1, 0.2, mesh);
            sparks(aspect, 10, &mut rng, 0.3, None, mesh);
        }
        "caustic-ripple" | "caustic-storm" => {
            ripples(
                aspect * 0.5,
                if profile.kind.ends_with("storm") { 0.82 } else { 0.8 },
                if profile.kind.ends_with("storm") { 0.5 } else { 0.34 },
                if profile.kind.ends_with("storm") { 4 } else { 5 },
                mesh,
            );
            sparks(aspect, if profile.kind.ends_with("storm") { 14 } else { 10 }, &mut rng, 0.3, None, mesh);
        }
        "caustic-surface" => {
            water(aspect, 0.1, 0.012, 7.0, mesh);
            sparks(aspect, 14, &mut rng, 0.3, None, mesh);
        }
        "caustic-ring" => {
            mesh.ellipse([aspect * 0.5, 0.5, 0.36, 0.36, 0.0, 0.0, TAU], 72, Pen::new(0.0024, 0.6));
            mesh.ellipse([aspect * 0.5, 0.5, 0.32, 0.32, 0.0, 0.0, TAU], 64, Pen::new(0.0009, 0.3).at(0.1));
            sparks(aspect, 10, &mut rng, 0.3, None, mesh);
        }
        "caustic-burn" => {
            glass(aspect * 0.5, 0.2, 0.1, mesh);
            sparks(aspect, 12, &mut rng, 0.3, None, mesh);
        }
        "caustic-shimmer" => {
            water(aspect, 0.86, 0.008, 9.0, mesh);
            sparks(aspect, 10, &mut rng, 0.3, None, mesh);
        }
        "caustic-crystal" => {
            gem(aspect * 0.5, 0.26, 0.05, mesh);
            sparks(aspect, 14, &mut rng, 0.3, None, mesh);
        }
        "caustic-drift" => sparks(aspect, 8, &mut rng, 0.3, None, mesh),
        "optics-convex" | "optics-concave" | "optics-mirror" | "optics-refract" | "optics-bench"
        | "optics-periscope" | "optics-fiber" | "optics-pinhole" | "optics-telescope" | "optics-raytrace" => {
            optics(profile.kind, aspect, &mut rng, mesh);
        }
        "wave-slits" | "wave-newton" | "wave-airy" | "wave-grating" | "wave-sources" | "wave-moire" | "wave-film"
        | "wave-polar" | "wave-standing" | "wave-holo" => wave(profile.kind, aspect, &mut rng, mesh),
        "botany-sprout" => sprout_plate(aspect, &mut rng, mesh),
        "botany-veins" => {
            leaf(aspect * 0.52, 0.42, 0.62, -0.45, mesh);
            sparks(aspect, 12, &mut rng, 0.3, None, mesh);
        }
        "botany-canopy" => {
            canopy(aspect, &mut rng, mesh);
            sparks(aspect, 16, &mut rng, 0.3, None, mesh);
        }
        "botany-fern" => {
            fern(aspect * 0.28, 0.42, 0.16, mesh);
            sparks(aspect, 12, &mut rng, 0.3, None, mesh);
        }
        "botany-seed" => {
            roots(aspect * 0.5, 0.7, &mut rng, mesh);
            sparks(aspect, 10, &mut rng, 0.3, None, mesh);
        }
        "botany-bloom" => {
            flower(aspect * 0.5, 0.42, 0.2, 8, mesh);
            sparks(aspect, 12, &mut rng, 0.3, None, mesh);
        }
        "botany-vine" => {
            vine(aspect * 0.72, 1.02, 0.08, 0.05, 4.0, mesh);
            sparks(aspect, 12, &mut rng, 0.3, None, mesh);
        }
        "botany-chloro" => {
            cells(aspect * 0.5, 0.45, 0.36, &mut rng, 22, mesh);
            sparks(aspect, 8, &mut rng, 0.3, None, mesh);
        }
        "botany-formula" => {
            molecule(aspect * 0.24, 0.3, 0.06, 0.0, mesh);
            molecule(aspect * 0.78, 0.74, 0.05, 0.2, mesh);
            halo(aspect * 0.5, 0.03, 0.28, 0.5, 0.0, mesh);
            sparks(aspect, 12, &mut rng, 0.3, None, mesh);
        }
        "botany-pollen" => sparks(aspect, 12, &mut rng, 0.3, None, mesh),
        "astral-eclipse" => {
            corona(aspect * 0.5, 0.34, 0.13, &mut rng, mesh);
            sparks(aspect, 20, &mut rng, 0.3, None, mesh);
        }
        "astral-orbit" => {
            orbits(aspect * 0.5, 0.42, &[0.12, 0.2, 0.3, 0.42], -0.18, 0.34, &mut rng, mesh);
            sparks(aspect, 20, &mut rng, 0.3, None, mesh);
        }
        "astral-trail" => trails(aspect * 0.72, 0.12, &mut rng, 60, 0.9, mesh),
        "astral-sextant" => {
            sextant(aspect * 0.4, 0.14, 0.32, mesh);
            sparks(aspect, 20, &mut rng, 0.3, None, mesh);
        }
        "astral-constellation" => {
            stars(aspect, &mut rng, 8, [0.1, 0.1, 0.9, 0.42], 0.0, mesh);
            stars(aspect, &mut rng, 5, [0.6, 0.62, 0.95, 0.9], 0.3, mesh);
        }
        "astral-corona" => {
            corona(aspect * 0.5, 0.12, 0.1, &mut rng, mesh);
            sparks(aspect, 20, &mut rng, 0.3, None, mesh);
        }
        "astral-moon" => {
            moon(aspect * 0.82, 0.12, 0.07, mesh);
            sparks(aspect, 20, &mut rng, 0.3, None, mesh);
        }
        "astral-nebula" => sparks(aspect, 20, &mut rng, 0.3, None, mesh),
        "astral-armillary" => {
            rings(aspect * 0.5, 0.42, 0.26, mesh);
            sparks(aspect, 20, &mut rng, 0.3, None, mesh);
        }
        "astral-meteor" => {
            meteors(aspect, &mut rng, 6, mesh);
            sparks(aspect, 20, &mut rng, 0.3, None, mesh);
        }
        "stage-spot" | "stage-cross" | "stage-search" | "stage-footlight" | "stage-pinspot" | "stage-strobe"
        | "stage-laser" => {
            lamps(profile, aspect, mesh);
            sparks(aspect, 10, &mut rng, 0.3, None, mesh);
        }
        "stage-backlight" => sparks(aspect, 18, &mut rng, 0.3, None, mesh),
        "stage-curtain" => {
            curtains(aspect, 0.06, mesh);
            sparks(aspect, 8, &mut rng, 0.3, Some([0.3, 0.2, 0.7, 0.9]), mesh);
        }
        "stage-projector" => {
            machine(aspect * 0.12, 0.4, 0.1, mesh);
            sparks(aspect, 10, &mut rng, 0.3, None, mesh);
        }
        "motes-mist" => {
            horizon(aspect, 0.64, mesh);
            sparks(aspect, 8, &mut rng, 0.3, Some([0.05, 0.05, 0.95, 0.55]), mesh);
        }
        kind if kind.starts_with("motes-") => sparks(aspect, 10, &mut rng, 0.3, None, mesh),
        _ => standard(aspect, &mut rng, mesh),
    }
}

fn dash(mut pen: Pen, on: f32, off: f32) -> Pen {
    pen.dash_on = on;
    pen.dash_off = off;
    pen
}

fn standard(aspect: f32, rng: &mut Rng, mesh: &mut Mesh) {
    halo(aspect * 0.5, 0.03, 0.3, 0.55, 0.0, mesh);
    frame(aspect, [0.07, 0.21, 0.93, 0.8], 0.1, 0.32, mesh);
    sparks(aspect, 16, rng, 0.3, None, mesh);
}

fn sparks(aspect: f32, count: usize, rng: &mut Rng, delay: f32, region: Option<[f32; 4]>, mesh: &mut Mesh) {
    let [x0, y0, x1, y1] = region.unwrap_or([0.05, 0.08, 0.95, 0.92]);
    for i in 0..count {
        let x = (x0 + (x1 - x0) * rng.next()) * aspect;
        let y = y0 + (y1 - y0) * rng.next();
        let size = 0.006 + rng.next() * 0.012;
        let at = delay + rng.next() * 0.5;
        mesh.node([x, y], size, at, i as f32 * 2.399);
    }
}

fn halo(cx: f32, cy: f32, radius: f32, alpha: f32, delay: f32, mesh: &mut Mesh) {
    let from = PI * 0.04;
    let to = PI * 0.96;
    let pen = Pen::new(0.0016, alpha);
    mesh.arc([cx, cy, radius, to, from], 96, Pen { span: 0.45, ..pen.at(delay) });
    mesh.arc(
        [cx, cy, radius * 0.62, from, to],
        72,
        Pen { width: 0.0013, alpha: alpha * 0.8, span: 0.4, ..pen.at(delay + 0.08) },
    );
    mesh.arc(
        [cx, cy, radius * 1.18, from + 0.1, to - 0.1],
        80,
        dash(Pen { width: 0.0011, alpha: alpha * 0.45, span: 0.4, ..pen.at(delay + 0.2) }, 0.004, 0.009),
    );
    let step = PI / 12.0;
    let mut a = PI / 2.0 - step * 5.0;
    let mut index = 0;
    while a <= PI / 2.0 + step * 5.0 + 1e-4 {
        let long = index % 2 == 0;
        let r0 = radius * 0.62 * 0.2;
        let r1 = radius * if long { 1.12 } else { 1.02 };
        let width = if long { 0.0013 } else { 0.0009 };
        let bright = alpha * if long { 0.7 } else { 0.45 };
        mesh.seg(
            [cx + a.cos() * r0, cy + a.sin() * r0],
            [cx + a.cos() * r1, cy + a.sin() * r1],
            Pen { width, alpha: bright, span: 0.3, ..pen.at(delay + 0.12 + (a - PI / 2.0).abs() * 0.12) },
        );
        if long {
            mesh.node([cx + a.cos() * radius, cy + a.sin() * radius], 0.022, delay + 0.45, index as f32 * 1.7);
        }
        index += 1;
        a += step;
    }
    let mut tick = from;
    while tick <= to {
        mesh.seg(
            [cx + tick.cos() * radius, cy + tick.sin() * radius],
            [cx + tick.cos() * radius * 0.975, cy + tick.sin() * radius * 0.975],
            Pen { width: 0.0008, alpha: alpha * 0.4, span: 0.15, ..pen.at(delay + 0.25 + (tick - from) * 0.05) },
        );
        tick += PI / 72.0;
    }
}

fn frame(aspect: f32, rect: [f32; 4], delay: f32, alpha: f32, mesh: &mut Mesh) {
    let [left, top, right, bottom] = rect;
    let left = left * aspect;
    let right = right * aspect;
    let corner = 0.03;
    mesh.path(
        &[[left, top], [right, top], [right, bottom], [left, bottom], [left, top]],
        Pen { width: 0.0009, alpha: alpha * 0.55, span: 0.6, ..Pen::new(0.0009, alpha).at(delay) },
    );
    let corners = [
        [[left, top + corner], [left, top], [left + corner, top]],
        [[right - corner, top], [right, top], [right, top + corner]],
        [[right, bottom - corner], [right, bottom], [right - corner, bottom]],
        [[left + corner, bottom], [left, bottom], [left, bottom - corner]],
    ];
    for (i, points) in corners.iter().enumerate() {
        mesh.path(
            points,
            Pen { width: 0.0018, span: 0.2, ..Pen::new(0.0018, alpha).at(delay + 0.1 + i as f32 * 0.05) },
        );
    }
    for i in 1..12 {
        let x = left + (right - left) * i as f32 / 12.0;
        let y = top + (bottom - top) * i as f32 / 12.0;
        let len = if i % 3 == 0 { 0.014 } else { 0.007 };
        let pen = Pen {
            width: 0.0008,
            alpha: alpha * 0.6,
            span: 0.1,
            ..Pen::new(0.0008, alpha).at(delay + 0.3 + i as f32 * 0.01)
        };
        mesh.seg([x, top], [x, top + len], pen);
        mesh.seg([x, bottom], [x, bottom - len], pen);
        mesh.seg([left, y], [left + len, y], pen);
        mesh.seg([right, y], [right - len, y], pen);
    }
    for (i, x) in [left + (right - left) * 0.14, right - (right - left) * 0.14].into_iter().enumerate() {
        mesh.seg(
            [x, top + 0.06],
            [x, bottom - 0.06],
            dash(
                Pen {
                    width: 0.001,
                    alpha: alpha * 0.5,
                    span: 0.4,
                    ..Pen::new(0.001, alpha).at(delay + 0.35 + i as f32 * 0.05)
                },
                0.003,
                0.012,
            ),
        );
    }
}

fn blinds(x: f32, y: f32, w: f32, h: f32, slats: usize, mesh: &mut Mesh) {
    mesh.path(&[[x, y], [x + w, y], [x + w, y + h], [x, y + h], [x, y]], Pen { span: 0.5, ..Pen::new(0.0018, 0.5) });
    for i in 1..slats {
        let sy = y + h * i as f32 / slats as f32;
        mesh.seg(
            [x, sy],
            [x + w, sy],
            Pen { width: 0.001, alpha: 0.3, span: 0.2, ..Pen::new(0.001, 0.3).at(0.1 + i as f32 * 0.03) },
        );
    }
    mesh.node([x + w, y + h], 0.018, 0.4, 1.0);
}

fn cross_window(x: f32, y: f32, w: f32, h: f32, mesh: &mut Mesh) {
    mesh.path(&[[x, y], [x + w, y], [x + w, y + h], [x, y + h], [x, y]], Pen::new(0.002, 0.55));
    mesh.seg([x + w / 2.0, y], [x + w / 2.0, y + h], Pen::new(0.0014, 0.45).at(0.15));
    mesh.seg([x, y + h / 2.0], [x + w, y + h / 2.0], Pen::new(0.0014, 0.45).at(0.2));
    mesh.node([x + w / 2.0, y + h / 2.0], 0.02, 0.4, 0.0);
}

fn arch(x: f32, y: f32, w: f32, h: f32, mesh: &mut Mesh) {
    let r = w / 2.0;
    let top = y + r;
    let mut points = vec![[x, y + h], [x, top]];
    for i in 0..=40 {
        let a = PI + (TAU - PI) * i as f32 / 40.0;
        points.push([x + r + a.cos() * r, top + a.sin() * r]);
    }
    points.push([x + w, y + h]);
    points.push([x, y + h]);
    mesh.path(&points, Pen::new(0.0018, 0.55));
    mesh.seg([x + r, y], [x + r, y + h], Pen::new(0.001, 0.35).at(0.2));
    mesh.node([x + r, y + 0.01], 0.018, 0.4, 0.0);
}

fn rose(cx: f32, cy: f32, r: f32, mesh: &mut Mesh) {
    for (i, radius) in [r, r * 0.62, r * 0.22].into_iter().enumerate() {
        mesh.ellipse([cx, cy, radius, radius, 0.0, 0.0, TAU], 64, Pen::new(0.0016, 0.55).at(i as f32 * 0.06));
    }
    for i in 0..12 {
        let a = i as f32 / 12.0 * TAU;
        mesh.seg(
            [cx + a.cos() * r * 0.22, cy + a.sin() * r * 0.22],
            [cx + a.cos() * r, cy + a.sin() * r],
            Pen { width: 0.001, alpha: 0.4, span: 0.2, ..Pen::new(0.001, 0.4).at(0.2 + i as f32 / 12.0 * 0.3) },
        );
        mesh.ellipse(
            [cx + a.cos() * r * 0.8, cy + a.sin() * r * 0.8, r * 0.14, r * 0.14, 0.0, 0.0, TAU],
            24,
            Pen::new(0.0009, 0.35).at(0.35 + i as f32 * 0.02),
        );
    }
    mesh.node([cx, cy], 0.028, 0.5, 0.0);
}

fn door(cx: f32, top: f32, bottom: f32, gap: f32, mesh: &mut Mesh) {
    mesh.seg([cx - gap / 2.0, top], [cx - gap / 2.0, bottom], Pen::new(0.0018, 0.55));
    mesh.seg([cx + gap / 2.0, top], [cx + gap / 2.0, bottom], Pen::new(0.0018, 0.55).at(0.05));
    mesh.seg([cx - gap / 2.0 - 0.25, bottom], [cx + gap / 2.0 + 0.25, bottom], Pen::new(0.001, 0.35).at(0.2));
    mesh.node([cx, top + 0.02], 0.02, 0.4, 0.0);
}

fn grid(origin: [f32; 2], size: [f32; 2], cols: usize, rows: usize, diagonal: bool, mesh: &mut Mesh) {
    let [x, y] = origin;
    let [w, h] = size;
    mesh.path(&[[x, y], [x + w, y], [x + w, y + h], [x, y + h], [x, y]], Pen::new(0.0018, 0.5));
    let pen = Pen { width: 0.0009, alpha: 0.32, span: 0.25, ..Pen::new(0.0009, 0.32).at(0.15) };
    if diagonal {
        let n = cols + rows;
        for i in 1..n {
            let t = i as f32 / n as f32;
            mesh.seg(
                [x + w * (t * 2.0).min(1.0), y + h * (t * 2.0 - 1.0).max(0.0)],
                [x + w * (t * 2.0 - 1.0).max(0.0), y + h * (t * 2.0).min(1.0)],
                pen,
            );
            mesh.seg(
                [x + w * (1.0 - (t * 2.0).min(1.0)), y + h * (t * 2.0 - 1.0).max(0.0)],
                [x + w * (1.0 - (t * 2.0 - 1.0).max(0.0)), y + h * (t * 2.0).min(1.0)],
                pen,
            );
        }
    } else {
        for i in 1..cols {
            let gx = x + w * i as f32 / cols as f32;
            mesh.seg([gx, y], [gx, y + h], pen);
        }
        for j in 1..rows {
            let gy = y + h * j as f32 / rows as f32;
            mesh.seg([x, gy], [x + w, gy], pen);
        }
    }
}

fn tall(x0: f32, x1: f32, y: f32, h: f32, count: usize, mesh: &mut Mesh) {
    for i in 0..count {
        let w = ((x1 - x0) / count as f32) * 0.45;
        let x = x0 + (x1 - x0) * (i as f32 + 0.5) / count as f32 - w / 2.0;
        arch(x, y, w, h, mesh);
    }
}

fn prism(cx: f32, cy: f32, size: f32, alpha: f32, mesh: &mut Mesh) {
    let h = size * 3.0_f32.sqrt() / 2.0;
    let top = [cx, cy - h * 2.0 / 3.0];
    let left = [cx - size / 2.0, cy + h / 3.0];
    let right = [cx + size / 2.0, cy + h / 3.0];
    mesh.path(&[top, right, left, top], Pen { span: 0.5, ..Pen::new(0.0022, alpha) });
    mesh.seg(top, [cx, cy + h / 3.0], dash(Pen::new(0.0008, alpha * 0.4).at(0.3), 0.004, 0.008));
    for (i, at) in [top, left, right].into_iter().enumerate() {
        mesh.node(at, 0.02, 0.4 + i as f32 * 0.05, i as f32 * 1.3);
    }
}

fn shards(aspect: f32, rng: &mut Rng, count: usize, mesh: &mut Mesh) {
    for i in 0..count {
        let cx = (0.1 + rng.next() * 0.8) * aspect;
        let cy = 0.12 + rng.next() * 0.76;
        let r = 0.025 + rng.next() * 0.05;
        let a = rng.next() * TAU;
        let mut points = [[0.0; 2]; 4];
        let mut k = 0.0;
        for point in &mut points {
            let ang = a + k * TAU / 3.0;
            let ry = r * (0.7 + rng.next() * 0.6);
            *point = [cx + ang.cos() * r, cy + ang.sin() * ry];
            k += 1.0;
        }
        mesh.path(&points, Pen { width: 0.0014, alpha: 0.5, span: 0.25, ..Pen::new(0.0014, 0.5).at(i as f32 * 0.04) });
    }
}

fn spectrum(x0: f32, x1: f32, y: f32, h: f32, rng: &mut Rng, count: usize, mesh: &mut Mesh) {
    mesh.seg([x0, y + h / 2.0], [x1, y + h / 2.0], Pen::new(0.001, 0.35));
    for i in 0..count {
        let x = x0 + (x1 - x0) * (0.05 + 0.9 * rng.next());
        let k = 0.3 + rng.next() * 0.7;
        let width = 0.0012 + rng.next() * 0.0012;
        mesh.seg(
            [x, y + h / 2.0],
            [x, y + h / 2.0 - h * k],
            Pen { width, alpha: 0.4 + k * 0.3, span: 0.2, ..Pen::new(width, 0.5).at(0.1 + i as f32 * 0.03) },
        );
    }
}

fn cube(cx: f32, cy: f32, size: f32, mesh: &mut Mesh) {
    let h = size / 2.0;
    mesh.path(
        &[[cx - h, cy - h], [cx + h, cy - h], [cx + h, cy + h], [cx - h, cy + h], [cx - h, cy - h]],
        Pen::new(0.002, 0.6),
    );
    mesh.seg([cx - h, cy + h], [cx + h, cy - h], Pen::new(0.0012, 0.45).at(0.2));
    mesh.node([cx, cy], 0.022, 0.35, 0.0);
}

fn rainbow(cx: f32, cy: f32, r: f32, mesh: &mut Mesh) {
    mesh.arc([cx, cy, r, PI * 1.08, PI * 1.92], 72, Pen::new(0.0018, 0.5));
    mesh.arc([cx, cy, r * 1.18, PI * 1.1, PI * 1.9], 72, dash(Pen::new(0.001, 0.3).at(0.15), 0.006, 0.01));
    mesh.ellipse([cx, cy - r * 0.4, 0.03, 0.03, 0.0, 0.0, TAU], 24, Pen::new(0.0012, 0.45).at(0.3));
}

fn cup(cx: f32, top: f32, w: f32, h: f32, mesh: &mut Mesh) {
    mesh.ellipse([cx, top, w / 2.0, w * 0.12, 0.0, 0.0, TAU], 48, Pen::new(0.0016, 0.5));
    mesh.path(
        &[[cx - w / 2.0, top], [cx - w * 0.4, top + h], [cx + w * 0.4, top + h], [cx + w / 2.0, top]],
        Pen::new(0.0016, 0.5).at(0.1),
    );
    mesh.ellipse([cx, top + h, w * 0.4, w * 0.09, 0.0, 0.0, PI], 32, Pen::new(0.001, 0.35).at(0.2));
    mesh.node([cx - w * 0.3, top + h * 0.3], 0.016, 0.35, 0.0);
}

fn ripples(cx: f32, cy: f32, r: f32, count: usize, mesh: &mut Mesh) {
    for i in 0..count {
        let k = (i + 1) as f32 / count as f32;
        mesh.ellipse(
            [cx, cy, r * k, r * 0.3 * k, 0.0, 0.0, TAU],
            48,
            Pen { alpha: 0.5 - i as f32 * 0.07, ..Pen::new(0.0012, 0.5).at(i as f32 * 0.08) },
        );
    }
    mesh.node([cx, cy], 0.02, 0.3, 0.0);
}

fn water(aspect: f32, y: f32, amplitude: f32, waves: f32, mesh: &mut Mesh) {
    let mut points = Vec::with_capacity(121);
    for i in 0..=120 {
        let t = i as f32 / 120.0;
        points.push([t * aspect, y + (t * waves * TAU).sin() * amplitude]);
    }
    mesh.path(&points, Pen { span: 0.6, ..Pen::new(0.0014, 0.45) });
}

fn glass(cx: f32, cy: f32, r: f32, mesh: &mut Mesh) {
    mesh.ellipse([cx, cy, r, r, 0.0, 0.0, TAU], 48, Pen::new(0.0022, 0.6));
    mesh.ellipse([cx, cy, r * 0.9, r * 0.9, 0.0, 0.0, TAU], 40, Pen::new(0.0009, 0.3).at(0.1));
    mesh.seg([cx + r * 0.7, cy + r * 0.7], [cx + r * 1.6, cy + r * 1.6], Pen::new(0.004, 0.5).at(0.2));
}

fn gem(cx: f32, cy: f32, r: f32, mesh: &mut Mesh) {
    let mut outer = [[0.0; 2]; 7];
    let mut turn = 0.0;
    for point in &mut outer {
        let a = turn * TAU / 6.0;
        *point = [cx + a.cos() * r, cy + a.sin() * r * 1.3];
        turn += 1.0;
    }
    mesh.path(&outer, Pen::new(0.002, 0.55));
    for i in 0..3 {
        mesh.seg(outer[i], outer[i + 3], Pen::new(0.0009, 0.3).at(0.15 + i as f32 * 0.05));
    }
    for (i, at) in outer.iter().take(6).enumerate() {
        if i % 2 == 0 {
            mesh.node(*at, 0.016, 0.4, i as f32);
        }
    }
}

fn optics(kind: &str, aspect: f32, rng: &mut Rng, mesh: &mut Mesh) {
    let axis = 0.4;
    match kind {
        "optics-convex" => {
            lens(aspect * 0.42, axis, 0.3, true, aspect, 0.0, mesh);
            for (i, y) in [0.3_f32, 0.4, 0.5].into_iter().enumerate() {
                ray_line(
                    aspect,
                    &[[0.02, y], [0.42, y], [0.7, axis], [0.92, axis + (axis - y) * 0.8]],
                    0.2 + i as f32 * 0.08,
                    mesh,
                );
            }
            focus(aspect * 0.7, axis, 0.5, mesh);
        }
        "optics-concave" => {
            lens(aspect * 0.42, axis, 0.28, false, aspect, 0.0, mesh);
            for (i, y) in [0.32_f32, 0.48].into_iter().enumerate() {
                ray_line(aspect, &[[0.02, y], [0.42, y], [0.62, y + (y - axis) * 2.2]], 0.2 + i as f32 * 0.1, mesh);
                mesh.seg([aspect * 0.42, y], [aspect * 0.26, axis], dash(Pen::new(0.0009, 0.35).at(0.5), 0.005, 0.006));
            }
            focus(aspect * 0.26, axis, 0.6, mesh);
        }
        "optics-mirror" => {
            mirror(aspect * 0.5, 0.6, 0.6, 0.0, mesh);
            ray_line(aspect, &[[0.12, 0.08], [0.5, 0.6], [0.88, 0.08]], 0.2, mesh);
        }
        "optics-refract" => {
            mesh.seg([aspect * 0.06, 0.5], [aspect * 0.94, 0.5], Pen { span: 0.5, ..Pen::new(0.002, 0.55) });
            for i in 0..24 {
                let x = aspect * (0.08 + i as f32 / 23.0 * 0.84);
                mesh.seg(
                    [x, 0.51],
                    [x - 0.02, 0.54],
                    Pen { span: 0.1, ..Pen::new(0.0008, 0.22).at(0.2 + i as f32 * 0.01) },
                );
            }
            ray_line(aspect, &[[0.18, 0.05], [0.46, 0.5], [0.58, 0.98]], 0.2, mesh);
            mesh.seg([aspect * 0.46, 0.18], [aspect * 0.46, 0.82], dash(Pen::new(0.0009, 0.35).at(0.4), 0.006, 0.006));
        }
        "optics-bench" => {
            for (i, x) in [0.3_f32, 0.5, 0.7].into_iter().enumerate() {
                lens(aspect * x, axis, 0.16 + (i % 2) as f32 * 0.04, i != 1, aspect, i as f32 * 0.1, mesh);
            }
            ruler(aspect * 0.06, aspect * 0.94, 0.58, 40, mesh);
        }
        "optics-periscope" => {
            mirror(aspect * 0.3, 0.14, 0.14, PI / 4.0, mesh);
            mirror(aspect * 0.3, 0.84, 0.14, PI / 4.0, mesh);
            ray_line(aspect, &[[0.98, 0.14], [0.3, 0.14], [0.3, 0.84], [0.98, 0.84]], 0.2, mesh);
        }
        "optics-fiber" => {
            fiber(
                [[aspect * 0.04, 0.86], [aspect * 0.4, 0.95], [aspect * 0.5, 0.1], [aspect * 0.84, 0.2]],
                0.012,
                mesh,
            );
        }
        "optics-pinhole" => box_of(aspect * 0.45 + 0.15, axis, 0.3, 0.3, mesh),
        "optics-telescope" => {
            lens(aspect * 0.28, axis, 0.36, true, aspect, 0.0, mesh);
            lens(aspect * 0.72, axis, 0.12, true, aspect, 0.1, mesh);
            for (i, y) in [0.3_f32, 0.5].into_iter().enumerate() {
                ray_line(
                    aspect,
                    &[
                        [0.02, y],
                        [0.28, y],
                        [0.62, axis],
                        [0.72, axis - (y - axis) * 0.3],
                        [0.98, axis - (y - axis) * 0.3],
                    ],
                    0.2 + i as f32 * 0.1,
                    mesh,
                );
            }
            focus(aspect * 0.62, axis, 0.5, mesh);
        }
        "optics-raytrace" => {
            for k in 0..4 {
                let mut points = [[0.0; 2]; 6];
                for (i, point) in points.iter_mut().enumerate() {
                    *point = [(0.05 + i as f32 / 5.0 * 0.9) * aspect, 0.08 + rng.next() * 0.6];
                }
                ray_line(aspect, &points, 0.1 + k as f32 * 0.1, mesh);
            }
            sparks(aspect, 16, rng, 0.3, None, mesh);
            return;
        }
        _ => {}
    }
    sparks(aspect, 10, rng, 0.3, Some([0.05, 0.05, 0.95, 0.6]), mesh);
}

fn lens(cx: f32, cy: f32, h: f32, convex: bool, aspect: f32, delay: f32, mesh: &mut Mesh) {
    let bulge = h * 0.18;
    let top = [cx, cy - h / 2.0];
    let bottom = [cx, cy + h / 2.0];
    if convex {
        mesh.cubic(
            top,
            [cx + bulge, cy - h / 4.0],
            [cx + bulge, cy + h / 4.0],
            bottom,
            32,
            Pen::new(0.002, 0.6).at(delay),
        );
        mesh.cubic(
            bottom,
            [cx - bulge, cy + h / 4.0],
            [cx - bulge, cy - h / 4.0],
            top,
            32,
            Pen::new(0.002, 0.6).at(delay),
        );
    } else {
        mesh.path(
            &[[cx - bulge * 0.7, cy - h / 2.0], [cx + bulge * 0.7, cy - h / 2.0]],
            Pen::new(0.002, 0.6).at(delay),
        );
        mesh.cubic(
            [cx + bulge * 0.7, cy - h / 2.0],
            [cx + bulge * 0.1, cy - h / 4.0],
            [cx + bulge * 0.1, cy + h / 4.0],
            [cx + bulge * 0.7, cy + h / 2.0],
            24,
            Pen::new(0.002, 0.6).at(delay),
        );
        mesh.cubic(
            [cx - bulge * 0.7, cy + h / 2.0],
            [cx - bulge * 0.1, cy + h / 4.0],
            [cx - bulge * 0.1, cy - h / 4.0],
            [cx - bulge * 0.7, cy - h / 2.0],
            24,
            Pen::new(0.002, 0.6).at(delay),
        );
    }
    mesh.seg(
        [aspect * 0.05, cy],
        [aspect * 0.95, cy],
        dash(Pen { span: 0.5, ..Pen::new(0.0009, 0.3).at(delay + 0.1) }, 0.008, 0.008),
    );
}

fn ray_line(aspect: f32, frac: &[[f32; 2]], delay: f32, mesh: &mut Mesh) {
    let mut points = Vec::with_capacity(frac.len());
    for p in frac {
        points.push([p[0] * aspect, p[1]]);
    }
    mesh.path(&points, Pen { width: 0.0014, alpha: 0.6, span: 0.5, ..Pen::new(0.0014, 0.6).at(delay) });
    if let Some(end) = points.last() {
        mesh.node(*end, 0.016, delay + 0.5, points.len() as f32);
    }
}

fn focus(x: f32, y: f32, delay: f32, mesh: &mut Mesh) {
    mesh.seg([x - 0.012, y], [x + 0.012, y], Pen::new(0.0012, 0.55).at(delay));
    mesh.seg([x, y - 0.012], [x, y + 0.012], Pen::new(0.0012, 0.55).at(delay));
    mesh.node([x, y], 0.024, delay + 0.3, 0.0);
}

fn mirror(cx: f32, cy: f32, length: f32, angle: f32, mesh: &mut Mesh) {
    let (dx, dy) = (angle.cos() * length / 2.0, angle.sin() * length / 2.0);
    let (nx, ny) = (-angle.sin(), angle.cos());
    mesh.seg([cx - dx, cy - dy], [cx + dx, cy + dy], Pen::new(0.003, 0.6));
    for i in 0..10 {
        let t = -0.45 + i as f32 / 9.0 * 0.9;
        let (bx, by) = (cx + dx * 2.0 * t, cy + dy * 2.0 * t);
        mesh.seg(
            [bx, by],
            [bx + nx * 0.015 - dx * 0.04, by + ny * 0.015 - dy * 0.04],
            Pen { span: 0.1, ..Pen::new(0.0008, 0.3).at(0.2 + i as f32 * 0.01) },
        );
    }
    mesh.seg(
        [cx, cy],
        [cx - nx * length * 0.5, cy - ny * length * 0.5],
        dash(Pen::new(0.0009, 0.35).at(0.25), 0.006, 0.006),
    );
    let normal = (-ny).atan2(-nx);
    mesh.arc([cx, cy, length * 0.12, normal - 0.5, normal + 0.5], 20, Pen::new(0.0009, 0.4).at(0.35));
}

fn ruler(x0: f32, x1: f32, y: f32, ticks: usize, mesh: &mut Mesh) {
    mesh.seg([x0, y], [x1, y], Pen { span: 0.5, ..Pen::new(0.0016, 0.5).at(0.3) });
    for i in 0..=ticks {
        let x = x0 + (x1 - x0) * i as f32 / ticks as f32;
        let long = i % 5 == 0;
        mesh.seg(
            [x, y],
            [x, y + if long { 0.02 } else { 0.01 }],
            Pen { span: 0.08, ..Pen::new(0.0008, 0.4).at(0.5 + i as f32 * 0.005) },
        );
    }
}

fn fiber(points: [[f32; 2]; 4], thickness: f32, mesh: &mut Mesh) {
    let mut center = Vec::with_capacity(61);
    for i in 0..=60 {
        let t = i as f32 / 60.0;
        let u = 1.0 - t;
        let (a, b, c, d) = (u * u * u, 3.0 * u * u * t, 3.0 * u * t * t, t * t * t);
        center.push([
            a * points[0][0] + b * points[1][0] + c * points[2][0] + d * points[3][0],
            a * points[0][1] + b * points[1][1] + c * points[2][1] + d * points[3][1],
        ]);
    }
    for sign in [1.0_f32, -1.0] {
        let mut side = Vec::with_capacity(center.len());
        for i in 0..center.len() {
            let prev = center[i.saturating_sub(1)];
            let next = center[(i + 1).min(center.len() - 1)];
            let (dx, dy) = (next[0] - prev[0], next[1] - prev[1]);
            let len = (dx * dx + dy * dy).sqrt().max(1e-4);
            side.push([center[i][0] - dy / len * thickness * sign, center[i][1] + dx / len * thickness * sign]);
        }
        mesh.path(&side, Pen { span: 0.6, ..Pen::new(0.0014, 0.5).at(if sign < 0.0 { 0.05 } else { 0.0 }) });
    }
    if let Some(end) = center.last() {
        mesh.node(*end, 0.024, 0.6, 0.0);
    }
}

fn box_of(cx: f32, cy: f32, w: f32, h: f32, mesh: &mut Mesh) {
    mesh.path(
        &[
            [cx - w / 2.0, cy - h / 2.0],
            [cx + w / 2.0, cy - h / 2.0],
            [cx + w / 2.0, cy + h / 2.0],
            [cx - w / 2.0, cy + h / 2.0],
            [cx - w / 2.0, cy - h / 2.0],
        ],
        Pen::new(0.0018, 0.5),
    );
    mesh.seg(
        [cx - w / 2.0 - 0.25, cy - h * 0.35],
        [cx - w / 2.0 - 0.25, cy + h * 0.35],
        Pen::new(0.0016, 0.45).at(0.1),
    );
    mesh.seg(
        [cx + w / 2.0 - 0.02, cy + h * 0.28],
        [cx + w / 2.0 - 0.02, cy - h * 0.28],
        dash(Pen::new(0.0014, 0.4).at(0.3), 0.005, 0.005),
    );
    mesh.seg([cx - w / 2.0 - 0.25, cy - h * 0.35], [cx + w / 2.0 - 0.02, cy + h * 0.28], Pen::new(0.0008, 0.3).at(0.2));
    mesh.seg([cx - w / 2.0 - 0.25, cy + h * 0.35], [cx + w / 2.0 - 0.02, cy - h * 0.28], Pen::new(0.0008, 0.3).at(0.2));
    mesh.node([cx - w / 2.0, cy], 0.02, 0.3, 0.0);
}

fn wave(kind: &str, aspect: f32, rng: &mut Rng, mesh: &mut Mesh) {
    match kind {
        "wave-slits" => {
            slit(aspect * 0.5, 0.3, aspect * 0.5, 0.012, 0.08, mesh);
            frame(aspect, [0.2, 0.52, 0.8, 0.92], 0.2, 0.28, mesh);
        }
        "wave-newton" => rings_at(aspect * 0.5, 0.5, 0.4, 6, mesh),
        "wave-airy" => rings_at(aspect * 0.5, 0.4, 0.3, 4, mesh),
        "wave-grating" => grating(aspect * 0.5, 0.3, aspect * 0.4, 16, mesh),
        "wave-sources" => {
            for (shift, delay) in [(-0.07, 0.0), (0.07, 0.1)] {
                for (i, k) in [0.25_f32, 0.5, 0.75, 1.0].into_iter().enumerate() {
                    mesh.ellipse(
                        [aspect * 0.5 + shift, 0.5, 0.3 * k, 0.3 * k, 0.0, 0.0, TAU],
                        40,
                        Pen::new(0.0011, 0.3).at(delay + i as f32 * 0.06),
                    );
                }
                mesh.node([aspect * 0.5 + shift, 0.5], 0.024, 0.3 + delay, delay * 10.0);
            }
        }
        "wave-moire" => moire(aspect * 0.5, 0.32, 0.4, 24, 0.08, mesh),
        "wave-film" => {
            mesh.ellipse([aspect * 0.5, 0.4, 0.3, 0.3, 0.0, 0.0, TAU], 64, Pen::new(0.0016, 0.45));
            mesh.ellipse([aspect * 0.5 - 0.08, 0.3, 0.06, 0.06, 0.0, 0.0, TAU], 24, Pen::new(0.001, 0.3).at(0.2));
        }
        "wave-polar" => polar(aspect * 0.5, 0.36, 0.1, 0.24, PI / 3.0, mesh),
        "wave-standing" => standing(aspect * 0.08, aspect * 0.92, 0.34, 0.07, 6.0, mesh),
        "wave-holo" => grid([aspect * 0.3, 0.15], [aspect * 0.4, 0.7], 6, 8, true, mesh),
        _ => {}
    }
    sparks(aspect, 12, rng, 0.3, None, mesh);
}

fn slit(cx: f32, y: f32, width: f32, gap: f32, separation: f32, mesh: &mut Mesh) {
    let a = cx - separation / 2.0;
    let b = cx + separation / 2.0;
    let spans = [[cx - width / 2.0, a - gap / 2.0], [a + gap / 2.0, b - gap / 2.0], [b + gap / 2.0, cx + width / 2.0]];
    for (i, [from, to]) in spans.into_iter().enumerate() {
        mesh.seg([from, y], [to, y], Pen { span: 0.3, ..Pen::new(0.0026, 0.6).at(i as f32 * 0.08) });
    }
    mesh.node([a, y], 0.022, 0.35, 0.0);
    mesh.node([b, y], 0.022, 0.4, 2.0);
}

fn rings_at(cx: f32, cy: f32, r: f32, count: usize, mesh: &mut Mesh) {
    for i in 0..count {
        let radius = r * ((i + 1) as f32 / count as f32).sqrt();
        mesh.ellipse([cx, cy, radius, radius, 0.0, 0.0, TAU], 64, Pen::new(0.0011, 0.45).at(i as f32 * 0.06));
    }
}

fn grating(cx: f32, y: f32, width: f32, slits: usize, mesh: &mut Mesh) {
    mesh.seg([cx - width / 2.0, y], [cx + width / 2.0, y], Pen::new(0.0022, 0.55));
    for i in 0..slits {
        let x = cx - width * 0.3 + width * 0.6 * i as f32 / (slits as f32 - 1.0).max(1.0);
        mesh.seg([x, y - 0.01], [x, y + 0.01], Pen { span: 0.1, ..Pen::new(0.0008, 0.4).at(0.15 + i as f32 * 0.01) });
    }
}

fn moire(cx: f32, cy: f32, size: f32, count: usize, rotation: f32, mesh: &mut Mesh) {
    for (set, angle) in [(0.0_f32, 0.0_f32), (0.2, rotation)] {
        for i in 0..count {
            let t = -0.5 + i as f32 / (count as f32 - 1.0);
            let ox = (angle + PI / 2.0).cos() * t * size;
            let oy = (angle + PI / 2.0).sin() * t * size;
            mesh.seg(
                [cx + ox - angle.cos() * size / 2.0, cy + oy - angle.sin() * size / 2.0],
                [cx + ox + angle.cos() * size / 2.0, cy + oy + angle.sin() * size / 2.0],
                Pen { width: 0.0008, alpha: 0.3, span: 0.15, ..Pen::new(0.0008, 0.3).at(set + i as f32 * 0.008) },
            );
        }
    }
}

fn polar(cx: f32, cy: f32, r: f32, gap: f32, rotation: f32, mesh: &mut Mesh) {
    for (x, angle, delay) in [(cx - gap / 2.0, 0.0, 0.0), (cx + gap / 2.0, rotation, 0.2)] {
        mesh.ellipse([x, cy, r, r, 0.0, 0.0, TAU], 40, Pen::new(0.0016, 0.5).at(delay));
        for i in 0..7 {
            let t = -0.75 + i as f32 / 6.0 * 1.5;
            let half = (1.0 - t * t).max(0.0).sqrt() * r;
            let ox = -angle.sin() * t * r;
            let oy = angle.cos() * t * r;
            mesh.seg(
                [x + ox - angle.cos() * half, cy + oy - angle.sin() * half],
                [x + ox + angle.cos() * half, cy + oy + angle.sin() * half],
                Pen { span: 0.15, ..Pen::new(0.0008, 0.3).at(delay + 0.15 + i as f32 * 0.02) },
            );
        }
    }
}

fn standing(x0: f32, x1: f32, y: f32, amplitude: f32, loops: f32, mesh: &mut Mesh) {
    for (j, k) in [-1.0_f32, -0.5, 0.5, 1.0].into_iter().enumerate() {
        let mut points = Vec::with_capacity(121);
        for i in 0..=120 {
            let t = i as f32 / 120.0;
            points.push([x0 + (x1 - x0) * t, y + (t * loops * PI).sin() * amplitude * k]);
        }
        mesh.path(&points, Pen { alpha: 0.35 + k.abs() * 0.2, span: 0.5, ..Pen::new(0.0012, 0.4).at(j as f32 * 0.06) });
    }
    let count = loops as usize + 1;
    for i in 0..count {
        mesh.node([x0 + (x1 - x0) * i as f32 / loops, y], 0.014, 0.5, i as f32);
    }
}

fn sprout_plate(aspect: f32, rng: &mut Rng, mesh: &mut Mesh) {
    let cx = aspect * 0.5;
    let base_y = 0.78;
    let size = 0.22;
    mesh.cubic(
        [cx, 1.04],
        [cx + 0.01, 0.94],
        [cx - 0.008, base_y + 0.08],
        [cx, base_y],
        32,
        Pen { span: 0.3, ..Pen::new(0.0016, 0.55) },
    );
    for (index, side) in [-1.0_f32, 1.0].into_iter().enumerate() {
        let lean = 0.4 * PI;
        let d = [lean.sin() * side, -lean.cos()];
        let n = [-d[1], d[0]];
        let at = |along: f32, across: f32| {
            [
                cx + d[0] * size * along + n[0] * across,
                base_y + d[1] * size * along + n[1] * across - size * 0.12 * along * along,
            ]
        };
        let tip = at(1.0, 0.0);
        let bulge = size * 0.34;
        let leaf_delay = 0.22 + index as f32 * 0.08;
        mesh.cubic(
            [cx, base_y],
            at(0.3, bulge),
            at(0.78, bulge * 0.9),
            tip,
            48,
            Pen { span: 0.35, ..Pen::new(0.0026, 0.55).at(leaf_delay) },
        );
        mesh.cubic(
            [cx, base_y],
            at(0.3, -bulge * 0.95),
            at(0.8, -bulge * 0.6),
            tip,
            48,
            Pen { span: 0.35, ..Pen::new(0.0026, 0.55).at(leaf_delay + 0.04) },
        );
        mesh.cubic(
            [cx, base_y],
            at(0.35, bulge * 0.08),
            at(0.7, bulge * 0.05),
            tip,
            32,
            Pen { width: 0.0011, alpha: 0.38, span: 0.3, ..Pen::new(0.0011, 0.38).at(leaf_delay + 0.12) },
        );
        mesh.node(tip, 0.03, leaf_delay + 0.3, index as f32 * 2.1);
    }
    mesh.node([cx, base_y], 0.02, 0.3, 0.5);
    halo(cx, 0.02, 0.22, 0.35, 0.2, mesh);
    frame(aspect, [0.07, 0.22, 0.93, 0.8], 0.25, 0.22, mesh);
    sparks(aspect, 12, rng, 0.35, Some([0.2, 0.3, 0.8, 0.9]), mesh);
}

fn leaf(cx: f32, cy: f32, length: f32, angle: f32, mesh: &mut Mesh) {
    let d = [angle.cos(), angle.sin()];
    let n = [-d[1], d[0]];
    let at = |along: f32, across: f32| {
        [cx + d[0] * length * (along - 0.5) + n[0] * across, cy + d[1] * length * (along - 0.5) + n[1] * across]
    };
    let bulge = length * 0.3;
    mesh.cubic(
        at(0.0, 0.0),
        at(0.3, bulge),
        at(0.75, bulge * 0.8),
        at(1.0, 0.0),
        48,
        Pen { span: 0.4, ..Pen::new(0.002, 0.6) },
    );
    mesh.cubic(
        at(0.0, 0.0),
        at(0.3, -bulge),
        at(0.75, -bulge * 0.8),
        at(1.0, 0.0),
        48,
        Pen { span: 0.4, ..Pen::new(0.002, 0.6).at(0.05) },
    );
    mesh.seg(at(-0.12, 0.0), at(1.0, 0.0), Pen { span: 0.4, ..Pen::new(0.0014, 0.5).at(0.15) });
    for v in 1..=6 {
        let f = v as f32 / 7.5;
        for s in [1.0_f32, -1.0] {
            mesh.cubic(
                at(f, 0.0),
                at(f + 0.05, s * bulge * 0.3),
                at(f + 0.1, s * bulge * 0.55),
                at(f + 0.15, s * bulge * (0.8 - f * 0.35)),
                16,
                Pen { width: 0.0009, alpha: 0.4, span: 0.18, ..Pen::new(0.0009, 0.4).at(0.25 + v as f32 * 0.05) },
            );
        }
    }
    mesh.node(at(1.0, 0.0), 0.026, 0.6, 0.0);
}

struct Shoot {
    x: f32,
    y: f32,
    angle: f32,
    length: f32,
    depth: i32,
    delay: f32,
}

fn canopy(aspect: f32, rng: &mut Rng, mesh: &mut Mesh) {
    fn grow(mesh: &mut Mesh, rng: &mut Rng, shoot: Shoot) {
        let Shoot { x, y, angle, length, depth, delay } = shoot;
        let x1 = x + angle.cos() * length;
        let y1 = y + angle.sin() * length;
        mesh.seg(
            [x, y],
            [x1, y1],
            Pen {
                width: 0.0008 + depth as f32 * 0.0005,
                alpha: 0.3 + depth as f32 * 0.08,
                span: 0.2,
                ..Pen::new(0.001, 0.4).at(delay)
            },
        );
        if depth <= 0 {
            return;
        }
        let left = rng.next();
        let right = rng.next();
        let shorter = length * 0.72;
        grow(
            mesh,
            rng,
            Shoot {
                x: x1,
                y: y1,
                angle: angle - 0.35 - left * 0.3,
                length: shorter,
                depth: depth - 1,
                delay: delay + 0.08,
            },
        );
        grow(
            mesh,
            rng,
            Shoot {
                x: x1,
                y: y1,
                angle: angle + 0.35 + right * 0.3,
                length: shorter,
                depth: depth - 1,
                delay: delay + 0.08,
            },
        );
    }
    for (i, fx) in [0.08_f32, 0.35, 0.65, 0.92].into_iter().enumerate() {
        let sway = (rng.next() - 0.5) * 0.6;
        grow(
            mesh,
            rng,
            Shoot { x: fx * aspect, y: -0.02, angle: PI / 2.0 + sway, length: 0.14, depth: 4, delay: i as f32 * 0.05 },
        );
    }
}

fn fern(cx: f32, cy: f32, r: f32, mesh: &mut Mesh) {
    let mut spiral = Vec::with_capacity(140);
    for i in 0..140 {
        let t = i as f32 / 139.0;
        let a = t * TAU * 2.2;
        let rr = r * (-t * 2.4).exp();
        spiral.push([cx + a.cos() * rr, cy + a.sin() * rr]);
    }
    mesh.seg([cx + r, cy], [cx + r, cy + r * 2.2], Pen::new(0.0016, 0.5));
    mesh.path(&spiral, Pen { span: 0.5, ..Pen::new(0.0016, 0.55).at(0.1) });
    for i in 0..9 {
        let [x, y] = spiral[(i * 90) / 9];
        let radius = 0.008 + (1.0 - i as f32 / 9.0) * 0.01;
        mesh.ellipse(
            [x, y, radius, radius, 0.0, 0.0, TAU],
            12,
            Pen { span: 0.1, ..Pen::new(0.0008, 0.35).at(0.4 + i as f32 * 0.03) },
        );
    }
}

fn roots(cx: f32, cy: f32, rng: &mut Rng, mesh: &mut Mesh) {
    mesh.ellipse([cx, cy, 0.025, 0.035, 0.3, 0.0, TAU], 32, Pen::new(0.002, 0.6));
    for i in 0..6 {
        let spread = (i as f32 - 2.5) * 0.05;
        mesh.cubic(
            [cx, cy + 0.03],
            [cx + spread * 0.4, cy + 0.08],
            [cx + spread, cy + 0.12],
            [cx + spread * 1.4 + (rng.next() - 0.5) * 0.04, cy + 0.2 + rng.next() * 0.06],
            24,
            Pen { width: 0.0009, alpha: 0.4, span: 0.35, ..Pen::new(0.0009, 0.4).at(0.2 + i as f32 * 0.05) },
        );
    }
    mesh.node([cx, cy - 0.04], 0.03, 0.2, 0.0);
}

fn flower(cx: f32, cy: f32, r: f32, petals: usize, mesh: &mut Mesh) {
    for i in 0..petals {
        let a = i as f32 / petals as f32 * TAU;
        let d = [a.cos(), a.sin()];
        let n = [-d[1], d[0]];
        let tip = [cx + d[0] * r, cy + d[1] * r];
        let w = r * 0.35;
        let mut points = Vec::new();
        for step in 0..=20 {
            let t = step as f32 / 20.0;
            let u = 1.0 - t;
            let (aa, bb, cc, dd) = (u * u * u, 3.0 * u * u * t, 3.0 * u * t * t, t * t * t);
            let p0 = [cx, cy];
            let p1 = [cx + d[0] * r * 0.4 + n[0] * w, cy + d[1] * r * 0.4 + n[1] * w];
            let p2 = [cx + d[0] * r * 0.9 + n[0] * w * 0.6, cy + d[1] * r * 0.9 + n[1] * w * 0.6];
            points.push([
                aa * p0[0] + bb * p1[0] + cc * p2[0] + dd * tip[0],
                aa * p0[1] + bb * p1[1] + cc * p2[1] + dd * tip[1],
            ]);
        }
        for step in 0..=20 {
            let t = step as f32 / 20.0;
            let u = 1.0 - t;
            let (aa, bb, cc, dd) = (u * u * u, 3.0 * u * u * t, 3.0 * u * t * t, t * t * t);
            let p1 = [cx + d[0] * r * 0.9 - n[0] * w * 0.6, cy + d[1] * r * 0.9 - n[1] * w * 0.6];
            let p2 = [cx + d[0] * r * 0.4 - n[0] * w, cy + d[1] * r * 0.4 - n[1] * w];
            let p3 = [cx, cy];
            points.push([
                aa * tip[0] + bb * p1[0] + cc * p2[0] + dd * p3[0],
                aa * tip[1] + bb * p1[1] + cc * p2[1] + dd * p3[1],
            ]);
        }
        mesh.path(&points, Pen { span: 0.3, ..Pen::new(0.0016, 0.5).at(i as f32 * 0.05) });
    }
    mesh.ellipse([cx, cy, r * 0.12, r * 0.12, 0.0, 0.0, TAU], 16, Pen::new(0.0012, 0.5).at(0.4));
    mesh.node([cx, cy], 0.03, 0.5, 0.0);
}

fn vine(x: f32, bottom: f32, top: f32, amplitude: f32, turns: f32, mesh: &mut Mesh) {
    let mut points = Vec::with_capacity(161);
    for i in 0..=160 {
        let t = i as f32 / 160.0;
        points.push([x + (t * turns * TAU).sin() * amplitude * (1.0 - t * 0.4), bottom + (top - bottom) * t]);
    }
    mesh.path(&points, Pen { span: 0.7, ..Pen::new(0.0016, 0.55) });
    mesh.seg([x, bottom], [x, top], dash(Pen { span: 0.5, ..Pen::new(0.0008, 0.25).at(0.1) }, 0.005, 0.008));
    mesh.node([x, top], 0.024, 0.7, 0.0);
}

fn cells(cx: f32, cy: f32, r: f32, rng: &mut Rng, count: usize, mesh: &mut Mesh) {
    mesh.ellipse([cx, cy, r, r, 0.0, 0.0, TAU], 64, Pen::new(0.0022, 0.5));
    for i in 0..count {
        let a = rng.next() * TAU;
        let d = rng.next().sqrt() * r * 0.8;
        let rx = 0.012 + rng.next() * 0.012;
        let ry = 0.008 + rng.next() * 0.008;
        let rot = rng.next() * PI;
        mesh.ellipse(
            [cx + a.cos() * d, cy + a.sin() * d, rx, ry, rot, 0.0, TAU],
            16,
            Pen { span: 0.15, ..Pen::new(0.0009, 0.4).at(0.15 + i as f32 * 0.02) },
        );
    }
}

fn molecule(cx: f32, cy: f32, r: f32, delay: f32, mesh: &mut Mesh) {
    let mut hex = [[0.0; 2]; 7];
    let mut turn = 0.0;
    for point in &mut hex {
        let a = turn * TAU / 6.0 + PI / 6.0;
        *point = [cx + a.cos() * r, cy + a.sin() * r];
        turn += 1.0;
    }
    mesh.path(&hex, Pen::new(0.0018, 0.55).at(delay));
    mesh.ellipse([cx, cy, r * 0.55, r * 0.55, 0.0, 0.0, TAU], 32, Pen::new(0.001, 0.35).at(delay + 0.2));
    for (i, k) in [0, 2, 4].into_iter().enumerate() {
        let [x, y] = hex[k];
        let end = [x + (x - cx) * 0.8, y + (y - cy) * 0.8];
        mesh.seg([x, y], end, Pen::new(0.0012, 0.45).at(delay + 0.3 + i as f32 * 0.05));
        mesh.node(end, 0.02, delay + 0.45, i as f32);
    }
}

fn orbits(cx: f32, cy: f32, radii: &[f32], tilt: f32, flatten: f32, rng: &mut Rng, mesh: &mut Mesh) {
    for (i, r) in radii.iter().enumerate() {
        mesh.ellipse(
            [cx, cy, *r, r * flatten, tilt, 0.0, TAU],
            64,
            Pen { span: 0.5, ..Pen::new(0.0012, 0.45).at(i as f32 * 0.07) },
        );
    }
    mesh.node([cx, cy], 0.04, 0.2, 0.0);
    for (i, r) in radii.iter().enumerate() {
        let a = rng.next() * TAU;
        let (co, si) = (tilt.cos(), tilt.sin());
        let (x, y) = (a.cos(), a.sin());
        mesh.node(
            [cx + x * r * co - y * r * flatten * si, cy + x * r * si + y * r * flatten * co],
            0.018 + rng.next() * 0.012,
            0.5 + i as f32 * 0.05,
            i as f32,
        );
    }
}

fn trails(cx: f32, cy: f32, rng: &mut Rng, count: usize, max_r: f32, mesh: &mut Mesh) {
    for i in 0..count {
        let r = max_r * (0.1 + rng.next() * 0.9);
        let a = rng.next() * TAU;
        let span = 0.4 + rng.next() * 0.8;
        let width = 0.0008 + rng.next() * 0.0008;
        let alpha = 0.25 + rng.next() * 0.3;
        mesh.arc(
            [cx, cy, r, a, a + span],
            24,
            Pen { width, alpha, span: 0.3, ..Pen::new(width, alpha).at(i as f32 / count as f32 * 0.4) },
        );
    }
    mesh.node([cx, cy], 0.03, 0.1, 0.0);
}

fn sextant(cx: f32, cy: f32, r: f32, mesh: &mut Mesh) {
    mesh.arc([cx, cy, r, PI * 0.33, PI * 0.67], 48, Pen::new(0.002, 0.6));
    mesh.seg([cx, cy], [cx + (PI * 0.33).cos() * r, cy + (PI * 0.33).sin() * r], Pen::new(0.0014, 0.5).at(0.1));
    mesh.seg([cx, cy], [cx + (PI * 0.67).cos() * r, cy + (PI * 0.67).sin() * r], Pen::new(0.0014, 0.5).at(0.12));
    mesh.seg(
        [cx, cy],
        [cx + (PI * 0.45).cos() * r * 1.1, cy + (PI * 0.45).sin() * r * 1.1],
        dash(Pen::new(0.001, 0.45).at(0.3), 0.006, 0.006),
    );
    mesh.node([cx, cy], 0.022, 0.3, 0.0);
    let step = (PI * 0.67 - PI * 0.33) / 30.0;
    for i in 0..30 {
        let a = PI * 0.33 + step * (i as f32 + 0.5);
        mesh.seg(
            [cx + a.cos() * r * 0.94, cy + a.sin() * r * 0.94],
            [cx + a.cos() * r, cy + a.sin() * r],
            Pen { span: 0.2, ..Pen::new(0.001, 0.4).at(0.2 + i as f32 / 30.0 * 0.3) },
        );
    }
}

fn stars(aspect: f32, rng: &mut Rng, count: usize, region: [f32; 4], delay: f32, mesh: &mut Mesh) {
    let [x0, y0, x1, y1] = region;
    let mut points = Vec::with_capacity(count);
    for _ in 0..count {
        points.push([(x0 + rng.next() * (x1 - x0)) * aspect, y0 + rng.next() * (y1 - y0)]);
    }
    points.sort_by(|a, b| a[0].partial_cmp(&b[0]).unwrap_or(std::cmp::Ordering::Equal));
    for i in 1..points.len() {
        mesh.seg(points[i - 1], points[i], Pen { span: 0.2, ..Pen::new(0.001, 0.4).at(delay + 0.2 + i as f32 * 0.06) });
    }
    for (i, at) in points.iter().enumerate() {
        mesh.node(*at, 0.018 + rng.next() * 0.016, delay + i as f32 * 0.05, i as f32);
    }
}

fn corona(cx: f32, cy: f32, r: f32, rng: &mut Rng, mesh: &mut Mesh) {
    mesh.ellipse([cx, cy, r, r, 0.0, 0.0, TAU], 64, Pen::new(0.0024, 0.65));
    for i in 0..40 {
        let a = i as f32 / 40.0 * TAU + rng.next() * 0.05;
        let len = r * (0.25 + rng.next() * 0.9);
        let alpha = 0.25 + rng.next() * 0.3;
        mesh.seg(
            [cx + a.cos() * r * 1.05, cy + a.sin() * r * 1.05],
            [cx + a.cos() * (r + len), cy + a.sin() * (r + len)],
            Pen { width: 0.0008, alpha, span: 0.15, ..Pen::new(0.0008, alpha).at(0.2 + i as f32 / 40.0 * 0.3) },
        );
    }
    mesh.node([cx + r * 0.7, cy - r * 0.7], 0.03, 0.5, 0.0);
}

fn moon(cx: f32, cy: f32, r: f32, mesh: &mut Mesh) {
    mesh.arc([cx, cy, r, PI * 0.35, PI * 1.65], 60, Pen::new(0.002, 0.6));
    mesh.arc([cx + r * 0.45, cy, r * 0.85, PI * 0.58, PI * 1.42], 60, Pen::new(0.0012, 0.4).at(0.15));
}

fn rings(cx: f32, cy: f32, r: f32, mesh: &mut Mesh) {
    mesh.ellipse([cx, cy, r, r, 0.0, 0.0, TAU], 64, Pen::new(0.002, 0.55));
    mesh.ellipse([cx, cy, r, r * 0.3, 0.0, 0.0, TAU], 48, Pen::new(0.0014, 0.45).at(0.1));
    mesh.ellipse([cx, cy, r, r * 0.3, 0.41, 0.0, TAU], 48, Pen::new(0.0014, 0.45).at(0.15));
    mesh.ellipse([cx, cy, r * 0.3, r, 0.0, 0.0, TAU], 48, Pen::new(0.0012, 0.4).at(0.2));
    mesh.seg(
        [cx - 0.41_f32.sin() * r * 1.25, cy - 0.41_f32.cos() * r * 1.25],
        [cx + 0.41_f32.sin() * r * 1.25, cy + 0.41_f32.cos() * r * 1.25],
        Pen::new(0.0012, 0.45).at(0.3),
    );
    mesh.node([cx, cy], 0.03, 0.3, 0.0);
}

fn meteors(aspect: f32, rng: &mut Rng, count: usize, mesh: &mut Mesh) {
    for i in 0..count {
        let x = (0.2 + rng.next() * 0.8) * aspect;
        let y = rng.next() * 0.5;
        let len = 0.12 + rng.next() * 0.25;
        let head = [x - len * 0.8, y + len * 0.6];
        let width = 0.0012 + rng.next() * 0.001;
        mesh.seg([x, y], head, Pen { width, span: 0.2, ..Pen::new(width, 0.45).at(i as f32 * 0.08) });
        mesh.node(head, 0.02, i as f32 * 0.08 + 0.2, i as f32);
    }
}

fn lamps(profile: &Profile, aspect: f32, mesh: &mut Mesh) {
    let (beams, count) = beams_for(profile, aspect, 1);
    for (i, beam) in beams.iter().take(count).enumerate() {
        lamp(beam.x * aspect, beam.y, beam.angle, 0.035, i as f32 * 0.05, mesh);
    }
}

fn lamp(x: f32, y: f32, angle: f32, size: f32, delay: f32, mesh: &mut Mesh) {
    let d = [angle.cos(), angle.sin()];
    let n = [-d[1], d[0]];
    let back = [x - d[0] * size, y - d[1] * size];
    let corners = [
        [back[0] + n[0] * size * 0.35, back[1] + n[1] * size * 0.35],
        [x + n[0] * size * 0.6, y + n[1] * size * 0.6],
        [x - n[0] * size * 0.6, y - n[1] * size * 0.6],
        [back[0] - n[0] * size * 0.35, back[1] - n[1] * size * 0.35],
        [back[0] + n[0] * size * 0.35, back[1] + n[1] * size * 0.35],
    ];
    mesh.path(&corners, Pen { span: 0.3, ..Pen::new(0.0018, 0.55).at(delay) });
}

fn curtains(aspect: f32, gap: f32, mesh: &mut Mesh) {
    for side in [-1.0_f32, 1.0] {
        for i in 0..6 {
            let base = aspect / 2.0 + side * (gap / 2.0 + i as f32 * 0.05);
            let mut points = Vec::with_capacity(41);
            for k in 0..=40 {
                let t = k as f32 / 40.0;
                points.push([base + (t * 5.0 + i as f32).sin() * 0.008 * side, -0.02 + t * 1.04]);
            }
            mesh.path(
                &points,
                Pen {
                    width: 0.001 + (5 - i) as f32 * 0.0002,
                    alpha: 0.2 + (5 - i) as f32 * 0.05,
                    span: 0.5,
                    ..Pen::new(0.001, 0.3).at(i as f32 * 0.05)
                },
            );
        }
    }
}

fn machine(x: f32, y: f32, size: f32, mesh: &mut Mesh) {
    mesh.path(
        &[
            [x - size, y - size * 0.3],
            [x, y - size * 0.3],
            [x, y + size * 0.3],
            [x - size, y + size * 0.3],
            [x - size, y - size * 0.3],
        ],
        Pen::new(0.0018, 0.55),
    );
    mesh.ellipse(
        [x - size * 0.75, y - size * 0.65, size * 0.33, size * 0.33, 0.0, 0.0, TAU],
        32,
        Pen::new(0.0014, 0.5).at(0.1),
    );
    mesh.ellipse(
        [x - size * 0.25, y - size * 0.65, size * 0.33, size * 0.33, 0.0, 0.0, TAU],
        32,
        Pen::new(0.0014, 0.5).at(0.15),
    );
    mesh.path(
        &[
            [x, y - size * 0.12],
            [x + size * 0.18, y - size * 0.18],
            [x + size * 0.18, y + size * 0.18],
            [x, y + size * 0.12],
        ],
        Pen::new(0.0014, 0.5).at(0.2),
    );
    mesh.node([x + size * 0.18, y], 0.024, 0.3, 0.0);
}

fn horizon(aspect: f32, y: f32, mesh: &mut Mesh) {
    mesh.seg([0.0, y], [aspect, y], Pen { span: 0.6, ..Pen::new(0.0014, 0.4) });
    for i in 0..16 {
        let x = aspect * (i as f32 + 0.5) / 16.0;
        mesh.seg([x, y], [x, y + 0.012], Pen { span: 0.08, ..Pen::new(0.0008, 0.3).at(0.3 + i as f32 * 0.01) });
    }
}

const LINE_FLOATS: usize = 16;
/// Line quads stop at this many floats so the GPU buffer can be created once.
pub const MAX_LINE_FLOATS: usize = 1_048_576;

struct Quad {
    along: f32,
    len: f32,
    total: f32,
    mid: [f32; 2],
    width: f32,
    alpha: f32,
    delay: f32,
    span: f32,
    gain: f32,
    begin: f32,
    end: f32,
}

/// Expand the mesh into line quads. `out` keeps its capacity across shots.
pub fn write_lines(mesh: &Mesh, begin: f32, end: f32, gain: f32, out: &mut Vec<f32>) {
    for stroke in &mesh.strokes {
        let start = stroke.start as usize;
        let count = stroke.count as usize;
        if count < 2 || start + count > mesh.pts.len() {
            continue;
        }
        let points = &mesh.pts[start..start + count];
        let mut lengths = [0.0_f32; 512];
        let n = points.len().min(lengths.len());
        for i in 1..n {
            let d = points[i][0] - points[i - 1][0];
            let e = points[i][1] - points[i - 1][1];
            lengths[i] = lengths[i - 1] + (d * d + e * e).sqrt();
        }
        let total = lengths[n - 1].max(1e-4);
        let mid = points[n / 2];
        let period = stroke.dash_on + stroke.dash_off;
        for i in 1..n {
            let seg_start = lengths[i - 1];
            let seg_end = lengths[i];
            if period <= 0.0 {
                emit(
                    points[i - 1],
                    points[i],
                    &quad_of(
                        [seg_start, seg_end - seg_start, total],
                        mid,
                        stroke,
                        [begin, end, gain],
                        [stroke.width, stroke.alpha],
                    ),
                    out,
                );
                continue;
            }
            let mut k = (seg_start / period).floor() as i32;
            while (k as f32) * period < seg_end {
                let from = seg_start.max(k as f32 * period);
                let to = seg_end.min(k as f32 * period + stroke.dash_on);
                if to > from {
                    let a = point_at(points, &lengths, n, i, from);
                    let b = point_at(points, &lengths, n, i, to);
                    emit(
                        a,
                        b,
                        &quad_of(
                            [from, to - from, total],
                            mid,
                            stroke,
                            [begin, end, gain],
                            [stroke.width, stroke.alpha],
                        ),
                        out,
                    );
                }
                k += 1;
                if k > 4096 {
                    break;
                }
            }
        }
    }
}

fn point_at(points: &[[f32; 2]], lengths: &[f32], n: usize, index: usize, distance: f32) -> [f32; 2] {
    let index = index.min(n - 1).max(1);
    let seg = (lengths[index] - lengths[index - 1]).max(1e-6);
    let f = ((distance - lengths[index - 1]) / seg).clamp(0.0, 1.0);
    let a = points[index - 1];
    let b = points[index];
    [a[0] + (b[0] - a[0]) * f, a[1] + (b[1] - a[1]) * f]
}

fn quad_of(span: [f32; 3], mid: [f32; 2], stroke: &Stroke, when: [f32; 3], ink: [f32; 2]) -> Quad {
    let [along, len, total] = span;
    let [begin, end, gain] = when;
    let [width, alpha] = ink;
    Quad { along, len, total, mid, width, alpha, delay: stroke.delay, span: stroke.span, gain, begin, end }
}

fn emit(a: [f32; 2], b: [f32; 2], quad: &Quad, out: &mut Vec<f32>) {
    if quad.len < 1e-6 || out.len() + LINE_FLOATS * 12 > MAX_LINE_FLOATS {
        return;
    }
    for (width, alpha) in [(quad.width * 3.2, quad.alpha * 0.12), (quad.width, quad.alpha)] {
        for (t, side) in [(0.0, -1.0), (1.0, -1.0), (1.0, 1.0), (0.0, -1.0), (1.0, 1.0), (0.0, 1.0)] {
            let p = [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t];
            out.extend_from_slice(&[
                p[0],
                p[1],
                b[0] - a[0],
                b[1] - a[1],
                quad.along + quad.len * t,
                side,
                width,
                alpha,
                quad.delay,
                quad.span,
                quad.total,
                quad.mid[0],
                quad.mid[1],
                quad.gain,
                quad.begin,
                quad.end,
            ]);
        }
    }
}
