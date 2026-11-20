// Per-window desktop capabilities and physical monitor geometry for companion surfaces.

use iced::{Point, Rectangle, Size};
use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
use winit::window::{Window, WindowLevel};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Capabilities {
    pub position: bool,
    pub pin: bool,
    pub taskbar: bool,
    pub click_through: bool,
}

#[derive(Debug, Clone)]
pub struct Geometry {
    pub position: Option<Point>,
    pub size: Size,
    pub scale: f32,
    pub work_areas: Vec<Rectangle>,
    pub current_work_area: Option<Rectangle>,
    pub capabilities: Capabilities,
}

pub fn capabilities(window: &Window) -> Capabilities {
    let wayland = window.window_handle().is_ok_and(|handle| matches!(handle.as_raw(), RawWindowHandle::Wayland(_)));
    #[cfg(target_os = "linux")]
    let taskbar = !wayland
        && crate::x11_window::taskbar_supported(window).unwrap_or_else(|error| {
            tracing::warn!(%error,"cannot read X11 taskbar capabilities");
            false
        });
    #[cfg(not(target_os = "linux"))]
    let taskbar = cfg!(windows);
    Capabilities { position: !wayland, pin: !wayland, taskbar, click_through: true }
}

fn work_area(monitor: &winit::monitor::MonitorHandle) -> Rectangle {
    let position = monitor.position();
    let size = monitor.size();
    let bounds = Rectangle::new(
        Point::new(position.x as f32, position.y as f32),
        Size::new(size.width as f32, size.height as f32),
    );
    #[cfg(windows)]
    return crate::native_frame::monitor_work_area(bounds).unwrap_or(bounds);
    #[cfg(not(windows))]
    bounds
}

pub fn geometry(window: &Window) -> Geometry {
    let scale = window.scale_factor() as f32;
    let size = window.inner_size().to_logical::<f32>(f64::from(scale));
    let mut result = Geometry {
        position: window.outer_position().ok().map(|point| Point::new(point.x as f32, point.y as f32)),
        size: Size::new(size.width, size.height),
        scale,
        work_areas: window.available_monitors().map(|monitor| work_area(&monitor)).collect(),
        current_work_area: window.current_monitor().map(|monitor| work_area(&monitor)),
        capabilities: capabilities(window),
    };
    if !result.capabilities.position {
        result.current_work_area = None;
    }
    #[cfg(target_os = "linux")]
    if result.capabilities.position {
        match crate::x11_window::work_area(window) {
            Ok(Some(work)) => {
                for area in &mut result.work_areas {
                    *area = area.intersection(&work).unwrap_or(*area);
                }
                result.current_work_area =
                    result.current_work_area.map(|area| area.intersection(&work).unwrap_or(area));
            }
            Ok(None) => {}
            Err(error) => tracing::warn!(%error,"cannot read X11 desktop work area"),
        }
    }
    result
}

pub fn restore_geometry(window: &Window, position: Option<[i32; 2]>) -> Geometry {
    let mut geometry = geometry(window);
    if let Some([x, y]) = position {
        for monitor in window.available_monitors() {
            let area = work_area(&monitor);
            let scale = monitor.scale_factor() as f32;
            if area.contains(Point::new(x as f32 + 300.0 * scale, y as f32 + 80.0 * scale)) {
                geometry.scale = scale;
                geometry.current_work_area =
                    Some(geometry.work_areas.iter().find_map(|known| known.intersection(&area)).unwrap_or(area));
                break;
            }
        }
    }
    geometry
}

pub fn resize_limits(window: &Window, minimum: Size, maximum: Size) {
    window.set_min_inner_size(Some(winit::dpi::LogicalSize::new(minimum.width, minimum.height)));
    window.set_max_inner_size(Some(winit::dpi::LogicalSize::new(maximum.width, maximum.height)));
}

pub fn set_taskbar(window: &Window, visible: bool) -> Result<(), String> {
    #[cfg(windows)]
    {
        use winit::platform::windows::WindowExtWindows;
        window.set_skip_taskbar(!visible);
        Ok(())
    }
    #[cfg(target_os = "linux")]
    {
        crate::x11_window::set_taskbar(window, visible)
    }
    #[cfg(not(any(windows, target_os = "linux")))]
    {
        let _ = (window, visible);
        Err("per-window taskbar visibility is unavailable on this backend".to_owned())
    }
}

pub fn set_pin(window: &Window, pinned: bool) -> Result<(), String> {
    if !capabilities(window).pin {
        return Err("always-on-top is unavailable on native Wayland".to_owned());
    }
    window.set_window_level(if pinned { WindowLevel::AlwaysOnTop } else { WindowLevel::Normal });
    Ok(())
}

pub fn set_click_through(window: &Window, enabled: bool) -> Result<(), String> {
    window.set_cursor_hittest(!enabled).map_err(|error| error.to_string())
}

/// Clips the companion hit area on Windows and X11; click-through remains authoritative.
pub fn set_shape(window: &Window, regions: &[Rectangle], interactive: bool) -> Result<(), String> {
    #[cfg(windows)]
    {
        let _ = interactive;
        crate::native_frame::set_shape(window, regions).map_err(|error| error.to_string())
    }
    #[cfg(target_os = "linux")]
    {
        if window
            .window_handle()
            .is_ok_and(|handle| matches!(handle.as_raw(), RawWindowHandle::Xlib(_) | RawWindowHandle::Xcb(_)))
        {
            return crate::x11_window::set_shape(window, regions, interactive);
        }
        Ok(())
    }
    #[cfg(not(any(windows, target_os = "linux")))]
    {
        let _ = (window, regions, interactive);
        Ok(())
    }
}

/// Physical movement and logical sizing keep the same card size across monitor scales.
pub fn place(window: &Window, position: Option<Point>, size: Size) {
    if let Some(position) = position {
        window.set_outer_position(winit::dpi::PhysicalPosition::new(position.x as i32, position.y as i32));
    }
    let _ = window.request_inner_size(winit::dpi::LogicalSize::new(size.width, size.height));
}
