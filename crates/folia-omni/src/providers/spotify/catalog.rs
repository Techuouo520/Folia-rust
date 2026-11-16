// Spotify artist search and exact catalog navigation through the signed-in client.

use folia_core::Track;
use serde_json::{Value, json};

use super::api::Client;
use super::{album_of, picture, plain_text, song_of, spotify_id};
use crate::{Artist, Collection, OmniError, Page, TrackCatalog};

pub async fn search(client: &Client, query: &str, limit: u32, offset: u32) -> Result<Page<Artist>, OmniError> {
    if query.trim().is_empty() {
        return Ok(Page::empty(offset));
    }
    let url = reqwest::Url::parse_with_params(
        "https://api.spotify.com/v1/search",
        &[
            ("q", query.trim().to_owned()),
            ("type", "artist".to_owned()),
            ("limit", limit.clamp(1, 10).to_string()),
            ("offset", offset.to_string()),
        ],
    )
    .map_err(|error| OmniError::Response(error.to_string()))?;
    let answer = client.call(url.as_str(), None, None, &[]).await?.ok_or(OmniError::Unavailable)?;
    let rows = answer
        .pointer("/artists/items")
        .and_then(Value::as_array)
        .ok_or_else(|| OmniError::Response("Spotify artist search is missing".to_owned()))?;
    let items = rows
        .iter()
        .filter_map(|row| {
            Some(Artist {
                provider: super::ID.to_owned(),
                id: row.get("id")?.as_str()?.to_owned(),
                name: row.get("name")?.as_str()?.to_owned(),
                picture_url: picture(row.get("images")),
                description: None,
            })
        })
        .collect();
    Ok(page(items, offset, rows.len(), answer.pointer("/artists/total").and_then(Value::as_u64)))
}

async fn overview(client: &Client, id: &str) -> Result<Value, OmniError> {
    client
        .query(
            "queryArtistOverview",
            json!({ "uri": format!("spotify:artist:{id}"), "locale": "", "preReleaseV2": false }),
            None,
        )
        .await
}

pub async fn detail(client: &Client, id: &str) -> Result<Artist, OmniError> {
    let answer = overview(client, id).await?;
    let row = answer.get("artistUnion").ok_or(OmniError::Unavailable)?;
    let id = row
        .get("uri")
        .and_then(Value::as_str)
        .and_then(|uri| spotify_id(uri, "artist"))
        .filter(|found| found == id)
        .ok_or(OmniError::Unavailable)?;
    Ok(Artist {
        provider: super::ID.to_owned(),
        id,
        name: row.pointer("/profile/name").and_then(Value::as_str).ok_or(OmniError::Unavailable)?.to_owned(),
        picture_url: picture(row.pointer("/visuals/avatarImage/sources")),
        description: row
            .pointer("/profile/biography/text")
            .and_then(Value::as_str)
            .map(plain_text)
            .filter(|text| !text.is_empty()),
    })
}

pub async fn songs(client: &Client, id: &str, limit: u32, offset: u32) -> Result<Page<Track>, OmniError> {
    let answer = overview(client, id).await?;
    let rows = answer
        .pointer("/artistUnion/discography/topTracks/items")
        .and_then(Value::as_array)
        .ok_or_else(|| OmniError::Response("Spotify artist tracks are missing".to_owned()))?;
    let selected: Vec<_> = rows.iter().skip(offset as usize).take(limit.clamp(1, 100) as usize).collect();
    let items = selected.iter().filter_map(|row| row.get("track")).filter_map(song_of).collect();
    Ok(page(items, offset, selected.len(), Some(rows.len() as u64)))
}

pub async fn albums(client: &Client, id: &str, limit: u32, offset: u32) -> Result<Page<Collection>, OmniError> {
    let answer = client.query("queryArtistDiscographyAll", json!({ "uri": format!("spotify:artist:{id}"), "offset": offset, "limit": limit.clamp(1, 100), "order": "DATE_DESC" }), None).await?;
    let all = answer.pointer("/artistUnion/discography/all").ok_or(OmniError::Unavailable)?;
    let rows = all
        .get("items")
        .and_then(Value::as_array)
        .ok_or_else(|| OmniError::Response("Spotify artist albums are missing".to_owned()))?;
    let items = rows.iter().filter_map(|row| row.pointer("/releases/items/0")).filter_map(album_of).collect();
    Ok(page(items, offset, rows.len(), all.get("totalCount").and_then(Value::as_u64)))
}

pub async fn album(client: &Client, id: &str) -> Result<Collection, OmniError> {
    let answer = client
        .query("getAlbum", json!({ "uri": format!("spotify:album:{id}"), "locale": "", "offset": 0, "limit": 1 }), None)
        .await?;
    answer.get("albumUnion").and_then(album_of).filter(|album| album.id == id).ok_or(OmniError::Unavailable)
}

pub async fn references(client: &Client, song: &Track) -> Result<TrackCatalog, OmniError> {
    if song.online_info.as_ref().is_some_and(|info| {
        !info.artists.is_empty() && info.artists.iter().all(|artist| !artist.id.is_empty()) && !info.album_id.is_empty()
    }) {
        return Ok(TrackCatalog::from_track(song));
    }
    let id = &song.online.as_ref().ok_or(OmniError::Unavailable)?.id;
    let answer = client.query("getTrack", json!({ "uri": format!("spotify:track:{id}") }), None).await?;
    let resolved = answer
        .get("trackUnion")
        .and_then(song_of)
        .filter(|track| track.online.as_ref().is_some_and(|online| online.id == *id))
        .ok_or(OmniError::Unavailable)?;
    Ok(TrackCatalog::from_track(&resolved))
}

fn page<T>(items: Vec<T>, offset: u32, count: usize, total: Option<u64>) -> Page<T> {
    let next_offset = offset.saturating_add(count as u32);
    let has_more = count > 0 && next_offset > offset && total.is_none_or(|total| u64::from(next_offset) < total);
    Page { items, total, has_more, next_offset }
}
