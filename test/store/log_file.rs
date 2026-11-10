// Checks the runtime log file: the line limit, lines kept from an earlier run and split writes.

use std::fs;
use std::io::Write;

use folia_store::{LogFile, LogMode};

fn lines(path: &std::path::Path) -> Vec<String> {
    fs::read_to_string(path).unwrap().lines().map(str::to_owned).collect()
}

#[test]
fn keeps_only_the_last_lines() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("logs").join("runtime").join("folia.log");
    let mut log = LogFile::open_mode(&path, 500, LogMode::Append).unwrap();
    for index in 0..600 {
        log.write_all(format!("line {index}\n").as_bytes()).unwrap();
    }
    log.sync();
    let kept = lines(&path);
    assert_eq!(kept.len(), 500);
    assert_eq!(kept[0], "line 100");
    assert_eq!(kept[499], "line 599");
}

#[test]
fn a_new_run_continues_the_old_file() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("folia.log");
    fs::write(&path, "a\nb\nc\nd\n").unwrap();
    let mut log = LogFile::open_mode(&path, 3, LogMode::Append).unwrap();
    log.write_all(b"e\n").unwrap();
    log.sync();
    assert_eq!(lines(&path), ["c", "d", "e"]);
}

#[test]
fn a_line_split_across_writes_stays_one_line() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("folia.log");
    let mut log = LogFile::open_mode(&path, 10, LogMode::Append).unwrap();
    log.write_all(b"first ha").unwrap();
    log.write_all(b"lf\nsecond\n").unwrap();
    log.sync();
    assert_eq!(lines(&path), ["first half", "second"]);
}
