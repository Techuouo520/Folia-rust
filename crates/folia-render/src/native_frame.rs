// Keeps the DirectComposition host frameless during Windows non-client recalculation and repaint.
use windows_sys::Win32::Foundation::{GetLastError, HWND, LPARAM, LRESULT, RECT, SetLastError, WPARAM};
use windows_sys::Win32::Graphics::Gdi::{BLACK_BRUSH, FillRect, GetDC, GetStockObject, HDC, ReleaseDC};
use windows_sys::Win32::System::Threading::GetCurrentThreadId;
use windows_sys::Win32::UI::Shell::{DefSubclassProc, RemoveWindowSubclass, SetWindowSubclass};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    GWL_STYLE, GetClientRect, GetWindowLongPtrW, GetWindowThreadProcessId, IsIconic, STYLESTRUCT, SWP_FRAMECHANGED,
    SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SWP_NOZORDER, SetWindowLongPtrW, SetWindowPos, WM_ERASEBKGND,
    WM_NCACTIVATE, WM_NCCALCSIZE, WM_NCDESTROY, WM_NCPAINT, WM_SETICON, WM_SETTEXT, WM_SIZE, WM_STYLECHANGING,
    WS_CAPTION,
};
use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};

const SUBCLASS_ID: usize = 0x464f_4c49;

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
}

pub fn install(window: &winit::window::Window, direct_composition: bool) -> Result<(), Error> {
    let RawWindowHandle::Win32(handle) = window.window_handle()?.as_raw() else { return Err(Error::HandleKind) };
    let hwnd = handle.hwnd.get() as HWND;
    // SAFETY: The borrowed winit window keeps this HWND alive throughout setup.
    let (owner, current) = unsafe { (GetWindowThreadProcessId(hwnd, std::ptr::null_mut()), GetCurrentThreadId()) };
    if owner != current {
        return Err(Error::Thread { owner, current });
    }
    // SAFETY: Setup runs on the owner thread; the callback is static and owns no reference data.
    if unsafe { SetWindowSubclass(hwnd, Some(frame_proc), SUBCLASS_ID, usize::from(direct_composition)) } == 0 {
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

unsafe extern "system" fn frame_proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    id: usize,
    direct_composition: usize,
) -> LRESULT {
    match message {
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
        // winit handles the NCCALCSIZE_PARAMS form, but delegates the simple RECT form to the native frame.
        WM_NCCALCSIZE if wparam == 0 => 0,
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
