// Per-style lyric tuning. Each table name is the original localStorage key (`classic_tuning`, ...).

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::Domain;

/// One file of every style's tuning. The style names live in the table keys, not in this type.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct VisualizerSettings {
    pub tables: BTreeMap<String, toml::Value>,
}

impl Serialize for VisualizerSettings {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.tables.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for VisualizerSettings {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let mut tables = BTreeMap::<String, toml::Value>::deserialize(deserializer)?;
        tables.remove("schema_version");
        Ok(Self { tables })
    }
}

impl VisualizerSettings {
    /// Writes one style's table and drops its older keys.
    pub fn insert_style(&mut self, key: &str, table: toml::map::Map<String, toml::Value>, older: &[&str]) {
        self.tables.insert(key.to_owned(), toml::Value::Table(table));
        for key in older {
            self.tables.remove(*key);
        }
    }
}

impl Domain for VisualizerSettings {
    const FILE: &'static str = "visualizer.toml";
    const SCHEMA_VERSION: u32 = 1;
}
