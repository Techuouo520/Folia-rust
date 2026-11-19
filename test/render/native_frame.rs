// Replays native frame, maximize, fullscreen, and hit testing without touching the input desktop.
use std::sync::Arc;
use std::time::Duration;
use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows_sys::Win32::Graphics::Gdi::{
    BI_RGB, BITMAPINFO, BITMAPINFOHEADER, CreateCompatibleDC, CreateDIBSection, DIB_RGB_COLORS, DeleteDC, DeleteObject,
    GdiFlush, GetMonitorInfoW, MONITOR_DEFAULTTONEAREST, MONITORINFO, MonitorFromWindow, SelectObject,
};
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{GetAsyncKeyState, VK_LBUTTON, VK_RBUTTON};
use windows_sys::Win32::UI::Shell::{DefSubclassProc, RemoveWindowSubclass, SetWindowSubclass};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    GWL_STYLE, GetClientRect, GetTitleBarInfo, GetWindowLongPtrW, GetWindowRect, HTCLIENT, HTNOWHERE,
    NCCALCSIZE_PARAMS, SendMessageW, SetWindowLongPtrW, TITLEBARINFO, WINDOWPOS, WM_ERASEBKGND, WM_NCACTIVATE,
    WM_NCCALCSIZE, WM_NCHITTEST, WS_CAPTION, WS_MAXIMIZE, WS_MAXIMIZEBOX, WS_MINIMIZEBOX, WS_SYSMENU, WS_THICKFRAME,
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
    client_rect_at(window, extended, RECT { left: 100, top: 100, right: 700, bottom: 500 })
}

fn client_rect_at(window: &Window, extended: bool, mut rect: RECT) -> [i32; 4] {
    let RawWindowHandle::Win32(handle) = window.window_handle().unwrap().as_raw() else {
        panic!("expected Win32 handle")
    };
    let hwnd = handle.hwnd.get() as HWND;
    let mut position = WINDOWPOS {
        hwnd,
        x: rect.left,
        y: rect.top,
        cx: rect.right - rect.left,
        cy: rect.bottom - rect.top,
        ..Default::default()
    };
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
    if std::env::var_os("FOLIA_PRIVATE_FRAME_TEST").is_none() {
        frame_case();
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "native_frame::frameless_client_rect_survives_native_theme_and_style_changes",
                "--nocapture",
            ])
            .env("FOLIA_PRIVATE_FRAME_TEST", "1")
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        return;
    }
    use windows_sys::Win32::System::StationsAndDesktops::{
        CloseDesktop, CreateDesktopW, DESKTOP_CREATEMENU, DESKTOP_CREATEWINDOW, DESKTOP_READOBJECTS,
        DESKTOP_WRITEOBJECTS, SetThreadDesktop,
    };
    let name: Vec<u16> = format!("folia-frame-test-{}\0", std::process::id()).encode_utf16().collect();
    // SAFETY: Creates a private desktop; it is never switched onto the user's display.
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
    assert!(!desktop.is_null(), "{}", std::io::Error::last_os_error());
    let handle = desktop as usize;
    let result = std::thread::spawn(move || {
        // SAFETY: This new thread has no windows; all test windows remain on its private desktop.
        assert_ne!(unsafe { SetThreadDesktop(handle as _) }, 0);
        maximized_case();
    })
    .join();
    // SAFETY: The owning thread and all its windows have exited before closing the private desktop.
    if unsafe { CloseDesktop(desktop) } == 0 {
        eprintln!(
            "private desktop retained by Windows helper threads until test-child exit: {}",
            std::io::Error::last_os_error()
        );
    }
    if let Err(error) = result {
        std::panic::resume_unwind(error);
    }
}

fn frame_case() {
    let mut events = EventLoop::builder().with_any_thread(true).build().unwrap();
    let mut fixture = Fixture::default();
    events.pump_app_events(Some(Duration::ZERO), &mut fixture);
    let window = fixture.window.as_ref().unwrap();
    let baseline = client_rect(window, false);
    eprintln!("winit simple non-client rectangle before host setup: {baseline:?}");
    assert!(baseline[1] > 100, "the fixture must exercise the original native caption inset");
    let RawWindowHandle::Win32(raw) = window.window_handle().unwrap().as_raw() else {
        panic!("expected Win32");
    };
    // SAFETY: This owned hidden window swallows OS move requests below the host so the test never moves a desktop window.
    assert_ne!(unsafe { SetWindowSubclass(raw.hwnd.get() as HWND, Some(swallow_move), 0x74657331, 0) }, 0);
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
    assert_maximized_client_respects_work_area(window, handle.hwnd.get() as HWND);
    folia_render::backend::configure_native_frame(window, false).unwrap();
    assert_eq!(backing_alpha(handle.hwnd.get() as HWND), 0xa5, "other presentation backends retain their pixels");
    #[cfg(not(feature = "ci"))]
    native_blur_roundtrip(window.clone());
    let owned = window.clone();
    assert_late_drag_does_not_latch(&mut events, &mut fixture, &owned);
    drop(owned);
    fixture.window.take();
    events.pump_app_events(Some(Duration::ZERO), &mut fixture);
}

fn maximized_case() {
    let mut events = EventLoop::builder().with_any_thread(true).build().unwrap();
    let mut fixture = Fixture::default();
    events.pump_app_events(Some(Duration::ZERO), &mut fixture);
    let window = fixture.window.as_ref().unwrap().clone();
    folia_render::backend::configure_native_frame(&window, false).unwrap();
    window.set_visible(true);
    let restored_size = window.outer_size();
    window.set_maximized(true);
    events.pump_app_events(Some(Duration::from_millis(50)), &mut fixture);
    let RawWindowHandle::Win32(handle) = window.window_handle().unwrap().as_raw() else { panic!("expected Win32") };
    assert_maximized_outer_bounds_respect_work_area(handle.hwnd.get() as HWND);
    for from_maximized in [false, true] {
        window.set_maximized(from_maximized);
        window.set_fullscreen(Some(winit::window::Fullscreen::Borderless(None)));
        events.pump_app_events(Some(Duration::from_millis(50)), &mut fixture);
        let mut info = MONITORINFO { cbSize: size_of::<MONITORINFO>() as u32, ..Default::default() };
        let mut actual = RECT::default();
        // SAFETY: Only reads the test window on the private desktop.
        unsafe {
            assert_ne!(
                GetMonitorInfoW(MonitorFromWindow(handle.hwnd.get() as HWND, MONITOR_DEFAULTTONEAREST), &mut info),
                0
            );
            assert_ne!(GetWindowRect(handle.hwnd.get() as HWND, &mut actual), 0);
        }
        let expected = info.rcMonitor;
        assert_eq!(
            [actual.left, actual.top, actual.right, actual.bottom],
            [expected.left, expected.top, expected.right, expected.bottom]
        );
        window.set_fullscreen(None);
        window.set_maximized(true);
        events.pump_app_events(Some(Duration::from_millis(50)), &mut fixture);
        assert_maximized_outer_bounds_respect_work_area(handle.hwnd.get() as HWND);
    }
    window.set_maximized(false);
    events.pump_app_events(Some(Duration::from_millis(50)), &mut fixture);
    assert_eq!(window.outer_size(), restored_size, "restoring must recover the original window size");
    fixture.window.take();
    drop(window);
    events.pump_app_events(Some(Duration::ZERO), &mut fixture);
}

static DRAG_MESSAGES: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

unsafe extern "system" fn swallow_move(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    _: usize,
    _: usize,
) -> LRESULT {
    if message == windows_sys::Win32::UI::WindowsAndMessaging::WM_NCLBUTTONDOWN {
        return 0;
    }
    // SAFETY: The callback forwards unchanged messages for its owned test window.
    unsafe { DefSubclassProc(hwnd, message, wparam, lparam) }
}

unsafe extern "system" fn count_move(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    _: usize,
    _: usize,
) -> LRESULT {
    if message == windows_sys::Win32::UI::WindowsAndMessaging::WM_NCLBUTTONDOWN {
        DRAG_MESSAGES.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    }
    // SAFETY: The callback forwards unchanged messages for its owned test window.
    unsafe { DefSubclassProc(hwnd, message, wparam, lparam) }
}

fn assert_late_drag_does_not_latch(events: &mut EventLoop<()>, fixture: &mut Fixture, window: &Window) {
    let RawWindowHandle::Win32(raw) = window.window_handle().unwrap().as_raw() else {
        panic!("expected Win32");
    };
    let hwnd = raw.hwnd.get() as HWND;
    let released = || {
        // SAFETY: Reads the current input state without generating input or changing any window.
        unsafe { GetAsyncKeyState(i32::from(VK_LBUTTON)) >= 0 && GetAsyncKeyState(i32::from(VK_RBUTTON)) >= 0 }
    };
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    while !released() {
        assert!(std::time::Instant::now() < deadline, "release mouse buttons for the hidden-window regression");
        std::thread::sleep(Duration::from_millis(10));
    }
    DRAG_MESSAGES.store(0, std::sync::atomic::Ordering::Relaxed);
    // SAFETY: Installs a temporary observer above the host on the owned hidden window.
    assert_ne!(unsafe { SetWindowSubclass(hwnd, Some(count_move), 0x74657332, 0) }, 0);
    for _ in 0..2 {
        window.drag_window().unwrap();
        events.pump_app_events(Some(Duration::from_millis(50)), fixture);
    }
    // SAFETY: Removes both test callbacks before their window is destroyed.
    unsafe {
        RemoveWindowSubclass(hwnd, Some(count_move), 0x74657332);
        RemoveWindowSubclass(hwnd, Some(swallow_move), 0x74657331);
    }
    assert_eq!(
        DRAG_MESSAGES.load(std::sync::atomic::Ordering::Relaxed),
        2,
        "a late drag request left winit permanently latched"
    );
}

fn assert_maximized_client_respects_work_area(window: &Window, hwnd: HWND) {
    let mut info = MONITORINFO { cbSize: size_of::<MONITORINFO>() as u32, ..Default::default() };
    // SAFETY: Simulate the maximized style on the owned hidden window without showing or activating it.
    let result = unsafe {
        let monitor = MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST);
        assert_ne!(GetMonitorInfoW(monitor, &mut info), 0);
        let style = GetWindowLongPtrW(hwnd, GWL_STYLE);
        SetWindowLongPtrW(hwnd, GWL_STYLE, style | WS_MAXIMIZE as isize);
        let simple = client_rect_at(window, false, info.rcMonitor);
        let extended = client_rect_at(window, true, info.rcMonitor);
        SetWindowLongPtrW(hwnd, GWL_STYLE, (style | WS_MAXIMIZE as isize) & !(WS_SYSMENU as isize));
        let fullscreen = client_rect_at(window, false, info.rcMonitor);
        let monitor = info.rcMonitor;
        assert_eq!(fullscreen, [monitor.left, monitor.top, monitor.right, monitor.bottom]);
        SetWindowLongPtrW(hwnd, GWL_STYLE, style);
        [simple, extended]
    };
    let work = info.rcWork;
    assert_eq!(result, [[work.left, work.top, work.right, work.bottom]; 2]);
}

fn assert_maximized_outer_bounds_respect_work_area(hwnd: HWND) {
    let mut info = MONITORINFO { cbSize: size_of::<MONITORINFO>() as u32, ..Default::default() };
    let mut actual = RECT::default();
    // SAFETY: Query the genuinely maximized window on the private, non-input desktop.
    unsafe {
        assert_ne!(GetMonitorInfoW(MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST), &mut info), 0);
        assert_ne!(GetWindowRect(hwnd, &mut actual), 0);
    }
    if info.rcWork.bottom < info.rcMonitor.bottom {
        let point = POINT {
            x: (info.rcWork.left + info.rcWork.right) / 2,
            y: (info.rcWork.bottom + info.rcMonitor.bottom) / 2,
        };
        let hit = |x: i32, y: i32| {
            let position = ((x as u32 & 0xffff) | ((y as u32 & 0xffff) << 16)) as LPARAM;
            // SAFETY: Queries the owned window's actual native hit-test path without generating input.
            unsafe { SendMessageW(hwnd, WM_NCHITTEST, 0, position) }
        };
        assert_eq!(hit(point.x, (info.rcWork.top + info.rcWork.bottom) / 2), HTCLIENT as isize);
        assert_eq!(hit(point.x, point.y), HTNOWHERE as isize, "maximized Folia intercepted a taskbar hit");
    }
    assert_eq!(
        [actual.left, actual.top, actual.right, actual.bottom],
        [info.rcWork.left, info.rcWork.top, info.rcWork.right, info.rcWork.bottom],
        "the outer window, not just the client painting area, must leave the taskbar reachable"
    );
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
