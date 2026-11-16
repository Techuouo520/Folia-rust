// Shared graphics backend selection and native fullscreen frame policy.

use parking_lot::Mutex;
use std::sync::{Arc, Weak};
use wgpu::Backends;

/// An explicit environment value is authoritative, including an empty/invalid selection.
pub fn candidates(windows: bool, requested: Option<&str>) -> Vec<Backends> {
    match requested {
        Some(value) => vec![Backends::from_comma_list(value)],
        None if windows => vec![Backends::DX12, Backends::VULKAN],
        None => vec![Backends::PRIMARY],
    }
}

pub fn configured() -> Vec<Backends> {
    candidates(cfg!(windows), std::env::var("WGPU_BACKEND").ok().as_deref())
}

pub fn power_preference() -> wgpu::PowerPreference {
    wgpu::PowerPreference::from_env().unwrap_or(wgpu::PowerPreference::HighPerformance)
}

/// Uses the same ordered attempts as the window compositor without creating a window.
pub async fn device() -> Result<(wgpu::Adapter, wgpu::Device, wgpu::Queue), String> {
    let mut errors = Vec::new();
    for backends in configured() {
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor { backends, ..Default::default() });
        let adapter = match instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: power_preference(),
                ..Default::default()
            })
            .await
        {
            Ok(adapter) => adapter,
            Err(err) => {
                errors.push(format!("{backends:?}: {err}"));
                continue;
            }
        };
        match adapter.request_device(&wgpu::DeviceDescriptor::default()).await {
            Ok((device, queue)) => return Ok((adapter, device, queue)),
            Err(err) => errors.push(format!("{backends:?}: {err}")),
        }
    }
    Err(errors.join("; "))
}

// Only a weak native-window handle is shared; winit dispatches native changes to its event loop.
static WINDOW: Mutex<Weak<winit::window::Window>> = Mutex::new(Weak::new());

// Prepared before window creation and read when the compositor receives the native window.
#[cfg(windows)]
static TASKBAR_ICON: Mutex<Option<winit::window::Icon>> = Mutex::new(None);

/// Supplies the Windows taskbar icon before the compositor opens its first surface.
#[cfg(windows)]
pub fn prepare_taskbar_icon(icon: iced::window::Icon) -> Result<(), winit::window::BadIcon> {
    let (rgba, size) = icon.into_raw();
    let icon = winit::window::Icon::from_rgba(rgba, size.width, size.height)?;
    *TASKBAR_ICON.lock() = Some(icon);
    Ok(())
}

pub(crate) fn remember(window: &impl std::any::Any, direct_composition: bool) {
    #[cfg(not(windows))]
    let _ = direct_composition;
    if let Some(window) = (window as &dyn std::any::Any).downcast_ref::<Arc<winit::window::Window>>() {
        *WINDOW.lock() = Arc::downgrade(window);
        #[cfg(windows)]
        if let Err(err) = configure_native_frame(window, direct_composition) {
            tracing::error!(error = &err as &dyn std::error::Error, "native frame setup failed");
        }
        #[cfg(windows)]
        if let Some(icon) = TASKBAR_ICON.lock().clone() {
            use winit::platform::windows::WindowExtWindows;
            window.set_taskbar_icon(Some(icon));
        }
    }
}

#[cfg(windows)]
pub use crate::native_frame::Error as NativeFrameError;

/// Configures the non-client message policy for an owned frameless compositor window.
/// DirectComposition hosts also clear the legacy bitmap underneath their swapchain visual.
#[cfg(windows)]
pub fn configure_native_frame(
    window: &winit::window::Window,
    direct_composition: bool,
) -> Result<(), NativeFrameError> {
    if window.is_decorated() {
        return Ok(());
    }
    crate::native_frame::install(window, direct_composition)?;
    tracing::info!(window = ?window.id(), direct_composition, "frameless window procedure installed");
    Ok(())
}

/// Creates the alpha-capable frameless window used by the host compositor.
pub fn window_settings(size: iced::Size, minimum: iced::Size) -> iced::window::Settings {
    #[cfg(windows)]
    let platform_specific = iced::window::settings::PlatformSpecific {
        // The non-client shadow inset makes DWM replace transparent pixels with white.
        undecorated_shadow: false,
        corner_preference: iced::window::settings::platform::CornerPreference::Round,
        ..Default::default()
    };
    #[cfg(not(windows))]
    let platform_specific = iced::window::settings::PlatformSpecific::default();
    iced::window::Settings {
        size,
        min_size: Some(minimum),
        decorations: false,
        transparent: true,
        platform_specific,
        ..Default::default()
    }
}

/// Alpha-capable windows keep the non-client shadow disabled, including after fullscreen.
pub fn fullscreen_frame(fullscreen: bool) {
    #[cfg(windows)]
    if let Some(window) = WINDOW.lock().upgrade() {
        use winit::platform::windows::{CornerPreference, WindowExtWindows};
        window.set_undecorated_shadow(false);
        window.set_corner_preference(if fullscreen { CornerPreference::DoNotRound } else { CornerPreference::Round });
    }
    #[cfg(not(windows))]
    let _ = fullscreen;
}
