// Keeps the DirectComposition host frameless during Windows non-client recalculation and repaint.
use windows_sys::Win32::Foundation::{GetLastError, HWND, LPARAM, LRESULT, POINT, RECT, SetLastError, WPARAM};
use windows_sys::Win32::Graphics::Dwm::{
    DWMSBT_NONE, DWMSBT_TRANSIENTWINDOW, DWMWA_SYSTEMBACKDROP_TYPE, DwmGetWindowAttribute, DwmSetWindowAttribute,
};
use windows_sys::Win32::Graphics::Gdi::{
    BLACK_BRUSH, FillRect, GetDC, GetMonitorInfoW, GetStockObject, HDC, MONITOR_DEFAULTTONEAREST,
    MONITOR_DEFAULTTONULL, MONITORINFO, MonitorFromRect, MonitorFromWindow, ReleaseDC,
};
use windows_sys::Win32::System::Threading::GetCurrentThreadId;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{GetAsyncKeyState, VK_LBUTTON, VK_RBUTTON};
use windows_sys::Win32::UI::Shell::{DefSubclassProc, RemoveWindowSubclass, SetWindowSubclass};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    GWL_STYLE, GetClientRect, GetCursorPos, GetSystemMetrics, GetWindowLongPtrW, GetWindowRect,
    GetWindowThreadProcessId, HTBOTTOMRIGHT, HTCAPTION, HTLEFT, IsIconic, IsZoomed, MINMAXINFO, SM_SWAPBUTTON,
    STYLESTRUCT, SWP_FRAMECHANGED, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SWP_NOZORDER, SetWindowLongPtrW,
    SetWindowPos, WINDOWPOS, WM_CANCELMODE, WM_ERASEBKGND, WM_EXITSIZEMOVE, WM_GETMINMAXINFO, WM_KILLFOCUS,
    WM_LBUTTONUP, WM_NCACTIVATE, WM_NCCALCSIZE, WM_NCDESTROY, WM_NCLBUTTONDOWN, WM_NCPAINT, WM_SETICON, WM_SETTEXT,
    WM_SIZE, WM_STYLECHANGING, WM_WINDOWPOSCHANGING, WS_CAPTION, WS_MAXIMIZE, WS_SYSMENU,
};
use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};

const SUBCLASS_ID: usize = 0x464f_4c49;
const DIRECT_COMPOSITION: usize = 1;
const DRAG_PENDING: usize = 2;

pub fn monitor_work_area(bounds: iced::Rectangle) -> Option<iced::Rectangle> {
    let rect = RECT {
        left: bounds.x as i32,
        top: bounds.y as i32,
        right: (bounds.x + bounds.width) as i32,
        bottom: (bounds.y + bounds.height) as i32,
    };
    let mut info = MONITORINFO { cbSize: size_of::<MONITORINFO>() as u32, ..Default::default() };
    // SAFETY: Both APIs borrow valid stack records and GetMonitorInfoW writes only the declared structure.
    let found = unsafe { GetMonitorInfoW(MonitorFromRect(&rect, MONITOR_DEFAULTTONULL), &mut info) };
    (found != 0).then(|| {
        iced::Rectangle::new(
            iced::Point::new(info.rcWork.left as f32, info.rcWork.top as f32),
            iced::Size::new(
                (info.rcWork.right - info.rcWork.left) as f32,
                (info.rcWork.bottom - info.rcWork.top) as f32,
            ),
        )
    })
}

pub fn set_shape(window: &winit::window::Window, regions: &[iced::Rectangle]) -> Result<(), Error> {
    use windows_sys::Win32::Graphics::Gdi::{
        CombineRgn, CreateRectRgn, CreateRoundRectRgn, DeleteObject, RGN_OR, SetWindowRgn,
    };
    let hwnd = owned_window(window)?;
    let scale = window.scale_factor() as f32;
    // SAFETY: The empty region is an owned GDI object until it is transferred to the window below.
    let combined = unsafe { CreateRectRgn(0, 0, 0, 0) };
    if combined.is_null() {
        return Err(Error::Style(std::io::Error::last_os_error()));
    }
    for rect in regions {
        // SAFETY: Coordinates are values; every temporary region is deleted after the union operation.
        let success = unsafe {
            let piece = CreateRoundRectRgn(
                (rect.x * scale).floor() as i32,
                (rect.y * scale).floor() as i32,
                ((rect.x + rect.width) * scale).ceil() as i32 + 1,
                ((rect.y + rect.height) * scale).ceil() as i32 + 1,
                (36.0 * scale) as i32,
                (36.0 * scale) as i32,
            );
            if piece.is_null() {
                false
            } else {
                let result = CombineRgn(combined, combined, piece, RGN_OR);
                DeleteObject(piece);
                result != 0
            }
        };
        if !success {
            // SAFETY: Ownership was not transferred; this releases the accumulated region.
            unsafe {
                DeleteObject(combined);
            }
            return Err(Error::Style(std::io::Error::last_os_error()));
        }
    }
    // SAFETY: On success Windows owns combined; on failure we still own and release it.
    if unsafe { SetWindowRgn(hwnd, combined, 1) } == 0 {
        // SAFETY: The failed transfer leaves ownership of this region with the caller.
        unsafe {
            DeleteObject(combined);
        }
        return Err(Error::Style(std::io::Error::last_os_error()));
    }
    Ok(())
}

pub fn system_reduced_motion() -> std::io::Result<bool> {
    use windows_sys::Win32::UI::WindowsAndMessaging::{SPI_GETCLIENTAREAANIMATION, SystemParametersInfoW};
    let mut enabled = 1i32;
    // SAFETY: SPI_GETCLIENTAREAANIMATION writes one BOOL to the valid local output pointer.
    let success = unsafe { SystemParametersInfoW(SPI_GETCLIENTAREAANIMATION, 0, (&mut enabled as *mut i32).cast(), 0) };
    if success == 0 {
        return Err(std::io::Error::last_os_error());
    }
    Ok(enabled == 0)
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("native window handle is unavailable")]
    Handle(#[from] winit::raw_window_handle::HandleError),
    #[error("frameless composition requires a Win32 window handle")]
    HandleKind,
    #[error("native frame setup must run on window thread {owner}, current thread is {current}")]
    Thread { owner: u32, current: u32 },
    #[error("could not install the native frame procedure")]
    Subclass,
    #[error("could not remove native caption styles")]
    Style(#[source] std::io::Error),
    #[error("could not refresh the native frame")]
    Refresh(#[source] std::io::Error),
    #[error("could not clear the native window backing bitmap")]
    Backing(#[source] std::io::Error),
    #[error("native backdrop operation failed with HRESULT {0:#x}")]
    Backdrop(i32),
}

fn owned_window(window: &winit::window::Window) -> Result<HWND, Error> {
    let RawWindowHandle::Win32(handle) = window.window_handle()?.as_raw() else { return Err(Error::HandleKind) };
    let hwnd = handle.hwnd.get() as HWND;
    // SAFETY: The borrowed winit window keeps this HWND alive throughout setup.
    let (owner, current) = unsafe { (GetWindowThreadProcessId(hwnd, std::ptr::null_mut()), GetCurrentThreadId()) };
    if owner != current {
        return Err(Error::Thread { owner, current });
    }
    Ok(hwnd)
}

pub fn backdrop(window: &winit::window::Window) -> Result<i32, Error> {
    let hwnd = owned_window(window)?;
    let mut kind = 0i32;
    // SAFETY: The borrowed window owns this HWND and DWM writes one i32 to the valid output pointer.
    let result = unsafe {
        DwmGetWindowAttribute(
            hwnd,
            DWMWA_SYSTEMBACKDROP_TYPE as u32,
            (&mut kind as *mut i32).cast(),
            size_of::<i32>() as u32,
        )
    };
    if result < 0 {
        return Err(Error::Backdrop(result));
    }
    Ok(kind)
}

pub fn set_backdrop(window: &winit::window::Window, enabled: bool) -> Result<(), Error> {
    let hwnd = owned_window(window)?;
    let kind = if enabled { DWMSBT_TRANSIENTWINDOW } else { DWMSBT_NONE };
    // SAFETY: This owner-thread call passes a live HWND and one initialized backdrop attribute value.
    let result = unsafe {
        DwmSetWindowAttribute(
            hwnd,
            DWMWA_SYSTEMBACKDROP_TYPE as u32,
            (&kind as *const i32).cast(),
            size_of::<i32>() as u32,
        )
    };
    if result < 0 {
        return Err(Error::Backdrop(result));
    }
    Ok(())
}

pub fn install(window: &winit::window::Window, direct_composition: bool) -> Result<(), Error> {
    let hwnd = owned_window(window)?;
    let flags = usize::from(direct_composition);
    // SAFETY: Setup runs on the owner thread; the callback is static and owns no reference data.
    if unsafe { SetWindowSubclass(hwnd, Some(frame_proc), SUBCLASS_ID, flags) } == 0 {
        return Err(Error::Subclass);
    }
    remove_caption_style(hwnd).map_err(Error::Style)?;
    // SAFETY: Recalculate this live window's frame without moving, resizing or activating it.
    if unsafe {
        SetWindowPos(
            hwnd,
            std::ptr::null_mut(),
            0,
            0,
            0,
            0,
            SWP_FRAMECHANGED | SWP_NOACTIVATE | SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER,
        )
    } == 0
    {
        return Err(Error::Refresh(std::io::Error::last_os_error()));
    }
    if direct_composition {
        clear_backing(hwnd).map_err(Error::Backing)?;
    }
    Ok(())
}

fn remove_caption_style(hwnd: HWND) -> Result<(), std::io::Error> {
    // SAFETY: The live window is borrowed by install, and this style change runs on its owner thread.
    unsafe {
        let style = GetWindowLongPtrW(hwnd, GWL_STYLE) as u32 & !WS_CAPTION;
        SetLastError(0);
        if SetWindowLongPtrW(hwnd, GWL_STYLE, style as isize) == 0 && GetLastError() != 0 {
            return Err(std::io::Error::last_os_error());
        }
    }
    Ok(())
}

// DirectComposition supplies every visible pixel; the separate legacy bitmap must stay alpha-zero.
fn erase_backing(hwnd: HWND, dc: HDC) -> Result<(), std::io::Error> {
    let mut rect = RECT::default();
    // SAFETY: The caller holds a live window and a valid client or WM_ERASEBKGND device context.
    unsafe {
        if GetClientRect(hwnd, &mut rect) == 0 {
            return Err(std::io::Error::last_os_error());
        }
        if rect.right == 0 || rect.bottom == 0 {
            return Ok(());
        }
        if FillRect(dc, &rect, GetStockObject(BLACK_BRUSH)) == 0 {
            return Err(std::io::Error::last_os_error());
        }
    }
    Ok(())
}

fn clear_backing(hwnd: HWND) -> Result<(), std::io::Error> {
    // SAFETY: This runs on the live window's owner thread and releases its DC before returning.
    unsafe {
        let dc = GetDC(hwnd);
        if dc.is_null() {
            return Err(std::io::Error::last_os_error());
        }
        let result = erase_backing(hwnd, dc);
        ReleaseDC(hwnd, dc);
        result
    }
}

// Match winit's maximized work-area calculation for the simple RECT form.
fn fit_maximized_client(hwnd: HWND, proposed: &mut RECT) {
    // SAFETY: The non-client callback supplies a live HWND and a valid proposed client rectangle.
    unsafe {
        // winit removes the system menu for real fullscreen, including when entered from maximized.
        if IsZoomed(hwnd) == 0 || GetWindowLongPtrW(hwnd, GWL_STYLE) as u32 & WS_SYSMENU == 0 {
            return;
        }
        let monitor = MonitorFromRect(proposed, MONITOR_DEFAULTTONULL);
        let mut info = MONITORINFO { cbSize: size_of::<MONITORINFO>() as u32, ..Default::default() };
        if !monitor.is_null() && GetMonitorInfoW(monitor, &mut info) != 0 {
            *proposed = info.rcWork;
        }
    }
}

// Constrain the actual window, including its hit-test region, before Windows maximizes it.
fn fit_maximized_window(hwnd: HWND, limits: &mut MINMAXINFO) {
    // SAFETY: The callback borrows a live window and the OS-owned sizing constraints.
    unsafe {
        if GetWindowLongPtrW(hwnd, GWL_STYLE) as u32 & WS_SYSMENU == 0 {
            return;
        }
        let mut info = MONITORINFO { cbSize: size_of::<MONITORINFO>() as u32, ..Default::default() };
        if GetMonitorInfoW(MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST), &mut info) == 0 {
            let err = std::io::Error::last_os_error();
            tracing::warn!(error = &err as &dyn std::error::Error, "maximized work area unavailable");
            return;
        }
        limits.ptMaxPosition =
            POINT { x: info.rcWork.left - info.rcMonitor.left, y: info.rcWork.top - info.rcMonitor.top };
        limits.ptMaxSize = POINT { x: info.rcWork.right - info.rcWork.left, y: info.rcWork.bottom - info.rcWork.top };
    }
}

// Fullscreen restoration can bypass the maximize command while retaining WS_MAXIMIZE.
fn fit_maximized_position(hwnd: HWND, position: &mut WINDOWPOS) {
    // SAFETY: The callback borrows this window and the OS-owned proposed position on its owner thread.
    unsafe {
        let style = GetWindowLongPtrW(hwnd, GWL_STYLE) as u32;
        if style & (WS_MAXIMIZE | WS_SYSMENU) != WS_MAXIMIZE | WS_SYSMENU || IsIconic(hwnd) != 0 {
            return;
        }
        let mut rect = RECT::default();
        if GetWindowRect(hwnd, &mut rect) == 0 {
            let err = std::io::Error::last_os_error();
            tracing::warn!(error = &err as &dyn std::error::Error, "maximized window bounds unavailable");
            return;
        }
        let width = if position.flags & SWP_NOSIZE == 0 { position.cx } else { rect.right - rect.left };
        let height = if position.flags & SWP_NOSIZE == 0 { position.cy } else { rect.bottom - rect.top };
        if position.flags & SWP_NOMOVE == 0 {
            rect.left = position.x;
            rect.top = position.y;
        }
        rect.right = rect.left + width;
        rect.bottom = rect.top + height;
        let mut info = MONITORINFO { cbSize: size_of::<MONITORINFO>() as u32, ..Default::default() };
        if GetMonitorInfoW(MonitorFromRect(&rect, MONITOR_DEFAULTTONEAREST), &mut info) == 0 {
            let err = std::io::Error::last_os_error();
            tracing::warn!(error = &err as &dyn std::error::Error, "maximized target work area unavailable");
            return;
        }
        position.x = info.rcWork.left;
        position.y = info.rcWork.top;
        position.cx = info.rcWork.right - info.rcWork.left;
        position.cy = info.rcWork.bottom - info.rcWork.top;
        position.flags &= !(SWP_NOMOVE | SWP_NOSIZE);
    }
}

fn set_drag_pending(hwnd: HWND, id: usize, flags: usize, pending: bool) {
    let flags = if pending { flags | DRAG_PENDING } else { flags & !DRAG_PENDING };
    // SAFETY: Updates the existing static callback on its owner thread without retaining Rust references.
    if unsafe { SetWindowSubclass(hwnd, Some(frame_proc), id, flags) } == 0 {
        tracing::error!(error = &Error::Subclass as &dyn std::error::Error, "native drag state update failed");
    }
}

fn primary_button_down() -> bool {
    // SAFETY: Reads the physical input state, accounting for Windows' logical button swap.
    unsafe {
        let key = if GetSystemMetrics(SM_SWAPBUTTON) == 0 { VK_LBUTTON } else { VK_RBUTTON };
        GetAsyncKeyState(i32::from(key)) < 0
    }
}

unsafe extern "system" fn frame_proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    id: usize,
    flags: usize,
) -> LRESULT {
    let direct_composition = flags & DIRECT_COMPOSITION;
    match message {
        WM_WINDOWPOSCHANGING if lparam != 0 => {
            // SAFETY: Apply native sizing policy before constraining the borrowed maximized outer rectangle.
            unsafe {
                DefSubclassProc(hwnd, message, wparam, lparam);
                fit_maximized_position(hwnd, &mut *(lparam as *mut WINDOWPOS));
            }
            0
        }
        WM_GETMINMAXINFO if lparam != 0 => {
            // SAFETY: Retain winit's min/max tracking constraints, then set only the maximized outer bounds.
            unsafe {
                DefSubclassProc(hwnd, message, wparam, lparam);
                fit_maximized_window(hwnd, &mut *(lparam as *mut MINMAXINFO));
            }
            0
        }
        WM_NCLBUTTONDOWN
            if wparam == HTCAPTION as usize || (HTLEFT as usize..=HTBOTTOMRIGHT as usize).contains(&wparam) =>
        {
            if !primary_button_down() {
                set_drag_pending(hwnd, id, flags, false);
                // SAFETY: Unwinds winit's pending move flag without entering an OS move loop after release.
                unsafe {
                    DefSubclassProc(hwnd, WM_EXITSIZEMOVE, 0, 0);
                }
                return 0;
            }
            let mut at = POINT::default();
            // SAFETY: Reads the live cursor without retaining the asynchronous message's stack pointer.
            if unsafe { GetCursorPos(&mut at) } == 0 {
                let err = std::io::Error::last_os_error();
                tracing::warn!(error = &err as &dyn std::error::Error, "native drag cursor unavailable");
                set_drag_pending(hwnd, id, flags, false);
                // SAFETY: Canceling the native operation must also release winit's pending move flag.
                unsafe { DefSubclassProc(hwnd, WM_EXITSIZEMOVE, 0, 0) };
                return 0;
            }
            set_drag_pending(hwnd, id, flags, true);
            let position = ((at.x as u32 & 0xffff) | ((at.y as u32 & 0xffff) << 16)) as LPARAM;
            // SAFETY: The remaining procedure chain owns the native move/resize operation.
            unsafe { DefSubclassProc(hwnd, message, wparam, position) }
        }
        WM_EXITSIZEMOVE => {
            set_drag_pending(hwnd, id, flags, false);
            // SAFETY: Retains winit's normal move-loop completion and synthetic client release.
            unsafe { DefSubclassProc(hwnd, message, wparam, lparam) }
        }
        WM_LBUTTONUP | WM_CANCELMODE | WM_KILLFOCUS if flags & DRAG_PENDING != 0 => {
            set_drag_pending(hwnd, id, flags, false);
            // SAFETY: A canceled or unentered move still releases winit's latch before its original message.
            unsafe {
                DefSubclassProc(hwnd, WM_EXITSIZEMOVE, 0, 0);
                DefSubclassProc(hwnd, message, wparam, lparam)
            }
        }
        WM_STYLECHANGING if wparam as i32 == GWL_STYLE && lparam != 0 => {
            // SAFETY: Windows lends this STYLESTRUCT during the synchronous style change notification.
            unsafe {
                DefSubclassProc(hwnd, message, wparam, lparam);
                let styles = &mut *(lparam as *mut STYLESTRUCT);
                styles.styleNew &= !WS_CAPTION;
            }
            0
        }
        WM_ERASEBKGND if direct_composition != 0 => match erase_backing(hwnd, wparam as HDC) {
            Ok(()) => 1,
            Err(err) => {
                tracing::error!(error = &err as &dyn std::error::Error, "native backing erase failed");
                0
            }
        },
        WM_SIZE | WM_SETTEXT | WM_SETICON if direct_composition != 0 => {
            // SAFETY: Preserve winit's resize/title/icon behavior before clearing any legacy paint it produced.
            let result = unsafe { DefSubclassProc(hwnd, message, wparam, lparam) };
            if let Err(err) = clear_backing(hwnd) {
                tracing::error!(error = &err as &dyn std::error::Error, "native backing clear failed");
            }
            result
        }
        WM_NCCALCSIZE if wparam == 0 && lparam != 0 => {
            // SAFETY: With a zero WPARAM, Windows lends a writable RECT for the duration of this callback.
            fit_maximized_client(hwnd, unsafe { &mut *(lparam as *mut RECT) });
            0
        }
        WM_NCPAINT => 0,
        WM_NCACTIVATE => {
            // SAFETY: This callback receives a live HWND; minimized windows retain default activation handling.
            let minimized = unsafe { IsIconic(hwnd) } != 0;
            // SAFETY: Forward activation to winit while the documented -1 parameter suppresses frame painting.
            unsafe { DefSubclassProc(hwnd, message, wparam, if minimized { lparam } else { -1 }) }
        }
        WM_NCDESTROY => {
            // SAFETY: Remove this exact static callback before the owning native window finishes destruction.
            if unsafe { RemoveWindowSubclass(hwnd, Some(frame_proc), id) } == 0 {
                tracing::warn!("native frame procedure removal failed");
            }
            // SAFETY: The remaining procedure chain owns destruction and must receive this final message.
            unsafe { DefSubclassProc(hwnd, message, wparam, lparam) }
        }
        // SAFETY: All remaining messages retain their original parameters and processing chain.
        _ => unsafe { DefSubclassProc(hwnd, message, wparam, lparam) },
    }
}
