// Runs the real daemon on a private Windows desktop with isolated settings and silent playback.

#[cfg(windows)]
fn main() {
    windows::run();
}

#[cfg(not(windows))]
fn main() {
    eprintln!("This probe requires a private Windows desktop.");
    std::process::exit(1);
}

#[cfg(windows)]
mod windows {
    use std::sync::{Arc, Mutex};
    use std::time::{Duration, Instant};

    use folia_core::{QueueSnapshot, Repeat, Track};
    use folia_render::{backend, desktop_window};
    use folia_store::{MemoryMonitor, PlaybackSession, PlaybackSettings, Store};
    use folia_ui::{Init, Message as App, PlaybackMessage, Shell, remote::Message as Remote};
    use iced::{Subscription, Task, window};
    use serde_json::{Value, json};
    use windows_sys::Win32::System::StationsAndDesktops::{
        CloseDesktop, CreateDesktopW, DESKTOP_CREATEMENU, DESKTOP_CREATEWINDOW, DESKTOP_READOBJECTS,
        DESKTOP_WRITEOBJECTS, SetThreadDesktop,
    };

    #[derive(Debug, Clone)]
    enum Message {
        App(App),
        Tick,
        Latest(Option<window::Id>),
        Raw(window::Id, u64),
        Shot(window::Screenshot),
    }

    struct Probe {
        shell: Box<Shell>,
        tick: u32,
        main: window::Id,
        main_raw: Option<u64>,
        window_only: bool,
        remote: Option<(window::Id, u64)>,
        closed_raw: Option<u64>,
        monitor: MemoryMonitor,
        started: Instant,
        last_frame: Option<Instant>,
        intervals: Vec<f64>,
        reports: Arc<Mutex<Vec<Value>>>,
        shots: Arc<Mutex<Vec<window::Screenshot>>>,
        card_y: f32,
    }

    impl Probe {
        fn action(&mut self, message: App) -> Task<Message> {
            self.shell.update(message).map(Message::App)
        }

        fn remote(&mut self, message: Remote) -> Task<Message> {
            self.action(App::Remote(message))
        }

        fn update(&mut self, message: Message) -> Task<Message> {
            match message {
                Message::App(message) => {
                    if let App::WindowReady(Some(id)) = &message {
                        self.main = *id;
                    }
                    if matches!(message, App::VisualFrame | App::UiFrame) {
                        let now = Instant::now();
                        if let Some(previous) = self.last_frame {
                            self.intervals.push(now.duration_since(previous).as_secs_f64() * 1000.0);
                        }
                        self.last_frame = Some(now);
                    }
                    self.action(message)
                }
                Message::Latest(Some(id)) => {
                    assert_ne!(id, self.main, "remote window was never created");
                    window::raw_id::<Message>(id).map(move |raw| Message::Raw(id, raw))
                }
                Message::Latest(None) => panic!("daemon lost all windows"),
                Message::Raw(id, raw) => {
                    if id == self.main {
                        self.main_raw = Some(raw);
                    } else {
                        self.remote = Some((id, raw));
                    }
                    Task::none()
                }
                Message::Shot(shot) => {
                    self.shots.lock().unwrap().push(shot);
                    Task::none()
                }
                Message::Tick => {
                    self.tick += 1;
                    self.step()
                }
            }
        }

        fn measure(&mut self, name: &str) {
            let history = self.monitor.history();
            let points: Vec<_> = history.points.iter().skip(1).collect();
            assert!(!points.is_empty());
            self.intervals.sort_by(f64::total_cmp);
            let latest = points.last().unwrap();
            let rate = self.intervals.len() as f64 / self.started.elapsed().as_secs_f64();
            if name == "main-minimized-playing" {
                assert!(rate < 2.0, "hidden windows retained a fast frame clock: {rate}");
            } else {
                assert!((45.0..=65.0).contains(&rate), "visible remote lost the shared 60fps clock: {rate}");
            }
            let report = json!({"phase":name,"cpu_percent_all_cores":points.iter().map(|p|p.cpu_percent).sum::<f32>()/points.len() as f32,
                "working_set_mb":latest.working_set_mb,"private_mb":latest.private_mb,"frame_updates_per_second":self.intervals.len() as f64/self.started.elapsed().as_secs_f64(),
                "median_interval_ms":self.intervals.get(self.intervals.len()/2),"p95_interval_ms":self.intervals.get(self.intervals.len()*95/100)});
            eprintln!("{report}");
            self.reports.lock().unwrap().push(report);
        }

        fn reset_measurement(&mut self, name: &str) {
            self.monitor.start(1000);
            self.started = Instant::now();
            self.last_frame = None;
            self.intervals.clear();
            eprintln!("MEASURE {name} pid={}", std::process::id());
        }

        fn step(&mut self) -> Task<Message> {
            match self.tick {
                1 => {
                    assert!(self.shell.title().contains("Remote probe"));
                    let main = self.main;
                    Task::batch([
                        self.remote(Remote::Open),
                        window::raw_id::<Message>(main).map(move |raw| Message::Raw(main, raw)),
                    ])
                }
                2 | 27 | 34 => window::latest().map(Message::Latest),
                3 => {
                    if self.window_only {
                        self.tick = 26;
                    }
                    Task::batch([
                        window::minimize(self.main, true),
                        self.action(App::Playback(PlaybackMessage::Toggle)),
                    ])
                }
                4 => {
                    self.reset_measurement("remote-playing");
                    Task::none()
                }
                15 => {
                    self.measure("remote-playing");
                    self.closed_raw = self.remote.map(|(_, raw)| raw);
                    self.remote(Remote::Close)
                }
                16 => {
                    assert!(backend::native_window(self.closed_raw.unwrap()).is_none(), "remote surface leaked");
                    self.reset_measurement("main-minimized-playing");
                    Task::none()
                }
                26 => {
                    self.measure("main-minimized-playing");
                    self.remote(Remote::Open)
                }
                28 => Task::batch([self.remote(Remote::ClickThrough(true)), self.remote(Remote::Taskbar(true))]),
                29 => {
                    self.assert_through(true);
                    Task::batch([self.remote(Remote::Unlock), self.remote(Remote::Taskbar(false))])
                }
                30 => {
                    self.assert_through(false);
                    let (_, raw) = self.remote.unwrap();
                    let native = backend::native_window(raw).unwrap();
                    let geometry = desktop_window::geometry(&native);
                    let area = geometry.current_work_area.unwrap();
                    self.card_y = area.y + area.height - geometry.size.height * geometry.scale - 8.0;
                    desktop_window::place(&native, Some(iced::Point::new(area.x + 20.0, self.card_y)), geometry.size);
                    self.remote(Remote::Queue)
                }
                31 => {
                    let native = backend::native_window(self.remote.unwrap().1).unwrap();
                    let geometry = desktop_window::geometry(&native);
                    assert!(geometry.size.height > 184.0);
                    assert!(geometry.position.unwrap().y < self.card_y, "queue failed to expand upwards");
                    self.remote(Remote::Queue)
                }
                32 => {
                    self.closed_raw = self.remote.map(|(_, raw)| raw);
                    self.remote(Remote::Close)
                }
                33 => {
                    assert!(backend::native_window(self.closed_raw.unwrap()).is_none());
                    self.remote(Remote::Open)
                }
                35 => Task::batch([window::minimize(self.main, false), self.action(App::ToggleFullscreen)]),
                36 => self.remote(Remote::MainPin(true)),
                37 => self.action(App::ToggleFullscreen),
                38 => {
                    let main = backend::native_window(self.main_raw.unwrap()).unwrap();
                    assert!(main.fullscreen().is_none());
                    assert!(main.is_resizable(), "remote settings changed fullscreen restoration");
                    window::minimize(self.main, true)
                }
                39 => {
                    let (id, _) = self.remote.unwrap();
                    self.remote(Remote::Event(
                        id,
                        iced::Event::Mouse(iced::mouse::Event::CursorMoved { position: iced::Point::new(300.0, 60.0) }),
                        iced::event::Status::Ignored,
                    ))
                }
                40 => window::screenshot(self.remote.unwrap().0).map(Message::Shot),
                41 => self.remote(Remote::Background),
                42 => window::screenshot(self.remote.unwrap().0).map(Message::Shot),
                43 => self.remote(Remote::Background),
                44 => window::screenshot(self.remote.unwrap().0).map(Message::Shot),
                45 => {
                    let id = self.remote.unwrap().0;
                    let volume =
                        self.action(App::RemoteInput(id, Box::new(App::Playback(PlaybackMessage::Volume(0.0)))));
                    Task::batch([volume, self.action(App::WindowClosing)])
                }
                50.. => panic!("daemon did not exit after flushing settings and playback"),
                _ => Task::none(),
            }
        }

        fn assert_through(&self, enabled: bool) {
            use windows_sys::Win32::UI::WindowsAndMessaging::{GWL_EXSTYLE, GetWindowLongPtrW, WS_EX_TRANSPARENT};
            use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
            let native = backend::native_window(self.remote.unwrap().1).unwrap();
            let RawWindowHandle::Win32(handle) = native.window_handle().unwrap().as_raw() else {
                panic!("expected Win32")
            };
            // SAFETY: Reads one style word from the live private-desktop companion window.
            let style = unsafe { GetWindowLongPtrW(handle.hwnd.get() as _, GWL_EXSTYLE) } as u32;
            assert_eq!(style & WS_EX_TRANSPARENT != 0, enabled);
        }

        fn subscription(&self) -> Subscription<Message> {
            Subscription::batch([
                self.shell.subscription().map(Message::App),
                iced::time::every(Duration::from_secs(1)).map(|_| Message::Tick),
            ])
        }
    }

    fn view(probe: &Probe, id: window::Id) -> iced::Element<'_, Message, iced::Theme, folia_render::HostRenderer> {
        folia_render::host(probe.shell.view_window(id).map(Message::App))
    }

    pub fn run() {
        let name: Vec<u16> = format!("folia-remote-probe-{}\0", std::process::id()).encode_utf16().collect();
        // SAFETY: This process has not created any windows; its desktop is never switched to the user's display.
        let desktop = unsafe {
            CreateDesktopW(
                name.as_ptr(),
                std::ptr::null(),
                std::ptr::null(),
                0,
                DESKTOP_CREATEWINDOW | DESKTOP_CREATEMENU | DESKTOP_READOBJECTS | DESKTOP_WRITEOBJECTS,
                std::ptr::null(),
            )
        };
        assert!(!desktop.is_null());
        // SAFETY: The main thread has no GUI objects and owns the live private desktop handle.
        assert_ne!(unsafe { SetThreadDesktop(desktop) }, 0);
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path();
        let store = Store::at(path.join("config"));
        let audio = path.join("probe.flac");
        std::fs::write(&audio, include_bytes!("../player/fixtures/seek.flac")).unwrap();
        std::fs::write(path.join("cover.jpg"), include_bytes!("../../assets/preview/placeholder_cover.jpg")).unwrap();
        std::fs::write(
            audio.with_extension("lrc"),
            "[00:00.00]原生遥控器逐字与窗口回归\n[00:01.00]Shared playback, one queue\n",
        )
        .unwrap();
        store
            .save(&PlaybackSession {
                queue: QueueSnapshot {
                    items: (1..=12)
                        .map(|id| Track {
                            id,
                            title: format!("Remote probe {id}"),
                            artist: Some("Folia".into()),
                            path: audio.clone(),
                            duration: 2.0,
                            ..Default::default()
                        })
                        .collect(),
                    current: Some(0),
                    ..Default::default()
                },
                ..Default::default()
            })
            .unwrap();
        let init = Init {
            storage: Default::default(),
            store: store.clone(),
            general: Default::default(),
            graphics: folia_store::GraphicsSettings { background_throttling: true, ..Default::default() },
            scene: Default::default(),
            system_reduced_motion: false,
            analysis_models: path.join("models"),
            appearance: folia_store::AppearanceSettings { theme: folia_store::ThemeMode::Light, ..Default::default() },
            theme: Default::default(),
            audio: Default::default(),
            playback: PlaybackSettings { volume: 0.0, repeat: Repeat::One, ..Default::default() },
            player: Default::default(),
            visualizer: Default::default(),
            developer: Default::default(),
            runtime_log: folia_store::RuntimeLog::new(path.join("runtime.log")),
            i18n: folia_i18n::I18n::new(folia_core::Language::ZhCn).unwrap(),
            system_locale: None,
            library_db: path.join("library.db"),
            cache_dir: path.join("cache"),
            data_dir: path.join("data"),
            keyring_service: format!("folia-private-remote-probe-{}", std::process::id()),
            open_files: vec![],
        };
        folia_ui::install_fonts();
        let reports = Arc::new(Mutex::new(Vec::new()));
        let collected = reports.clone();
        let shots = Arc::new(Mutex::new(Vec::new()));
        let images = shots.clone();
        let monitor_path = path.join("memory.log");
        let window_only = std::env::var_os("FOLIA_REMOTE_WINDOW_ONLY").is_some();
        iced::daemon(
            move || {
                let mut settings = backend::window_settings(iced::Size::new(1200.0, 800.0), folia_ui::MIN_WINDOW_SIZE);
                settings.exit_on_close_request = false;
                let (shell, initial) = Shell::boot(init.clone(), settings);
                let main = window::Id::unique();
                (
                    Probe {
                        shell: Box::new(shell),
                        tick: 0,
                        main,
                        main_raw: None,
                        window_only,
                        remote: None,
                        closed_raw: None,
                        monitor: MemoryMonitor::new(monitor_path.clone()),
                        started: Instant::now(),
                        last_frame: None,
                        intervals: Vec::new(),
                        reports: collected.clone(),
                        shots: images.clone(),
                        card_y: 0.0,
                    },
                    initial.map(Message::App),
                )
            },
            Probe::update,
            view,
        )
        .title(|probe: &Probe, id| probe.shell.title_for(id))
        .theme(|probe: &Probe, _| probe.shell.theme())
        .style(|probe: &Probe, theme| iced::theme::Style {
            background_color: iced::Color::TRANSPARENT,
            ..probe.shell.window_style(theme)
        })
        .subscription(Probe::subscription)
        .run()
        .unwrap();
        assert_eq!(reports.lock().unwrap().len(), if window_only { 0 } else { 2 });
        let screenshots = shots.lock().unwrap();
        assert_eq!(screenshots.len(), 3);
        assert_eq!(screenshots[0].rgba[(100 * 602 + 300) * 4 + 3], 255);
        assert!(
            (180..230).contains(&screenshots[2].rgba[(100 * 602 + 300) * 4 + 3]),
            "transparent companion lost intermediate alpha"
        );
        if let Some(directory) = std::env::args_os().nth(1) {
            let directory = std::path::PathBuf::from(directory);
            std::fs::create_dir_all(&directory).unwrap();
            std::fs::write(
                directory.join("metrics.json"),
                serde_json::to_vec_pretty(&*reports.lock().unwrap()).unwrap(),
            )
            .unwrap();
            for (index, shot) in screenshots.iter().enumerate() {
                image::save_buffer(
                    directory.join(format!("remote-{index}.png")),
                    &shot.rgba,
                    shot.size.width,
                    shot.size.height,
                    image::ColorType::Rgba8,
                )
                .unwrap();
            }
        }
        assert!(
            store.load::<folia_store::RemoteSettings>().unwrap().appearance.background
                == folia_store::RemoteBackground::Transparent
        );
        assert_eq!(store.load::<PlaybackSettings>().unwrap().volume, 0.0, "close lost the last remote volume change");
        // SAFETY: The event loop has ended. Windows releases any remaining helper-thread references at process exit.
        if unsafe { CloseDesktop(desktop) } == 0 {
            eprintln!("desktop cleanup deferred to process exit");
        }
        eprintln!("REMOTE_PROBE_OK");
    }
}
