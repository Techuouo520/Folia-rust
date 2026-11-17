// Exercises picker buttons and verifies style thumbnails stay independent of the chosen background.
use super::player_fixture::{complete, init, run_gpu_child};
use folia_ui::{Message, Page, PanelMessage, PanelTab, Shell};
use iced::advanced::renderer::Headless;
use iced::{Color, Event, Rectangle, Size};
use iced_runtime::user_interface::{Cache, UserInterface};
use std::time::Duration;

const WINDOW: Size = Size::new(1200.0, 800.0);

#[derive(Default)]
struct Labels(Vec<(String, Rectangle)>);
impl iced::advanced::widget::Operation for Labels {
    fn traverse(&mut self, operate: &mut dyn FnMut(&mut dyn iced::advanced::widget::Operation)) {
        operate(self);
    }
    fn text(&mut self, _: Option<&iced::widget::Id>, bounds: Rectangle, text: &str) {
        self.0.push((text.into(), bounds));
    }
}

pub(super) fn click(shell: &mut Shell, renderer: &mut iced::Renderer, cache: &mut Cache, size: Size, label: &str) {
    let mut ui = UserInterface::build(shell.view(), size, std::mem::take(cache), renderer);
    let mut labels = Labels::default();
    ui.operate(renderer, &mut labels);
    let point = {
        let found: Vec<_> = labels.0.iter().filter(|(text, _)| text == label).collect();
        assert_eq!(found.len(), 1, "ambiguous or missing {label}: {:?}", labels.0);
        found[0].1.center()
    };
    let mut messages = Vec::new();
    ui.update(
        &[
            Event::Mouse(iced::mouse::Event::ButtonPressed(iced::mouse::Button::Left)),
            Event::Mouse(iced::mouse::Event::ButtonReleased(iced::mouse::Button::Left)),
        ],
        iced::mouse::Cursor::Available(point),
        renderer,
        &mut iced::advanced::clipboard::Null,
        &mut messages,
    );
    *cache = ui.into_cache();
    assert!(!messages.is_empty(), "click on {label:?} produced no message");
    for message in messages {
        let task = shell.update(message);
        complete(shell, task);
    }
    std::thread::sleep(Duration::from_millis(350));
}

fn reopen_panel(shell: &mut Shell) {
    drop(shell.update(Message::Panel(PanelMessage::Close)));
    drop(shell.update(Message::Panel(PanelMessage::Toggle)));
    std::thread::sleep(Duration::from_millis(350));
}

fn picture(shell: &Shell, renderer: &mut iced::Renderer, cache: &mut Cache, size: Size) -> Vec<u8> {
    let mut ui = UserInterface::build(shell.view(), size, std::mem::take(cache), renderer);
    let mut labels = Labels::default();
    ui.operate(renderer, &mut labels);
    let caption = labels.0.iter().find(|(label, _)| label == "云阶").unwrap().1;
    let trigger = labels.0.iter().find(|(label, _)| label == "歌词样式").unwrap().1;
    assert!(caption.y > trigger.y + trigger.height, "the picker must open below its row: {caption:?}, {trigger:?}");
    let more = labels.0.iter().find(|(label, _)| label == "更多设置").unwrap().1;
    assert!(more.y + more.height <= size.height, "the picker footer must fit inside {size:?}: {more:?}");
    ui.draw(
        renderer,
        &shell.theme(),
        &iced::advanced::renderer::Style { text_color: Color::BLACK },
        iced::mouse::Cursor::Unavailable,
    );
    *cache = ui.into_cache();
    let image = renderer.screenshot(
        &iced::advanced::graphics::Viewport::with_physical_size(Size::new(size.width as u32, size.height as u32), 1.0),
        Color::WHITE,
    );
    let left = (caption.center_x() - 36.0).round() as usize;
    let top = (caption.y - 50.0).round() as usize;
    let mut crop = Vec::new();
    for y in top..(top + 40).min(size.height as usize) {
        let offset = (y * size.width as usize + left) * 4;
        crop.extend_from_slice(&image[offset..offset + 72 * 4]);
    }
    crop
}

#[test]
#[cfg_attr(feature = "ci", ignore = "needs a GPU for picker pictures")]
fn background_selection_updates_settings_without_changing_style_pictures() {
    if run_gpu_child("picker::background_selection_updates_settings_without_changing_style_pictures") {
        return;
    }
    folia_ui::install_fonts();
    let temp = tempfile::tempdir().unwrap();
    let mut setup = init(temp.path());
    setup.appearance.set_background_mode(Some("common"));
    let store = setup.store.clone();
    let (mut shell, initial) = Shell::new(setup);
    complete(&mut shell, initial);
    drop(shell.update(Message::SplashFrame));
    std::thread::sleep(Duration::from_millis(350));
    drop(shell.update(Message::SplashFrame));
    drop(shell.update(Message::Navigate(Page::NowPlaying)));
    drop(shell.update(Message::Panel(PanelMessage::Toggle)));
    drop(shell.update(Message::Panel(PanelMessage::Choose(PanelTab::Controls))));
    std::thread::sleep(Duration::from_millis(400));
    let mut renderer =
        pollster::block_on(<iced::Renderer as Headless>::new(folia_ui::INTERFACE_FONT, iced::Pixels(16.0), None))
            .unwrap();
    let mut cache = Cache::default();
    click(&mut shell, &mut renderer, &mut cache, WINDOW, "流光");
    let before = picture(&shell, &mut renderer, &mut cache, WINDOW);
    reopen_panel(&mut shell);
    click(&mut shell, &mut renderer, &mut cache, WINDOW, "通用");
    click(&mut shell, &mut renderer, &mut cache, WINDOW, "莫奈");
    assert_eq!(store.load::<folia_store::AppearanceSettings>().unwrap().stored_background_mode(), Some("monet"));
    reopen_panel(&mut shell);
    click(&mut shell, &mut renderer, &mut cache, WINDOW, "流光");
    let after = picture(&shell, &mut renderer, &mut cache, WINDOW);
    assert!(before == after, "style thumbnails must use their theme surface without the playback background");
    for size in [Size::new(960.0, 600.0), Size::new(683.0, 384.0)] {
        drop(shell.update(Message::WindowResized(size)));
        reopen_panel(&mut shell);
        click(&mut shell, &mut renderer, &mut cache, size, "流光");
        picture(&shell, &mut renderer, &mut cache, size);
    }
}

#[test]
#[cfg_attr(feature = "ci", ignore = "needs a GPU")]
fn appearance_opens_the_workbench_and_returns_to_its_settings() {
    if run_gpu_child("picker::appearance_opens_the_workbench_and_returns_to_its_settings") {
        return;
    }
    folia_ui::install_fonts();
    let directory = tempfile::tempdir().unwrap();
    let mut setup = init(directory.path());
    setup.appearance.visualizer = "monet".into();
    let store = setup.store.clone();
    let (mut shell, initial) = Shell::new(setup);
    complete(&mut shell, initial);
    drop(shell.update(Message::SplashFrame));
    std::thread::sleep(Duration::from_millis(350));
    drop(shell.update(Message::SplashFrame));
    drop(shell.update(Message::OpenSettings));
    std::thread::sleep(Duration::from_millis(350));
    let mut renderer =
        pollster::block_on(<iced::Renderer as Headless>::new(folia_ui::INTERFACE_FONT, iced::Pixels(16.0), None))
            .unwrap();
    let mut cache = Cache::default();
    click(&mut shell, &mut renderer, &mut cache, WINDOW, "歌词动画样式");
    let task = shell.update(Message::Workbench(folia_ui::WorkbenchMessage::Toggle("mirrorLayout", true)));
    complete(&mut shell, task);
    let task = shell
        .update(Message::Workbench(folia_ui::WorkbenchMessage::Text("customDescription", "Custom description".into())));
    complete(&mut shell, task);
    let mut ui = UserInterface::build(shell.view(), WINDOW, cache, &mut renderer);
    let mut labels = Labels::default();
    ui.operate(&renderer, &mut labels);
    assert!(labels.0.iter().any(|(label, _)| label == "歌词左右镜像"));
    assert!(labels.0.iter().any(|(label, _)| label == "自定义描述"));
    ui.draw(
        &mut renderer,
        &shell.theme(),
        &iced::advanced::renderer::Style { text_color: Color::BLACK },
        iced::mouse::Cursor::Unavailable,
    );
    if let Some(directory) = std::env::var_os("FOLIA_APPEARANCE_CAPTURE") {
        std::fs::create_dir_all(&directory).unwrap();
        let pixels = renderer.screenshot(
            &iced::advanced::graphics::Viewport::with_physical_size(Size::new(1200, 800), 1.0),
            Color::WHITE,
        );
        image::RgbaImage::from_raw(1200, 800, pixels)
            .unwrap()
            .save(std::path::PathBuf::from(directory).join("appearance-workbench.png"))
            .unwrap();
    }
    cache = ui.into_cache();
    let task = shell.update(Message::Workbench(folia_ui::WorkbenchMessage::Close));
    complete(&mut shell, task);
    let stored: folia_store::VisualizerSettings = store.load().unwrap();
    let values = folia_visualizer::Style::Monet.load(&stored.tables);
    assert_eq!(values.text("customDescription"), "Custom description");
    assert!(values.flag(folia_visualizer::Style::Monet.manifest(), "mirrorLayout"));
    click(&mut shell, &mut renderer, &mut cache, WINDOW, "深色");
    assert_eq!(folia_store::AppearanceSettings::load(&store).unwrap().theme, folia_store::ThemeMode::Dark);
    click(&mut shell, &mut renderer, &mut cache, WINDOW, "莫奈");
}
