// Applies native graphics preferences and reads the platform accessibility preference off the UI thread.
use folia_store::{GpuPreference, MotionSurface};
use iced::Task;

#[derive(Debug, Clone)]
pub enum Message {
    Static(bool),
    NativeBlur(bool),
    BackgroundThrottling(bool),
    Gpu(GpuPreference),
    FollowSystem(bool),
    Motion(MotionSurface, bool),
    RenderScale(f32),
    Antialias(folia_core::LyricAntialias),
    Poll,
    SystemPreference(Result<bool, String>),
}

impl crate::Shell {
    pub(crate) fn sync_native_blur(&mut self) {
        let desired = self.page == crate::Page::NowPlaying
            && self.graphics.enable_player_page_native_blur
            && !self.theme_settings.transparent_player_background;
        if self.window.is_none() || desired == self.native_blur_applied || self.native_blur_failed {
            return;
        }
        match folia_render::backend::set_native_blur(desired) {
            Ok(()) => self.native_blur_applied = desired,
            Err(error) => {
                tracing::error!(error = &error as &dyn std::error::Error, "native player blur could not be applied");
                self.native_blur_failed = true;
            }
        }
    }

    pub(crate) fn monet_background_tuning(&self) -> folia_visualizer::monet_background::Tuning {
        let mut tuning =
            folia_visualizer::monet_background::Tuning::read(self.visualizer.tables.get("monet_background_tuning"));
        if self.graphics.motion.reduced(MotionSurface::MonetBackground, self.system_reduced_motion) {
            tuning.drift = false;
        }
        tuning
    }

    pub(crate) fn graphics_message(&mut self, message: Message) -> Task<crate::Message> {
        let before = self.graphics.clone();
        match message {
            Message::Static(value) => self.graphics.static_mode = value,
            Message::NativeBlur(value) => {
                self.graphics.enable_player_page_native_blur = value;
                self.native_blur_failed = false;
                if let Some(settings) = &mut self.settings {
                    settings.dismiss_native_blur();
                }
            }
            Message::BackgroundThrottling(value) => self.graphics.background_throttling = value,
            Message::Gpu(value) => self.graphics.gpu_preference = value,
            Message::FollowSystem(value) => self.graphics.motion.follow_system = value,
            Message::Motion(surface, value) => self.graphics.motion.set(surface, value),
            Message::RenderScale(value) => {
                self.graphics.lyric_render_quality.scale = value;
                self.graphics.lyric_render_quality = self.graphics.lyric_render_quality.normalized();
            }
            Message::Antialias(value) => self.graphics.lyric_render_quality.antialias = value,
            Message::Poll => return poll(),
            Message::SystemPreference(result) => {
                match result {
                    Ok(value) => {
                        self.system_reduced_motion = value;
                        self.system_motion_error = None;
                    }
                    Err(error) if self.system_motion_error.as_ref() != Some(&error) => {
                        tracing::warn!(%error, "cannot read system animation preference");
                        self.system_motion_error = Some(error);
                    }
                    Err(_) => {}
                }
                crate::motion::configure(&self.graphics.motion, self.system_reduced_motion);
                return Task::none();
            }
        }
        crate::motion::configure(&self.graphics.motion, self.system_reduced_motion);
        let persist = crate::background::save(&self.store, &self.graphics);
        if let Some(settings) = &mut self.settings {
            settings.graphics_changed(&before, &self.graphics);
        }
        if self.graphics.motion.follow_system { Task::batch([persist, poll()]) } else { persist }
    }
}

pub fn power_preference(preference: GpuPreference) -> folia_render::backend::PowerPreference {
    match preference {
        GpuPreference::Auto => folia_render::backend::PowerPreference::None,
        GpuPreference::HighPerformance => folia_render::backend::PowerPreference::HighPerformance,
        GpuPreference::LowPower => folia_render::backend::PowerPreference::LowPower,
    }
}

pub fn poll() -> Task<crate::Message> {
    Task::perform(crate::background::blocking(system_preference), |result| {
        crate::Message::Graphics(Message::SystemPreference(
            result.unwrap_or_else(|| Err("system animation preference worker stopped".to_owned())),
        ))
    })
}

pub fn native_blur_question<'a>(
    i18n: &'a folia_i18n::I18n,
    tokens: crate::settings_kit::Tokens,
    window: iced::Size,
) -> iced::Element<'a, crate::Message> {
    use crate::settings_kit::sheet_button;
    use crate::typeface::text;
    use iced::widget::{column, container, mouse_area, opaque, row, space, stack};
    use iced::{Color, Length};
    let cancel = crate::Message::Settings(crate::settings::Message::CancelNativeBlur);
    let veil = container(space()).width(Length::Fill).height(Length::Fill).style(|_| container::Style {
        background: Some(Color::from_rgba(0.0, 0.0, 0.0, 0.5).into()),
        ..Default::default()
    });
    let footer = row![
        space().width(Length::Fill),
        sheet_button(tokens, i18n.t("ui.cancel"), false, Some(cancel.clone())),
        sheet_button(
            tokens,
            i18n.t("options.nativeBlurConfirmAction"),
            true,
            Some(crate::Message::Graphics(Message::NativeBlur(true))),
        ),
    ]
    .spacing(8);
    let body = column![
        text(i18n.t("options.nativeBlurConfirmTitle")).size(crate::typeface::size::DIALOG_TITLE).color(tokens.t1),
        text(i18n.t("options.nativeBlurConfirmDesc")).size(crate::typeface::size::CONTROL).color(tokens.t2),
        footer,
    ]
    .spacing(16);
    let panel = container(body)
        .padding(24)
        .width(440.0_f32.min(window.width - 32.0))
        .style(move |_| crate::settings_kit::rounded(tokens.win, 16.0));
    stack![opaque(mouse_area(veil).on_press(cancel)), container(opaque(panel)).center(Length::Fill)].into()
}

#[cfg(windows)]
pub fn system_preference() -> Result<bool, String> {
    folia_render::backend::system_reduced_motion().map_err(|error| error.to_string())
}

#[cfg(target_os = "macos")]
pub fn system_preference() -> Result<bool, String> {
    read_preference("defaults", &["read", "com.apple.universalaccess", "reduceMotion"], "1")
}

#[cfg(target_os = "linux")]
pub fn system_preference() -> Result<bool, String> {
    read_preference("gsettings", &["get", "org.gnome.desktop.interface", "enable-animations"], "false")
}

#[cfg(not(any(windows, target_os = "macos", target_os = "linux")))]
pub fn system_preference() -> Result<bool, String> {
    Err("system animation preference is unavailable on this platform".to_owned())
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
fn read_preference(program: &str, args: &[&str], reduced: &str) -> Result<bool, String> {
    let output = std::process::Command::new(program).args(args).output().map_err(|error| error.to_string())?;
    if !output.status.success() {
        return Err(format!("{program} could not read the system animation preference ({})", output.status));
    }
    let value = String::from_utf8_lossy(&output.stdout);
    let value = value.trim();
    if value == reduced {
        return Ok(true);
    }
    let animated = if reduced == "false" { "true" } else { "0" };
    if value == animated {
        return Ok(false);
    }
    Err(format!("{program} returned an invalid system animation preference"))
}
