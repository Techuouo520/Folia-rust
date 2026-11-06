// A style's settings as data: groups of sliders, switches, list members and choices.
// The workbench draws this and never names a style. Each style fills its own list.

use std::collections::BTreeMap;

/// One choice of a [`Item::Select`].
#[derive(Clone, Copy, Debug)]
pub struct Choice {
    pub value: &'static str,
    pub label_key: &'static str,
}

/// One row. Keys are the original tuning's field names and never change.
#[derive(Clone, Copy, Debug)]
pub enum Item {
    Slider {
        key: &'static str,
        label_key: &'static str,
        hint_key: Option<&'static str>,
        min: f32,
        max: f32,
        step: f32,
        default: f32,
    },
    Switch {
        key: &'static str,
        label_key: &'static str,
        hint_key: Option<&'static str>,
        default: bool,
    },
    /// `option` belongs to the string list stored at `key`. The last one left on cannot turn off.
    Member {
        key: &'static str,
        option: &'static str,
        label_key: &'static str,
        default_on: bool,
    },
    Select {
        key: &'static str,
        label_key: &'static str,
        hint_key: Option<&'static str>,
        options: &'static [Choice],
        default: &'static str,
    },
}

impl Item {
    pub fn key(self) -> &'static str {
        match self {
            Item::Slider { key, .. }
            | Item::Switch { key, .. }
            | Item::Member { key, .. }
            | Item::Select { key, .. } => key,
        }
    }
}

/// Rows of one card. `foot_key` is the note under the card.
#[derive(Clone, Copy, Debug)]
pub struct Group {
    pub id: &'static str,
    pub title_key: &'static str,
    pub foot_key: Option<&'static str>,
    pub items: &'static [Item],
}

/// What one style declares, and the original localStorage key it is saved under.
#[derive(Clone, Copy, Debug)]
pub struct Manifest {
    pub storage_key: &'static str,
    /// Read when `storage_key` holds nothing. Never written.
    pub older_keys: &'static [&'static str],
    pub groups: &'static [Group],
}

impl Manifest {
    pub fn items(self) -> impl Iterator<Item = Item> {
        self.groups.iter().flat_map(|group| group.items.iter().copied())
    }

    fn slider(self, key: &str) -> Option<(f32, f32, f32)> {
        self.items().find_map(|item| match item {
            Item::Slider { key: item_key, min, max, default, .. } if item_key == key => Some((min, max, default)),
            _ => None,
        })
    }
}

/// The saved numbers, switches, choices and lists of one style. Missing keys read as the manifest default.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Values {
    numbers: BTreeMap<String, f32>,
    flags: BTreeMap<String, bool>,
    choices: BTreeMap<String, String>,
    lists: BTreeMap<String, Vec<String>>,
}

impl Values {
    pub fn number(&self, manifest: Manifest, key: &str) -> f32 {
        let Some((min, max, default)) = manifest.slider(key) else {
            return 0.0;
        };
        self.numbers.get(key).copied().unwrap_or(default).clamp(min, max)
    }

    pub fn flag(&self, manifest: Manifest, key: &str) -> bool {
        let default = manifest.items().find_map(|item| match item {
            Item::Switch { key: item_key, default, .. } if item_key == key => Some(default),
            _ => None,
        });
        self.flags.get(key).copied().or(default).unwrap_or(false)
    }

    pub fn choice<'a>(&'a self, manifest: Manifest, key: &str) -> &'a str {
        if let Some(held) = self.choices.get(key) {
            return held.as_str();
        }
        manifest
            .items()
            .find_map(|item| match item {
                Item::Select { key: item_key, default, .. } if item_key == key => Some(default),
                _ => None,
            })
            .unwrap_or("")
    }

    pub fn member(&self, manifest: Manifest, key: &str, option: &str) -> bool {
        match self.lists.get(key) {
            Some(list) => list.iter().any(|item| item == option),
            None => manifest.items().any(|item| match item {
                Item::Member { key: item_key, option: item_option, default_on, .. } => {
                    item_key == key && item_option == option && default_on
                }
                _ => false,
            }),
        }
    }

    pub fn set_number(&mut self, manifest: Manifest, key: &str, value: f32) {
        if let Some((min, max, _)) = manifest.slider(key) {
            self.numbers.insert(key.to_owned(), value.clamp(min, max));
        }
    }

    pub fn set_flag(&mut self, manifest: Manifest, key: &str, value: bool) {
        if manifest.items().any(|item| matches!(item, Item::Switch { key: item_key, .. } if item_key == key)) {
            self.flags.insert(key.to_owned(), value);
        }
    }

    pub fn set_choice(&mut self, manifest: Manifest, key: &str, value: &str) {
        let known = manifest.items().any(|item| match item {
            Item::Select { key: item_key, options, .. } => {
                item_key == key && options.iter().any(|choice| choice.value == value)
            }
            _ => false,
        });
        if known {
            self.choices.insert(key.to_owned(), value.to_owned());
        }
    }

    /// Turns one list entry on or off. The last entry of that list stays on, and then this returns false.
    pub fn toggle_member(&mut self, manifest: Manifest, key: &str, option: &str, on: bool) -> bool {
        let options: Vec<&str> = manifest
            .items()
            .filter_map(|item| match item {
                Item::Member { key: item_key, option, .. } if item_key == key => Some(option),
                _ => None,
            })
            .collect();
        if !options.contains(&option) {
            return false;
        }
        let mut list: Vec<String> =
            options.iter().filter(|entry| self.member(manifest, key, entry)).map(|s| (*s).to_owned()).collect();
        if on {
            if !list.iter().any(|entry| entry == option) {
                list = options
                    .iter()
                    .filter(|entry| **entry == option || list.iter().any(|held| held == *entry))
                    .map(|s| (*s).to_owned())
                    .collect();
            }
        } else if list.len() <= 1 && list.first().is_some_and(|entry| entry == option) {
            return false;
        } else {
            list.retain(|entry| entry != option);
        }
        self.lists.insert(key.to_owned(), list);
        true
    }

    pub fn reset(&mut self, manifest: Manifest) {
        for item in manifest.items() {
            match item {
                Item::Slider { key, .. } => {
                    self.numbers.remove(key);
                }
                Item::Switch { key, .. } => {
                    self.flags.remove(key);
                }
                Item::Select { key, .. } => {
                    self.choices.remove(key);
                }
                Item::Member { key, .. } => {
                    self.lists.remove(key);
                }
            }
        }
    }

    /// Stable mix of the saved fields, so a picture cache can tell two tunings apart.
    pub fn fingerprint(&self) -> u64 {
        let mut hash = 0xcbf2_9ce4_8422_2325u64;
        let mix = |hash: u64, text: &str| {
            text.bytes().fold(hash, |hash, byte| hash.wrapping_mul(0x100000001b3).wrapping_add(u64::from(byte)))
        };
        for (key, value) in &self.numbers {
            hash = mix(hash, key).wrapping_add(value.to_bits() as u64);
        }
        for (key, value) in &self.flags {
            hash = mix(hash, key).wrapping_add(u64::from(*value));
        }
        for (key, value) in &self.choices {
            hash = mix(hash, key);
            hash = mix(hash, value);
        }
        for (key, list) in &self.lists {
            hash = mix(hash, key);
            for entry in list {
                hash = mix(hash, entry);
            }
        }
        hash
    }

    pub fn from_table(manifest: Manifest, table: Option<&toml::map::Map<String, toml::Value>>) -> Self {
        let Some(table) = table else {
            return Self::default();
        };
        let mut values = Self::default();
        for item in manifest.items() {
            match item {
                Item::Slider { key, min, max, .. } => {
                    if let Some(number) = table.get(key).and_then(number_of) {
                        values.numbers.insert(key.to_owned(), number.clamp(min, max));
                    }
                }
                Item::Switch { key, .. } => {
                    if let Some(toml::Value::Boolean(flag)) = table.get(key) {
                        values.flags.insert(key.to_owned(), *flag);
                    }
                }
                Item::Select { key, options, .. } => {
                    if let Some(toml::Value::String(text)) = table.get(key)
                        && options.iter().any(|choice| choice.value == text)
                    {
                        values.choices.insert(key.to_owned(), text.clone());
                    }
                }
                Item::Member { key, .. } => {
                    if values.lists.contains_key(key) {
                        continue;
                    }
                    if let Some(toml::Value::Array(entries)) = table.get(key) {
                        let allowed: Vec<&str> = manifest
                            .items()
                            .filter_map(|item| match item {
                                Item::Member { key: item_key, option, .. } if item_key == key => Some(option),
                                _ => None,
                            })
                            .collect();
                        let list = allowed
                            .iter()
                            .filter(|option| entries.iter().any(|entry| entry.as_str() == Some(option)))
                            .map(|option| (*option).to_owned())
                            .collect::<Vec<_>>();
                        if !list.is_empty() {
                            values.lists.insert(key.to_owned(), list);
                        }
                    }
                }
            }
        }
        values
    }

    /// The whole tuning, with defaults filled in, under the original field names.
    pub fn to_table(&self, manifest: Manifest) -> toml::map::Map<String, toml::Value> {
        let mut table = toml::map::Map::new();
        let mut lists: BTreeMap<&str, Vec<String>> = BTreeMap::new();
        for item in manifest.items() {
            match item {
                Item::Slider { key, .. } => {
                    table.insert(key.to_owned(), toml::Value::Float(f64::from(self.number(manifest, key))));
                }
                Item::Switch { key, .. } => {
                    table.insert(key.to_owned(), toml::Value::Boolean(self.flag(manifest, key)));
                }
                Item::Select { key, .. } => {
                    table.insert(key.to_owned(), toml::Value::String(self.choice(manifest, key).to_owned()));
                }
                Item::Member { key, option, .. } => {
                    if self.member(manifest, key, option) {
                        lists.entry(key).or_default().push(option.to_owned());
                    }
                }
            }
        }
        for (key, list) in lists {
            let entries = list.into_iter().map(toml::Value::String).collect();
            table.insert(key.to_owned(), toml::Value::Array(entries));
        }
        table
    }
}

fn number_of(value: &toml::Value) -> Option<f32> {
    match value {
        toml::Value::Float(number) => Some(*number as f32),
        toml::Value::Integer(number) => Some(*number as f32),
        toml::Value::String(text) => text.parse().ok(),
        _ => None,
    }
}
