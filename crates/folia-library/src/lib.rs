// Local music library: watched folders, an incremental scan into SQLite, and lyric lookup.

mod cover;
mod lyrics;
mod playlists;
mod tags;

pub use cover::{Cover, CoverCache, cover_for, cover_rgba};
pub use lyrics::{decode_text, lyrics_for};
pub use playlists::Playlist;
pub use tags::gain_from_tag;
pub use tags::read as read_track;

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use folia_core::Track;
use rayon::prelude::*;
use rusqlite::{Connection, OptionalExtension, Transaction, params};

const SCHEMA_VERSION: i32 = 2;
const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS folders (path TEXT PRIMARY KEY);
CREATE TABLE IF NOT EXISTS tracks (
    id INTEGER PRIMARY KEY,
    path TEXT NOT NULL UNIQUE,
    title TEXT NOT NULL,
    artist TEXT,
    album TEXT,
    album_artist TEXT,
    track_no INTEGER,
    disc_no INTEGER,
    duration REAL NOT NULL,
    size INTEGER NOT NULL,
    mtime INTEGER NOT NULL,
    replay_gain_track REAL,
    replay_gain_album REAL
);
CREATE TABLE IF NOT EXISTS playlists (
    id INTEGER PRIMARY KEY,
    name TEXT NOT NULL,
    favorite INTEGER NOT NULL DEFAULT 0,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);
CREATE UNIQUE INDEX IF NOT EXISTS playlists_favorite ON playlists (favorite) WHERE favorite = 1;
CREATE TABLE IF NOT EXISTS playlist_songs (
    playlist_id INTEGER NOT NULL,
    track_id INTEGER NOT NULL,
    position INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS playlist_songs_order ON playlist_songs (playlist_id, position);";

/// Audio file extensions the library picks up, as in the original app.
pub const AUDIO_EXTENSIONS: [&str; 16] =
    ["mp3", "flac", "m4a", "wav", "ogg", "opus", "aac", "alac", "ape", "wv", "tta", "wma", "aif", "aiff", "caf", "dsf"];

#[derive(Debug, thiserror::Error)]
pub enum LibraryError {
    #[error("library database error")]
    Db(#[from] rusqlite::Error),
    #[error("cannot create {path}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("path is not valid UTF-8: {0}")]
    NonUtf8Path(PathBuf),
    #[error("a playlist needs a name")]
    EmptyPlaylistName,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ScanReport {
    pub updated: usize,
    pub removed: usize,
    pub total: usize,
}

pub struct Library {
    conn: Connection,
}

struct FoundFile {
    path: PathBuf,
    key: String,
    size: i64,
    mtime: i64,
}

impl Library {
    /// Opens or creates the database; an index from another schema version is dropped and rebuilt by the next scan.
    pub fn open(db_path: &Path) -> Result<Self, LibraryError> {
        if let Some(dir) = db_path.parent() {
            std::fs::create_dir_all(dir).map_err(|source| LibraryError::Io { path: dir.to_owned(), source })?;
        }
        let conn = Connection::open(db_path)?;
        let folders = Self::init(&conn)?;
        let library = Self { conn };
        for folder in folders {
            library.conn.execute("INSERT OR IGNORE INTO folders (path) VALUES (?1)", [folder])?;
        }
        Ok(library)
    }

    /// Creates the schema when needed and returns the folders to keep across a rebuild. Playlists are the
    /// listener's own data and are never dropped by a rebuild.
    fn init(conn: &Connection) -> Result<Vec<String>, LibraryError> {
        conn.pragma_update(None, "journal_mode", "WAL")?;
        let version: i32 = conn.pragma_query_value(None, "user_version", |row| row.get(0))?;
        if version == SCHEMA_VERSION {
            conn.execute_batch(SCHEMA)?;
            return Ok(Vec::new());
        }
        let has_folders: bool = conn
            .query_row("SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'folders'", [], |_| Ok(true))
            .optional()?
            .unwrap_or(false);
        let folders = if has_folders { query_strings(conn, "SELECT path FROM folders")? } else { Vec::new() };
        if version != 0 {
            tracing::info!(from = version, to = SCHEMA_VERSION, "rebuilding library index");
        }
        conn.execute_batch("DROP TABLE IF EXISTS folders; DROP TABLE IF EXISTS tracks;")?;
        conn.execute_batch(SCHEMA)?;
        conn.pragma_update(None, "user_version", SCHEMA_VERSION)?;
        Ok(folders)
    }

    pub fn folders(&self) -> Result<Vec<PathBuf>, LibraryError> {
        Ok(query_strings(&self.conn, "SELECT path FROM folders ORDER BY path")?
            .into_iter()
            .map(PathBuf::from)
            .collect())
    }

    pub fn add_folder(&self, folder: &Path) -> Result<(), LibraryError> {
        self.conn.execute("INSERT OR IGNORE INTO folders (path) VALUES (?1)", [utf8(folder)?])?;
        Ok(())
    }

    /// Forgets a folder and every track that only it covered.
    pub fn remove_folder(&mut self, folder: &Path) -> Result<usize, LibraryError> {
        let tx = self.conn.transaction()?;
        tx.execute("DELETE FROM folders WHERE path = ?1", [utf8(folder)?])?;
        let remaining = query_strings(&tx, "SELECT path FROM folders")?;
        let orphans: Vec<String> = query_strings(&tx, "SELECT path FROM tracks")?
            .into_iter()
            .filter(|path| !remaining.iter().any(|root| Path::new(path).starts_with(root)))
            .collect();
        let removed = delete_tracks(&tx, &orphans)?;
        tx.commit()?;
        Ok(removed)
    }

    /// Walks every folder, re-reads files whose size or modification time changed, and drops vanished files.
    pub fn scan(&mut self) -> Result<ScanReport, LibraryError> {
        let found = self.find_files()?;
        let known: HashMap<String, (i64, i64)> = {
            let mut stmt = self.conn.prepare("SELECT path, size, mtime FROM tracks")?;
            stmt.query_map([], |row| Ok((row.get(0)?, (row.get(1)?, row.get(2)?))))?.collect::<Result<_, _>>()?
        };

        let changed: Vec<(&FoundFile, Track)> = found
            .par_iter()
            .filter(|file| known.get(&file.key) != Some(&(file.size, file.mtime)))
            .map(|file| (file, tags::read(&file.path)))
            .collect();
        let found_keys: HashSet<&str> = found.iter().map(|file| file.key.as_str()).collect();
        let vanished: Vec<String> = known.into_keys().filter(|key| !found_keys.contains(key.as_str())).collect();

        let tx = self.conn.transaction()?;
        {
            let mut upsert = tx.prepare(
                "INSERT INTO tracks (path, title, artist, album, album_artist, track_no, disc_no, duration, size, mtime,
                 replay_gain_track, replay_gain_album)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)
                 ON CONFLICT(path) DO UPDATE SET title = ?2, artist = ?3, album = ?4, album_artist = ?5,
                 track_no = ?6, disc_no = ?7, duration = ?8, size = ?9, mtime = ?10,
                 replay_gain_track = ?11, replay_gain_album = ?12",
            )?;
            for (file, t) in &changed {
                upsert.execute(params![
                    file.key,
                    t.title,
                    t.artist,
                    t.album,
                    t.album_artist,
                    t.track_no,
                    t.disc_no,
                    t.duration,
                    file.size,
                    file.mtime,
                    t.replay_gain.and_then(|gain| gain.track_db),
                    t.replay_gain.and_then(|gain| gain.album_db)
                ])?;
            }
        }
        let removed = delete_tracks(&tx, &vanished)?;
        tx.commit()?;

        let report = ScanReport { updated: changed.len(), removed, total: found.len() };
        tracing::info!(updated = report.updated, removed = report.removed, total = report.total, "library scanned");
        Ok(report)
    }

    /// All tracks, ordered by artist, album, disc, track number and title.
    pub fn tracks(&self) -> Result<Vec<Track>, LibraryError> {
        let mut stmt = self.conn.prepare(
            "SELECT id, path, title, artist, album, album_artist, track_no, disc_no, duration,
             replay_gain_track, replay_gain_album FROM tracks
             ORDER BY COALESCE(album_artist, artist) COLLATE NOCASE, album COLLATE NOCASE,
             disc_no, track_no, title COLLATE NOCASE",
        )?;
        let tracks = stmt.query_map([], |row| {
            let track_gain: Option<f32> = row.get(9)?;
            let album_gain: Option<f32> = row.get(10)?;
            Ok(Track {
                id: row.get(0)?,
                path: PathBuf::from(row.get::<_, String>(1)?),
                title: row.get(2)?,
                artist: row.get(3)?,
                album: row.get(4)?,
                album_artist: row.get(5)?,
                track_no: row.get(6)?,
                disc_no: row.get(7)?,
                duration: row.get(8)?,
                replay_gain: (track_gain.is_some() || album_gain.is_some())
                    .then_some(folia_core::ReplayGain { track_db: track_gain, album_db: album_gain }),
                ..Track::default()
            })
        })?;
        Ok(tracks.collect::<Result<_, _>>()?)
    }

    /// Every local playlist, the liked-songs list first.
    pub fn playlists(&self) -> Result<Vec<Playlist>, LibraryError> {
        playlists::all(&self.conn)
    }

    pub fn create_playlist(&self, name: &str) -> Result<Playlist, LibraryError> {
        playlists::create(&self.conn, name)
    }

    /// The liked-songs list, created on first use (`getFavoriteLocalPlaylist`).
    pub fn favorite_playlist(&self) -> Result<Playlist, LibraryError> {
        playlists::favorite(&self.conn)
    }

    /// Appends tracks not already in the playlist; returns how many were added.
    pub fn add_to_playlist(&self, playlist: i64, tracks: &[i64]) -> Result<usize, LibraryError> {
        playlists::add(&self.conn, playlist, tracks)
    }

    pub fn remove_from_playlist(&self, playlist: i64, tracks: &[i64]) -> Result<usize, LibraryError> {
        playlists::remove(&self.conn, playlist, tracks)
    }

    /// `isLocalSongLiked`.
    pub fn is_favorite(&self, track: i64) -> Result<bool, LibraryError> {
        playlists::is_favorite(&self.conn, track)
    }

    /// `setLocalSongFavorite`.
    pub fn set_favorite(&self, track: i64, favorite: bool) -> Result<(), LibraryError> {
        playlists::set_favorite(&self.conn, track, favorite)
    }

    pub fn delete_playlist(&self, playlist: i64) -> Result<(), LibraryError> {
        playlists::delete(&self.conn, playlist)
    }

    fn find_files(&self) -> Result<Vec<FoundFile>, LibraryError> {
        let mut seen = HashSet::new();
        let mut files = Vec::new();
        for folder in self.folders()? {
            for entry in walkdir::WalkDir::new(&folder).follow_links(true).into_iter().filter_map(Result::ok) {
                if !entry.file_type().is_file() || !is_audio(entry.path()) {
                    continue;
                }
                let Some(key) = entry.path().to_str().map(str::to_owned) else {
                    tracing::warn!(path = %entry.path().display(), "skipping non UTF-8 path");
                    continue;
                };
                let Ok(meta) = entry.metadata() else { continue };
                let mtime = meta
                    .modified()
                    .ok()
                    .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
                    .map_or(0, |since| since.as_secs() as i64);
                if seen.insert(key.clone()) {
                    files.push(FoundFile { path: entry.into_path(), key, size: meta.len() as i64, mtime });
                }
            }
        }
        Ok(files)
    }
}

pub fn is_audio(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| AUDIO_EXTENSIONS.iter().any(|known| known.eq_ignore_ascii_case(ext)))
}

fn delete_tracks(tx: &Transaction<'_>, paths: &[String]) -> Result<usize, LibraryError> {
    let mut removed = 0;
    let mut ids = Vec::new();
    {
        let mut id_of = tx.prepare("SELECT id FROM tracks WHERE path = ?1")?;
        let mut delete = tx.prepare("DELETE FROM tracks WHERE path = ?1")?;
        for path in paths {
            match id_of.query_row([path], |row| row.get::<_, i64>(0)).optional()? {
                Some(id) => ids.push(id),
                None => continue,
            }
            removed += delete.execute([path])?;
        }
    }
    playlists::forget_tracks(tx, &ids)?;
    Ok(removed)
}

fn query_strings(conn: &Connection, sql: &str) -> Result<Vec<String>, rusqlite::Error> {
    let mut stmt = conn.prepare(sql)?;
    stmt.query_map([], |row| row.get(0))?.collect()
}

fn utf8(path: &Path) -> Result<&str, LibraryError> {
    path.to_str().ok_or_else(|| LibraryError::NonUtf8Path(path.to_owned()))
}
