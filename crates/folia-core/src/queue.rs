// Play queue: the order of items, the current position and the repeat mode.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Repeat {
    #[default]
    Off,
    All,
    One,
}

/// Where the queue was started, for as long as every item is still one of those songs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Origin {
    pub name: String,
}

/// Durable queue contents, including a playing item detached from the editable list.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, bound(deserialize = "T: Deserialize<'de>"))]
pub struct QueueSnapshot<T> {
    pub items: Vec<T>,
    pub current: Option<usize>,
    pub detached: Option<T>,
    pub origin: Option<Origin>,
}

impl<T> Default for QueueSnapshot<T> {
    fn default() -> Self {
        Self { items: Vec::new(), current: None, detached: None, origin: None }
    }
}

#[derive(Debug, Clone)]
pub struct Queue<T> {
    items: Vec<T>,
    revision: u64,
    current: Option<usize>,
    detached: Option<T>,
    origin: Option<Origin>,
    pub repeat: Repeat,
}

impl<T: PartialEq> PartialEq for Queue<T> {
    fn eq(&self, other: &Self) -> bool {
        self.items == other.items
            && self.current == other.current
            && self.detached == other.detached
            && self.origin == other.origin
            && self.repeat == other.repeat
    }
}

impl<T> Default for Queue<T> {
    fn default() -> Self {
        Self { items: Vec::new(), revision: 0, current: None, detached: None, origin: None, repeat: Repeat::Off }
    }
}

impl<T> Queue<T> {
    pub fn snapshot(&self) -> QueueSnapshot<T>
    where
        T: Clone,
    {
        QueueSnapshot {
            items: self.items.clone(),
            current: self.current,
            detached: self.detached.clone(),
            origin: self.origin.clone(),
        }
    }

    /// Restores contents without starting playback or changing the current repeat preference.
    pub fn restore(&mut self, snapshot: QueueSnapshot<T>) {
        self.current = snapshot.current.filter(|&index| index < snapshot.items.len());
        self.detached = if self.current.is_none() { snapshot.detached } else { None };
        self.origin = snapshot.origin.filter(|_| !snapshot.items.is_empty());
        self.items = snapshot.items;
        self.revision = self.revision.wrapping_add(1);
    }

    /// Replaces the queue and makes `start` current. A replacement is not the previous source's queue.
    pub fn replace(&mut self, items: Vec<T>, start: usize) {
        self.detached = None;
        self.current = (start < items.len()).then_some(start);
        self.revision = self.revision.wrapping_add(1);
        self.items = items;
        self.origin = None;
    }

    /// `replace`, and records `origin` as where these songs were started.
    pub fn replace_from(&mut self, items: Vec<T>, start: usize, origin: Origin) {
        self.replace(items, start);
        self.origin = Some(origin);
    }

    pub fn origin(&self) -> Option<&Origin> {
        self.origin.as_ref()
    }

    /// Changes when the queue contents or their order can have changed.
    pub fn revision(&self) -> u64 {
        self.revision
    }

    pub fn items(&self) -> &[T] {
        &self.items
    }

    pub fn current_index(&self) -> Option<usize> {
        self.current
    }

    pub fn current(&self) -> Option<&T> {
        self.current.and_then(|index| self.items.get(index)).or(self.detached.as_ref())
    }

    pub fn current_mut(&mut self) -> Option<&mut T> {
        self.revision = self.revision.wrapping_add(1);
        self.current.and_then(|index| self.items.get_mut(index)).or(self.detached.as_mut())
    }

    /// `applyQueueAddBehavior` with `append`: the incoming items, each once and not the current one, go to the end;
    /// copies already queued move there with them. The current item stays current. Returns how many were added.
    pub fn append(&mut self, incoming: Vec<T>, same: impl Fn(&T, &T) -> bool) -> usize {
        let mut targets: Vec<T> = Vec::new();
        for item in incoming {
            let is_current = self.current().is_some_and(|current| same(current, &item));
            if !is_current && !targets.iter().any(|target| same(target, &item)) {
                targets.push(item);
            }
        }
        if targets.is_empty() {
            return 0;
        }
        let current = self.current;
        let mut kept = Vec::with_capacity(self.items.len() + targets.len());
        let mut next_current = None;
        for (index, item) in std::mem::take(&mut self.items).into_iter().enumerate() {
            if Some(index) == current {
                next_current = Some(kept.len());
            } else if targets.iter().any(|target| same(target, &item)) {
                continue;
            }
            kept.push(item);
        }
        let added = targets.len();
        kept.extend(targets);
        self.revision = self.revision.wrapping_add(1);
        self.items = kept;
        self.current = next_current;
        self.origin = None;
        added
    }

    /// Makes `index` the current item.
    pub fn select(&mut self, index: usize) -> Option<&T> {
        if index >= self.items.len() {
            return None;
        }
        self.detached = None;
        self.current = Some(index);
        self.items.get(index)
    }

    /// Removes `index`. The item that slides into a removed current item becomes current.
    pub fn remove(&mut self, index: usize) -> Option<T> {
        if index >= self.items.len() {
            return None;
        }
        self.revision = self.revision.wrapping_add(1);
        let item = self.items.remove(index);
        self.current = match self.current {
            Some(current) if current > index => Some(current - 1),
            Some(current) if current == index => {
                if self.items.is_empty() {
                    None
                } else if current >= self.items.len() {
                    Some(self.items.len() - 1)
                } else {
                    Some(current)
                }
            }
            other => other,
        };
        Some(item)
    }

    /// Clears the editable list while the player continues using the current song.
    pub fn clear_list(&mut self) -> bool {
        if self.items.is_empty() {
            return false;
        }
        if let Some(index) = self.current.take() {
            self.detached = Some(self.items.swap_remove(index));
        }
        self.items.clear();
        self.origin = None;
        self.revision = self.revision.wrapping_add(1);
        true
    }

    /// `clearQueue` while the playing item keeps playing: every other item leaves and the current one moves to
    /// the head. Returns whether anything left. An empty queue or a lone current item is unchanged.
    pub fn clear_except_current(&mut self) -> bool {
        let Some(current) = self.current.filter(|&index| index < self.items.len()) else {
            self.revision = self.revision.wrapping_add(1);
            let had = !self.items.is_empty();
            self.items.clear();
            self.origin = None;
            return had;
        };
        if self.items.len() <= 1 {
            return false;
        }
        self.revision = self.revision.wrapping_add(1);
        let current = self.items.remove(current);
        self.items.clear();
        self.items.push(current);
        self.current = Some(0);
        self.origin = None;
        true
    }

    /// Places `index` immediately after the current item. The current item stays current.
    pub fn move_after_current(&mut self, index: usize) -> bool {
        if self.detached.is_some() {
            return self.move_to(index, 0);
        }
        let Some(current) = self.current else { return false };
        if index >= self.items.len() || index == current {
            return false;
        }
        self.revision = self.revision.wrapping_add(1);
        let item = self.items.remove(index);
        let current = if index < current { current - 1 } else { current };
        let insert = (current + 1).min(self.items.len());
        self.items.insert(insert, item);
        self.current = Some(current);
        true
    }

    /// Moves the item at `from` so that it ends up at `to` (`moveQueueSong`); the current item stays current.
    pub fn move_to(&mut self, from: usize, to: usize) -> bool {
        if from >= self.items.len() || to >= self.items.len() || from == to {
            return false;
        }
        self.revision = self.revision.wrapping_add(1);
        let item = self.items.remove(from);
        self.items.insert(to, item);
        self.current = self.current.map(|current| match current {
            current if current == from => to,
            current if from < current && current <= to => current - 1,
            current if to <= current && current < from => current + 1,
            current => current,
        });
        true
    }

    /// Moves `index` to the end of the queue.
    pub fn move_to_end(&mut self, index: usize) -> bool {
        if index >= self.items.len() {
            return false;
        }
        self.revision = self.revision.wrapping_add(1);
        let item = self.items.remove(index);
        self.items.push(item);
        if let Some(current) = self.current {
            if current == index {
                self.current = Some(self.items.len() - 1);
            } else if current > index {
                self.current = Some(current - 1);
            }
        }
        true
    }

    /// Shuffles the queue as the original's `shuffleQueue`: the current item goes to the head and the rest are
    /// permuted with a seeded generator. An unselected queue shuffles in full.
    pub fn shuffle(&mut self, seed: u64) -> bool {
        if self.items.len() < 2 {
            return false;
        }
        let mut rest = std::mem::take(&mut self.items);
        let current_item = self.current.map(|index| rest.remove(index));
        let mut state = seed | 1;
        let mut next = move || {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            state
        };
        for index in (1..rest.len()).rev() {
            let other = (next() % (index as u64 + 1)) as usize;
            rest.swap(index, other);
        }
        if let Some(current) = current_item {
            rest.insert(0, current);
            self.current = Some(0);
        }
        self.revision = self.revision.wrapping_add(1);
        self.items = rest;
        true
    }

    /// Moves to the next item. `finished` is true when the current item played to its end,
    /// in which case repeat-one replays it.
    pub fn advance(&mut self, finished: bool) -> Option<&T> {
        if finished && self.repeat == Repeat::One {
            return self.current();
        }
        self.select(self.next_index()?)
    }

    /// Moves to the previous item, wrapping only when repeating the whole queue.
    pub fn back(&mut self) -> Option<&T> {
        self.select(self.previous_index()?)
    }

    /// The item a skip forward would play, wrapping only when repeating the whole queue.
    pub fn next_index(&self) -> Option<usize> {
        if self.detached.is_some() {
            return (!self.items.is_empty()).then_some(0);
        }
        let current = self.current?;
        match self.repeat {
            Repeat::All => Some((current + 1) % self.items.len()),
            _ => (current + 1 < self.items.len()).then_some(current + 1),
        }
    }

    /// The item a skip back would play, wrapping only when repeating the whole queue.
    pub fn previous_index(&self) -> Option<usize> {
        match (self.current?, self.repeat) {
            (0, Repeat::All) => self.items.len().checked_sub(1),
            (0, _) => None,
            (current, _) => Some(current - 1),
        }
    }
}
