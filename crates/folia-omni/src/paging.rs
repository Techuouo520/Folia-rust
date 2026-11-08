// How each platform decides a search page continues. The formulas are the original providers' own.

/// NetEase `searchSongs`: `songCount` of 0 is ignored, as `songCount || items.length` does, and
/// `hasMore` is `offset + items < total`.
pub fn netease_search_page(offset: u32, len: u32, song_count: Option<u64>) -> (bool, u32, u64) {
    let total = match song_count {
        Some(count) if count != 0 => count,
        _ => u64::from(len),
    };
    let next = offset.saturating_add(len);
    (u64::from(next) < total, next, total)
}

/// QQ session search: `hasMore` is the answer's `more`, and the next page starts at `page * pageSize`.
pub fn qq_session_search_page(more: bool, page: u32, page_size: u32) -> (bool, u32) {
    (more, page.saturating_mul(page_size))
}

/// QQ anonymous search: a page that came back full has another, at `page * pageSize`.
pub fn qq_anonymous_search_page(len: u32, page: u32, page_size: u32) -> (bool, u32) {
    (len == page_size, page.saturating_mul(page_size))
}

/// KuGou `pageOfWithRawItemCount`: the raw row count advances the offset, and a missing total means a full page.
pub fn kugou_search_page(offset: u32, raw_count: u32, limit: u32, total: Option<u64>) -> (bool, u32) {
    let next = offset.saturating_add(raw_count);
    let has_more = match total {
        Some(total) => u64::from(next) < total,
        None => raw_count == limit,
    };
    (has_more, next)
}

/// A Bodian `bodianPagination` cursor, already checked only for the two fields the page helper reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BodianCursor {
    pub next_offset: u64,
    pub has_more: bool,
}

fn bodian_cursor_ok(cursor: BodianCursor, offset: u64, len: u64) -> bool {
    cursor.next_offset < (1_u64 << 53)
        && cursor.next_offset >= offset.saturating_add(len)
        && (!cursor.has_more || cursor.next_offset > offset)
}

/// Bodian `bodianPage`: a sane cursor wins; otherwise a known total, or a full page of rows.
pub fn bodian_search_page(
    len: u64,
    total: Option<f64>,
    offset: u64,
    limit: u64,
    cursor: Option<BodianCursor>,
) -> (bool, u64, Option<u64>) {
    let known = total.filter(|count| count.is_finite() && *count >= 0.0);
    let count = known.unwrap_or(0.0);
    let accepted = cursor.filter(|cursor| bodian_cursor_ok(*cursor, offset, len));
    let next = accepted.map(|cursor| cursor.next_offset).unwrap_or_else(|| offset.saturating_add(len));
    let has_more = match accepted {
        Some(cursor) => cursor.has_more,
        None => len > 0 && known.map_or(len == limit, |_| (next as f64) < count),
    };
    (has_more, next, known.map(|count| count as u64))
}

/// Qishui's track group: an empty batch ends the search; otherwise the group's `has_more` stands.
pub fn qishui_search_continues(batch_empty: bool, has_more: bool) -> bool {
    !batch_empty && has_more
}

/// Spotify's walk is finished when a page added nothing or 300 rows have been asked.
pub fn spotify_search_done(fresh: u32, asked: u32) -> bool {
    fresh == 0 || asked >= 300
}

/// Spotify `searchSongs`: `hasMore` while the walk is not finished, or the slice did not reach its end.
pub fn spotify_search_page(done: bool, offset: u32, kept: u32, page_len: u32) -> (bool, u32) {
    let next = offset.saturating_add(page_len);
    (!done || next < kept, next)
}

/// YouTube Music's walk is finished when the continuation is missing or repeats.
pub fn ytm_walk_done(has_continuation: bool, repeated: bool) -> bool {
    !has_continuation || repeated
}

/// YouTube Music `page`: `hasMore` while the walk is open or the slice stops before the rows kept so far.
pub fn ytm_search_page(done: bool, offset: u32, kept: u32, page_len: u32) -> (bool, u32, Option<u64>) {
    let next = offset.saturating_add(page_len);
    (!done || next < kept, next, done.then_some(u64::from(kept)))
}
