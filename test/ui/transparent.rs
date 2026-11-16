// Transparency follows the player page and persists through the normal panel message path.
use super::player_fixture::complete;
use folia_ui::{Init, Message, Page, PanelMessage, Shell};
use iced::advanced::renderer::Headless;
use iced_runtime::user_interface::{Cache, UserInterface};

#[test]
#[cfg_attr(feature = "ci", ignore = "needs a GPU for alpha readback")]
fn player_transparency_persists_and_clears_only_the_player() {
    if super::player_fixture::run_gpu_child("transparent::player_transparency_persists_and_clears_only_the_player") {
        return;
    }
    folia_ui::install_fonts();
    let temp = tempfile::tempdir().unwrap();
    let init = super::player_fixture::init(temp.path());
    let store = init.store.clone();
    let (mut shell, initial) = Shell::new(init.clone());
    complete(&mut shell, initial);
    drop(shell.update(Message::SplashFrame));
    drop(shell.update(Message::OpenFiles(vec![folia_core::Track {
        id: 1,
        title: "透明测试".into(),
        duration: 200.0,
        ..Default::default()
    }])));
    let lyrics = folia_core::Lyrics {
        lines: vec![folia_core::Line {
            start: -3.0,
            end: 100.0,
            text: "透明测试歌词".into(),
            words: vec![folia_core::Word { text: "透明测试歌词".into(), start: -3.0, end: 100.0 }],
            ..Default::default()
        }],
        ..Default::default()
    };
    drop(shell.update(Message::Playback(folia_ui::PlaybackMessage::LyricsLoaded(1, 1, Some(lyrics), None))));
    std::thread::sleep(std::time::Duration::from_millis(350));
    drop(shell.update(Message::SplashFrame));
    let mut renderer =
        pollster::block_on(<iced::Renderer as Headless>::new(folia_ui::INTERFACE_FONT, iced::Pixels(16.0), None))
            .unwrap();
    let mut cache = Cache::default();
    for enabled in [true, false, true] {
        let task = shell.update(Message::Panel(PanelMessage::TransparentBackground(enabled)));
        complete(&mut shell, task);
        let saved = store.load::<folia_store::ThemeSettings>().unwrap();
        assert_eq!(saved.transparent_player_background, enabled);
        for page in [Page::NowPlaying, Page::Library, Page::NowPlaying] {
            drop(shell.update(Message::Navigate(page)));
            std::thread::sleep(std::time::Duration::from_millis(350));
            let background = shell.window_style(&shell.theme()).background_color;
            let expected = if enabled && page == Page::NowPlaying { 0 } else { 255 };
            assert_eq!((background.a * 255.0).round() as u8, expected);
            let mut ui = UserInterface::build(shell.view(), iced::Size::new(1200.0, 800.0), cache, &mut renderer);
            ui.draw(
                &mut renderer,
                &shell.theme(),
                &iced::advanced::renderer::Style { text_color: iced::Color::WHITE },
                iced::mouse::Cursor::Unavailable,
            );
            cache = ui.into_cache();
            let pixels = renderer.screenshot(
                &iced::advanced::graphics::Viewport::with_physical_size(iced::Size::new(1200, 800), 1.0),
                background,
            );
            assert_eq!(pixels[(200 * 1200 + 200) * 4 + 3], expected, "enabled={enabled} page={page:?}");
            if enabled && page == Page::NowPlaying {
                let alpha = pixels[(764 * 1200 + 600) * 4 + 3];
                assert!((80..200).contains(&alpha), "the player capsule must retain translucent fill, alpha={alpha}");
            }
        }
    }
    let (mut restored, initial) = Shell::new(Init { theme: store.load().unwrap(), ..init });
    drop(initial);
    drop(restored.update(Message::Navigate(Page::NowPlaying)));
    assert_eq!(restored.window_style(&restored.theme()).background_color.a, 0.0);
}
