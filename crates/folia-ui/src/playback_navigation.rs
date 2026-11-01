// Automatic player navigation is committed only by the still-wanted playback start.

use crate::Page;

/// Keeps a play button's navigation intent separate from the asynchronous audio preparation.
#[derive(Debug, Default)]
pub struct PlaybackNavigation {
    pending: Option<(u64, Page)>,
}

impl PlaybackNavigation {
    pub fn begin(&mut self, request: u64, from: Page) {
        self.pending = Some((request, from));
    }

    /// A ready source may open the player only if neither a newer play nor navigation overtook it.
    pub fn ready(&mut self, request: u64, page: Page) -> bool {
        let Some((wanted, from)) = self.pending else { return false };
        if wanted != request {
            return false;
        }
        self.pending = None;
        from == page
    }

    pub fn failed(&mut self, request: u64) {
        if self.pending.is_some_and(|(wanted, _)| wanted == request) {
            self.cancel();
        }
    }

    pub fn cancel(&mut self) {
        self.pending = None;
    }
}
