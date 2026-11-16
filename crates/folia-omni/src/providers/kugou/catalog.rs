// KuGou author pages and hash-verified recovery of canonical artist and album identifiers.

use folia_core::{ArtistRef, Track};
use serde_json::{Value, json};

use super::gateway::{self, Client, Request};
use super::{SongKey, collection, cover_of, data_of, list_of, number_of, text_of, track, value_of};
use crate::{Artist, Collection, CollectionKind, OmniError, Page, TrackCatalog};

pub(super) fn references(raw: &Value) -> Vec<ArtistRef> {
    let singer_info = raw.get("singerinfo");
    let rows = value_of(raw, &["Singers", "singers", "singerinfo", "authors", "artists"]);
    rows.and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|row| {
            let row = row.get("base").unwrap_or(row);
            let id = text_of(row, &["author_id", "authorId", "AuthorId", "AuthorID"])
                .or_else(|| (rows == singer_info).then(|| text_of(row, &["id"])).flatten())
                .filter(|id| id != "0")
                .unwrap_or_default();
            Some(ArtistRef { id, name: text_of(row, &["name", "author_name", "singername"])? })
        })
        .collect()
}

fn artist(raw: &Value) -> Option<Artist> {
    Some(Artist {
        provider: super::ID.to_owned(),
        id: text_of(raw, &["author_id", "AuthorID", "AuthorId", "authorId"]).filter(|id| id != "0")?,
        name: text_of(raw, &["author_name", "AuthorName", "name", "singername"])?,
        picture_url: cover_of(raw)
            .or_else(|| text_of(raw, &["Avatar"]).map(|url| url.replacen("http://", "https://", 1))),
        description: text_of(raw, &["intro", "description", "brief_desc", "briefDesc", "brief_description", "desc"]),
    })
}

fn page<T>(items: Vec<T>, answer: &Value, count: u32, limit: u32, offset: u32) -> Page<T> {
    let total = number_of(data_of(answer), &["total", "total_count", "count"])
        .or_else(|| number_of(answer, &["total", "total_count", "count"]))
        .filter(|total| *total >= 0.0)
        .map(|total| total as u64);
    let next_offset = (offset / limit + 1).saturating_mul(limit);
    let has_more = count > 0 && total.map_or(count == limit, |total| u64::from(next_offset) < total);
    Page { items, total, has_more, next_offset }
}

pub async fn search(client: &Client, query: &str, limit: u32, offset: u32) -> Result<Page<Artist>, OmniError> {
    let limit = limit.clamp(1, 100);
    let answer = client
        .call(|| {
            Request::get("/v1/search/author")
                .router("complexsearch.kugou.com")
                .param("keyword", query)
                .param("page", offset / limit + 1)
                .param("pagesize", limit)
                .param("platform", "AndroidFilter")
                .param("albumhide", 0)
                .param("iscorrection", 1)
                .param("nocollect", 0)
        })
        .await?;
    let rows = list_of(&answer);
    Ok(page(rows.iter().filter_map(artist).collect(), &answer, rows.len() as u32, limit, offset))
}

pub async fn detail(client: &Client, id: &str) -> Result<Artist, OmniError> {
    let answer = client
        .call(|| {
            let mut request = Request::post("/kmr/v3/author", json!({ "author_id": id })).router("openapi.kugou.com");
            request.headers.push(("kg-tid", "36"));
            request
        })
        .await?;
    artist(data_of(&answer))
        .filter(|artist| artist.id == id)
        .ok_or_else(|| OmniError::Response("KuGou artist record missing".to_owned()))
}

pub async fn songs(client: &Client, id: &str, limit: u32, offset: u32) -> Result<Page<Track>, OmniError> {
    let limit = limit.clamp(1, 100);
    let answer = client
        .call(|| {
            let time = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs();
            let body = json!({ "appid": gateway::APPID, "clientver": gateway::CLIENTVER, "mid": client.session().mid,
            "clienttime": time, "key": gateway::sign_params_key(&time.to_string()), "author_id": id,
            "pagesize": limit, "page": offset / limit + 1, "sort": 1, "area_code": "all" });
            let mut request = Request::post("/kmr/v1/audio_group/author", body)
                .base("https://openapi.kugou.com")
                .router("openapi.kugou.com");
            request.headers.push(("kg-tid", "220"));
            request
        })
        .await?;
    let rows = list_of(&answer);
    Ok(page(rows.iter().filter_map(track).collect(), &answer, rows.len() as u32, limit, offset))
}

pub async fn albums(client: &Client, id: &str, limit: u32, offset: u32) -> Result<Page<Collection>, OmniError> {
    let limit = limit.clamp(1, 100);
    let answer = client.call(|| {
        let body = json!({ "author_id": id, "pagesize": limit, "page": offset / limit + 1, "sort": 1, "category": 1, "area_code": "all" });
        let mut request = Request::post("/kmr/v1/author/albums", body).router("openapi.kugou.com");
        request.headers.push(("kg-tid", "36"));
        request
    }).await?;
    let rows = list_of(&answer);
    Ok(page(
        rows.iter().filter_map(|row| collection(row, CollectionKind::Album)).collect(),
        &answer,
        rows.len() as u32,
        limit,
        offset,
    ))
}

pub async fn album(client: &Client, id: &str) -> Result<Collection, OmniError> {
    let info = super::library::album_info(client, id).await?;
    collection(&info, CollectionKind::Album).ok_or_else(|| OmniError::Response("KuGou album record missing".to_owned()))
}

pub async fn track_catalog(client: &Client, song: &Track) -> Result<TrackCatalog, OmniError> {
    let key = SongKey::parse(&song.online.as_ref().ok_or(OmniError::Unavailable)?.id);
    let needs_refs = song.online_info.as_ref().is_none_or(|info| {
        info.album_id.is_empty() || info.artists.is_empty() || info.artists.iter().any(|artist| artist.id.is_empty())
    });
    if !needs_refs || key.album_audio_id.is_empty() {
        return Ok(TrackCatalog::from_track(song));
    }
    let entity: u64 =
        key.album_audio_id.parse().map_err(|_| OmniError::Response("invalid KuGou audio id".to_owned()))?;
    let answer = client
        .call(|| {
            let body =
                json!({ "data": [{ "entity_id": entity }], "fields": "album_info,authors.base,base,audio_info" });
            let mut request = Request::post("/kmr/v2/audio", body).router("openapi.kugou.com");
            request.headers.push(("kg-tid", "238"));
            request
        })
        .await?;
    let rows = list_of(&answer);
    let metadata = rows.first().ok_or(OmniError::Unavailable)?;
    let audio = metadata.get("audio_info").or_else(|| metadata.get("audioInfo")).unwrap_or(&Value::Null);
    let matches = ["hash", "FileHash", "fileHash", "hash_128", "hash128"]
        .iter()
        .any(|field| text_of(audio, &[*field]).is_some_and(|hash| hash.eq_ignore_ascii_case(&key.hash)));
    if !matches {
        return Err(OmniError::Response("KuGou catalog song hash mismatch".to_owned()));
    }
    let resolved = track(metadata).ok_or(OmniError::Unavailable)?;
    Ok(TrackCatalog::from_track(&resolved))
}
