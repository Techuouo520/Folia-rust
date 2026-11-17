// Exercises both non-client size messages on a real hidden window before and after host setup.
use std::sync::Arc;
use std::time::Duration;
use windows_sys::Win32::Foundation::{HWND, RECT};
use windows_sys::Win32::Graphics::Gdi::{
    BI_RGB, BITMAPINFO, BITMAPINFOHEADER, CreateCompatibleDC, CreateDIBSection, DIB_RGB_COLORS, DeleteDC, DeleteObject,
    GdiFlush, SelectObject,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    GWL_STYLE, GetClientRect, GetTitleBarInfo, GetWindowLongPtrW, NCCALCSIZE_PARAMS, SendMessageW, TITLEBARINFO,
    WINDOWPOS, WM_ERASEBKGND, WM_NCACTIVATE, WM_NCCALCSIZE, WS_CAPTION, WS_MAXIMIZEBOX, WS_MINIMIZEBOX, WS_SYSMENU,
    WS_THICKFRAME,
};
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::platform::pump_events::EventLoopExtPumpEvents;
use winit::platform::windows::{EventLoopBuilderExtWindows, WindowAttributesExtWindows, WindowExtWindows};
use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
use winit::window::{Theme, Window, WindowId};

const STATE_SYSTEM_INVISIBLE: u32 = 0x8000;

#[derive(Default)]
struct Fixture {
    window: Option<Arc<Window>>,
}

impl ApplicationHandler for Fixture {
    fn resumed(&mut self, events: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        let settings =
            folia_render::backend::window_settings(iced::Size::new(600.0, 400.0), iced::Size::new(350.0, 100.0));
        self.window = Some(Arc::new(
            events
                .create_window(
                    Window::default_attributes()
                        .with_visible(false)
                        .with_decorations(settings.decorations)
                        .with_transparent(settings.transparent)
                        .with_undecorated_shadow(settings.platform_specific.undecorated_shadow)
                        .with_inner_size(winit::dpi::LogicalSize::new(600.0, 400.0)),
                )
                .unwrap(),
        ));
    }
    fn window_event(&mut self, _: &ActiveEventLoop, _: WindowId, _: WindowEvent) {}
}

fn client_rect(window: &Window, extended: bool) -> [i32; 4] {
    let RawWindowHandle::Win32(handle) = window.window_handle().unwrap().as_raw() else {
        panic!("expected Win32 handle")
    };
    let hwnd = handle.hwnd.get() as HWND;
    let mut rect = RECT { left: 100, top: 100, right: 700, bottom: 500 };
    let mut position = WINDOWPOS { hwnd, x: 100, y: 100, cx: 600, cy: 400, ..Default::default() };
    let mut params = NCCALCSIZE_PARAMS { rgrc: [rect; 3], lppos: &mut position };
    let pointer =
        if extended { (&mut params as *mut NCCALCSIZE_PARAMS) as isize } else { (&mut rect as *mut RECT) as isize };
    // SAFETY: The owning test thread sends a synchronous message with live stack-backed Win32 parameters.
    unsafe {
        SendMessageW(hwnd, WM_NCCALCSIZE, usize::from(extended), pointer);
    }
    let rect = if extended { params.rgrc[0] } else { rect };
    [rect.left, rect.top, rect.right, rect.bottom]
}

#[test]
fn frameless_client_rect_survives_native_theme_and_style_changes() {
    let mut events = EventLoop::builder().with_any_thread(true).build().unwrap();
    let mut fixture = Fixture::default();
    events.pump_app_events(Some(Duration::ZERO), &mut fixture);
    let window = fixture.window.as_ref().unwrap();
    let baseline = client_rect(window, false);
    eprintln!("winit simple non-client rectangle before host setup: {baseline:?}");
    assert!(baseline[1] > 100, "the fixture must exercise the original native caption inset");
    folia_render::backend::configure_native_frame(window, true).unwrap();
    folia_render::backend::configure_native_frame(window, true).unwrap();
    let RawWindowHandle::Win32(handle) = window.window_handle().unwrap().as_raw() else {
        panic!("expected Win32 handle")
    };
    assert_native_frame_hidden(handle.hwnd.get() as HWND, true);
    assert_eq!(backing_alpha(handle.hwnd.get() as HWND), 0, "legacy pixels must be cleared beneath the visual");
    for theme in [Theme::Light, Theme::Dark, Theme::Light] {
        window.set_theme(Some(theme));
        for resizable in [false, true] {
            window.set_resizable(resizable);
            window.set_title("A changed track title");
            window.set_skip_taskbar(resizable);
            assert_native_frame_hidden(handle.hwnd.get() as HWND, resizable);
            assert_eq!(backing_alpha(handle.hwnd.get() as HWND), 0);
            for extended in [false, true] {
                assert_eq!(client_rect(window, extended), [100, 100, 700, 500]);
            }
            let RawWindowHandle::Win32(handle) = window.window_handle().unwrap().as_raw() else {
                panic!("expected Win32 handle")
            };
            // SAFETY: Activation is delivered synchronously to this owned hidden window.
            unsafe {
                assert_ne!(SendMessageW(handle.hwnd.get() as HWND, WM_NCACTIVATE, 0, 0), 0);
                SendMessageW(handle.hwnd.get() as HWND, WM_NCACTIVATE, 1, 0);
            }
        }
    }
    folia_render::backend::configure_native_frame(window, false).unwrap();
    assert_eq!(backing_alpha(handle.hwnd.get() as HWND), 0xa5, "other presentation backends retain their pixels");
    #[cfg(not(feature = "ci"))]
    native_blur_roundtrip(window.clone());
    fixture.window.take();
    events.pump_app_events(Some(Duration::ZERO), &mut fixture);
}

#[cfg(not(feature = "ci"))]
fn native_blur_roundtrip(window: Arc<Window>) {
    use iced_wgpu::graphics::Compositor as _;
    use windows_sys::Win32::Graphics::Dwm::{
        DWMSBT_NONE, DWMSBT_TRANSIENTWINDOW, DWMWA_SYSTEMBACKDROP_TYPE, DwmGetWindowAttribute,
    };
    type HostCompositor = <folia_render::HostRenderer as iced_wgpu::graphics::compositor::Default>::Compositor;
    let mut compositor = pollster::block_on(HostCompositor::with_backend(
        iced_wgpu::graphics::Settings::default(),
        window.clone(),
        window.clone(),
        iced_wgpu::graphics::Shell::headless(),
        Some("wgpu"),
    ))
    .unwrap();
    let _surface = compositor.create_surface(window.clone(), 600, 400).unwrap();
    if !folia_render::backend::native_blur_supported() {
        eprintln!("native backdrop attributes are unavailable on this Windows version");
        return;
    }
    let RawWindowHandle::Win32(handle) = window.window_handle().unwrap().as_raw() else { panic!("expected Win32") };
    for enabled in [true, false, true, false] {
        folia_render::backend::set_native_blur(enabled).unwrap();
        let mut kind = 0i32;
        // SAFETY: DWM writes one i32 attribute for this owned hidden window.
        let result = unsafe {
            DwmGetWindowAttribute(
                handle.hwnd.get() as HWND,
                DWMWA_SYSTEMBACKDROP_TYPE as u32,
                (&mut kind as *mut i32).cast(),
                size_of::<i32>() as u32,
            )
        };
        assert!(result >= 0);
        assert_eq!(kind, if enabled { DWMSBT_TRANSIENTWINDOW } else { DWMSBT_NONE });
        assert_native_frame_hidden(handle.hwnd.get() as HWND, true);
        assert_eq!(client_rect(&window, false), [100, 100, 700, 500]);
    }
    assert!(!window.is_visible().unwrap());
}

fn assert_native_frame_hidden(hwnd: HWND, resizable: bool) {
    // SAFETY: These queries only read the owned hidden window's native style and title bar metadata.
    unsafe {
        let style = GetWindowLongPtrW(hwnd, GWL_STYLE) as u32;
        let mut title = TITLEBARINFO { cbSize: size_of::<TITLEBARINFO>() as u32, ..Default::default() };
        assert_ne!(GetTitleBarInfo(hwnd, &mut title), 0);
        assert_eq!(style & WS_CAPTION, 0, "native title bar still enabled, style={style:#x}");
        assert_ne!(
            title.rgstate[0] & STATE_SYSTEM_INVISIBLE,
            0,
            "Windows still exposes a visible title bar: {:?}",
            title.rgstate
        );
        assert_eq!(style & WS_THICKFRAME != 0, resizable);
        let controls = WS_SYSMENU | WS_MINIMIZEBOX | WS_MAXIMIZEBOX;
        assert_eq!(style & controls, controls, "window management must remain available");
    }
}

// Exercises background erasure on a private memory bitmap, without showing or capturing the desktop.
fn backing_alpha(hwnd: HWND) -> u8 {
    // SAFETY: All GDI objects are owned here and released after the synchronous message and pixel read.
    unsafe {
        let mut rect = RECT::default();
        assert_ne!(GetClientRect(hwnd, &mut rect), 0);
        let info = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: rect.right,
                biHeight: -rect.bottom,
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB,
                ..Default::default()
            },
            ..Default::default()
        };
        let dc = CreateCompatibleDC(std::ptr::null_mut());
        assert!(!dc.is_null());
        let mut bits = std::ptr::null_mut();
        let bitmap = CreateDIBSection(dc, &info, DIB_RGB_COLORS, &mut bits, std::ptr::null_mut(), 0);
        assert!(!bitmap.is_null());
        let previous = SelectObject(dc, bitmap);
        let len = rect.right as usize * rect.bottom as usize * 4;
        std::ptr::write_bytes(bits, 0xa5, len);
        SendMessageW(hwnd, WM_ERASEBKGND, dc as usize, 0);
        GdiFlush();
        let pixels = std::slice::from_raw_parts(bits as *const u8, len);
        let value = pixels[0];
        let uniform = pixels.iter().all(|byte| *byte == value);
        SelectObject(dc, previous);
        DeleteObject(bitmap);
        DeleteDC(dc);
        assert!(uniform, "background erasure must cover the entire client area including alpha");
        value
    }
}
