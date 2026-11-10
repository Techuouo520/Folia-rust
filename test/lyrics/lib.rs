// Checks format detection and every lyric parser against small hand-written samples.

mod chorus;
mod paragraphs;

use base64::Engine;
use folia_core::{Line, Lyrics};
use folia_lyrics::{Format, is_js_space, parse, split_sentences};

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-6
}

fn sung(lyrics: &Lyrics) -> Vec<&Line> {
    lyrics.lines.iter().filter(|line| !line.is_interlude()).collect()
}

#[test]
fn detects_formats() {
    assert_eq!(Format::detect("[00:01.00]hello\n[00:02.00]world"), Format::Lrc);
    assert_eq!(Format::detect("[00:01.00]<00:01.00>he<00:01.50>llo"), Format::EnhancedLrc);
    assert_eq!(Format::detect("[00:01.00]he[00:01.50]llo[00:02.00]"), Format::EnhancedLrc);
    assert_eq!(Format::detect("WEBVTT\n\n00:01.000 --> 00:02.000\nhi"), Format::Vtt);
    assert_eq!(Format::detect("go --> there"), Format::Lrc);
    assert_eq!(Format::detect("<tt xmlns=\"http://www.w3.org/ns/ttml\"></tt>"), Format::Ttml);
    assert_eq!(Format::from_extension("QRC"), Some(Format::Qrc));
    assert_eq!(Format::from_extension("lrc"), None);
}

#[test]
fn lrc_lines_translations_and_interludes() {
    let lrc = "\u{feff}[ti:ignored]\n[00:05.00]第一句\n[00:07.50]second line\n[00:30.00]last";
    let translation = "[00:05.00]first\n[00:30.20]final";
    let lyrics = parse(Format::Lrc, lrc, translation, "");

    assert!(!lyrics.word_timed);
    assert!(lyrics.lines[0].is_interlude());
    assert!(close(lyrics.lines[0].start, 0.5) && close(lyrics.lines[0].end, 4.5));

    let lines = sung(&lyrics);
    assert_eq!(lines.len(), 3);
    assert_eq!(lines[0].text, "第一句");
    assert_eq!(lines[0].translation.as_deref(), Some("first"));
    assert_eq!(lines[1].translation, None);
    assert_eq!(lines[2].translation.as_deref(), Some("final"));
    assert_eq!(lines[0].words.iter().map(|w| w.text.as_str()).collect::<String>(), "第一句");
    assert!(close(lines[1].end, 7.5 + 11.0 * 0.5 + 2.0), "long gap is capped by reading time");
    assert!(lyrics.lines.iter().any(|line| line.is_interlude() && line.start > 10.0));
}

#[test]
fn lrc_duet_lines_get_one_translation_each() {
    let lyrics = parse(Format::Lrc, "[00:01.00]a\n[00:01.00]b", "[00:01.00]A\n[00:01.00]B", "");
    let texts: Vec<_> = lyrics.lines.iter().map(|l| l.translation.as_deref()).collect();
    assert_eq!(texts, [Some("A"), Some("B")]);
}

#[test]
fn enhanced_lrc_word_times_and_metadata() {
    let content =
        "[ti:Song]\n[ar:Singer]\n[00:01.00]<00:01.00>Hel<00:01.50>lo <00:02.00>you<00:03.00>\n[00:04.00]plain";
    let lyrics = parse(Format::EnhancedLrc, content, "", "");
    assert_eq!(lyrics.title.as_deref(), Some("Song"));
    assert_eq!(lyrics.artist.as_deref(), Some("Singer"));
    assert!(lyrics.word_timed);

    let first = &lyrics.lines[0];
    assert_eq!(first.text, "Hello you");
    let words: Vec<_> = first.words.iter().map(|w| (w.text.as_str(), w.start, w.end)).collect();
    assert_eq!(words, [("Hel", 1.0, 1.5), ("lo ", 1.5, 2.0), ("you", 2.0, 3.0)]);
    assert_eq!(lyrics.lines[1].text, "plain");
    assert!(!lyrics.lines[1].words.is_empty());
}

#[test]
fn yrc_words() {
    let content = "{\"t\":0,\"c\":[]}\n[1000,2000](1000,500,0)你(1500,500,0)好(2000,1000,0)啊";
    let lyrics = parse(Format::Yrc, content, "[00:01.00]hello", "");
    let line = &lyrics.lines[0];
    assert!(lyrics.word_timed);
    assert_eq!(line.text, "你好啊");
    assert!(close(line.start, 1.0) && close(line.end, 3.0));
    assert!(close(line.words[2].start, 2.0) && close(line.words[2].end, 3.0));
    assert_eq!(line.translation.as_deref(), Some("hello"));
}

#[test]
fn qrc_text_before_or_after_tags() {
    let leading = parse(Format::Qrc, "[0,1000]A(0,500)B(500,500)", "", "");
    assert_eq!(leading.lines[0].words.iter().map(|w| w.text.as_str()).collect::<Vec<_>>(), ["A", "B"]);
    let trailing = parse(Format::Qrc, "[0,1000](0,500)A(500,500)B", "", "");
    assert_eq!(trailing.lines[0].text, "AB");
    assert!(close(trailing.lines[0].words[1].start, 0.5));
}

#[test]
fn krc_relative_offsets_and_embedded_translation() {
    let tag = r#"{"content":[{"type":1,"lyricContent":[["你好"],["再见"]]}]}"#;
    let encoded = base64::engine::general_purpose::STANDARD.encode(tag);
    let content = format!("[language:{encoded}]\n[1000,1000]<0,500,0>Hel<500,500,0>lo\n[3000,1000]<0,1000,0>Bye");
    let lyrics = parse(Format::Krc, &content, "", "");
    let first = &lyrics.lines[0];
    assert_eq!(first.text, "Hello");
    assert!(close(first.words[1].start, 1.5));
    assert_eq!(first.translation.as_deref(), Some("你好"));
    assert_eq!(lyrics.lines[1].translation.as_deref(), Some("再见"));
}

#[test]
fn vtt_cues() {
    let content = "WEBVTT\n\nNOTE skip me\n\n1\n00:00:01.000 --> 00:00:03.500 align:start\n<i>Hello</i> &amp;   world\n\n00:04.000 --> 00:05.000\nnext";
    let lyrics = parse(Format::Vtt, content, "", "");
    assert_eq!(lyrics.lines.len(), 2);
    assert_eq!(lyrics.lines[0].text, "Hello & world");
    assert!(close(lyrics.lines[0].end, 3.5));
    assert!(close(lyrics.lines[1].start, 4.0));
}

#[test]
fn line_at_time() {
    let lyrics = parse(Format::Lrc, "[00:01.00]a\n[00:02.00]b", "", "");
    assert_eq!(lyrics.line_at(0.5), None);
    assert_eq!(lyrics.line_at(1.0), Some(0));
    assert_eq!(lyrics.line_at(2.5), Some(1));
}

#[test]
fn ttml_word_timing_translations_and_entities() {
    let ttml = r#"<tt xmlns="http://www.w3.org/ns/ttml" xmlns:itunes="http://music.apple.com/lyric-ttml-internal" xmlns:ttm="http://www.w3.org/ns/ttml#metadata" itunes:timing="Word">
<head><metadata><iTunesMetadata><translations><translation xml:lang="zh"><text for="L2">第二行</text></translation></translations></iTunesMetadata></metadata></head>
<body><div>
<p begin="00:01.000" end="00:03.000" itunes:key="L1"><span begin="00:01.000" end="00:01.400">Hel</span><span begin="00:01.400" end="00:01.800">lo</span> <span begin="00:02.000" end="00:03.000">you&amp;me</span><span ttm:role="x-translation">你好</span></p>
<p begin="4.5s" end="6s" itunes:key="L2"><span begin="4.5s" end="5s">雨</span><span begin="5s" end="6s">天</span></p>
</div></body></tt>"#;
    assert_eq!(Format::detect(ttml), Format::Ttml);
    let lyrics = parse(Format::Ttml, ttml, "", "");
    assert!(lyrics.word_timed);
    let first = &lyrics.lines[0];
    assert_eq!(first.text, "Hello you&me");
    let words: Vec<_> = first.words.iter().map(|w| (w.text.as_str(), w.start, w.end)).collect();
    assert_eq!(words, [("Hello", 1.0, 1.8), ("you&me", 2.0, 3.0)]);
    assert_eq!(first.translation.as_deref(), Some("你好"));

    let second = sung(&lyrics)[1];
    assert_eq!(second.words.len(), 2, "CJK syllables stay separate");
    assert!(close(second.start, 4.5) && close(second.end, 6.0));
    assert_eq!(second.translation.as_deref(), Some("第二行"));
}

#[test]
fn ttml_line_timing() {
    let ttml = r#"<tt xmlns="http://www.w3.org/ns/ttml" xmlns:itunes="x" itunes:timing="Line"><body><div><p begin="00:00:02.000" end="00:00:04.000">plain line here</p></div></body></tt>"#;
    let lyrics = parse(Format::Ttml, ttml, "", "");
    assert!(!lyrics.word_timed);
    let line = &lyrics.lines[0];
    assert_eq!(line.text, "plain line here");
    assert_eq!(line.words.len(), 3);
    assert!(close(line.start, 2.0) && close(line.end, 4.0));
    assert_eq!(parse(Format::Ttml, "<tt><p>broken", "", "").lines.len(), 0);
}

fn timed(texts: &[&str]) -> Line {
    let words: Vec<folia_core::Word> = texts
        .iter()
        .enumerate()
        .map(|(i, t)| folia_core::Word { text: (*t).to_owned(), start: i as f64, end: i as f64 + 1.0 })
        .collect();
    Line { text: texts.concat(), start: 0.0, end: texts.len() as f64, words, ..Line::default() }
}

#[test]
fn layout_attaches_contractions_and_punctuation() {
    let line = timed(&["It", "’", "s", " ", "un", "believable", ","]);
    let units = folia_lyrics::layout_units(&line, true);
    let texts: Vec<_> = folia_lyrics::display_words(&line, &units).into_iter().map(|w| w.text).collect();
    assert_eq!(texts, ["It’s", " ", "un", "believable,"]);
    assert!(units[0].sticky && close(units[0].end, 3.0));
}

#[test]
fn layout_groups_cjk_words() {
    let line = timed(&["我", "喜", "欢", "你", "。"]);
    let units = folia_lyrics::layout_units(&line, true);
    let texts: Vec<_> = units.iter().map(|u| u.text.as_str()).collect();
    assert_eq!(texts, ["我", "喜欢", "你。"]);
    assert_eq!(units.iter().map(|u| u.semantic).collect::<Vec<_>>(), [false, true, false]);
    let display: Vec<_> = folia_lyrics::display_words(&line, &units).into_iter().map(|w| w.text).collect();
    assert_eq!(display, ["我", "喜", "欢", "你。"]);

    let plain = folia_lyrics::layout_units(&line, false);
    assert_eq!(plain.len(), 4, "without semantic grouping only the full stop attaches");
}

#[test]
fn sentence_splits_match_the_original() {
    let split = |text, count| split_sentences(text, count, 1.0);
    assert_eq!(split("你好，世界。", 2), ["你好，", "世界。"]);
    assert_eq!(split("你好，世界。再见！", 2), ["你好，世界。", "再见！"]);
    assert_eq!(split("a，b，c，d", 2), ["a，b，", "c，d"]);
    assert_eq!(split("「你好」世界", 2), ["「你好」", "世界"]);
    assert_eq!(split("(hi) there", 2), ["(hi)", " there"]);
    assert_eq!(split("say \"hello\" now", 2), ["say \"hello\"", " now"]);
    assert_eq!(split("A: B: C", 2), ["A: B:", " C"]);
    assert_eq!(split("标题：副歌：尾声", 2), ["标题：", "副歌：尾声"]);
    assert_eq!(split("播放 Hello World 歌曲", 2), ["播放 ", "Hello World 歌曲"]);
    assert_eq!(split("你好 世界 再 见", 3), ["你好 ", "世界 ", "再 见"]);
    assert_eq!(split_sentences("......", 3, 1.0), ["...", ".", ".."]);
    assert_eq!(split("我喜欢你", 2), ["我", "喜欢你"]);
    assert_eq!(split("one two three", 2), ["one two ", "three"]);
    assert_eq!(split_sentences("保持一整行", 1, 0.0), ["保持一整行"]);
}

#[test]
fn js_space_matches_the_original_class() {
    assert!(is_js_space(' ') && is_js_space('\t') && is_js_space('\n') && is_js_space('\u{3000}'));
    assert!(is_js_space('\u{00a0}') && is_js_space('\u{2003}') && is_js_space('\u{feff}'));
    assert!(!is_js_space('a') && !is_js_space('你') && !is_js_space('。'));
}

#[test]
fn grapheme_timeline_splits_words_evenly() {
    let line = Line {
        text: "光落 在".to_owned(),
        start: 1.0,
        end: 4.0,
        words: vec![
            folia_core::Word { text: "光落".to_owned(), start: 1.0, end: 2.0 },
            folia_core::Word { text: "在".to_owned(), start: 2.5, end: 3.0 },
        ],
        ..Line::default()
    };
    assert_eq!(folia_lyrics::graphemes(&line.text).len(), 4);
    let timeline = folia_lyrics::grapheme_timeline(&line);
    let starts: Vec<f64> = timeline.iter().map(|timing| timing.start).collect();
    assert_eq!(starts, vec![1.0, 1.5, 2.5, 2.5]);
    assert!(close(timeline[1].end, 2.0));
    assert!(close(timeline[2].end, 2.5));
}

#[test]
fn word_spans_count_graphemes_and_mark_blanks() {
    let spans = folia_lyrics::word_spans("hello, 世界");
    assert_eq!(spans.first().map(|span| (span.start, span.end, span.blank)), Some((0, 5, false)));
    assert!(spans.iter().any(|span| span.blank && span.end - span.start == 1));
    assert_eq!(spans.last().map(|span| span.end), Some(9));
}
