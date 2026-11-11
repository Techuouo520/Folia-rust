// Folia entry point: sets up logging, settings and text, then hands control to the UI.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use folia_i18n::I18n;
use folia_store::{
    AppDirs, AppearanceSettings, DeveloperSettings, GeneralSettings, PlaybackSettings, PlayerSettings, RuntimeLog,
    Store,
};
use folia_ui::{Init, Shell};
use tracing::{Event, Subscriber};
use tracing_subscriber::EnvFilter;
use tracing_subscriber::layer::{Context, Layer, SubscriberExt};
use tracing_subscriber::util::SubscriberInitExt;

// Debug builds keep their data apart from release ones, and both stay apart from the Electron app's "Folia" folder.
const APP_DIR: &str = if cfg!(debug_assertions) { "Folia-rust-dev" } else { "Folia-rust" };
const DEFAULT_LOG: &str =
    if cfg!(debug_assertions) { "debug,wgpu=warn,naga=warn,iced=info,cosmic_text=warn" } else { "info" };

fn window_settings() -> anyhow::Result<iced::window::Settings> {
    let image =
        image::load_from_memory_with_format(include_bytes!("../../../assets/icon.png"), image::ImageFormat::Png)?
            .into_rgba8();
    let (width, height) = image.dimensions();
    let icon = iced::window::icon::from_rgba(image.into_raw(), width, height)?;
    Ok(iced::window::Settings {
        size: folia_ui::INITIAL_WINDOW,
        min_size: Some(folia_ui::MIN_WINDOW_SIZE),
        icon: Some(icon),
        decorations: false,
        platform_specific: platform_window(),
        ..Default::default()
    })
}

#[cfg(target_os = "windows")]
fn platform_window() -> iced::window::settings::PlatformSpecific {
    iced::window::settings::PlatformSpecific {
        undecorated_shadow: true,
        corner_preference: iced::window::settings::platform::CornerPreference::Round,
        ..Default::default()
    }
}

#[cfg(not(target_os = "windows"))]
fn platform_window() -> iced::window::settings::PlatformSpecific {
    iced::window::settings::PlatformSpecific::default()
}

/// Feeds every event to the in-memory session log and, when its switch is on, the file.
struct SessionLines {
    log: RuntimeLog,
}

impl<S: Subscriber> Layer<S> for SessionLines {
    fn on_event(&self, event: &Event<'_>, _context: Context<'_, S>) {
        self.log.record_event(event);
    }
}

// Console output, the session log in memory, and a file only while the developer page asks for one.
fn init_logging(dirs: &AppDirs, developer: &DeveloperSettings) -> RuntimeLog {
    let filter = EnvFilter::try_from_env("FOLIA_LOG").unwrap_or_else(|_| EnvFilter::new(DEFAULT_LOG));
    let path = dirs.data.join("logs").join("runtime").join("folia.log");
    let log = RuntimeLog::new(path.clone());
    log.configure(developer.console_capture_enabled, developer.runtime_log_enabled, developer.runtime_log_mode);
    let session = SessionLines { log: log.clone() };
    tracing_subscriber::registry().with(filter).with(tracing_subscriber::fmt::layer()).with(session).init();
    if developer.runtime_log_enabled {
        tracing::info!(path = %path.display(), mode = ?developer.runtime_log_mode, "runtime log file on");
    }
    let hook = log.clone();
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let backtrace = std::backtrace::Backtrace::force_capture();
        tracing::error!(%info, %backtrace, "panic");
        if hook.file_enabled() {
            hook.sync();
        }
        previous(info);
    }));
    log
}

fn main() -> anyhow::Result<()> {
    let dirs = AppDirs::new(APP_DIR)?;
    let store = Store::at(&dirs.config);
    let developer: DeveloperSettings = store.load_or_default();
    let log = init_logging(&dirs, &developer);
    let general: GeneralSettings = store.load_or_default();
    let system_locale = sys_locale::get_locale();
    let i18n = I18n::new(general.language.resolve(system_locale.as_deref()))?;
    tracing::info!(config = %store.dir().display(), language = i18n.language().code(), "starting");
    folia_ui::install_fonts();
    let graphics_failure = i18n.t("status.graphicsInitializationFailed").to_owned();

    let init = Init {
        appearance: store.load_or_default::<AppearanceSettings>(),
        theme: store.load_or_default::<folia_store::ThemeSettings>(),
        audio: store.load_or_default::<folia_store::AudioSettings>(),
        playback: store.load_or_default::<PlaybackSettings>(),
        player: store.load_or_default::<PlayerSettings>(),
        visualizer: store.load_or_default::<folia_store::VisualizerSettings>(),
        developer,
        runtime_log: log.clone(),
        library_db: dirs.data.join("library.db"),
        cache_dir: dirs.cache.clone(),
        data_dir: dirs.data.clone(),
        keyring_service: APP_DIR.to_owned(),
        open_files: std::env::args_os().skip(1).map(std::path::PathBuf::from).collect(),
        store,
        general,
        i18n,
        system_locale,
    };
    let interface_font = folia_ui::interface_font_for(init.i18n.language());
    let result = iced::application(move || Shell::new(init.clone()), Shell::update, host_view)
        .default_font(interface_font)
        .title(Shell::title)
        .theme(Shell::theme)
        .subscription(Shell::subscription)
        .window(window_settings()?)
        .run();
    if let Err(err) = result {
        tracing::error!(error = &err as &dyn std::error::Error, "application initialization failed");
        log.sync();
        rfd::MessageDialog::new()
            .set_level(rfd::MessageLevel::Error)
            .set_title("Folia")
            .set_description(format!("{graphics_failure}\n\n{err}"))
            .show();
        return Err(err.into());
    }
    log.sync();
    Ok(())
}

fn host_view(shell: &Shell) -> iced::Element<'_, folia_ui::Message, iced::Theme, folia_render::HostRenderer> {
    folia_render::host(shell.view())
}
