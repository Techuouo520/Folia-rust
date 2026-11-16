// QQ artist catalog pages and songmid-based recovery of singer and album mids.

use folia_core::{ArtistRef, Track};
use serde_json::{Value, json};

use super::musicu::Client;
use super::{SongKey, original_cover, pick, text, track, tracks};
use crate::{Artist, Collection, OmniError, Page, TrackCatalog};

fn mid(raw: &Value, keys: &[&str]) -> Option<String> {
    text(raw, keys).filter(|id| {
        id.bytes().all(|byte| byte.is_ascii_alphanumeric()) && id.bytes().any(|byte| byte.is_ascii_alphabetic())
    })
}

pub(super) fn references(raw: &Value) -> Vec<ArtistRef> {
    pick(raw, &["singer", "singers", "artists"])
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|row| {
            Some(ArtistRef {
                id: mid(row, &["mid", "singermid", "singerMid", "singerMID"]).unwrap_or_default(),
                name: text(row, &["name", "title", "singername", "singerName"])?,
            })
        })
        .collect()
}

fn artist(raw: &Value) -> Option<Artist> {
    let id = mid(raw, &["mid", "singermid", "singerMid", "singerMID"])?;
    let picture_url = text(raw, &["singerPic", "picUrl", "pic"])
        .map(|url| original_cover(&url))
        .or_else(|| Some(format!("https://y.gtimg.cn/music/photo_new/T001R800x800M000{id}.jpg")));
    Some(Artist {
        provider: super::ID.to_owned(),
        id,
        name: text(raw, &["name", "singername", "singerName"])?,
        picture_url,
        description: text(raw, &["singer_brief", "desc", "description"]),
    })
}

pub async fn search(client: &Client, query: &str, limit: u32, offset: u32) -> Result<Page<Artist>, OmniError> {
    let size = limit.clamp(1, 20);
    let keyword: String = query.trim().chars().take(60).collect();
    if keyword.is_empty() {
        return Ok(Page::empty(offset));
    }
    let page = offset / size + 1;
    let data = client.search(super::library::search_param(&keyword, page, size, 1)).await?;
    let rows = data
        .pointer("/body/singer")
        .and_then(Value::as_array)
        .ok_or_else(|| OmniError::Response("QQ artist search has no singer list".to_owned()))?;
    let items = rows.iter().filter_map(artist).collect();
    let total = data.pointer("/meta/sum").and_then(Value::as_u64);
    let next_offset = page.saturating_mul(size);
    let has_more =
        !rows.is_empty() && total.map_or(rows.len() == size as usize, |total| u64::from(next_offset) < total);
    Ok(Page { items, total, has_more, next_offset })
}

async fn singer_data(client: &Client, id: &str, limit: u32, offset: u32) -> Result<Value, OmniError> {
    client
        .anonymous(
            "music.web_singer_info_svr",
            "get_singer_detail_info",
            json!({ "sort": 5, "singermid": id, "sin": offset, "num": limit }),
        )
        .await
}

pub async fn detail(client: &Client, id: &str) -> Result<Artist, OmniError> {
    let data = singer_data(client, id, 1, 0).await?;
    let mut result = artist(&data["singer_info"])
        .filter(|artist| artist.id == id)
        .ok_or_else(|| OmniError::Response("QQ artist record missing".to_owned()))?;
    result.description = text(&data, &["singer_brief"]);
    Ok(result)
}

pub async fn songs(client: &Client, id: &str, limit: u32, offset: u32) -> Result<Page<Track>, OmniError> {
    let limit = limit.clamp(1, 100);
    let data = singer_data(client, id, limit, offset).await?;
    let count = data.get("songlist").and_then(Value::as_array).map_or(0, |rows| rows.len() as u32);
    let total = data.get("total_song").and_then(Value::as_u64);
    let next_offset = offset.saturating_add(count);
    let has_more = count > 0 && total.map_or(count == limit, |total| u64::from(next_offset) < total);
    Ok(Page { items: tracks(data.get("songlist")), total, has_more, next_offset })
}

pub async fn albums(client: &Client, id: &str, limit: u32, offset: u32) -> Result<Page<Collection>, OmniError> {
    let limit = limit.clamp(1, 100);
    let data = client
        .anonymous(
            "music.musichallAlbum.AlbumListServer",
            "GetAlbumList",
            json!({ "sort": 5, "singermid": id, "begin": offset, "num": limit }),
        )
        .await?;
    let rows = data.get("albumList").and_then(Value::as_array);
    let count = rows.map_or(0, |rows| rows.len() as u32);
    let items = rows.into_iter().flatten().filter_map(super::album).collect();
    let total = data.get("total").and_then(Value::as_u64);
    let next_offset = offset.saturating_add(count);
    let has_more = count > 0 && total.map_or(count == limit, |total| u64::from(next_offset) < total);
    Ok(Page { items, total, has_more, next_offset })
}

pub async fn album(client: &Client, id: &str) -> Result<Collection, OmniError> {
    let params = [("albummid", id.to_owned()), ("format", "json".to_owned()), ("outCharset", "utf-8".to_owned())];
    let answer = client.legacy("/v8/fcg-bin/fcg_v8_album_info_cp.fcg", &params).await?;
    if let Some(code) = answer.get("code").and_then(Value::as_i64).filter(|code| *code != 0) {
        return Err(OmniError::Platform(code));
    }
    let mut data = answer.get("data").cloned().unwrap_or(Value::Null);
    if let Some(data) = data.as_object_mut() {
        data.insert("mid".to_owned(), json!(id));
    }
    super::album(&data).ok_or_else(|| OmniError::Response("QQ album record missing".to_owned()))
}

pub async fn track_catalog(client: &Client, song: &Track) -> Result<TrackCatalog, OmniError> {
    let mut resolved = song.clone();
    if song.online_info.as_ref().is_none_or(|info| {
        info.album_id.is_empty() || info.artists.is_empty() || info.artists.iter().any(|artist| artist.id.is_empty())
    }) {
        let key = SongKey::parse(&song.online.as_ref().ok_or(OmniError::Unavailable)?.id);
        let data = client
            .anonymous(
                "music.pf_song_detail_svr",
                "get_song_detail_yqq",
                json!({ "song_type": 0, "song_mid": key.mid }),
            )
            .await?;
        resolved = track(&data["track_info"])
            .filter(|track| track.online.as_ref().is_some_and(|online| SongKey::parse(&online.id).mid == key.mid))
            .ok_or(OmniError::Unavailable)?;
    }
    Ok(TrackCatalog::from_track(&resolved))
}
