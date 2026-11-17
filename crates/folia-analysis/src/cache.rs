// Versioned complete analysis results, keyed by the exact playback representation and model hash.
use std::path::Path;
use std::sync::Arc;

use rusqlite::{Connection, OptionalExtension, params};

use crate::models::ModelKind;
use crate::{AnalysisData, AnalysisError};

pub struct Cache {
    connection: Connection,
}

impl Cache {
    pub fn at(path: &Path) -> Result<Self, AnalysisError> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let connection = Connection::open(path)?;
        connection.busy_timeout(std::time::Duration::from_secs(5))?;
        let version: i64 = connection.pragma_query_value(None, "user_version", |row| row.get(0))?;
        if version != 0 && version != 1 {
            tracing::warn!(version, "rebuilding unsupported music analysis cache");
            connection.execute("DROP TABLE IF EXISTS music_analysis", [])?;
        }
        connection.execute_batch("PRAGMA journal_mode=WAL; CREATE TABLE IF NOT EXISTS music_analysis (track TEXT NOT NULL, kind TEXT NOT NULL, revision TEXT NOT NULL, payload BLOB NOT NULL, PRIMARY KEY(track,kind)); PRAGMA user_version=1;")?;
        Ok(Self { connection })
    }

    pub fn get(&self, key: &str, model: ModelKind) -> Result<Option<AnalysisData>, AnalysisError> {
        let found: Option<(String, Vec<u8>)> = self
            .connection
            .query_row(
                "SELECT revision,payload FROM music_analysis WHERE track=?1 AND kind=?2",
                params![key, model.file()],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?;
        let Some((revision, payload)) = found else { return Ok(None) };
        if revision != revision_of(model)? {
            return Ok(None);
        }
        let decoded = match model {
            ModelKind::BeatThis => serde_json::from_slice(&payload).map(|value| AnalysisData::Beats(Arc::new(value))),
            ModelKind::Htdemucs => serde_json::from_slice(&payload).map(|value| AnalysisData::Stems(Arc::new(value))),
        };
        match decoded {
            Ok(data) if valid(&data) => Ok(Some(data)),
            Ok(_) => {
                tracing::warn!(model = model.file(), "discarding invalid analysis timeline");
                self.remove(key, model)?;
                Ok(None)
            }
            Err(error) => {
                tracing::warn!(
                    error = &error as &dyn std::error::Error,
                    model = model.file(),
                    "discarding unreadable analysis cache entry"
                );
                self.remove(key, model)?;
                Ok(None)
            }
        }
    }

    pub fn put(&self, key: &str, data: &AnalysisData) -> Result<(), AnalysisError> {
        if !valid(data) {
            return Err(AnalysisError::Model("cannot cache an invalid analysis timeline".to_owned()));
        }
        let model = data.model();
        let payload = match data {
            AnalysisData::Beats(value) => serde_json::to_vec(value.as_ref())?,
            AnalysisData::Stems(value) => serde_json::to_vec(value.as_ref())?,
        };
        self.connection.execute("INSERT INTO music_analysis(track,kind,revision,payload) VALUES(?1,?2,?3,?4) ON CONFLICT(track,kind) DO UPDATE SET revision=excluded.revision,payload=excluded.payload",params![key,model.file(),revision_of(model)?,payload])?;
        Ok(())
    }

    fn remove(&self, key: &str, model: ModelKind) -> Result<(), AnalysisError> {
        self.connection.execute("DELETE FROM music_analysis WHERE track=?1 AND kind=?2", params![key, model.file()])?;
        Ok(())
    }
}

fn revision_of(model: ModelKind) -> Result<String, AnalysisError> {
    Ok(format!("1:{}", model.specification()?.sha256))
}

fn sorted_finite(values: &[f64]) -> bool {
    values.iter().all(|value| value.is_finite() && *value >= 0.0) && values.windows(2).all(|pair| pair[0] <= pair[1])
}

fn valid(data: &AnalysisData) -> bool {
    match data {
        AnalysisData::Beats(events) => {
            events.iter().all(|event| {
                event.time.is_finite()
                    && event.time >= 0.0
                    && event.impact.is_none_or(|value| value.is_finite() && (0.0..=1.0).contains(&value))
            }) && events.windows(2).all(|pair| pair[0].time <= pair[1].time)
        }
        AnalysisData::Stems(value) => {
            let vocal = &value.vocal;
            let voice_valid = vocal.fps == 50
                && vocal.from == 0
                && vocal.top.is_finite()
                && vocal.db.iter().all(|value| value.is_finite())
                && vocal.onsets.iter().all(|frame| (*frame as usize) < vocal.db.len())
                && vocal.onsets.windows(2).all(|pair| pair[0] < pair[1]);
            voice_valid
                && value.drums.as_ref().is_none_or(|hits| {
                    hits.times.len() == hits.strengths.len()
                        && hits.times.len() == hits.kinds.len()
                        && sorted_finite(&hits.times)
                        && hits.strengths.iter().all(|value| value.is_finite() && (0.0..=1.0).contains(value))
                        && hits.kinds.iter().all(|kind| *kind <= 2)
                })
        }
    }
}
