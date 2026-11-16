// Bodian artist search, catalog pages and exact song references.

use folia_core::Track;
use serde_json::Value;

use super::api::{Call, Client};
use super::{collection, secure, text, track};
use crate::{Artist, Collection, CollectionKind, OmniError, Page, TrackCatalog};

fn artist(raw: &Value) -> Option<Artist> {
    Some(Artist {
        provider: super::ID.to_owned(),
        id: text(raw, &["artistId", "id"])?,
        name: text(raw, &["name"])?,
        picture_url: text(raw, &["pic"]).map(|url| secure(&url)),
        description: text(raw, &["description", "info", "desc"]),
    })
}

pub async fn search(client: &Client, query: &str, limit: u32, offset: u32) -> Result<Page<Artist>, OmniError> {
    if query.trim().is_empty() {
        return Ok(Page::empty(offset));
    }
    page(
        client,
        "/api/search/artist/list",
        vec![("keyword", query.trim().to_owned()), ("correct", "1".to_owned())],
        limit,
        offset,
        0,
        artist,
    )
    .await
}

pub async fn detail(client: &Client, id: &str) -> Result<Artist, OmniError> {
    let id = super::library::digits(id)?;
    let answer = client.call(&format!("/api/service/artist/{id}"), Call::default()).await?;
    answer.pointer("/data/artistInfo").and_then(artist).filter(|artist| artist.id == id).ok_or(OmniError::Unavailable)
}

pub async fn songs(client: &Client, id: &str, limit: u32, offset: u32) -> Result<Page<Track>, OmniError> {
    let id = super::library::digits(id)?;
    page(client, &format!("/api/service/artist/music/{id}"), Vec::new(), limit, offset, 1, track).await
}

pub async fn albums(client: &Client, id: &str, limit: u32, offset: u32) -> Result<Page<Collection>, OmniError> {
    let id = super::library::digits(id)?;
    page(client, &format!("/api/service/artist/album/{id}"), Vec::new(), limit, offset, 1, |row| {
        collection(row, CollectionKind::Album, 6)
    })
    .await
}

pub async fn album(client: &Client, id: &str) -> Result<Collection, OmniError> {
    let id = super::library::digits(id)?;
    let answer = client.call(&format!("/api/service/album/{id}"), Call::default()).await?;
    answer
        .pointer("/data/albumInfo")
        .and_then(|row| collection(row, CollectionKind::Album, 6))
        .filter(|album| album.id == id)
        .ok_or(OmniError::Unavailable)
}

pub async fn references(client: &Client, song: &Track) -> Result<TrackCatalog, OmniError> {
    if song.online_info.as_ref().is_some_and(|info| {
        !info.artists.is_empty() && info.artists.iter().all(|artist| !artist.id.is_empty()) && !info.album_id.is_empty()
    }) {
        return Ok(TrackCatalog::from_track(song));
    }
    let id = super::library::digits(&song.online.as_ref().ok_or(OmniError::Unavailable)?.id)?;
    let answer = client
        .call("/api/service/music/info", Call { params: vec![("musicId", id.to_owned())], ..Call::default() })
        .await?;
    let resolved = answer
        .get("data")
        .and_then(track)
        .filter(|track| track.online.as_ref().is_some_and(|online| online.id == id))
        .ok_or(OmniError::Unavailable)?;
    Ok(TrackCatalog::from_track(&resolved))
}

// Bodian uses numbered pages and can omit unavailable rows, so advance by the requested boundary.
async fn page<T>(
    client: &Client,
    path: &str,
    mut params: Vec<(&str, String)>,
    limit: u32,
    offset: u32,
    first_page: u32,
    normalize: impl Fn(&Value) -> Option<T>,
) -> Result<Page<T>, OmniError> {
    let limit = limit.clamp(1, 100);
    params.extend([("pn", (offset / limit).saturating_add(first_page).to_string()), ("rn", limit.to_string())]);
    let answer = client.call(path, Call { params, ..Call::default() }).await?;
    let data = answer.get("data").unwrap_or(&Value::Null);
    let rows = data
        .get("resultList")
        .and_then(Value::as_array)
        .ok_or_else(|| OmniError::Response("Bodian catalog list is missing".to_owned()))?;
    let total = data.get("total").and_then(|value| value.as_u64().or_else(|| value.as_str()?.parse().ok()));
    let page_end = (offset / limit).saturating_add(1).saturating_mul(limit);
    let items: Vec<T> = rows.iter().skip((offset % limit) as usize).filter_map(normalize).collect();
    let has_more = !rows.is_empty() && total.map_or(rows.len() == limit as usize, |total| u64::from(page_end) < total);
    let next_offset = if has_more {
        page_end
    } else {
        total
            .map_or(page_end, |total| page_end.min(total.min(u64::from(u32::MAX)) as u32))
            .max(offset.saturating_add(items.len() as u32))
    };
    Ok(Page { items, total, has_more: has_more && next_offset > offset, next_offset })
}
