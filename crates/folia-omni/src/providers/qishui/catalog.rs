// Qishui artist catalog over the car protocol, whose hot list is bounded to fifty songs.

use folia_core::Track;
use serde_json::{Value, json};

use super::client::{self, Client};
use super::{image, text, track};
use crate::{Artist, OmniError, Page, TrackCatalog};

fn artist(row: &Value) -> Option<Artist> {
    let base = row.get("base_info").unwrap_or(row);
    Some(Artist {
        provider: super::ID.to_owned(),
        id: text(base, "id")?,
        name: text(base, "name")?,
        picture_url: image(base.get("url_avatar")),
        description: text(base, "desc").or_else(|| text(base, "intro")),
    })
}

pub async fn search(client: &Client, query: &str, limit: u32, offset: u32) -> Result<Page<Artist>, OmniError> {
    if query.trim().is_empty() {
        return Ok(Page::empty(offset));
    }
    let search_id = client::uuid(&client.rng);
    let mut cursor = offset;
    let mut items = Vec::new();
    for _ in 0..20 {
        let answer = client.platform("/search/", json!({
            "q": query.trim(), "cursor": cursor.to_string(), "search_id": search_id, "from_search_id": search_id,
            "search_method": "input", "search_type": "artist", "search_scene": "normal", "scene_name": "search",
            "ab_param": "", "last_viewed_media": {},
        }), false).await?;
        let groups = answer
            .get("result_groups")
            .and_then(Value::as_array)
            .ok_or_else(|| OmniError::Response("Qishui artist search groups are missing".to_owned()))?;
        let Some(group) = groups.iter().find(|group| group.get("id").and_then(Value::as_str) == Some("artists")) else {
            if groups.is_empty() {
                return Ok(Page { items, total: None, has_more: false, next_offset: cursor });
            }
            return Err(OmniError::Response("Qishui returned a different search category".to_owned()));
        };
        let rows = group
            .get("data")
            .and_then(Value::as_array)
            .ok_or_else(|| OmniError::Response("Qishui artist results are missing".to_owned()))?;
        items.extend(rows.iter().filter_map(|row| row.pointer("/entity/artist")).filter_map(artist));
        let has_more = !rows.is_empty() && group.get("has_more").and_then(Value::as_bool) == Some(true);
        if !has_more {
            return Ok(Page {
                next_offset: offset.saturating_add(items.len() as u32),
                items,
                total: None,
                has_more: false,
            });
        }
        let next = group
            .get("next_cursor")
            .and_then(|value| value.as_u64().or_else(|| value.as_str()?.parse().ok()))
            .and_then(|next| u32::try_from(next).ok())
            .filter(|next| *next > cursor)
            .ok_or_else(|| OmniError::Response("Qishui artist cursor did not advance".to_owned()))?;
        cursor = next;
        if items.len() >= limit.clamp(1, 100) as usize {
            return Ok(Page { items, total: None, has_more, next_offset: cursor });
        }
    }
    Err(OmniError::Response("Qishui artist search page limit reached".to_owned()))
}

pub async fn detail(client: &Client, id: &str) -> Result<Artist, OmniError> {
    let answer = client.platform("/artist/detail/", json!({ "artist_id": id }), false).await?;
    answer.get("artist_info").and_then(artist).filter(|artist| artist.id == id).ok_or(OmniError::Unavailable)
}

pub async fn songs(client: &Client, id: &str, limit: u32, offset: u32) -> Result<Page<Track>, OmniError> {
    let answer =
        client.platform("/artist/tracks/", json!({ "artist_id": id, "count": 50, "cursor": "0" }), false).await?;
    let rows = answer
        .get("tracks")
        .and_then(Value::as_array)
        .ok_or_else(|| OmniError::Response("Qishui artist tracks are missing".to_owned()))?;
    let selected: Vec<_> = rows.iter().skip(offset as usize).take(limit.clamp(1, 100) as usize).collect();
    let next_offset = offset.saturating_add(selected.len() as u32);
    Ok(Page {
        items: selected.into_iter().filter_map(track).collect(),
        total: Some(rows.len() as u64),
        has_more: (next_offset as usize) < rows.len(),
        next_offset,
    })
}

pub async fn references(client: &Client, song: &Track) -> Result<TrackCatalog, OmniError> {
    let mut references = if song
        .online_info
        .as_ref()
        .is_some_and(|info| !info.artists.is_empty() && info.artists.iter().all(|artist| !artist.id.is_empty()))
    {
        TrackCatalog::from_track(song)
    } else {
        let id = &song.online.as_ref().ok_or(OmniError::Unavailable)?.id;
        let raw = super::library::track_entity(client, id).await?;
        let resolved = track(&raw).ok_or(OmniError::Unavailable)?;
        TrackCatalog::from_track(&resolved)
    };
    references.album = None;
    Ok(references)
}
