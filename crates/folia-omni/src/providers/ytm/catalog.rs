// YouTube Music artist pages and browse links, retained exactly as the server names them.

use folia_core::{ArtistRef, OnlineTrackInfo, Track};
use serde_json::{Value, json};

use super::innertube::{Client, find_all, find_first, runs_text};
use super::{continuation_token, is_entity_id, rows, thumbnail};
use crate::{Artist, Collection, CollectionKind, OmniError, Page, TrackCatalog};

pub async fn search(client: &Client, query: &str, limit: u32, offset: u32) -> Result<Page<Artist>, OmniError> {
    if query.trim().is_empty() {
        return Ok(Page::empty(offset));
    }
    let mut answer = client.call("search", json!({ "query": query.trim(), "params": "EgWKAQIgAQ%3D%3D" })).await?;
    let mut items = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for _ in 0..100 {
        let mut found = Vec::new();
        find_all(&answer, "musicResponsiveListItemRenderer", &mut found);
        items.extend(found.into_iter().filter_map(search_artist));
        let next = continuation_token(&answer);
        if items.len() >= offset.saturating_add(limit.clamp(1, 100)) as usize || next.is_none() {
            return Ok(slice(&items, limit, offset, next.is_none()));
        }
        let token = next.ok_or(OmniError::Unavailable)?;
        if !seen.insert(token.clone()) {
            return Err(OmniError::Response("YouTube Music artist search cursor repeated".to_owned()));
        }
        answer = client.call("search", json!({ "continuation": token })).await?;
    }
    Err(OmniError::Response("YouTube Music artist search page limit reached".to_owned()))
}

fn search_artist(row: &Value) -> Option<Artist> {
    let id = row.pointer("/navigationEndpoint/browseEndpoint/browseId")?.as_str()?;
    if !id.starts_with("UC") || !is_entity_id(id) {
        return None;
    }
    Some(Artist {
        provider: super::ID.to_owned(),
        id: id.to_owned(),
        name: runs_text(row.pointer("/flexColumns/0/musicResponsiveListItemFlexColumnRenderer/text"))?,
        picture_url: thumbnail(row.get("thumbnail")),
        description: None,
    })
}

async fn artist_page(client: &Client, id: &str) -> Result<Value, OmniError> {
    if !id.starts_with("UC") || !is_entity_id(id) {
        return Err(OmniError::Response("Invalid YouTube Music artist id".to_owned()));
    }
    client.call("browse", json!({ "browseId": id })).await
}

pub async fn detail(client: &Client, id: &str) -> Result<Artist, OmniError> {
    let answer = artist_page(client, id).await?;
    let header = find_first(&answer, "musicImmersiveHeaderRenderer")
        .or_else(|| find_first(&answer, "musicVisualHeaderRenderer"))
        .ok_or(OmniError::Unavailable)?;
    Ok(Artist {
        provider: super::ID.to_owned(),
        id: id.to_owned(),
        name: runs_text(header.get("title")).ok_or(OmniError::Unavailable)?,
        picture_url: thumbnail(header.get("thumbnail")).or_else(|| thumbnail(header.get("foregroundThumbnail"))),
        description: runs_text(header.get("description")),
    })
}

pub async fn songs(client: &Client, id: &str, limit: u32, offset: u32) -> Result<Page<Track>, OmniError> {
    let answer = artist_page(client, id).await?;
    let shelf = find_first(&answer, "musicShelfRenderer").unwrap_or(&Value::Null);
    let songs = browse_items(client, shelf.clone(), |page| rows(page, None)).await?;
    Ok(slice(&songs, limit, offset, true))
}

pub async fn albums(client: &Client, id: &str, limit: u32, offset: u32) -> Result<Page<Collection>, OmniError> {
    let answer = artist_page(client, id).await?;
    let mut shelves = Vec::new();
    find_all(&answer, "musicCarouselShelfRenderer", &mut shelves);
    let mut items = Vec::new();
    for shelf in shelves {
        let cards = album_cards(shelf);
        if cards.is_empty() {
            continue;
        }
        let mut endpoints = Vec::new();
        find_all(shelf.get("header").unwrap_or(&Value::Null), "browseEndpoint", &mut endpoints);
        if let Some(endpoint) = endpoints
            .into_iter()
            .find(|endpoint| endpoint.get("browseId").and_then(Value::as_str).is_some_and(|id| id.starts_with("MPAD")))
        {
            let page = client.call("browse", endpoint.clone()).await?;
            items.extend(browse_items(client, page, album_cards).await?);
        } else {
            items.extend(browse_items(client, shelf.clone(), album_cards).await?);
        }
    }
    let mut seen = std::collections::HashSet::new();
    items.retain(|album| seen.insert(album.id.clone()));
    Ok(slice(&items, limit, offset, true))
}

// Follow all continuation pages before publishing the finite hot list or discography total.
async fn browse_items<T>(client: &Client, mut page: Value, read: fn(&Value) -> Vec<T>) -> Result<Vec<T>, OmniError> {
    let mut items = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for _ in 0..100 {
        items.extend(read(&page));
        let Some(token) = continuation_token(&page) else {
            return Ok(items);
        };
        if !seen.insert(token.clone()) {
            return Err(OmniError::Response("YouTube Music catalog cursor repeated".to_owned()));
        }
        page = client.call("browse", json!({ "continuation": token })).await?;
    }
    Err(OmniError::Response("YouTube Music catalog page limit reached".to_owned()))
}

fn album_cards(page: &Value) -> Vec<Collection> {
    let mut cards = Vec::new();
    find_all(page, "musicTwoRowItemRenderer", &mut cards);
    cards
        .into_iter()
        .filter_map(|card| {
            let id = card.pointer("/navigationEndpoint/browseEndpoint/browseId")?.as_str()?;
            if !id.starts_with("MPRE") || !is_entity_id(id) {
                return None;
            }
            Some(Collection {
                provider: super::ID.to_owned(),
                id: id.to_owned(),
                kind: CollectionKind::Album,
                name: runs_text(card.get("title"))?,
                cover_url: thumbnail(card.get("thumbnailRenderer")),
                track_count: None,
                creator: None,
                description: None,
                owned: false,
            })
        })
        .collect()
}

pub async fn album(client: &Client, id: &str) -> Result<Collection, OmniError> {
    if !id.starts_with("MPRE") || !is_entity_id(id) {
        return Err(OmniError::Response("Invalid YouTube Music album id".to_owned()));
    }
    let answer = client.call("browse", json!({ "browseId": id })).await?;
    let header = find_first(&answer, "musicResponsiveHeaderRenderer")
        .or_else(|| find_first(&answer, "musicDetailHeaderRenderer"))
        .ok_or(OmniError::Unavailable)?;
    let artists = artist_refs(header);
    Ok(Collection {
        provider: super::ID.to_owned(),
        id: id.to_owned(),
        kind: CollectionKind::Album,
        name: runs_text(header.get("title")).ok_or(OmniError::Unavailable)?,
        cover_url: thumbnail(header.get("thumbnail")),
        track_count: None,
        creator: (!artists.is_empty())
            .then(|| artists.iter().map(|artist| artist.name.as_str()).collect::<Vec<_>>().join(" / ")),
        description: runs_text(header.get("description")),
        owned: false,
    })
}

pub(super) fn artist_refs(node: &Value) -> Vec<ArtistRef> {
    let mut runs = Vec::new();
    find_all(node, "runs", &mut runs);
    let mut seen = std::collections::HashSet::new();
    runs.into_iter()
        .filter_map(Value::as_array)
        .flatten()
        .filter_map(|run| {
            let id = run.pointer("/navigationEndpoint/browseEndpoint/browseId")?.as_str()?;
            let name = run.get("text")?.as_str()?;
            (id.starts_with("UC") && is_entity_id(id) && seen.insert(id.to_owned()))
                .then(|| ArtistRef { id: id.to_owned(), name: name.to_owned() })
        })
        .collect()
}

pub async fn references(client: &Client, song: &Track) -> Result<TrackCatalog, OmniError> {
    if song.online_info.as_ref().is_some_and(|info| {
        !info.artists.is_empty() && info.artists.iter().all(|artist| !artist.id.is_empty()) && !info.album_id.is_empty()
    }) {
        return Ok(TrackCatalog::from_track(song));
    }
    let id = &song.online.as_ref().ok_or(OmniError::Unavailable)?.id;
    if !super::is_video_id(id) {
        return Err(OmniError::Unavailable);
    }
    let answer = client.call("next", json!({ "videoId": id })).await?;
    let mut videos = Vec::new();
    find_all(&answer, "playlistPanelVideoRenderer", &mut videos);
    let row = videos
        .into_iter()
        .find(|row| row.get("videoId").and_then(Value::as_str) == Some(id))
        .ok_or(OmniError::Unavailable)?;
    let mut resolved = song.clone();
    let mut album_id = String::new();
    let mut runs = Vec::new();
    find_all(row, "runs", &mut runs);
    for run in runs.into_iter().filter_map(Value::as_array).flatten() {
        if let Some(id) = run
            .pointer("/navigationEndpoint/browseEndpoint/browseId")
            .and_then(Value::as_str)
            .filter(|id| id.starts_with("MPRE") && is_entity_id(id))
        {
            album_id = id.to_owned();
            resolved.album = run.get("text").and_then(Value::as_str).map(str::to_owned);
        }
    }
    resolved.online_info = Some(OnlineTrackInfo { artists: artist_refs(row), album_id, ..Default::default() });
    resolved.cover_url = thumbnail(row.get("thumbnail")).or(resolved.cover_url);
    Ok(TrackCatalog::from_track(&resolved))
}

fn slice<T: Clone>(items: &[T], limit: u32, offset: u32, done: bool) -> Page<T> {
    let selected: Vec<T> = items.iter().skip(offset as usize).take(limit.clamp(1, 100) as usize).cloned().collect();
    let next_offset = offset.saturating_add(selected.len() as u32);
    Page {
        items: selected,
        total: done.then_some(items.len() as u64),
        has_more: !done || (next_offset as usize) < items.len(),
        next_offset,
    }
}
