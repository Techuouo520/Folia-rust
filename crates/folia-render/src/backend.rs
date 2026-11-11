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

pub(crate) fn remember(window: &impl std::any::Any) {
    if let Some(window) = (window as &dyn std::any::Any).downcast_ref::<Arc<winit::window::Window>>() {
        *WINDOW.lock() = Arc::downgrade(window);
    }
}

/// No DWM glass margins or rounded corners in fullscreen; restore the original windowed frame.
pub fn fullscreen_frame(fullscreen: bool) {
    #[cfg(windows)]
    if let Some(window) = WINDOW.lock().upgrade() {
        use winit::platform::windows::{CornerPreference, WindowExtWindows};
        window.set_undecorated_shadow(!fullscreen);
        window.set_corner_preference(if fullscreen { CornerPreference::DoNotRound } else { CornerPreference::Round });
    }
    #[cfg(not(windows))]
    let _ = fullscreen;
}
