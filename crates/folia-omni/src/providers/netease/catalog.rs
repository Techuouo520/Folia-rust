// NetEase artist search, catalog pages and exact song-reference recovery.

use folia_core::Track;
use serde_json::{Value, json};

use super::{Netease, collection, https, tracks};
use crate::{Artist, Collection, CollectionKind, OmniError, Page, TrackCatalog};

fn artist(raw: &Value) -> Option<Artist> {
    Some(Artist {
        provider: super::ID.to_owned(),
        id: raw.get("id")?.as_u64().filter(|id| *id > 0)?.to_string(),
        name: raw.get("name")?.as_str()?.to_owned(),
        picture_url: raw
            .get("picUrl")
            .or_else(|| raw.get("img1v1Url"))
            .and_then(Value::as_str)
            .map(|url| https(url.to_owned())),
        description: raw.get("briefDesc").and_then(Value::as_str).filter(|text| !text.is_empty()).map(str::to_owned),
    })
}

fn id_number(id: &str) -> Result<u64, OmniError> {
    id.parse::<u64>()
        .ok()
        .filter(|id| *id > 0)
        .ok_or_else(|| OmniError::Response("invalid NetEase catalog id".to_owned()))
}

pub async fn search(client: &Netease, query: &str, limit: u32, offset: u32) -> Result<Page<Artist>, OmniError> {
    let data: Value = client
        .post(
            "/api/cloudsearch/pc",
            json!({ "s": query, "type": 100, "limit": limit.clamp(1, 100), "offset": offset, "total": true }),
        )
        .await?;
    let rows = data.pointer("/result/artists").and_then(Value::as_array);
    let count = rows.map_or(0, |rows| rows.len() as u32);
    let items = rows.into_iter().flatten().filter_map(artist).collect();
    let total = data.pointer("/result/artistCount").and_then(Value::as_u64);
    let (has_more, next_offset, total) = crate::netease_search_page(offset, count, total);
    Ok(Page { items, total: Some(total), has_more, next_offset })
}

pub async fn detail(client: &Netease, id: &str) -> Result<Artist, OmniError> {
    let id = id_number(id)?;
    let data: Value = client.post(&format!("/api/v1/artist/{id}"), json!({})).await?;
    artist(&data["artist"]).ok_or_else(|| OmniError::Response("NetEase artist record missing".to_owned()))
}

pub async fn songs(client: &Netease, id: &str, limit: u32, offset: u32) -> Result<Page<Track>, OmniError> {
    let limit = limit.clamp(1, 100);
    let data: Value = client
        .post(
            "/api/v1/artist/songs",
            json!({ "id": id_number(id)?, "order": "hot", "limit": limit, "offset": offset, "total": true }),
        )
        .await?;
    let count = data.get("songs").and_then(Value::as_array).map_or(0, |rows| rows.len() as u32);
    let total = data.get("total").and_then(Value::as_u64);
    let next_offset = offset.saturating_add(count);
    let has_more = count > 0
        && data
            .get("more")
            .and_then(Value::as_bool)
            .unwrap_or_else(|| total.map_or(count == limit, |total| u64::from(next_offset) < total));
    Ok(Page { items: tracks(data.get("songs")), total, has_more, next_offset })
}

pub async fn albums(client: &Netease, id: &str, limit: u32, offset: u32) -> Result<Page<Collection>, OmniError> {
    let limit = limit.clamp(1, 100);
    let data: Value = client
        .post(
            &format!("/api/artist/albums/{}", id_number(id)?),
            json!({ "limit": limit, "offset": offset, "total": true }),
        )
        .await?;
    let rows = data.get("hotAlbums").and_then(Value::as_array);
    let count = rows.map_or(0, |rows| rows.len() as u32);
    let items = rows.into_iter().flatten().filter_map(|row| collection(row, CollectionKind::Album)).collect();
    let total = data.pointer("/artist/albumSize").and_then(Value::as_u64);
    let next_offset = offset.saturating_add(count);
    let has_more = count > 0
        && data
            .get("more")
            .and_then(Value::as_bool)
            .unwrap_or_else(|| total.map_or(count == limit, |total| u64::from(next_offset) < total));
    Ok(Page { items, total, has_more, next_offset })
}

pub async fn album(client: &Netease, id: &str) -> Result<Collection, OmniError> {
    let data: Value = client.post(&format!("/api/v1/album/{}", id_number(id)?), json!({})).await?;
    collection(&data["album"], CollectionKind::Album)
        .ok_or_else(|| OmniError::Response("NetEase album record missing".to_owned()))
}

pub async fn track_catalog(client: &Netease, track: &Track) -> Result<TrackCatalog, OmniError> {
    let mut resolved = track.clone();
    if track.online_info.as_ref().is_none_or(|info| {
        info.album_id.is_empty() || info.artists.is_empty() || info.artists.iter().any(|artist| artist.id.is_empty())
    }) {
        let id = id_number(&track.online.as_ref().ok_or(OmniError::Unavailable)?.id)?;
        let data: Value = client.post("/api/v3/song/detail", json!({ "c": json!([{ "id": id }]).to_string() })).await?;
        resolved = tracks(data.get("songs"))
            .into_iter()
            .find(|song| song.online == track.online)
            .ok_or(OmniError::Unavailable)?;
    }
    Ok(TrackCatalog::from_track(&resolved))
}
