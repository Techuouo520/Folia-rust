// Checks the local playlists in the library database, the liked-songs list and the index migration.

use folia_library::{Library, gain_from_tag};
use rusqlite::Connection;

fn insert_track(conn: &Connection, path: &str, title: &str) -> i64 {
    conn.execute(
        "INSERT INTO tracks (path, title, artist, album, album_artist, track_no, disc_no, duration, size, mtime,
         replay_gain_track, replay_gain_album) VALUES (?1, ?2, NULL, NULL, NULL, NULL, NULL, 1.0, 1, 1, NULL, NULL)",
        [path, title],
    )
    .unwrap();
    conn.last_insert_rowid()
}

#[test]
fn playlists_and_the_liked_list_round_trip() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("library.db");
    let library = Library::open(&path).unwrap();
    assert!(library.playlists().unwrap().is_empty());

    let favorite = library.favorite_playlist().unwrap();
    assert!(favorite.favorite);
    let mix = library.create_playlist("Mix").unwrap();
    assert!(!mix.favorite);

    // Tracks exist only through a scan; a direct insert stands in for one.
    let conn = Connection::open(&path).unwrap();
    let first = insert_track(&conn, "a.mp3", "A");
    let second = insert_track(&conn, "b.mp3", "B");
    drop(conn);

    assert_eq!(library.add_to_playlist(mix.id, &[first, second, first]).unwrap(), 2);
    assert_eq!(library.add_to_playlist(favorite.id, &[first]).unwrap(), 1);
    assert!(library.is_favorite(first).unwrap());
    assert!(!library.is_favorite(second).unwrap());
    library.set_favorite(first, false).unwrap();
    assert!(!library.is_favorite(first).unwrap());
    library.set_favorite(second, true).unwrap();
    assert!(library.is_favorite(second).unwrap());

    let playlists = library.playlists().unwrap();
    assert_eq!(playlists.len(), 2);
    assert!(playlists[0].favorite);
    assert_eq!(playlists[0].song_ids, vec![second]);
    assert_eq!(playlists[1].song_ids, vec![first, second]);

    library.remove_from_playlist(mix.id, &[first]).unwrap();
    assert_eq!(library.playlists().unwrap()[1].song_ids, vec![second]);

    library.delete_playlist(mix.id).unwrap();
    let left = library.playlists().unwrap();
    assert_eq!(left.len(), 1);
    // The liked list is never deleted.
    library.delete_playlist(left[0].id).unwrap();
    assert_eq!(library.playlists().unwrap().len(), 1);
}

#[test]
fn an_old_index_is_kept_and_playlists_are_added() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("library.db");
    {
        let conn = Connection::open(&path).unwrap();
        conn.execute_batch(
            "PRAGMA user_version = 1;
             CREATE TABLE folders (path TEXT PRIMARY KEY);
             CREATE TABLE tracks (
                 id INTEGER PRIMARY KEY, path TEXT NOT NULL UNIQUE, title TEXT NOT NULL, artist TEXT, album TEXT,
                 album_artist TEXT, track_no INTEGER, disc_no INTEGER, duration REAL NOT NULL, size INTEGER NOT NULL,
                 mtime INTEGER NOT NULL);
             INSERT INTO folders (path) VALUES ('music');
             INSERT INTO tracks (path, title, duration, size, mtime) VALUES ('music/old.mp3', 'Old', 2.0, 3, 4);",
        )
        .unwrap();
    }
    let library = Library::open(&path).unwrap();
    // The index is rebuilt from the kept folders; the playlists are added beside it.
    assert!(library.tracks().unwrap().is_empty());
    assert_eq!(library.folders().unwrap().len(), 1);
    let favorite = library.favorite_playlist().unwrap();
    assert!(favorite.favorite);
    let conn = Connection::open(&path).unwrap();
    let track = insert_track(&conn, "music/old.mp3", "Old");
    drop(conn);
    assert_eq!(library.add_to_playlist(favorite.id, &[track]).unwrap(), 1);
}

#[test]
fn replay_gain_tag_text_parses() {
    assert_eq!(gain_from_tag("-6.54 dB"), Some(-6.54));
    assert_eq!(gain_from_tag("+3.0 dB"), Some(3.0));
    assert_eq!(gain_from_tag("-6,54"), Some(-6.54));
    assert_eq!(gain_from_tag("  -1.2 dB  "), Some(-1.2));
    assert_eq!(gain_from_tag("loud"), None);
    assert_eq!(gain_from_tag(""), None);
}
