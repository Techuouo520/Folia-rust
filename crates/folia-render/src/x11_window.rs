// EWMH capabilities missing from winit's portable window API; native Wayland never enters this path.

use iced::{Point, Rectangle, Size};
use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
use winit::window::Window;
use x11rb::connection::Connection;
use x11rb::protocol::xproto::{AtomEnum, ClientMessageEvent, ConnectionExt, EventMask};
use x11rb::rust_connection::RustConnection;

fn connect(window: &Window) -> Result<(RustConnection, u32, u32), String> {
    let window = match window.window_handle().map_err(|error| error.to_string())?.as_raw() {
        RawWindowHandle::Xlib(handle) => u32::try_from(handle.window).map_err(|error| error.to_string())?,
        RawWindowHandle::Xcb(handle) => handle.window.get(),
        _ => return Err("window does not use X11".into()),
    };
    let (connection, _) = x11rb::connect(None).map_err(|error| error.to_string())?;
    let root = connection
        .get_geometry(window)
        .map_err(|error| error.to_string())?
        .reply()
        .map_err(|error| error.to_string())?
        .root;
    Ok((connection, root, window))
}

fn atom(connection: &RustConnection, name: &[u8]) -> Result<u32, String> {
    connection
        .intern_atom(false, name)
        .map_err(|error| error.to_string())?
        .reply()
        .map(|reply| reply.atom)
        .map_err(|error| error.to_string())
}

fn property(connection: &RustConnection, window: u32, name: &[u8], kind: AtomEnum) -> Result<Vec<u32>, String> {
    let property = atom(connection, name)?;
    let reply = connection
        .get_property(false, window, property, kind, 0, 4096)
        .map_err(|error| error.to_string())?
        .reply()
        .map_err(|error| error.to_string())?;
    Ok(reply.value32().map_or_else(Vec::new, Iterator::collect))
}

pub fn taskbar_supported(window: &Window) -> Result<bool, String> {
    let (connection, root, _) = connect(window)?;
    let skip = atom(&connection, b"_NET_WM_STATE_SKIP_TASKBAR")?;
    Ok(property(&connection, root, b"_NET_SUPPORTED", AtomEnum::ATOM)?.contains(&skip))
}

pub fn set_taskbar(window: &Window, visible: bool) -> Result<(), String> {
    let (connection, root, window) = connect(window)?;
    let state = atom(&connection, b"_NET_WM_STATE")?;
    let taskbar = atom(&connection, b"_NET_WM_STATE_SKIP_TASKBAR")?;
    let pager = atom(&connection, b"_NET_WM_STATE_SKIP_PAGER")?;
    let event = ClientMessageEvent::new(32, window, state, [u32::from(!visible), taskbar, pager, 1, 0]);
    connection
        .send_event(false, root, EventMask::SUBSTRUCTURE_REDIRECT | EventMask::SUBSTRUCTURE_NOTIFY, event)
        .map_err(|error| error.to_string())?
        .check()
        .map_err(|error| error.to_string())?;
    connection.flush().map_err(|error| error.to_string())
}

pub fn work_area(window: &Window) -> Result<Option<Rectangle>, String> {
    let (connection, root, _) = connect(window)?;
    let desktop =
        property(&connection, root, b"_NET_CURRENT_DESKTOP", AtomEnum::CARDINAL)?.first().copied().unwrap_or(0)
            as usize;
    let areas = property(&connection, root, b"_NET_WORKAREA", AtomEnum::CARDINAL)?;
    let Some(area) =
        desktop.checked_mul(4).and_then(|offset| offset.checked_add(4).and_then(|end| areas.get(offset..end)))
    else {
        return Ok(None);
    };
    Ok(Some(Rectangle::new(
        Point::new(area[0] as i32 as f32, area[1] as i32 as f32),
        Size::new(area[2] as f32, area[3] as f32),
    )))
}

pub fn set_shape(window: &Window, regions: &[Rectangle], interactive: bool) -> Result<(), String> {
    use x11rb::protocol::shape::{ConnectionExt as _, SK, SO};
    use x11rb::protocol::xproto::{ClipOrdering, Rectangle as XRectangle};
    let scale = window.scale_factor() as f32;
    let (connection, _, window) = connect(window)?;
    let rectangles: Vec<_> = regions
        .iter()
        .filter(|_| interactive)
        .map(|rect| XRectangle {
            x: (rect.x * scale).floor() as i16,
            y: (rect.y * scale).floor() as i16,
            width: (rect.width * scale).ceil() as u16,
            height: (rect.height * scale).ceil() as u16,
        })
        .collect();
    connection
        .shape_rectangles(SO::SET, SK::INPUT, ClipOrdering::UNSORTED, window, 0, 0, &rectangles)
        .map_err(|error| error.to_string())?
        .check()
        .map_err(|error| error.to_string())?;
    connection.flush().map_err(|error| error.to_string())
}
