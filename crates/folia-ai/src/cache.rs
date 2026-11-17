// SQLite song-theme cache uses exact platform identities or local paths and preserves complete theme pairs.

use std::path::Path;

use folia_core::Track;
use folia_store::DualTheme;
use rusqlite::{Connection, OptionalExtension};

use crate::Error;

pub fn song_key(track: &Track) -> String {
    if let Some(online) = &track.online {
        return format!("{}:{}:{}", online.provider.len(), online.provider, online.id);
    }
    let path: String = track.path.as_os_str().as_encoded_bytes().iter().map(|byte| format!("{byte:02x}")).collect();
    format!("local:{path}")
}

pub struct ThemeCache {
    connection: Connection,
}

impl ThemeCache {
    pub fn last(&self) -> Result<Option<DualTheme>, Error> {
        self.get("@last")
    }

    pub fn remember(&self, theme: &DualTheme, revision: i64) -> Result<(), Error> {
        self.put_revision("@last", theme, revision)
    }
    pub fn open(path: &Path) -> Result<Self, Error> {
        if let Some(parent) = path.parent().filter(|path| !path.as_os_str().is_empty()) {
            std::fs::create_dir_all(parent)?;
        }
        let mut connection = Connection::open(path)?;
        connection.busy_timeout(std::time::Duration::from_secs(5))?;
        connection.pragma_update(None, "journal_mode", "WAL")?;
        let transaction = connection.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        let version: u32 = transaction.pragma_query_value(None, "user_version", |row| row.get(0))?;
        if version > 2 {
            return Err(Error::Configuration("unsupported song-theme cache schema"));
        }
        transaction.execute_batch("CREATE TABLE IF NOT EXISTS song_themes (song TEXT PRIMARY KEY, theme TEXT NOT NULL, revision INTEGER NOT NULL DEFAULT 0);")?;
        if version == 1 {
            transaction.execute_batch("ALTER TABLE song_themes ADD COLUMN revision INTEGER NOT NULL DEFAULT 0;")?;
        }
        transaction.pragma_update(None, "user_version", 2)?;
        transaction.commit()?;
        Ok(Self { connection })
    }

    pub fn get(&self, song: &str) -> Result<Option<DualTheme>, Error> {
        let value: Option<String> = self
            .connection
            .query_row("SELECT theme FROM song_themes WHERE song = ?1", [song], |row| row.get(0))
            .optional()?;
        value.map(|value| serde_json::from_str(&value).map_err(Error::from)).transpose()
    }

    pub fn put(&self, song: &str, theme: &DualTheme) -> Result<(), Error> {
        self.put_revision(song, theme, Self::reserve_revision())
    }

    /// Captures request order before background work starts, including across independent cache connections.
    pub fn reserve_revision() -> i64 {
        use std::sync::atomic::{AtomicI64, Ordering};
        static LAST: AtomicI64 = AtomicI64::new(0);
        let time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
            .min(i64::MAX as u128 - 1) as i64;
        let previous = LAST.fetch_max(time, Ordering::Relaxed);
        if previous >= time { LAST.fetch_add(1, Ordering::Relaxed) + 1 } else { time }
    }

    pub fn put_revision(&self, song: &str, theme: &DualTheme, revision: i64) -> Result<(), Error> {
        let value = serde_json::to_string(theme)?;
        self.connection.execute("INSERT INTO song_themes(song, theme, revision) VALUES(?1, ?2, ?3) ON CONFLICT(song) DO UPDATE SET theme = excluded.theme, revision = excluded.revision WHERE song_themes.revision <= excluded.revision", rusqlite::params![song, value, revision])?;
        Ok(())
    }
}
