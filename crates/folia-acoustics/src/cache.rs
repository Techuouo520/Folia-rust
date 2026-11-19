// Bounded SQLite cache for room responses keyed by every geometry and listening parameter.

use crate::Error;
use folia_core::{RoomImpulseResponse, RoomOutput, RoomRequest};
use ring::digest::{Context, SHA256};
use rusqlite::{Connection, OptionalExtension, params};
use std::path::Path;

pub(crate) fn key(request: &RoomRequest) -> String {
    let mut hash = Context::new(&SHA256);
    hash.update(b"folia-room-4.8.1-rays32768-bounces128-order3-seconds2-bass200-v1");
    hash.update(&request.sample_rate.to_le_bytes());
    hash.update(&[u8::from(request.output == RoomOutput::Headphones)]);
    for vector in
        request.speakers.into_iter().chain([request.listener.position, request.listener.forward, request.listener.up])
    {
        for value in vector {
            hash.update(&value.to_le_bytes());
        }
    }
    hash.update(&(request.world.materials.len() as u64).to_le_bytes());
    for material in &request.world.materials {
        for value in material.absorption.into_iter().chain(material.transmission).chain([material.scattering]) {
            hash.update(&value.to_le_bytes());
        }
    }
    hash.update(&(request.world.meshes.len() as u64).to_le_bytes());
    for mesh in &request.world.meshes {
        for length in [mesh.positions.len(), mesh.triangles.len(), mesh.triangle_materials.len()] {
            hash.update(&(length as u64).to_le_bytes());
        }
        for value in mesh.positions.iter().flatten() {
            hash.update(&value.to_le_bytes());
        }
        for value in mesh.triangles.iter().flatten() {
            hash.update(&value.to_le_bytes());
        }
        for value in &mesh.triangle_materials {
            hash.update(&value.to_le_bytes());
        }
    }
    hash.finish().as_ref().iter().map(|byte| format!("{byte:02x}")).collect()
}

pub(crate) struct Cache {
    connection: Connection,
    _lease: folia_store::CacheLease,
}

impl Cache {
    pub fn open(path: &Path) -> Result<Self, Error> {
        let root = path.parent().ok_or(Error::Invalid("room cache has no directory"))?;
        let lease = folia_store::CacheLease::shared(root)?;
        if let Some(parent) = path.parent().filter(|path| !path.as_os_str().is_empty()) {
            std::fs::create_dir_all(parent)?;
        }
        let connection = Connection::open(path)?;
        connection.busy_timeout(std::time::Duration::from_secs(5))?;
        connection.pragma_update(None, "journal_mode", "WAL")?;
        let version: u32 = connection.pragma_query_value(None, "user_version", |row| row.get(0))?;
        if version > 1 {
            return Err(Error::Invalid("unknown room cache schema"));
        }
        connection.execute_batch("CREATE TABLE IF NOT EXISTS responses (key TEXT PRIMARY KEY, rate INTEGER NOT NULL, frames INTEGER NOT NULL, samples BLOB NOT NULL, touched INTEGER NOT NULL); PRAGMA user_version = 1;")?;
        Ok(Self { connection, _lease: lease })
    }

    pub fn get(&self, key: &str) -> Result<Option<RoomImpulseResponse>, Error> {
        let value: Option<(u32, u32, Vec<u8>)> = self
            .connection
            .query_row("SELECT rate, frames, samples FROM responses WHERE key=?1", [key], |row| {
                Ok((row.get(0)?, row.get(1)?, row.get(2)?))
            })
            .optional()?;
        let Some((rate, frames, bytes)) = value else {
            return Ok(None);
        };
        let frames = frames as usize;
        if !(8000..=192000).contains(&rate) || frames != rate as usize * 2 || bytes.len() != frames * 16 {
            return Err(Error::Invalid("corrupt room cache entry"));
        }
        let channels: [Vec<f32>; 4] = std::array::from_fn(|channel| {
            bytes[channel * frames * 4..(channel + 1) * frames * 4]
                .as_chunks::<4>()
                .0
                .iter()
                .map(|value| f32::from_le_bytes(*value))
                .collect()
        });
        if channels.iter().flatten().any(|sample| !sample.is_finite()) {
            return Err(Error::Invalid("non-finite room cache entry"));
        }
        self.connection.execute("UPDATE responses SET touched=unixepoch() WHERE key=?1", [key])?;
        Ok(Some(RoomImpulseResponse { sample_rate: rate, channels, gain: 1.0 }))
    }

    pub fn put(&self, key: &str, impulse: &RoomImpulseResponse) -> Result<(), Error> {
        let bytes: Vec<u8> = impulse.channels.iter().flatten().flat_map(|value| value.to_le_bytes()).collect();
        self.connection.execute("INSERT INTO responses(key,rate,frames,samples,touched) VALUES(?1,?2,?3,?4,unixepoch()) ON CONFLICT(key) DO UPDATE SET rate=excluded.rate,frames=excluded.frames,samples=excluded.samples,touched=excluded.touched",params![key,impulse.sample_rate,impulse.channels[0].len() as u32,bytes])?;
        self.connection.execute("DELETE FROM responses WHERE key NOT IN (SELECT key FROM responses ORDER BY touched DESC,rowid DESC LIMIT 32)",[])?;
        Ok(())
    }
}
