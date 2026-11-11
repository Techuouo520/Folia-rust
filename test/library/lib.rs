// Checks folder scanning, incremental rescans, folder removal, lyric lookup and cover cropping on temporary files.

#![cfg(test)]

use std::fs;
use std::path::Path;

use folia_library::{Library, ScanStep, cover_bytes, cover_for, decode_text, has_cover, is_audio, lyrics_for};

mod playlists;

/// A 1×1 PNG. The library must hand these bytes back without turning them into RGBA.
fn png_bytes() -> Vec<u8> {
    let image = image::RgbaImage::from_pixel(1, 1, image::Rgba([1, 2, 3, 255]));
    let mut bytes = Vec::new();
    image.write_to(&mut std::io::Cursor::new(&mut bytes), image::ImageFormat::Png).unwrap();
    bytes
}

/// Writes a short silent 16-bit stereo WAV so lofty can read a real duration.
fn write_wav(path: &Path, seconds: u32) {
    let rate = 8000u32;
    let data_len = rate * seconds * 4;
    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"RIFF");
    bytes.extend_from_slice(&(36 + data_len).to_le_bytes());
    bytes.extend_from_slice(b"WAVEfmt ");
    bytes.extend_from_slice(&16u32.to_le_bytes());
    bytes.extend_from_slice(&1u16.to_le_bytes());
    bytes.extend_from_slice(&2u16.to_le_bytes());
    bytes.extend_from_slice(&rate.to_le_bytes());
    bytes.extend_from_slice(&(rate * 4).to_le_bytes());
    bytes.extend_from_slice(&4u16.to_le_bytes());
    bytes.extend_from_slice(&16u16.to_le_bytes());
    bytes.extend_from_slice(b"data");
    bytes.extend_from_slice(&data_len.to_le_bytes());
    bytes.resize(bytes.len() + data_len as usize, 0);
    fs::write(path, bytes).unwrap();
}

#[test]
fn scans_rescans_and_removes() {
    let dir = tempfile::tempdir().unwrap();
    let music = dir.path().join("music");
    fs::create_dir_all(music.join("sub")).unwrap();
    write_wav(&music.join("01. Singer - Song.wav"), 2);
    write_wav(&music.join("sub/Other-Band.wav"), 1);
    fs::write(music.join("notes.txt"), "x").unwrap();

    let mut library = Library::open(&dir.path().join("db/library.db")).unwrap();
    library.add_folder(&music).unwrap();
    library.add_folder(&music).unwrap();
    let report = library.scan().unwrap();
    assert_eq!((report.updated, report.removed, report.total), (2, 0, 2));

    let tracks = library.tracks().unwrap();
    let names: Vec<_> = tracks.iter().map(|t| (t.title.as_str(), t.artist.as_deref())).collect();
    assert!(names.contains(&("Song", Some("Singer"))), "{names:?}");
    assert!(names.contains(&("Other", Some("Band"))), "{names:?}");
    let song = tracks.iter().find(|t| t.title == "Song").unwrap();
    assert!((song.duration - 2.0).abs() < 0.01, "{}", song.duration);

    assert_eq!(library.scan().unwrap().updated, 0, "unchanged files are not re-read");

    fs::remove_file(music.join("sub/Other-Band.wav")).unwrap();
    let report = library.scan().unwrap();
    assert_eq!((report.updated, report.removed, report.total), (0, 1, 1));

    assert_eq!(library.remove_folder(&music).unwrap(), 1);
    assert!(library.tracks().unwrap().is_empty());
    assert!(library.folders().unwrap().is_empty());
}

#[test]
fn reports_each_file_and_reads_one_track_by_id() {
    let dir = tempfile::tempdir().unwrap();
    let music = dir.path().join("music");
    let with_cover = music.join("with");
    let bare_dir = music.join("bare");
    fs::create_dir_all(&with_cover).unwrap();
    fs::create_dir_all(&bare_dir).unwrap();
    write_wav(&with_cover.join("01. Singer - Song.wav"), 1);
    write_wav(&bare_dir.join("02. Other - Tune.wav"), 1);
    fs::write(with_cover.join("cover.png"), png_bytes()).unwrap();

    let mut library = Library::open(&dir.path().join("library.db")).unwrap();
    library.add_folder(&music).unwrap();
    let seen = std::sync::Mutex::new(Vec::new());
    library
        .scan_with(|progress| {
            seen.lock().unwrap().push((progress.step, progress.index, progress.total, progress.path));
        })
        .unwrap();
    let seen = seen.into_inner().unwrap();
    let found: Vec<_> = seen.iter().filter(|item| item.0 == ScanStep::Found).collect();
    let reading: Vec<_> = seen.iter().filter(|item| item.0 == ScanStep::Reading).collect();
    assert_eq!(found.len(), 2);
    assert!(found.iter().all(|item| item.2.is_none()));
    let mut indexes: Vec<_> = found.iter().map(|item| item.1).collect();
    indexes.sort_unstable();
    assert_eq!(indexes, [1, 2]);
    assert_eq!(reading.len(), 2);
    assert!(reading.iter().all(|item| item.2 == Some(2)));

    let tracks = library.tracks().unwrap();
    let song = tracks.iter().find(|track| track.title == "Song").unwrap();
    assert_eq!(library.track(song.id).unwrap().as_ref().map(|track| track.title.as_str()), Some("Song"));
    assert!(library.track(song.id + 1000).unwrap().is_none());
    assert!(has_cover(&song.path));
    let bytes = cover_bytes(&song.path).unwrap();
    assert_eq!(&bytes[..8], &[0x89, b'P', b'N', b'G', b'\r', b'\n', 0x1a, b'\n']);
    assert_ne!(bytes.len(), 32 * 32 * 4, "raw bytes are not a cropped RGBA tile");
    let bare = tracks.iter().find(|track| track.title == "Tune").unwrap();
    assert!(!has_cover(&bare.path));
    assert!(cover_bytes(&bare.path).is_none());
}

#[test]
fn reopening_keeps_tracks_and_ids() {
    let dir = tempfile::tempdir().unwrap();
    write_wav(&dir.path().join("a.wav"), 1);
    let db = dir.path().join("library.db");
    let id = {
        let mut library = Library::open(&db).unwrap();
        library.add_folder(dir.path()).unwrap();
        library.scan().unwrap();
        library.tracks().unwrap()[0].id
    };
    let library = Library::open(&db).unwrap();
    assert_eq!(library.tracks().unwrap()[0].id, id);
}

#[test]
fn finds_sidecar_lyrics_with_translation() {
    let dir = tempfile::tempdir().unwrap();
    let track = dir.path().join("song.flac");
    fs::write(&track, b"not really flac").unwrap();
    assert!(lyrics_for(&track).is_none());

    fs::write(dir.path().join("song.lrc"), "[00:01.00]hello").unwrap();
    fs::write(dir.path().join("song.t.lrc"), "[00:01.00]你好").unwrap();
    let lyrics = lyrics_for(&track).unwrap();
    assert_eq!(lyrics.lines[0].text, "hello");
    assert_eq!(lyrics.lines[0].translation.as_deref(), Some("你好"));
}

#[test]
fn decodes_legacy_encodings() {
    let (gbk, _, _) = encoding_rs::GB18030.encode("[00:01.00]你好");
    assert_eq!(decode_text(&gbk), "[00:01.00]你好");
    assert_eq!(decode_text("\u{feff}abc".as_bytes()), "abc");
    assert_eq!(decode_text(&[0xff, 0xfe, b'h', 0, b'i', 0]), "hi");
}

#[test]
fn recognises_audio_extensions() {
    assert!(is_audio(Path::new("a/b.FLAC")));
    assert!(is_audio(Path::new("x.mp3")));
    assert!(!is_audio(Path::new("x.lrc")));
    assert!(!is_audio(Path::new("noext")));
}

#[test]
fn crops_sidecar_cover_to_tile() {
    let dir = tempfile::tempdir().unwrap();
    let song = dir.path().join("song.wav");
    write_wav(&song, 1);
    assert!(cover_for(&song, 64, 64).is_none());

    let art = image::RgbaImage::from_fn(200, 100, |x, _| {
        if x < 100 { image::Rgba([255, 0, 0, 255]) } else { image::Rgba([0, 0, 255, 255]) }
    });
    art.save(dir.path().join("cover.png")).unwrap();

    let square = cover_for(&song, 50, 50).unwrap();
    assert_eq!((square.width, square.height), (50, 50));
    let pixel = |x: u32, y: u32| &square.rgba[((y * square.width + x) * 4) as usize..][..3];
    assert_eq!(pixel(5, 25), [255, 0, 0]);
    assert_eq!(pixel(45, 25), [0, 0, 255]);

    let wide = cover_for(&song, 400, 100).unwrap();
    assert_eq!((wide.width, wide.height), (200, 50), "crops to the tile's shape but never enlarges");
}

#[test]
fn cover_cache_drops_the_least_recently_used_past_its_budget() {
    let mut cache = folia_library::CoverCache::new(100);
    cache.insert(1, Some("a"), 40);
    cache.insert(2, Some("b"), 40);
    cache.insert(3, None, 0);
    cache.touch(&1);
    cache.insert(4, Some("d"), 40);
    assert!(cache.peek(&2).is_none(), "2 was used least recently");
    assert_eq!(cache.peek(&1), Some(&Some("a")));
    assert_eq!(cache.peek(&3), Some(&None), "a song without a cover is remembered");
    assert_eq!(cache.used_bytes(), 80);
    cache.insert(1, Some("a2"), 10);
    assert_eq!(cache.used_bytes(), 50, "replacing a cover frees its old pixels");
    cache.insert(5, Some("huge"), 500);
    assert_eq!(cache.peek(&5), Some(&Some("huge")), "the newest stays even when it alone passes the budget");
    assert_eq!(cache.used_bytes(), 500);
}

#[test]
fn cover_cache_keeps_covers_in_use_this_pass_past_its_budget() {
    let mut cache = folia_library::CoverCache::new(100);
    cache.begin_pass();
    for key in 1..=3 {
        cache.insert(key, Some(key), 40);
    }
    assert_eq!(cache.used_bytes(), 120, "a view needing more than the budget keeps all its covers");
    cache.begin_pass();
    cache.touch(&2);
    cache.touch(&3);
    cache.insert(4, Some(4), 40);
    assert!(cache.peek(&1).is_none(), "1 left the view and goes first");
    assert_eq!(cache.used_bytes(), 120);
    assert!([2, 3, 4].iter().all(|key| cache.peek(key).is_some()), "what the view uses stays");
}
