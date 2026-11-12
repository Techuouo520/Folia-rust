// Small cover previews survive panning while large decoded images remain bounded.
use folia_ui::{Wall, WallCover, WallMessage, WallShape, WallTile};
use iced::futures::StreamExt;
use iced::{Size, Task, Vector};

fn messages(task: Task<WallMessage>) -> Vec<WallMessage> {
    iced::futures::executor::block_on(async {
        let Some(stream) = iced_runtime::task::into_stream(task) else { return Vec::new() };
        stream
            .filter_map(|action| async move {
                match action {
                    iced_runtime::Action::Output(message) => Some(message),
                    _ => None,
                }
            })
            .collect()
            .await
    })
}

#[test]
fn late_thumbnails_survive_fast_pan_without_pinning_full_images() {
    let temp = tempfile::tempdir().unwrap();
    image::RgbaImage::from_pixel(8, 8, image::Rgba([40, 90, 130, 255])).save(temp.path().join("cover.png")).unwrap();
    let tiles: Vec<_> = (0..1642)
        .map(|id| WallTile {
            id,
            title: format!("Song {id}"),
            subtitle: String::new(),
            badge: None,
            cover: WallCover::Local(temp.path().join("song.wav")),
            heading: false,
            hue: 0.0,
            stack: false,
        })
        .collect();
    let mut wall = Wall::new();
    let _ = wall.relayout(&tiles, WallShape::Blocks, None);
    let (task, _) = wall.update(WallMessage::Viewport(Size::new(1200.0, 800.0)), &tiles, None);
    let old = messages(task);
    let ids = |messages: &[WallMessage]| {
        messages
            .iter()
            .flat_map(|message| match message {
                WallMessage::CoverLoaded { id, .. } => vec![*id],
                _ => Vec::new(),
            })
            .collect::<Vec<_>>()
    };
    let old_ids = ids(&old);
    assert!(!old_ids.is_empty());
    let (task, _) = wall.update(WallMessage::Pan(Vector::new(4000.0, 2000.0)), &tiles, None);
    let new = messages(task);
    let current_ids = ids(&new);
    let stale: Vec<_> = old_ids.iter().filter(|id| !current_ids.contains(id)).collect();
    assert!(!stale.is_empty());
    for message in old {
        let _ = wall.update(message, &tiles, None);
    }
    assert!(
        stale.iter().all(|id| wall.cover(**id).is_some()),
        "completed previews were discarded on leaving the screen"
    );
    assert!(wall.cover_memory() <= 6 * 64 * 64 * 4);
    assert!(old_ids.len() <= 6, "decoding should be bounded to six current covers");
}

fn settle(wall: &mut Wall, tiles: &[WallTile], mut task: Task<WallMessage>) {
    for _ in 0..100 {
        let ready = messages(task);
        if ready.is_empty() {
            return;
        }
        task = Task::batch(ready.into_iter().map(|message| wall.update(message, tiles, None).0).collect::<Vec<_>>());
    }
    panic!("cover requests did not settle");
}

#[test]
fn returning_to_a_large_playlist_keeps_a_ready_cover() {
    let temp = tempfile::tempdir().unwrap();
    image::RgbaImage::from_pixel(512, 512, image::Rgba([40, 90, 130, 255]))
        .save(temp.path().join("cover.png"))
        .unwrap();
    let tiles: Vec<_> = (0..1642)
        .map(|id| WallTile {
            id,
            title: format!("Song {id}"),
            subtitle: String::new(),
            badge: None,
            cover: WallCover::Local(temp.path().join(format!("{id}.wav"))),
            heading: false,
            hue: 0.0,
            stack: false,
        })
        .collect();
    let mut wall = Wall::new();
    let _ = wall.relayout(&tiles, WallShape::Blocks, None);
    let (task, _) = wall.update(WallMessage::Viewport(Size::new(1200.0, 800.0)), &tiles, None);
    let first = messages(task);
    let ids: Vec<_> = first
        .iter()
        .filter_map(|message| match message {
            WallMessage::CoverLoaded { id, px: 64, .. } => Some(*id),
            _ => None,
        })
        .collect();
    assert!(!ids.is_empty());
    let task = Task::batch(first.into_iter().map(|message| wall.update(message, &tiles, None).0).collect::<Vec<_>>());
    settle(&mut wall, &tiles, task);
    let is_preview = |wall: &Wall, id| matches!(wall.cover(id), Some(iced::widget::image::Handle::Rgba { width, height, .. }) if width <= 64 && height <= 64);
    assert!(ids.iter().all(|id| !is_preview(&wall, *id)), "initial covers must reach full resolution");
    let mut distance = Vector::ZERO;
    for _ in 0..30 {
        let (task, _) = wall.update(WallMessage::Pan(Vector::new(2000.0, 1000.0)), &tiles, None);
        settle(&mut wall, &tiles, task);
        distance += Vector::new(2000.0, 1000.0);
        if ids.iter().all(|id| is_preview(&wall, *id)) {
            break;
        }
    }
    assert!(ids.iter().all(|id| is_preview(&wall, *id)), "large images must be evicted to exercise preview reuse");
    let (back, _) = wall.update(WallMessage::Pan(distance * -1.0), &tiles, None);
    assert!(ids.iter().all(|id| wall.cover(*id).is_some()), "returning to seen covers waited for decoding");
    assert!(wall.cover_memory() <= 80 * 1024 * 1024);
    drop(back);
}

#[test]
fn old_cover_source_cannot_replace_a_changed_track_cover() {
    let temp = tempfile::tempdir().unwrap();
    image::RgbaImage::from_pixel(8, 8, image::Rgba([255, 0, 0, 255])).save(temp.path().join("cover.png")).unwrap();
    let mut tiles = vec![WallTile {
        id: 1,
        title: "Song".into(),
        subtitle: String::new(),
        badge: None,
        cover: WallCover::Local(temp.path().join("old.wav")),
        heading: false,
        hue: 0.0,
        stack: false,
    }];
    let mut wall = Wall::new();
    let _ = wall.relayout(&tiles, WallShape::Blocks, None);
    let (task, _) = wall.update(WallMessage::Viewport(Size::new(1200.0, 800.0)), &tiles, None);
    let old = messages(task);
    tiles[0].cover = WallCover::Local(temp.path().join("new.wav"));
    let _ = wall.relayout(&tiles, WallShape::Blocks, None);
    for message in old {
        let _ = wall.update(message, &tiles, None);
    }
    assert!(wall.cover(1).is_none());
}
