// Per-instance CSS hover transitions for the poster under the pointer and those it just left.

use crate::{focus_card::SlotId, motion::Tween};
use folia_visualizer::Ease;
use std::time::Instant;

#[derive(Default)]
pub(super) struct Hover {
    pub slot: Option<SlotId>,
    faces: Vec<(SlotId, Face)>,
}

#[derive(Clone, Copy)]
struct Face {
    reveal: Tween,
    pop: Tween,
    pages: Tween,
    filter: Tween,
}

impl Face {
    fn new(now: Instant) -> Self {
        let ease = Ease::new(0.25, 0.1, 0.25, 1.0);
        Self {
            reveal: Tween::new(0.0, 260.0, ease, now),
            pop: Tween::new(0.0, 300.0, Ease::new(0.0, 0.0, 0.58, 1.0), now),
            pages: Tween::new(0.0, 300.0, ease, now),
            filter: Tween::new(0.0, 600.0, ease, now),
        }
    }
    fn set(&mut self, value: f32, now: Instant) {
        self.reveal.set(value, now);
        self.pop.set(value, now);
        self.pages.set(value, now);
        self.filter.set(value, now);
    }
}

impl Hover {
    pub fn set(&mut self, next: Option<SlotId>, now: Instant) {
        self.faces.retain(|(slot, face)| Some(*slot) == self.slot || face.filter.running(now));
        if self.slot == next {
            return;
        }
        self.slot = next;
        for (slot, face) in &mut self.faces {
            face.set(if Some(*slot) == next { 1.0 } else { 0.0 }, now);
        }
        if let Some(slot) = next
            && !self.faces.iter().any(|(key, _)| *key == slot)
        {
            let mut face = Face::new(now);
            face.set(1.0, now);
            self.faces.push((slot, face));
        }
    }

    fn face(&self, slot: SlotId) -> Option<&Face> {
        self.faces.iter().find(|(key, _)| *key == slot).map(|(_, f)| f)
    }
    pub fn pop(&self, slot: SlotId, now: Instant) -> f32 {
        self.face(slot).map_or(0.0, |f| f.pop.value(now))
    }
    pub fn reveal(&self, slot: SlotId, now: Instant) -> f32 {
        self.face(slot).map_or(0.0, |f| f.reveal.value(now))
    }
    pub fn pages(&self, slot: SlotId, now: Instant) -> f32 {
        self.face(slot).map_or(0.0, |f| f.pages.value(now))
    }
    pub fn filter(&self, slot: SlotId, now: Instant) -> f32 {
        self.face(slot).map_or(0.0, |f| f.filter.value(now))
    }
    pub fn active(&self) -> bool {
        !self.faces.is_empty()
    }
    pub fn running(&self, now: Instant) -> bool {
        self.faces.iter().any(|(_, f)| f.filter.running(now))
    }
}
