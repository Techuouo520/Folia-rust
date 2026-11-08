// The listener's own playlists, including the liked-songs list the heart writes to.

use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::{Connection, OptionalExtension, params};

use crate::LibraryError;

/// A local playlist: its name, its songs in order and whether it is the liked-songs list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Playlist {
    pub id: i64,
    pub name: String,
    pub favorite: bool,
    pub song_ids: Vec<i64>,
    pub created_at: i64,
    pub updated_at: i64,
}

pub(super) fn all(conn: &Connection) -> Result<Vec<Playlist>, LibraryError> {
    let mut stmt = conn.prepare(
        "SELECT id, name, favorite, created_at, updated_at FROM playlists ORDER BY favorite DESC, created_at, id",
    )?;
    let rows = stmt.query_map([], |row| {
        Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?, row.get::<_, i64>(2)?, row.get(3)?, row.get(4)?))
    })?;
    let mut playlists = Vec::new();
    for row in rows {
        let (id, name, favorite, created_at, updated_at) = row?;
        playlists.push(Playlist {
            id,
            name,
            favorite: favorite != 0,
            song_ids: songs(conn, id)?,
            created_at,
            updated_at,
        });
    }
    Ok(playlists)
}

fn songs(conn: &Connection, playlist: i64) -> Result<Vec<i64>, rusqlite::Error> {
    let mut stmt = conn.prepare("SELECT track_id FROM playlist_songs WHERE playlist_id = ?1 ORDER BY position")?;
    stmt.query_map([playlist], |row| row.get(0))?.collect()
}

/// Adds a playlist with no songs and returns it.
pub(super) fn create(conn: &Connection, name: &str) -> Result<Playlist, LibraryError> {
    let name = name.trim();
    if name.is_empty() {
        return Err(LibraryError::EmptyPlaylistName);
    }
    let now = now_millis();
    conn.execute(
        "INSERT INTO playlists (name, favorite, created_at, updated_at) VALUES (?1, 0, ?2, ?2)",
        params![name, now],
    )?;
    let id = conn.last_insert_rowid();
    tracing::info!(playlist = id, "local playlist created");
    Ok(Playlist { id, name: name.to_owned(), favorite: false, song_ids: Vec::new(), created_at: now, updated_at: now })
}

/// The liked-songs playlist, created the first time it is asked for.
pub(super) fn favorite(conn: &Connection) -> Result<Playlist, LibraryError> {
    let existing: Option<(i64, String, i64, i64)> = conn
        .query_row("SELECT id, name, created_at, updated_at FROM playlists WHERE favorite = 1", [], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
        })
        .optional()?;
    match existing {
        Some((id, name, created_at, updated_at)) => {
            Ok(Playlist { id, name, favorite: true, song_ids: songs(conn, id)?, created_at, updated_at })
        }
        None => {
            let now = now_millis();
            conn.execute(
                "INSERT INTO playlists (name, favorite, created_at, updated_at) VALUES ('Liked Songs', 1, ?1, ?1)",
                [now],
            )?;
            let id = conn.last_insert_rowid();
            Ok(Playlist {
                id,
                name: "Liked Songs".to_owned(),
                favorite: true,
                song_ids: Vec::new(),
                created_at: now,
                updated_at: now,
            })
        }
    }
}

/// Appends the tracks not already in the playlist; returns how many were added.
pub(super) fn add(conn: &Connection, playlist: i64, tracks: &[i64]) -> Result<usize, LibraryError> {
    let mut existing = songs(conn, playlist)?;
    let mut position = existing.len() as i64;
    let mut added = 0;
    for &track in tracks {
        if existing.contains(&track) {
            continue;
        }
        conn.execute(
            "INSERT INTO playlist_songs (playlist_id, track_id, position) VALUES (?1, ?2, ?3)",
            params![playlist, track, position],
        )?;
        existing.push(track);
        position += 1;
        added += 1;
    }
    touch(conn, playlist)?;
    Ok(added)
}

/// Removes the tracks from the playlist; returns how many left.
pub(super) fn remove(conn: &Connection, playlist: i64, tracks: &[i64]) -> Result<usize, LibraryError> {
    let mut removed = 0;
    for &track in tracks {
        removed += conn
            .execute("DELETE FROM playlist_songs WHERE playlist_id = ?1 AND track_id = ?2", params![playlist, track])?;
    }
    touch(conn, playlist)?;
    Ok(removed)
}

/// Answers whether the track is in the liked-songs list (`isLocalSongLiked`).
pub(super) fn is_favorite(conn: &Connection, track: i64) -> Result<bool, LibraryError> {
    let found: Option<i64> = conn
        .query_row(
            "SELECT 1 FROM playlist_songs WHERE track_id = ?1 AND playlist_id IN
             (SELECT id FROM playlists WHERE favorite = 1)",
            [track],
            |row| row.get(0),
        )
        .optional()?;
    Ok(found.is_some())
}

/// Puts the track in the liked-songs list or takes it out (`setLocalSongFavorite`).
pub(super) fn set_favorite(conn: &Connection, track: i64, favorite: bool) -> Result<(), LibraryError> {
    let list = self::favorite(conn)?;
    if favorite {
        add(conn, list.id, &[track])?;
    } else {
        remove(conn, list.id, &[track])?;
    }
    Ok(())
}

/// Drops a playlist unless it is the liked-songs list.
pub(super) fn delete(conn: &Connection, playlist: i64) -> Result<(), LibraryError> {
    conn.execute("DELETE FROM playlist_songs WHERE playlist_id = ?1", [playlist])?;
    conn.execute("DELETE FROM playlists WHERE id = ?1 AND favorite = 0", [playlist])?;
    Ok(())
}

/// Drops the playlist rows of tracks that left the index.
pub(super) fn forget_tracks(tx: &Connection, tracks: &[i64]) -> Result<(), rusqlite::Error> {
    let mut stmt = tx.prepare("DELETE FROM playlist_songs WHERE track_id = ?1")?;
    for track in tracks {
        stmt.execute([track])?;
    }
    Ok(())
}

fn touch(conn: &Connection, playlist: i64) -> Result<(), rusqlite::Error> {
    conn.execute("UPDATE playlists SET updated_at = ?1 WHERE id = ?2", params![now_millis(), playlist])?;
    Ok(())
}

fn now_millis() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |since| since.as_millis() as i64)
}
