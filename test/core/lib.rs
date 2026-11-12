// Checks locale-tag mapping, language preference storage and play queue navigation.

#![cfg(test)]

use folia_core::{Language, LanguagePreference, Origin, Queue, Repeat};

#[test]
fn keyword_colors_match_phrases_and_whole_latin_words() {
    use folia_core::{WordColor, word_color_ranges};
    let colors = vec![
        WordColor { word: "花火".into(), color: "#ff6600".into() },
        WordColor { word: "花".into(), color: "#00ff00".into() },
        WordColor { word: "Love sky".into(), color: "rgb(20, 60, 200)".into() },
    ];
    let text = "火车旁的花火和花 LOVE lovely Sky";
    let ranges = word_color_ranges(text, &colors);
    assert_eq!(
        ranges.iter().map(|entry| &text[entry.range.clone()]).collect::<Vec<_>>(),
        ["花火", "花", "LOVE", "Sky"]
    );
    assert_eq!(ranges[0].color, folia_core::Rgb::hex(0xff6600));
    assert_eq!(ranges[1].color, folia_core::Rgb::hex(0x00ff00));
    assert_eq!(ranges[2].color, folia_core::Rgb { r: 20, g: 60, b: 200 });
}

#[test]
fn maps_os_locale_tags() {
    let cases = [
        ("zh-CN", Language::ZhCn),
        ("zh", Language::ZhCn),
        ("zh-Hans-SG", Language::ZhCn),
        ("zh-TW", Language::ZhTw),
        ("zh_HK", Language::ZhTw),
        ("zh-Hant", Language::ZhTw),
        ("id-ID", Language::In),
        ("in", Language::In),
        ("vi-VN", Language::Vi),
        ("en-US", Language::En),
        ("fr-FR", Language::En),
    ];
    for (tag, expected) in cases {
        assert_eq!(Language::from_locale(tag), expected, "{tag}");
    }
}

#[test]
fn resolves_preference() {
    assert_eq!(LanguagePreference::System.resolve(Some("vi-VN")), Language::Vi);
    assert_eq!(LanguagePreference::System.resolve(None), Language::En);
    assert_eq!(LanguagePreference::Fixed(Language::ZhTw).resolve(Some("en-US")), Language::ZhTw);
}

#[derive(serde::Serialize, serde::Deserialize, PartialEq, Debug)]
struct Wrapper {
    language: LanguagePreference,
}

#[test]
fn preference_round_trips_as_plain_codes() {
    for (preference, text) in [
        (LanguagePreference::System, "language = \"system\"\n"),
        (LanguagePreference::Fixed(Language::ZhCn), "language = \"zh-CN\"\n"),
        (LanguagePreference::Fixed(Language::In), "language = \"in\"\n"),
    ] {
        let wrapper = Wrapper { language: preference };
        assert_eq!(toml::to_string(&wrapper).unwrap(), text);
        assert_eq!(toml::from_str::<Wrapper>(text).unwrap(), wrapper);
    }
}

#[test]
fn queue_advances_and_repeats() {
    let mut queue = Queue::default();
    queue.replace(vec!['a', 'b', 'c'], 1);
    assert_eq!(queue.current(), Some(&'b'));
    assert_eq!(queue.advance(true), Some(&'c'));
    assert_eq!(queue.advance(true), None);
    assert_eq!(queue.current(), Some(&'c'), "stays on the last item");

    queue.repeat = Repeat::All;
    assert_eq!(queue.advance(false), Some(&'a'));
    assert_eq!(queue.back(), Some(&'c'));

    queue.repeat = Repeat::One;
    assert_eq!(queue.advance(true), Some(&'c'));
    assert_eq!(queue.advance(false), None, "skipping ignores repeat-one");

    queue.repeat = Repeat::Off;
    queue.replace(vec!['x'], 0);
    assert_eq!(queue.back(), None);
    queue.replace(Vec::new(), 0);
    assert_eq!(queue.current(), None);
}

#[test]
fn queue_origin_lasts_until_the_queue_is_replaced() {
    let mut queue = Queue::default();
    queue.replace_from(vec!["a", "b"], 0, Origin { name: "全部歌曲".to_owned() });
    assert_eq!(queue.origin().map(|origin| origin.name.as_str()), Some("全部歌曲"));
    queue.remove(1);
    assert_eq!(queue.origin().map(|origin| origin.name.as_str()), Some("全部歌曲"));
    queue.replace(vec!["c"], 0);
    assert!(queue.origin().is_none());
}

#[test]
fn clearing_the_queue_keeps_the_current_item_playing() {
    let mut queue = Queue::default();
    queue.replace_from(vec!['a', 'b', 'c', 'd'], 2, Origin { name: "全部歌曲".to_owned() });
    assert!(queue.clear_except_current());
    assert_eq!(queue.items(), &['c']);
    assert_eq!(queue.current(), Some(&'c'));
    assert_eq!(queue.current_index(), Some(0));
    assert!(queue.origin().is_none());
    assert!(!queue.clear_except_current(), "a lone current item is unchanged");

    let mut empty = Queue::<char>::default();
    assert!(!empty.clear_except_current());
    assert!(empty.items().is_empty());
}

#[test]
fn queue_neighbors_follow_repeat_without_moving() {
    let mut queue = Queue::default();
    queue.replace(vec!['a', 'b', 'c'], 0);
    assert_eq!((queue.previous_index(), queue.next_index()), (None, Some(1)));
    queue.repeat = Repeat::All;
    assert_eq!((queue.previous_index(), queue.next_index()), (Some(2), Some(1)));
    queue.select(2);
    assert_eq!(queue.next_index(), Some(0));
    queue.repeat = Repeat::One;
    assert_eq!(queue.next_index(), None, "repeat-one is not a neighbour");
    assert_eq!(queue.current(), Some(&'c'));
}

#[test]
fn queue_move_to_keeps_the_current_item_current() {
    let mut queue = Queue::default();
    queue.replace(vec!['a', 'b', 'c', 'd', 'e'], 2);
    assert!(queue.move_to(0, 3));
    assert_eq!(queue.items(), ['b', 'c', 'd', 'a', 'e']);
    assert_eq!(queue.current(), Some(&'c'));
    assert!(queue.move_to(4, 0));
    assert_eq!(queue.items(), ['e', 'b', 'c', 'd', 'a']);
    assert_eq!(queue.current(), Some(&'c'));
    assert!(queue.move_to(2, 4));
    assert_eq!(queue.current(), Some(&'c'));
    assert_eq!(queue.current_index(), Some(4));
    assert!(!queue.move_to(1, 1));
    assert!(!queue.move_to(9, 0));
}

#[test]
fn shuffle_keeps_the_playing_song_and_repeats_with_the_same_seed() {
    let mut queue = Queue::default();
    queue.replace((0..8).collect::<Vec<_>>(), 3);
    assert!(queue.shuffle(0x1234_5678));
    assert_eq!(queue.current(), Some(&3));
    assert_eq!(queue.current_index(), Some(0));
    assert_eq!(queue.items().len(), 8);
    let mut again = Queue::default();
    again.replace((0..8).collect::<Vec<_>>(), 3);
    assert!(again.shuffle(0x1234_5678));
    assert_eq!(again.items(), queue.items());
    let only_one = Queue::default();
    let mut single = only_one;
    single.replace(vec![7], 0);
    assert!(!single.shuffle(9));
}
