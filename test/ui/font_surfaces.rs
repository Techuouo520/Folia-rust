// Real UI surfaces rendered without a window, at both requested sizes and themes.

use folia_core::{AUDIO_QUALITY_LEVELS, AudioQuality, Language, LanguagePreference, Theme};
use folia_i18n::I18n;
use folia_store::{AppearanceSettings, DeveloperSettings, HomeTab, RuntimeLog};
use folia_ui::*;
use iced::advanced::renderer::Headless;
use iced::{Color, Element, Pixels, Size};
use iced_runtime::user_interface::{Cache, UserInterface};
use std::time::{Duration, Instant};

struct Capture {
    renderer: iced::Renderer,
    dir: std::path::PathBuf,
    size: Size,
    daylight: bool,
    background: Color,
}

impl Capture {
    fn paint<M>(&mut self, name: &str, root: Element<'_, M>) {
        let mut ui = UserInterface::build(root, self.size, Cache::default(), &mut self.renderer);
        let theme = if self.daylight { iced::Theme::Light } else { iced::Theme::Dark };
        let fg = if self.daylight { Color::from_rgb8(24, 24, 27) } else { Color::from_rgb8(244, 244, 245) };
        ui.draw(
            &mut self.renderer,
            &theme,
            &iced::advanced::renderer::Style { text_color: fg },
            iced::mouse::Cursor::Unavailable,
        );
        let (w, h) = (self.size.width as u32, self.size.height as u32);
        let rgba = self
            .renderer
            .screenshot(&iced::advanced::graphics::Viewport::with_physical_size(Size::new(w, h), 1.0), self.background);
        assert!(rgba.chunks_exact(4).any(|p| p[0] != rgba[0] || p[1] != rgba[1]), "{name} is blank");
        image::RgbaImage::from_raw(w, h, rgba)
            .unwrap()
            .save(self.dir.join(format!("{name}-{w}-{h}-{}.png", if self.daylight { "day" } else { "night" })))
            .unwrap();
    }
}

#[test]
#[cfg_attr(feature = "ci", ignore = "requires a hardware GPU")]
fn all_interface_surfaces() {
    install_fonts();
    let temp = tempfile::tempdir().unwrap();
    let output = std::env::var_os("FOLIA_FONT_CAPTURE")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| temp.path().join("shots"));
    std::fs::create_dir_all(&output).unwrap();
    let i18n = I18n::new(Language::ZhCn).unwrap();
    let appearance = AppearanceSettings::default();
    let developer = Developer::default();
    let developer_settings = DeveloperSettings::default();
    let runtime_log = RuntimeLog::new(temp.path().join("runtime.log"));
    let memory_path = temp.path().join("memory.log");
    let sources = Sources::default();
    let omni =
        folia_omni::Omni::new(temp.path().join("audio"), format!("folia-font-regression-{}", std::process::id()))
            .unwrap();
    let (mut online, _) = Online::new(Some(omni.clone()), None);
    let account =
        folia_omni::Account { id: "font-fixture".into(), nickname: "Tech悠悠".into(), avatar_url: None, vip: false };
    let _ = online.update(OnlineMessage::Account("netease", Ok(Some(account.clone()))));
    for size in [Size::new(1200.0, 800.0), Size::new(1920.0, 1080.0)] {
        for (theme, daylight) in [(Theme::DAYLIGHT, true), (Theme::MIDNIGHT, false)] {
            let color = |rgb: folia_core::Rgb| Color::from_rgb8(rgb.r, rgb.g, rgb.b);
            let mut capture = Capture {
                renderer: pollster::block_on(<iced::Renderer as Headless>::new(
                    folia_ui::INTERFACE_FONT,
                    Pixels(16.0),
                    None,
                ))
                .unwrap(),
                dir: output.clone(),
                size,
                daylight,
                background: color(theme.background),
            };
            let tokens = Tokens::new(&theme, daylight);
            let now = Instant::now();
            let mut login = Login::new(now);
            let _ = login.open(Some("netease"), &online, None);
            capture.paint(
                "login",
                login
                    .view(
                        LoginLook {
                            i18n: &i18n,
                            tokens,
                            daylight,
                            window: size,
                            platforms: omni.platforms(),
                            active: "netease",
                            online: &online,
                            account: &AccountState::SignedIn(account.clone()),
                            avatar: None,
                            has_track: false,
                        },
                        now + Duration::from_secs(1),
                    )
                    .unwrap(),
            );
            let _ = login.update(LoginMessage::SignOut, None, &online, false);
            capture.paint(
                "confirm",
                login
                    .view(
                        LoginLook {
                            i18n: &i18n,
                            tokens,
                            daylight,
                            window: size,
                            platforms: omni.platforms(),
                            active: "netease",
                            online: &online,
                            account: &AccountState::SignedIn(account.clone()),
                            avatar: None,
                            has_track: false,
                        },
                        now + Duration::from_secs(1),
                    )
                    .unwrap(),
            );
            let mut settings = Settings::open();
            for (section, name) in [
                (SettingsSection::Appearance, "appearance"),
                (SettingsSection::Player, "player"),
                (SettingsSection::General, "general"),
                (SettingsSection::Graphics, "graphics"),
                (SettingsSection::Developer, "developer"),
                (SettingsSection::Sources, "sources"),
            ] {
                let _ = settings.update(SettingsMessage::Section(section));
                std::thread::sleep(Duration::from_millis(320));
                capture.paint(
                    &format!("settings-{name}"),
                    settings.view(
                        &i18n,
                        &theme,
                        daylight,
                        size,
                        SettingsValues {
                            sources: &sources,
                            language: LanguagePreference::System,
                            resolved: Language::ZhCn,
                            appearance: &appearance,
                            style: Some(folia_visualizer::Style::Classic),
                            bar_offset: 32.0,
                            bar_max: 200.0,
                            has_track: false,
                            developer: &developer,
                            developer_settings: &developer_settings,
                            runtime_log: &runtime_log,
                            memory_log_path: &memory_path,
                        },
                    ),
                );
            }
            let mut search = SearchPage::default();
            search.begin("Tech 悠悠".into());
            search.reveal();
            capture.paint(
                "search",
                search.view(
                    SearchView {
                        i18n: &i18n,
                        text: color(theme.primary),
                        secondary: color(theme.secondary),
                        background: color(theme.background),
                        accent: color(theme.accent),
                        daylight,
                        width: size.width,
                        source: "网易云",
                    },
                    Instant::now() + Duration::from_secs(1),
                ),
            );
            let home = HomeChrome::new(size.width);
            for spine in [false, true] {
                let width = if spine { 64.0 } else { 120.0 };
                capture.paint(
                    if spine { "home-spine" } else { "home" },
                    home_overlay(
                        &home,
                        HomeHost {
                            i18n: &i18n,
                            text: color(theme.primary),
                            background: color(theme.background),
                            accent: color(theme.accent),
                            daylight,
                            has_track: false,
                            busy: false,
                            explore_refresh: None,
                            volume: 1.0,
                            muted: false,
                            tokens,
                            queue: &[],
                            queue_index: None,
                            fm: false,
                            window_width: size.width,
                            window_height: size.height,
                            seam: Seam {
                                center: size.width * 0.5,
                                width,
                                collapsed: false,
                                before: false,
                                horizontal: false,
                            },
                            spine,
                            horizontal: false,
                            opacity: 1.0,
                            bottom: 32.0,
                            status: None,
                            show_tab: false,
                            tab_hidden: false,
                            tab: 0.0,
                            tabs: vec![
                                (HomeTab::Playlist, "歌单"),
                                (HomeTab::Radio, "电台"),
                                (HomeTab::Explore, "探索"),
                                (HomeTab::Albums, "专辑"),
                                (HomeTab::Local, "本地"),
                            ],
                            current: HomeTab::Playlist,
                            account: None,
                            layer: None,
                            poster_ground: None,
                            poster_tilted: false,
                            list_hover: None,
                            search: None,
                            modal: false,
                        },
                        Instant::now(),
                    ),
                );
            }
            let mut panel = PlayerPanel::new(Instant::now());
            let queue = [folia_core::Track {
                id: 424242,
                title: "字体对照 Font rendering".into(),
                artist: Some("Tech悠悠".into()),
                album: Some("夜航".into()),
                duration: 210.0,
                ..folia_core::Track::default()
            }];
            let room = || PanelRoom {
                width: size.width,
                height: size.height,
                bottom: 32.0,
                queue: &queue,
                index: Some(0),
                quality_levels: &AUDIO_QUALITY_LEVELS,
                quality: AudioQuality::High,
            };
            let _ = panel.update(&PanelMessage::Toggle, "classic", room());
            for (tab, name) in [
                (PanelTab::Cover, "cover"),
                (PanelTab::Controls, "controls"),
                (PanelTab::Queue, "queue"),
                (PanelTab::Quality, "account"),
                (PanelTab::Source, "source"),
            ] {
                let _ = panel.update(&PanelMessage::Choose(tab), "classic", room());
                let mut view = super::panelfix::panel_view(&i18n, &theme, &queue);
                view.track = Some(&queue[0]);
                view.index = Some(0);
                view.width = size.width;
                view.height = size.height;
                view.daylight = daylight;
                view.account = Some(&account);
                capture.paint(&format!("panel-{name}"), panel.view(view, Instant::now() + Duration::from_secs(1)));
            }
            let tiles: Vec<_> =
                ["字体对照 Font rendering", "Tech悠悠", "书库 歌单 电台 探索", "哪有什么不合适", "呓语", "晚风 Night"]
                    .iter()
                    .enumerate()
                    .map(|(index, title)| WallTile {
                        id: index as i64,
                        title: (*title).into(),
                        subtitle: "Tech悠悠 · 夜航".into(),
                        badge: Some("专辑".into()),
                        cover: WallCover::None,
                        heading: false,
                        hue: index as f32 * 50.0,
                        stack: true,
                    })
                    .collect();
            let mut wall = Wall::new();
            let _ = wall.update(WallMessage::Viewport(size), &tiles, None);
            let _ = wall.relayout(&tiles, WallShape::Blocks, None);
            std::thread::sleep(Duration::from_millis(700));
            capture.paint(
                "cover-wall",
                iced::widget::canvas(wall.view(&tiles, &theme, daylight, None, None))
                    .width(iced::Length::Fill)
                    .height(iced::Length::Fill)
                    .into(),
            );
            let mut sheets = Sheets::default();
            sheets.open_editor(None, daylight);
            capture.paint(
                "theme-dialog",
                sheets_view(&sheets, SheetsLook { i18n: &i18n, daylight, window: size, has_lyrics: false, tokens })
                    .unwrap(),
            );
            sheets.open_lyric_file();
            capture.paint(
                "lyric-file-dialog",
                sheets_view(&sheets, SheetsLook { i18n: &i18n, daylight, window: size, has_lyrics: false, tokens })
                    .unwrap(),
            );
            let mut timeline = Timeline::default();
            let now = Instant::now();
            timeline.open(None, None, 210.0, 0.0, size, now);
            capture.paint(
                "timeline-dialog",
                timeline.view(&i18n, &theme, daylight, size, None, now + Duration::from_secs(1)).unwrap(),
            );
            let mut workbench = Workbench::open();
            for (tab, name) in [
                (WorkbenchTab::Lyrics, "visualizer"),
                (WorkbenchTab::Background, "background"),
                (WorkbenchTab::Font, "common"),
                (WorkbenchTab::Subtitle, "subtitle"),
                (WorkbenchTab::Global, "global"),
            ] {
                let style = folia_visualizer::Style::Classic;
                let _ = workbench.update(WorkbenchMessage::Tab(tab), style.manifest(), &appearance);
                capture.paint(
                    &format!("workbench-{name}"),
                    workbench.view(WorkbenchLook {
                        i18n: &i18n,
                        theme: &theme,
                        daylight,
                        window: size,
                        style,
                        font_scale: 1.0,
                        live: None,
                        audio: &folia_core::AudioFrame::SILENT,
                        appearance: &appearance,
                        fonts: &[],
                    }),
                );
            }
            let mut dialog = PlaylistDialog::default();
            dialog.open();
            let choices =
                [PlaylistChoice { local: Some(1), online: None, name: "夜航".into(), detail: "12 首歌曲".into() }];
            capture.paint(
                "playlist-dialog",
                dialog
                    .view(PlaylistLook {
                        i18n: &i18n,
                        tokens,
                        daylight,
                        window: size,
                        playlists: &choices,
                        can_create: true,
                    })
                    .unwrap(),
            );
            capture.paint(
                "equalizer-dialog",
                equalizer_view(
                    &folia_core::EqualizerSettings::default(),
                    &EqualizerLook { i18n: &i18n, tokens, daylight, window: size },
                ),
            );
        }
    }
}
