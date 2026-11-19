// Shared isolated shell fixtures for full-player GPU integration checks.
use folia_ui::{Init, Message, Shell};
use iced::futures::StreamExt;

pub(super) fn complete(shell: &mut Shell, task: iced::Task<Message>) {
    let messages = pollster::block_on(async {
        let mut messages = Vec::new();
        if let Some(mut stream) = iced_runtime::task::into_stream(task) {
            while let Some(action) = stream.next().await {
                if let iced_runtime::Action::Output(message) = action {
                    messages.push(message);
                }
            }
        }
        messages
    });
    for message in messages {
        drop(shell.update(message));
    }
}

pub(super) fn init(path: &std::path::Path) -> Init {
    Init {
        storage: Default::default(),
        store: folia_store::Store::at(path.join("config")),
        general: Default::default(),
        graphics: Default::default(),
        scene: Default::default(),
        system_reduced_motion: false,
        analysis_models: path.join("models"),
        appearance: folia_store::AppearanceSettings { theme: folia_store::ThemeMode::Light, ..Default::default() },
        theme: Default::default(),
        audio: Default::default(),
        playback: Default::default(),
        player: Default::default(),
        visualizer: Default::default(),
        developer: Default::default(),
        runtime_log: folia_store::RuntimeLog::new(path.join("runtime.log")),
        i18n: folia_i18n::I18n::new(folia_core::Language::ZhCn).unwrap(),
        system_locale: None,
        library_db: path.join("library.db"),
        cache_dir: path.join("cache"),
        data_dir: path.join("data"),
        keyring_service: format!("folia-transparent-test-{}", path.display()),
        open_files: Vec::new(),
    }
}

// Returns true after the parent has executed and verified the isolated GPU test.
pub(super) fn run_gpu_child(name: &str) -> bool {
    if std::env::var("FOLIA_UI_GPU_CASE").as_deref() == Ok(name) {
        return false;
    }
    let output = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", name, "--nocapture", "--include-ignored"])
        .env("FOLIA_UI_GPU_CASE", name)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    true
}
