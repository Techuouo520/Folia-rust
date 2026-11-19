// RadioWall's continuous rail, queue identities and geometry from the original radioWallModel.

use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};

use iced::{Point, Rectangle, Size};

use crate::wall::Tile;

#[derive(Clone, Copy, Debug)]
pub struct Metrics {
    pub size: Size,
    pub now_size: f32,
    pub card_size: f32,
    pub gap: f32,
    pub center: Point,
    pub radius: f32,
    pub side_drop: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rect {
    pub bounds: Rectangle,
    pub rotate: f32,
}

impl Metrics {
    pub fn new(size: Size) -> Self {
        let compact = size.width < 640.0;
        let now_size = if compact {
            (size.width - 48.0).min(size.height * 0.62)
        } else {
            (size.height * 0.56).min(size.width * 0.46).min(760.0)
        }
        .round()
        .max(160.0);
        Self {
            size,
            now_size,
            card_size: (now_size * if compact { 0.4 } else { 0.58 }).round().max(88.0),
            gap: if compact { 12.0 } else { (now_size * 0.06).clamp(16.0, 36.0).round() },
            center: Point::new(size.width / 2.0, size.height * 0.55),
            radius: (size.width * if compact { 1.6 } else { 1.5 }).max(900.0),
            side_drop: (now_size * 0.12).round(),
        }
    }

    pub fn step(self) -> f32 {
        self.now_size / 2.0 + self.gap + self.card_size / 2.0
    }

    pub fn wave_center(self) -> Point {
        Point::new(self.center.x, self.center.y + self.radius + self.side_drop)
    }

    pub fn upper_rings(self) -> [f32; 3] {
        [0.2, 0.45, 0.7].map(|top| self.wave_center().y - (self.center.y - self.now_size / 2.0) * top)
    }

    pub fn rect(self, offset: f32) -> Rect {
        let at = offset.clamp(-6.0, 6.0);
        let low = at.floor();
        let a = self.slot(low as i32);
        let b = self.slot(low as i32 + 1);
        let t = at - low;
        let mix = |a, b| a + (b - a) * t;
        Rect {
            bounds: Rectangle {
                x: mix(a.bounds.x, b.bounds.x),
                y: mix(a.bounds.y, b.bounds.y),
                width: mix(a.bounds.width, b.bounds.width),
                height: mix(a.bounds.height, b.bounds.height),
            },
            rotate: mix(a.rotate, b.rotate),
        }
    }

    fn slot(self, offset: i32) -> Rect {
        let steps = offset.unsigned_abs();
        let mut size = self.now_size;
        let mut along = 0.0;
        if steps > 0 {
            size = 0.0;
            for step in 0..steps {
                let next = (self.card_size * 0.9_f32.powi(step as i32)).round().max(56.0);
                along += size / 2.0 + self.gap + next / 2.0;
                size = next;
            }
        }
        let angle = along / self.radius;
        let x = self.center.x
            + offset.signum() as f32 * if steps == 0 { 0.0 } else { self.now_size / 2.0 + self.radius * angle.sin() };
        let y = self.center.y + if steps == 0 { 0.0 } else { self.radius * (1.0 - angle.cos()) + self.side_drop };
        Rect {
            bounds: Rectangle::new(Point::new(x - size / 2.0, y - size / 2.0), Size::new(size, size)),
            rotate: offset as f32 * 1.6,
        }
    }

    pub fn station_radius(self) -> f32 {
        self.rect(1.0).bottom().0.distance(self.wave_center()) - 14.0
    }

    pub fn station_drop(self, rect: Rect) -> f32 {
        let (bottom, down) = rect.bottom();
        let p = bottom - self.wave_center();
        let b = down.x * p.x + down.y * p.y;
        let c = p.x * p.x + p.y * p.y - self.station_radius().powi(2);
        let disc = b * b - c;
        if c <= 0.0 || disc < 0.0 { 0.0 } else { (-b - disc.sqrt()).max(0.0) }
    }
}

impl Rect {
    pub fn contains(self, point: Point) -> bool {
        let d = point - self.bounds.center();
        let (sin, cos) = self.rotate.to_radians().sin_cos();
        (cos * d.x + sin * d.y).abs() <= self.bounds.width / 2.0
            && (-sin * d.x + cos * d.y).abs() <= self.bounds.height / 2.0
    }

    fn bottom(self) -> (Point, iced::Vector) {
        let (sin, cos) = self.rotate.to_radians().sin_cos();
        let down = iced::Vector::new(-sin, cos);
        (self.bounds.center() + down * (self.bounds.height / 2.0), down)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Role {
    Played,
    Now,
    Upcoming,
}

#[derive(Clone, Debug)]
pub(crate) struct Card {
    pub track: usize,
    pub id: i64,
    pub lap: i32,
    pub offset: i32,
    pub position: f32,
    pub role: Role,
    pub revealed: bool,
    pub born: Instant,
    pub opening: Option<usize>,
    pub flip: Option<(Instant, bool)>,
    pub spin_direction: f32,
    pub colour: crate::motion::Tween,
    pub expanded: crate::motion::Tween,
}

/// Time-sampled radio rotation and scale, retaining momentum between pointer targets.
#[derive(Clone, Copy)]
pub struct Tilt {
    from: [f64; 3],
    to: [f64; 3],
    velocity: [f64; 3],
    at: Instant,
}
impl Tilt {
    pub fn new(now: Instant) -> Self {
        Self { from: [0.0, 0.0, 1.0], to: [0.0, 0.0, 1.0], velocity: [0.0; 3], at: now }
    }

    /// Targets x/y rotation in degrees and a scale factor, without restarting from rest.
    pub fn retarget(&mut self, to: [f32; 3], now: Instant) {
        if reduced() {
            *self = Self::new(now);
            return;
        }
        let to = to.map(f64::from);
        if self.to != to {
            let (from, velocity) = self.sample(now);
            *self = Self { from, to, velocity, at: now };
        }
    }

    pub fn at(self, now: Instant) -> [f32; 3] {
        if reduced() {
            return [0.0, 0.0, 1.0];
        }
        self.sample(now).0.map(|value| value as f32)
    }
    fn running(self, now: Instant) -> bool {
        !reduced()
            && (self.from != self.to || self.velocity != [0.0; 3])
            && now.saturating_duration_since(self.at).as_secs_f64() < 0.8
    }

    // The original 320/26/0.6 underdamped spring, including its instantaneous velocity.
    fn sample(self, now: Instant) -> ([f64; 3], [f64; 3]) {
        let t = now.saturating_duration_since(self.at).as_secs_f64();
        if t >= 0.8 {
            return (self.to, [0.0; 3]);
        }
        let decay_rate: f64 = 26.0 / (2.0 * 0.6);
        let frequency = (320.0 / 0.6 - decay_rate.powi(2)).sqrt();
        let decay = (-decay_rate * t).exp();
        let (sin, cos) = (frequency * t).sin_cos();
        let mut position = [0.0; 3];
        let mut velocity = [0.0; 3];
        for i in 0..3 {
            let offset = self.from[i] - self.to[i];
            let wave = (self.velocity[i] + decay_rate * offset) / frequency;
            let value = offset * cos + wave * sin;
            position[i] = self.to[i] + decay * value;
            velocity[i] = decay * (-decay_rate * value + frequency * (wave * cos - offset * sin));
        }
        (position, velocity)
    }
}

#[derive(Clone, Copy, Debug)]
pub enum Input {
    Down(Point),
    Drag(Point),
    Up(Point),
    Cancel,
    Wheel(f32),
    Click(i32, Point),
}

#[derive(Clone, Copy, Debug)]
struct Rail {
    from: f32,
    to: f32,
    at: Instant,
}

impl Rail {
    fn value(self, now: Instant) -> f32 {
        if reduced() || !self.moving(now) {
            return self.to;
        }
        folia_visualizer::spring(
            self.from.into(),
            self.to.into(),
            now.saturating_duration_since(self.at).as_secs_f64(),
            240.0 / 0.9,
            30.0 / 0.9,
        ) as f32
    }

    fn moving(self, now: Instant) -> bool {
        !reduced() && (self.from - self.to).abs() > 0.0001 && now.saturating_duration_since(self.at).as_secs_f32() < 0.9
    }
}

#[derive(Clone)]
pub(crate) struct State {
    pub cards: Vec<Card>,
    pub outgoing: Vec<Card>,
    pub metrics: Metrics,
    pub anchor: f32,
    pub browse: i32,
    pub current: Option<usize>,
    pub wrap: bool,
    pub hidden: bool,
    pub fm: bool,
    pub word: String,
    pub opened: Instant,
    pub hold: Duration,
    pub levels: [f32; 36],
    pub elapsed: f32,
    pub ripples: [Option<f32>; 6],
    pub pointer: Option<Point>,
    pub art: HashMap<i64, crate::radio_art::Artwork>,
    pub pending_art: HashMap<i64, iced::advanced::image::Id>,
    pub projection: folia_render::radio::Projection,
    tilts: HashMap<(i64, i32), Tilt>,
    rail: Rail,
    seen: HashSet<i64>,
    drag: Option<(Point, f32, bool)>,
    count: usize,
    wheel_sum: f32,
    wheel_last: Instant,
    wheel_lock: Instant,
    tick: Instant,
    bass_average: f32,
    last_ripple: f32,
    span: Option<(i32, i32, i32, u32, u32)>,
}

impl State {
    pub fn new(now: Instant) -> Self {
        Self {
            cards: Vec::new(),
            outgoing: Vec::new(),
            metrics: Metrics::new(Size::ZERO),
            anchor: 0.0,
            browse: 0,
            current: None,
            wrap: false,
            hidden: false,
            fm: false,
            word: String::new(),
            opened: now,
            hold: Duration::ZERO,
            levels: [0.0; 36],
            elapsed: 0.0,
            ripples: [None; 6],
            pointer: None,
            art: HashMap::new(),
            pending_art: HashMap::new(),
            projection: Default::default(),
            tilts: HashMap::new(),
            rail: Rail { from: 0.0, to: 0.0, at: now },
            seen: HashSet::new(),
            drag: None,
            count: 0,
            wheel_sum: 0.0,
            wheel_last: now,
            wheel_lock: now,
            tick: now,
            bass_average: 0.0,
            last_ripple: 0.0,
            span: None,
        }
    }

    pub fn range(&self) -> (i32, i32) {
        let count = self.count as i32;
        match self.current {
            Some(_) if self.wrap => (-(count - 1).max(0), (count - 1).max(0)),
            Some(current) => (-(current as i32), (count - current as i32 - 1).max(0)),
            None => (0, count),
        }
    }

    pub fn at(&self, now: Instant) -> f32 {
        self.rail.value(now)
    }

    pub fn resting(&self, now: Instant) -> bool {
        !self.rail.moving(now) && !self.dragging()
    }

    pub fn entering(&self, now: Instant) -> bool {
        !reduced() && now.saturating_duration_since(self.opened) < self.hold + Duration::from_millis(1300)
    }

    pub fn dragging(&self) -> bool {
        self.drag.is_some_and(|(_, _, active)| active)
    }

    pub fn animating(&self, now: Instant, playing: bool) -> bool {
        (!self.cards.is_empty() && self.projection.pending())
            || !reduced()
                && (playing
                    || self.entering(now)
                    || self.rail.moving(now)
                    || self
                        .cards
                        .iter()
                        .any(|c| c.flip.is_some_and(|(at, _)| now.saturating_duration_since(at).as_secs_f32() < 0.56))
                    || !self.outgoing.is_empty()
                    || self.levels.iter().any(|v| *v > 0.002)
                    || self.tilts.values().any(|t| t.running(now))
                    || self.cards.iter().any(|c| c.colour.running(now) || c.expanded.running(now)))
    }

    pub fn sync(&mut self, tiles: &[Tile], current: Option<usize>, wrap: bool, hidden: bool, now: Instant) {
        let next_id = current.and_then(|i| tiles.get(i)).map(|t| t.id);
        let old_id = self.cards.iter().find(|c| c.role == Role::Now).map(|c| c.id);
        let changed = next_id != old_id;
        self.count = tiles.len();
        self.current = current;
        self.wrap = wrap;
        self.hidden = hidden;
        self.seen.retain(|id| tiles.iter().any(|tile| tile.id == *id));
        if let Some(id) = next_id {
            self.seen.insert(id);
        }
        if changed {
            if let Some(card) = self.cards.iter().find(|c| Some(c.id) == next_id && c.lap == 0) {
                self.anchor = card.position;
            }
            self.browse = 0;
            self.drag = None;
            self.target(self.anchor, now);
        }
        let (min, max) = self.range();
        self.browse = self.browse.clamp(min, max);
        self.span = None;
    }

    fn target(&mut self, to: f32, now: Instant) {
        let mut from = self.at(now);
        if (from - to).abs() > 6.0 {
            from = to + (from - to).signum() * 3.0;
        }
        self.rail = Rail { from, to, at: now };
    }

    pub fn browse_to(&mut self, offset: i32, now: Instant) {
        let (min, max) = self.range();
        let offset = if self.wrap && self.count > 0 && (offset < min || offset > max) {
            offset - (offset.signum() * self.count as i32)
        } else {
            offset
        };
        self.browse = offset.clamp(min, max);
        self.target(self.anchor + self.browse as f32, now);
    }

    pub fn wheel(&mut self, delta: f32, now: Instant) {
        if self.dragging() {
            return;
        }
        if now.saturating_duration_since(self.wheel_last) > Duration::from_millis(200) {
            self.wheel_sum = 0.0;
        }
        self.wheel_last = now;
        if now < self.wheel_lock {
            if delta.abs() < 50.0 {
                self.wheel_lock = now + Duration::from_millis(260);
            }
            return;
        }
        self.wheel_sum += delta;
        if self.wheel_sum.abs() < 90.0 {
            return;
        }
        self.browse_to(self.browse + self.wheel_sum.signum() as i32, now);
        self.wheel_sum = 0.0;
        self.wheel_lock = now + Duration::from_millis(260);
    }

    pub fn down(&mut self, point: Point, now: Instant) {
        self.drag = Some((point, self.at(now), false));
    }

    pub fn drag(&mut self, point: Point, now: Instant) {
        let Some((start, from, active)) = self.drag else { return };
        if !active && (point.x - start.x).abs() < 8.0 {
            return;
        }
        self.drag = Some((start, from, true));
        let (min, max) = self.range();
        let raw = from - (point.x - start.x) / self.metrics.step();
        let clamped = raw.clamp(self.anchor + min as f32, self.anchor + max as f32);
        let value = clamped + (raw - clamped) * 0.3;
        self.rail = Rail { from: value, to: value, at: now };
    }

    pub fn release(&mut self, point: Option<Point>, now: Instant) {
        let Some((start, from, active)) = self.drag.take() else { return };
        if !active {
            return;
        }
        let mut next = (self.at(now) - self.anchor).round() as i32;
        if let Some(point) = point
            && next == (from - self.anchor).round() as i32
            && (point.x - start.x).abs() > 48.0
        {
            next -= (point.x - start.x).signum() as i32;
        }
        self.browse_to(next, now);
    }

    pub fn selected(&self) -> Option<usize> {
        self.cards.iter().find(|c| c.offset == self.browse).map(|c| c.track)
    }

    pub fn pointer(&mut self, point: Option<Point>, now: Instant) {
        self.pointer = point;
        let at = self.at(now);
        let dragging = self.dragging();
        let pressed = self.drag.is_some();
        for card in &mut self.cards {
            let rect = self.metrics.rect(card.position - at);
            let focused = card.offset == self.browse;
            let expanded = focused && card.role == Role::Now;
            let hovering = !dragging && point.is_some_and(|p| rect.contains(p));
            card.colour.set(if card.role != Role::Played || focused || hovering { 1.0 } else { 0.0 }, now);
            let mut target = [0.0, 0.0, 1.0];
            if hovering && let Some(p) = point {
                let on_control = focused
                    && if expanded {
                        p.y > rect.bounds.y + rect.bounds.height - 112.0
                    } else {
                        p.x > rect.bounds.x + rect.bounds.width * 0.78
                            && p.y > rect.bounds.y + rect.bounds.height * 0.78
                    };
                if !on_control {
                    let nx = ((p.x - rect.bounds.x) / rect.bounds.width * 2.0 - 1.0).clamp(-1.0, 1.0);
                    let ny = ((p.y - rect.bounds.y) / rect.bounds.height * 2.0 - 1.0).clamp(-1.0, 1.0);
                    let depth = if pressed { 1.25 } else { 1.0 };
                    let degrees = if expanded { 2.5 } else { 7.0 };
                    target = [
                        -ny * degrees * depth,
                        nx * degrees * depth,
                        if pressed {
                            if expanded { 0.99 } else { 0.97 }
                        } else if expanded {
                            0.995
                        } else {
                            0.98
                        },
                    ];
                }
            }
            self.tilts.entry((card.id, card.lap)).or_insert_with(|| Tilt::new(now)).retarget(target, now);
        }
        self.tilts.retain(|key, _| self.cards.iter().any(|c| (c.id, c.lap) == *key));
    }

    pub fn tilt(&self, card: &Card, now: Instant) -> [f32; 3] {
        self.tilts.get(&(card.id, card.lap)).map_or([0.0, 0.0, 1.0], |t| t.at(now))
    }

    // Stable lap keys prevent wrapped duplicates from revealing a future FM card or replacing its neighbour.
    pub fn rebuild(&mut self, tiles: &[Tile], now: Instant) -> bool {
        self.outgoing.retain(|c| !reduced() && now.saturating_duration_since(c.born).as_secs_f32() < 0.5);
        let count = tiles.len() as i32;
        if count == 0 {
            self.cards.clear();
            self.outgoing.clear();
            self.art.clear();
            self.pending_art.clear();
            self.projection = Default::default();
            return false;
        }
        let current = self.current.map_or(-1, |i| i as i32);
        let high = if self.wrap && current >= 0 { 3.min(count - 1) } else { 3.min(count - current - 1) };
        let low = if self.wrap && current >= 0 { -2.min(count - high - 1) } else { -2.min(current.max(0)) };
        let at = self.at(now) - self.anchor;
        let left = at.floor().min(self.browse as f32) as i32;
        let right = at.ceil().max(self.browse as f32) as i32;
        let first = if left < 0 { low.min(left - 3) } else { low };
        let last = if right > 0 { high.max(right + 3) } else { high };
        let span = (first, last, self.browse, self.metrics.size.width.to_bits(), self.metrics.size.height.to_bits());
        if self.span == Some(span) {
            return false;
        }
        self.span = Some(span);
        let mut old = std::mem::take(&mut self.cards);
        let mut cards = Vec::with_capacity(16);
        for offset in first..=last {
            if (offset as f32 - at).abs() > 4.0 && (offset - self.browse).abs() > 4 {
                continue;
            }
            if !self.wrap && !(0..count).contains(&(current + offset)) {
                continue;
            }
            if current < 0 && offset < 1 {
                continue;
            }
            let track = (current + offset).rem_euclid(count) as usize;
            let lap = if self.wrap && current >= 0 { (offset - (high - count + 1)).div_euclid(count) } else { 0 };
            let home = offset - lap * count;
            let role = if offset == 0 && current >= 0 {
                Role::Now
            } else if offset > 0 || home > 0 {
                Role::Upcoming
            } else {
                Role::Played
            };
            let id = tiles[track].id;
            let revealed = role != Role::Upcoming || !self.hidden || self.seen.contains(&id);
            let prior = old.iter().position(|c| c.id == id && c.lap == lap).map(|i| old.remove(i));
            let mut flip = prior.as_ref().and_then(|c| {
                if c.revealed != revealed || c.role != Role::Now && role == Role::Now {
                    Some((now, !c.revealed))
                } else {
                    c.flip
                }
            });
            let born = prior.as_ref().map_or(now, |c| c.born);
            let expanded_goal = if role == Role::Now && offset == self.browse { 1.0 } else { 0.0 };
            let mut expanded = prior.as_ref().map_or_else(
                || {
                    crate::motion::Tween::new(
                        expanded_goal,
                        520.0,
                        folia_visualizer::Ease::new(0.25, 0.1, 0.25, 1.0),
                        now,
                    )
                    .independent()
                },
                |c| c.expanded,
            );
            expanded.set(expanded_goal, now);
            let colour = prior.as_ref().map_or_else(
                || {
                    crate::motion::Tween::new(
                        if role == Role::Played && offset != self.browse { 0.0 } else { 1.0 },
                        400.0,
                        folia_visualizer::Ease::new(0.25, 0.1, 0.25, 1.0),
                        now,
                    )
                    .independent()
                },
                |c| c.colour,
            );
            let opening = prior
                .as_ref()
                .map_or_else(|| self.entering(now).then_some((offset - low).max(0) as usize), |c| c.opening);
            if prior.is_none() && opening.is_none() && offset < 0 && revealed {
                flip = Some((now, true));
            }
            let spin_direction = prior.as_ref().map_or(-1.0, |c| {
                if c.role != Role::Now && role == Role::Now {
                    if c.role == Role::Played { 1.0 } else { -1.0 }
                } else {
                    c.spin_direction
                }
            });
            cards.push(Card {
                track,
                id,
                lap,
                offset,
                position: self.anchor + offset as f32,
                role,
                revealed,
                born,
                opening,
                flip,
                spin_direction,
                colour,
                expanded,
            });
        }
        self.outgoing.retain(|c| !cards.iter().any(|next| next.id == c.id && next.lap == c.lap));
        for mut card in old.into_iter().filter(|c| tiles.get(c.track).is_some_and(|tile| tile.id == c.id)) {
            card.born = now;
            self.outgoing.push(card);
        }
        if self.outgoing.len() > 16 {
            self.outgoing.drain(..self.outgoing.len() - 16);
        }
        self.outgoing.retain(|c| !reduced() && now.saturating_duration_since(c.born).as_secs_f32() < 0.5);
        self.cards = cards;
        self.pointer(self.pointer, now);
        true
    }

    pub fn sample_audio(&mut self, audio: &folia_core::AudioFrame, playing: bool, now: Instant) {
        let dt = now.saturating_duration_since(self.tick).as_secs_f32().min(0.1);
        self.tick = now;
        if reduced() {
            self.levels.fill(0.0);
            return;
        }
        let playing = playing && !self.entering(now);
        if playing {
            self.elapsed += dt;
        }
        for (i, value) in self.levels.iter_mut().enumerate() {
            let bin = (4 + ((1.0 - i as f32 / 36.0 * 2.0).abs().powf(1.6) * 456.0) as usize).min(459);
            let target = if playing && audio.has_spectrum {
                ((f32::from(audio.spectrum[bin]) + f32::from(audio.spectrum[(bin + 1).min(459)])) / 510.0).powf(1.7)
            } else {
                0.0
            };
            let factor = 1.0 - (if target > *value { 0.55_f32 } else { 0.88_f32 }).powf(dt * 60.0);
            *value += (target - *value) * factor;
            if *value < 0.002 && target == 0.0 {
                *value = 0.0;
            }
        }
        let bass = audio.bass / 255.0;
        self.bass_average += (bass - self.bass_average) * (1.0 - 0.95_f32.powf(dt * 60.0));
        let gap = self.elapsed - self.last_ripple;
        if playing && (gap > 3.2 || gap > 0.7 && bass > 0.35 && bass > self.bass_average * 1.3) {
            self.ripples.rotate_left(1);
            self.ripples[5] = Some(self.elapsed);
            self.last_ripple = self.elapsed;
        }
    }
}

pub(crate) fn reduced() -> bool {
    crate::motion::reduced(folia_store::MotionSurface::CollectionMorph)
}

pub(crate) fn redaction(title: &str, artist: &str) -> (f32, f32) {
    let units = |text: &str| {
        text.chars()
            .map(|c| {
                if matches!(c as u32, 0x2e80..=0x9fff | 0xac00..=0xd7af | 0xf900..=0xfaff | 0xff00..=0xffef) {
                    1.0
                } else {
                    0.55
                }
            })
            .sum::<f32>()
    };
    (
        (12.0 + units(title) * 6.5).clamp(24.0, 72.0).round() / 100.0,
        (8.0 + units(artist) * 4.0).clamp(14.0, 46.0).round() / 100.0,
    )
}
