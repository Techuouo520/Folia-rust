// A style's settings as data: groups of sliders, switches, list members and choices.
// The workbench draws this and never names a style. Each style fills its own list.

use std::collections::BTreeMap;

use folia_core::Theme;
use iced::Rectangle;
use iced::widget::canvas::Frame;

/// One choice of a [`Item::Select`].
#[derive(Clone, Copy, Debug)]
pub struct Choice {
    pub value: &'static str,
    pub label_key: &'static str,
    /// The choice stays unselectable until this saved path is non-empty.
    pub requires: Option<&'static str>,
}

/// An option drawn as a small picture instead of a word, as the original `PictureGrid` choices are.
#[derive(Clone, Copy, Debug)]
pub struct PictureChoice {
    pub value: &'static str,
    pub label_key: &'static str,
    /// The choice stays unselectable until this saved path is non-empty.
    pub requires: Option<&'static str>,
    /// Paints the 16:10 picture inside `rect`; the style owns its look.
    pub picture: fn(&mut Frame, Rectangle, &Theme),
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
    /// Like [`Item::Select`], drawn as pictures.
    Picture {
        key: &'static str,
        label_key: &'static str,
        hint_key: Option<&'static str>,
        options: &'static [PictureChoice],
        default: &'static str,
    },
    /// A button. `path_key` shows that saved path's file name as the hint.
    Button {
        key: &'static str,
        label_key: &'static str,
        path_key: Option<&'static str>,
    },
}

impl Item {
    pub fn key(self) -> &'static str {
        match self {
            Item::Slider { key, .. }
            | Item::Switch { key, .. }
            | Item::Member { key, .. }
            | Item::Select { key, .. }
            | Item::Picture { key, .. }
            | Item::Button { key, .. } => key,
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
    /// Shown, but not editable: the effect needs a module that is not ported.
    pub locked: &'static [&'static str],
    /// Hide `item` while the gate flag is on, or its choice is `"true"`.
    pub hidden_when: &'static [(&'static str, &'static str)],
    /// Hide `item` while the gate flag is off.
    pub hidden_unless: &'static [(&'static str, &'static str)],
    /// Stored and read, never drawn. Monet's portrait offset is set by dragging.
    pub concealed: &'static [&'static str],
    /// Free strings kept in the table beside the rows, such as an uploaded path.
    pub paths: &'static [&'static str],
    /// Original reset toast, `notifications.*Reset`.
    pub reset_key: Option<&'static str>,
}

impl Manifest {
    pub const fn plain(
        storage_key: &'static str,
        older_keys: &'static [&'static str],
        groups: &'static [Group],
    ) -> Self {
        Self {
            storage_key,
            older_keys,
            groups,
            locked: &[],
            hidden_when: &[],
            hidden_unless: &[],
            concealed: &[],
            paths: &[],
            reset_key: None,
        }
    }

    pub fn locked(self, key: &str) -> bool {
        self.locked.contains(&key)
    }

    pub fn hidden(self, values: &Values, key: &str) -> bool {
        self.concealed.contains(&key)
            || self
                .hidden_when
                .iter()
                .any(|(item, gate)| *item == key && (values.flag(self, gate) || values.choice(self, gate) == "true"))
            || self.hidden_unless.iter().any(|(item, gate)| *item == key && !values.flag(self, gate))
    }
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
    paths: BTreeMap<String, String>,
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
                Item::Select { key: item_key, default, .. } | Item::Picture { key: item_key, default, .. }
                    if item_key == key =>
                {
                    Some(default)
                }
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
            Item::Picture { key: item_key, options, .. } => {
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

    /// Makes exactly `options` the list at `key` (`setMembers`); an empty list is refused.
    pub fn set_members(&mut self, manifest: Manifest, key: &str, options: &[&str]) -> bool {
        let allowed: Vec<&str> = manifest
            .items()
            .filter_map(|item| match item {
                Item::Member { key: item_key, option, .. } if item_key == key => Some(option),
                _ => None,
            })
            .collect();
        let list: Vec<String> =
            allowed.iter().filter(|option| options.contains(*option)).map(|option| (*option).to_owned()).collect();
        if list.is_empty() {
            return false;
        }
        self.lists.insert(key.to_owned(), list);
        true
    }

    pub fn path<'a>(&'a self, key: &str) -> Option<&'a str> {
        self.paths.get(key).map(String::as_str).filter(|text| !text.is_empty())
    }

    pub fn set_path(&mut self, manifest: Manifest, key: &str, value: Option<&str>) {
        if !manifest.paths.contains(&key) {
            return;
        }
        match value.map(str::trim).filter(|text| !text.is_empty()) {
            Some(text) => {
                self.paths.insert(key.to_owned(), text.to_owned());
            }
            None => {
                self.paths.remove(key);
            }
        }
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
                Item::Picture { key, .. } => {
                    self.choices.remove(key);
                }
                Item::Member { key, .. } => {
                    self.lists.remove(key);
                }
                Item::Button { .. } => {}
            }
        }
        for key in manifest.paths {
            self.paths.remove(*key);
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
        for (key, value) in &self.paths {
            hash = mix(hash, key);
            hash = mix(hash, value);
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
                    let text = match table.get(key) {
                        Some(toml::Value::String(text)) => Some(text.clone()),
                        Some(toml::Value::Boolean(flag)) => Some(if *flag { "true" } else { "false" }.to_owned()),
                        _ => None,
                    };
                    if let Some(text) = text
                        && options.iter().any(|choice| choice.value == text)
                    {
                        values.choices.insert(key.to_owned(), text);
                    }
                }
                Item::Picture { key, options, .. } => {
                    let text = match table.get(key) {
                        Some(toml::Value::String(text)) => Some(text.clone()),
                        Some(toml::Value::Boolean(flag)) => Some(if *flag { "true" } else { "false" }.to_owned()),
                        _ => None,
                    };
                    if let Some(text) = text
                        && options.iter().any(|choice| choice.value == text)
                    {
                        values.choices.insert(key.to_owned(), text);
                    }
                }
                Item::Button { .. } => {}
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
        for key in manifest.paths {
            if let Some(toml::Value::String(text)) = table.get(*key)
                && !text.is_empty()
            {
                values.paths.insert((*key).to_owned(), text.clone());
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
                Item::Select { key, options, .. } => {
                    let value = self.choice(manifest, key);
                    let boolean = options.iter().any(|choice| choice.value == "true")
                        && options.iter().any(|choice| choice.value == "false");
                    let stored = if boolean {
                        toml::Value::Boolean(value == "true")
                    } else {
                        toml::Value::String(value.to_owned())
                    };
                    table.insert(key.to_owned(), stored);
                }
                Item::Picture { key, .. } => {
                    table.insert(key.to_owned(), toml::Value::String(self.choice(manifest, key).to_owned()));
                }
                Item::Button { .. } => {}
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
        for key in manifest.paths {
            if let Some(path) = self.path(key) {
                table.insert((*key).to_owned(), toml::Value::String(path.to_owned()));
            }
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
