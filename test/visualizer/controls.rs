// Each ported style's manifest against the original control list in fixtures/controls.txt.

use std::collections::BTreeMap;

use folia_core::AnimationIntensity;
use folia_store::{Store, VisualizerSettings};
use folia_visualizer::styles::classic::{ClassicTuning, word_configs};
use folia_visualizer::{Item, Style, Values, apply_quick, quick_controls};

#[test]
fn manifests_match_the_original_control_list() {
    let text = include_str!("fixtures/controls.txt");
    for spec in parse(text) {
        let style = Style::from_id(&spec.id).unwrap_or_else(|| panic!("unknown style {}", spec.id));
        let manifest = style.manifest();
        assert_eq!(manifest.storage_key, spec.storage, "{}", spec.id);
        assert_eq!(manifest.reset_key, spec.reset.as_deref(), "{}", spec.id);
        let older: Vec<&str> = spec.older.iter().map(String::as_str).collect();
        let locked: Vec<&str> = spec.locked.iter().map(String::as_str).collect();
        let paths: Vec<&str> = spec.paths.iter().map(String::as_str).collect();
        let concealed: Vec<&str> = spec.concealed.iter().map(String::as_str).collect();
        let hidden_when: Vec<(&str, &str)> =
            spec.hidden_when.iter().map(|(item, gate)| (item.as_str(), gate.as_str())).collect();
        let hidden_unless: Vec<(&str, &str)> =
            spec.hidden_unless.iter().map(|(item, gate)| (item.as_str(), gate.as_str())).collect();
        assert_eq!(manifest.older_keys, older.as_slice(), "{}", spec.id);
        assert_eq!(manifest.locked, locked.as_slice(), "{}", spec.id);
        assert_eq!(manifest.paths, paths.as_slice(), "{}", spec.id);
        assert_eq!(manifest.concealed, concealed.as_slice(), "{}", spec.id);
        assert_eq!(manifest.hidden_when, hidden_when.as_slice(), "{} hidden when", spec.id);
        assert_eq!(manifest.hidden_unless, hidden_unless.as_slice(), "{} hidden unless", spec.id);
        let items: Vec<Item> = manifest.items().filter(|item| !matches!(item, Item::Button { .. })).collect();
        let rows: Vec<&Row> = spec.rows.iter().filter(|row| row.kind != "button").collect();
        assert_eq!(items.len(), rows.len(), "{} item count", spec.id);
        for (item, row) in items.iter().zip(rows) {
            assert_eq!(item.key(), row.key.as_str(), "{}", spec.id);
            match (item, row.kind.as_str()) {
                (Item::Text { max_length, .. }, "text") => assert_eq!(*max_length, row.number as usize),
                (Item::Switch { default, .. }, "switch") => assert_eq!(*default, row.flag, "{} {}", spec.id, row.key),
                (Item::Slider { min, max, step, default, .. }, "slider") => {
                    assert_eq!(
                        (*min, *max, *step, *default),
                        (row.min, row.max, row.step, row.number),
                        "{} {}",
                        spec.id,
                        row.key
                    );
                }
                (Item::Select { default, options, .. }, "select") => {
                    assert_eq!(*default, row.text.as_str(), "{} {}", spec.id, row.key);
                    let values: Vec<&str> = options.iter().map(|choice| choice.value).collect();
                    let expected: Vec<&str> = row.options.iter().map(String::as_str).collect();
                    assert_eq!(values, expected, "{} {}", spec.id, row.key);
                }
                (Item::Picture { default, options, .. }, "picture") => {
                    assert_eq!(*default, row.text.as_str(), "{} {}", spec.id, row.key);
                    let values: Vec<&str> = options.iter().map(|choice| choice.value).collect();
                    let expected: Vec<&str> = row.options.iter().map(String::as_str).collect();
                    assert_eq!(values, expected, "{} {}", spec.id, row.key);
                }
                (Item::Member { option, default_on, .. }, "member") => {
                    assert_eq!((*option, *default_on), (row.option.as_str(), row.flag), "{} {}", spec.id, row.key);
                }
                other => panic!("{} {} unexpected {other:?}", spec.id, row.key),
            }
        }
        let buttons: Vec<&str> = manifest
            .items()
            .filter_map(|item| match item {
                Item::Button { key, .. } => Some(key),
                _ => None,
            })
            .collect();
        let wanted: Vec<&str> =
            spec.rows.iter().filter(|row| row.kind == "button").map(|row| row.key.as_str()).collect();
        assert_eq!(buttons, wanted, "{}", spec.id);
    }
}

#[test]
fn classic_legacy_layout_uses_a_fixed_gap() {
    let modern = ClassicTuning::default();
    let legacy = ClassicTuning { legacy: true, ..modern };
    let modern_gap = word_configs(3.0, AnimationIntensity::Normal, false, &[40.0, 40.0], 60.0, modern);
    let legacy_gap = word_configs(3.0, AnimationIntensity::Normal, false, &[40.0, 40.0], 60.0, legacy);
    assert!((legacy_gap[0].margin_right - 12.8).abs() < 1.0e-6);
    assert!((modern_gap[0].margin_right - legacy_gap[0].margin_right).abs() > 1.0);
    let protected = ClassicTuning { avoid_overlap: true, word_spacing: 0.0, ..modern };
    let widths = [180.0, 30.0, 240.0, 90.0];
    for seed in [0.0, 1.5, 3.25, 10.0] {
        let words = word_configs(seed, AnimationIntensity::Chaotic, false, &widths, 60.0, protected);
        for index in 0..words.len() - 1 {
            let pair = folia_visualizer::styles::glow::word_pair_gap(
                [f64::from(widths[index]), f64::from(widths[index + 1])],
                [&words[index], &words[index + 1]],
                60.0,
            );
            assert!(words[index].margin_right >= pair + 3.0, "word spacing zero closed a protected gap");
        }
    }
    assert_eq!(
        legacy_gap,
        word_configs(
            3.0,
            AnimationIntensity::Normal,
            false,
            &[40.0, 40.0],
            60.0,
            ClassicTuning { avoid_overlap: true, ..legacy }
        ),
    );
}

#[test]
fn glow_overlap_bounds_match_the_original_turn_samples() {
    use folia_visualizer::styles::glow::{WordConfig, word_reach};
    for (width, font, scale, rotate, passed, expected) in [
        (120.0, 72.0, 1.2, 5.0, 22.5, [105.68760488902632, 71.56476814622162]),
        (36.0, 48.0, 0.85, -30.0, 20.0, [32.83026414906268, 35.44368553208356]),
        (320.0, 64.0, 1.65, 3.0, -9.0, [372.9621499307688, 93.16206463645011]),
        (40.0, 16.0, 0.9, 0.0, 0.0, [25.2, 10.08]),
    ] {
        let config = WordConfig { scale, rotate, passed_rotate: passed, x: 0.0, y: 0.0, margin_right: 0.0 };
        let actual = word_reach(width, font, &config);
        assert!((actual[0] - expected[0]).abs() < 1e-10 && (actual[1] - expected[1]).abs() < 1e-10);
    }
}

#[test]
fn monet_visibility_migrates_switches_and_limits_description() {
    let manifest = Style::Monet.manifest();
    let table: toml::Table = toml::from_str("showDescription = false\nshowPortraitDragHanger = true\n").unwrap();
    let mut values = Values::from_table(manifest, Some(&table));
    assert_eq!(values.choice(manifest, "descriptionVisibility"), "hidden");
    assert_eq!(values.choice(manifest, "portraitHangerVisibility"), "auto");
    assert!(manifest.hidden(&values, "customDescription"));
    values.set_choice(manifest, "descriptionVisibility", "always");
    assert!(values.flag(manifest, "showDescription"));
    values.set_text(manifest, "customDescription", &"🎵".repeat(100));
    assert_eq!(values.text("customDescription").encode_utf16().count(), 120);
    values.set_choice(manifest, "portraitSource", "none");
    assert!(manifest.hidden(&values, "portraitStyle"));
    assert!(manifest.hidden(&values, "portraitHangerVisibility"));
    let restored = Values::from_table(manifest, Some(&values.to_table(manifest)));
    assert_eq!(restored.to_table(manifest), values.to_table(manifest));
    values.reset(manifest);
    assert_eq!(values.choice(manifest, "descriptionVisibility"), "auto");
    assert_eq!(values.text("customDescription"), "");
    assert_eq!(values.choice(manifest, "portraitSource"), "cover");
}

#[test]
fn tuning_round_trip_reads_a_boolean_legacy_flag_and_a_portrait_path() {
    let manifest = Style::Classic.manifest();
    let mut table = toml::map::Map::new();
    table.insert("useLegacyLayout".into(), toml::Value::Boolean(true));
    let values = Values::from_table(manifest, Some(&table));
    assert_eq!(values.choice(manifest, "useLegacyLayout"), "true");
    let stored = values.to_table(manifest);
    assert_eq!(stored.get("useLegacyLayout"), Some(&toml::Value::Boolean(true)));

    let monet = Style::Monet.manifest();
    let mut values = Values::default();
    values.set_path(monet, "portraitPath", Some("portraits/monet.png"));
    values.set_number(monet, "fontScale", 1.0);
    let dir = tempfile::tempdir().unwrap();
    let store = Store::at(dir.path());
    let mut settings = VisualizerSettings::default();
    settings.insert_style(monet.storage_key, values.to_table(monet), monet.older_keys);
    store.save(&settings).unwrap();
    let loaded = store.load::<VisualizerSettings>().unwrap();
    let again = Style::Monet.load(&loaded.tables);
    assert_eq!(again.path("portraitPath"), Some("portraits/monet.png"));
    assert!((again.number(monet, "fontScale") - 1.0).abs() < 1.0e-6);

    let mut old = BTreeMap::new();
    old.insert("cadenze_tuning".into(), toml::Value::Table(toml::map::Map::new()));
    let from_old = Style::Cadenza.load(&old);
    assert!(from_old.to_table(Style::Cadenza.manifest()).is_empty());
}

#[test]
fn picture_choices_round_trip_through_the_style_table() {
    let monet = Style::Monet.manifest();
    let mut values = Values::default();
    assert_eq!(values.choice(monet, "portraitStyle"), "square");
    assert_eq!(values.choice(monet, "audioStyle"), "bar");
    values.set_choice(monet, "portraitStyle", "rectangular");
    values.set_choice(monet, "audioStyle", "line");
    let table = values.to_table(monet);
    assert_eq!(table.get("portraitStyle"), Some(&toml::Value::String("rectangular".into())));
    let again = Values::from_table(monet, Some(&table));
    assert_eq!(again.choice(monet, "portraitStyle"), "rectangular");
    assert_eq!(again.choice(monet, "audioStyle"), "line");
    // An unknown value is not stored, and a stored bad value reads as the default.
    let mut bad = toml::map::Map::new();
    bad.insert("portraitStyle".into(), toml::Value::String("round".into()));
    let values = Values::from_table(monet, Some(&bad));
    assert_eq!(values.choice(monet, "portraitStyle"), "square");

    let tilt = Style::Tilt.manifest();
    let mut values = Values::default();
    values.set_choice(tilt, "colorScheme", "accentAll");
    let table = values.to_table(tilt);
    assert_eq!(Values::from_table(tilt, Some(&table)).choice(tilt, "colorScheme"), "accentAll");
}

#[test]
fn a_background_mode_resolves_only_when_this_build_can_draw_it() {
    use folia_visualizer::Background;
    assert_eq!(Background::resolve(None), Background::Latent);
    assert_eq!(Background::resolve(Some("common")), Background::Common);
    assert_eq!(Background::resolve(Some("latent")), Background::Latent);
    assert_eq!(Background::resolve(Some("nonsense")), Background::Latent);
    assert_eq!(Background::ALL.map(Background::id), ["common", "monet", "nomand", "latent", "sora"]);
    for mode in Background::ALL {
        assert_eq!(Background::resolve(Some(mode.id())), mode);
    }
    assert_eq!(Background::Common.id(), "common");
    assert_eq!(Background::Common.label_key(), "options.visualizerBackgroundModeCommon");
}

#[test]
fn hidden_rules_follow_the_manifest() {
    let classic = Style::Classic.manifest();
    let mut values = Values::default();
    assert!(!classic.hidden(&values, "wordSpacing"));
    values.set_choice(classic, "useLegacyLayout", "true");
    assert!(classic.hidden(&values, "wordSpacing"));

    let monet = Style::Monet.manifest();
    let mut values = Values::default();
    assert!(!monet.hidden(&values, "audioStyle"));
    values.set_flag(monet, "showAudioVisualization", false);
    assert!(monet.hidden(&values, "audioStyle"));
}

struct Spec {
    id: String,
    storage: String,
    reset: Option<String>,
    older: Vec<String>,
    locked: Vec<String>,
    paths: Vec<String>,
    concealed: Vec<String>,
    hidden_when: Vec<(String, String)>,
    hidden_unless: Vec<(String, String)>,
    rows: Vec<Row>,
}

struct Row {
    kind: String,
    key: String,
    flag: bool,
    number: f32,
    min: f32,
    max: f32,
    step: f32,
    text: String,
    option: String,
    options: Vec<String>,
}

fn parse(text: &str) -> Vec<Spec> {
    let mut specs = Vec::new();
    let mut current: Option<Spec> = None;
    for raw in text.lines() {
        let line = raw.trim();
        if line.is_empty() {
            continue;
        }
        if let Some(name) = line.strip_prefix('[').and_then(|rest| rest.strip_suffix(']')) {
            if let Some(spec) = current.take() {
                specs.push(spec);
            }
            current = Some(Spec {
                id: name.to_owned(),
                storage: String::new(),
                reset: None,
                older: Vec::new(),
                locked: Vec::new(),
                paths: Vec::new(),
                concealed: Vec::new(),
                hidden_when: Vec::new(),
                hidden_unless: Vec::new(),
                rows: Vec::new(),
            });
            continue;
        }
        let spec = current.as_mut().unwrap();
        let mut parts = line.split_whitespace();
        let head = parts.next().unwrap();
        match head {
            "storage" => spec.storage = parts.next().unwrap().to_owned(),
            "reset" => spec.reset = Some(parts.next().unwrap().to_owned()),
            "older" => spec.older = parts.map(str::to_owned).collect(),
            "locked" => spec.locked = parts.map(str::to_owned).collect(),
            "path" => spec.paths = parts.map(str::to_owned).collect(),
            "concealed" => spec.concealed = parts.map(str::to_owned).collect(),
            "hiddenWhen" => {
                let item = parts.next().unwrap().to_owned();
                let gate = parts.next().unwrap().to_owned();
                spec.hidden_when.push((item, gate));
            }
            "hiddenUnless" => {
                let item = parts.next().unwrap().to_owned();
                let gate = parts.next().unwrap().to_owned();
                spec.hidden_unless.push((item, gate));
            }
            "switch" => spec.rows.push(row(head, parts)),
            "slider" => spec.rows.push(row(head, parts)),
            "select" => spec.rows.push(row(head, parts)),
            "picture" => spec.rows.push(row(head, parts)),
            "member" => spec.rows.push(row(head, parts)),
            "button" => spec.rows.push(row(head, parts)),
            "text" => spec.rows.push(row(head, parts)),
            other => panic!("bad fixture line {other}"),
        }
    }
    if let Some(spec) = current {
        specs.push(spec);
    }
    specs
}

fn row<'a>(kind: &str, mut parts: impl Iterator<Item = &'a str>) -> Row {
    let key = parts.next().unwrap().to_owned();
    let mut row = Row {
        kind: kind.to_owned(),
        key,
        flag: false,
        number: 0.0,
        min: 0.0,
        max: 0.0,
        step: 0.0,
        text: String::new(),
        option: String::new(),
        options: Vec::new(),
    };
    match kind {
        "text" => row.number = parts.next().unwrap().parse().unwrap(),
        "switch" => row.flag = parts.next().unwrap() == "true",
        "slider" => {
            row.min = parts.next().unwrap().parse().unwrap();
            row.max = parts.next().unwrap().parse().unwrap();
            row.step = parts.next().unwrap().parse().unwrap();
            row.number = parts.next().unwrap().parse().unwrap();
        }
        "select" => {
            row.text = parts.next().unwrap().to_owned();
            row.options = parts.next().unwrap().split(',').map(str::to_owned).collect();
        }
        "picture" => {
            row.text = parts.next().unwrap().to_owned();
            row.options = parts.next().unwrap().split(',').map(str::to_owned).collect();
        }
        "member" => {
            row.option = parts.next().unwrap().to_owned();
            row.flag = parts.next().unwrap() == "true";
        }
        "button" => {}
        _ => {}
    }
    row
}

#[test]
fn nebbia_quick_labels_follow_the_presets() {
    let labels = |values: &Values| -> Vec<&'static str> {
        quick_controls(Style::Nebbia, values).iter().map(|control| control.label_key).collect()
    };
    let mut values = Values::default();
    assert_eq!(labels(&values), ["options.nebbiaAppearAuto", "options.nebbiaFadeFog"]);
    assert!(apply_quick(Style::Nebbia, &mut values, "appear"));
    assert!(apply_quick(Style::Nebbia, &mut values, "fade"));
    assert_eq!(labels(&values), ["options.nebbiaAppearWrite", "options.nebbiaFadeFingers"]);
}
