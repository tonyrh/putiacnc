//! Whether the window manager has the window maximized, read from the X server.
//!
//! egui only learns that the window is maximized from our own Maximize / Restore commands: on Linux eframe never asks the window
//! (winit's `is_maximized` is only polled on Windows). So when the window manager changes it, the program did not notice: a maximized
//! window dragged by its header comes out of maximized in Cinnamon, and the program still drew the Restore button and kept its edges
//! from resizing the window (issue #43). Under X11 this reads `_NET_WM_STATE` of the window every frame and puts the answer in the
//! frame's input, so everything that asks egui (the buttons, the resize edges, the saved window state) sees the real state.
//! Under Wayland there is nothing to read; egui's own idea stays.

use eframe::egui;
use std::cell::RefCell;

thread_local! {
    static STATE: RefCell<Watch> = RefCell::new(Watch::default());
}

#[derive(Default)]
struct Watch {
    tried: bool,
    #[cfg(target_os = "linux")]
    x: Option<x11::Conn>,
    /// The last answer.
    maximized: Option<bool>,
}

/// After a frame: finds the window (once; the window handle is only given to `update`).
pub fn find(frame: &eframe::Frame) {
    if std::env::var_os("PUTIACNC_NO_WMSTATE").is_some() { return; }
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        if s.tried { return; }
        s.tried = true;
        #[cfg(target_os = "linux")]
        { s.x = x11::Conn::new(frame); }
        #[cfg(not(target_os = "linux"))]
        { let _ = frame; }
    });
}

/// Before a frame: asks the window manager and tells egui. It is read before the frame (not after the previous one) so that the
/// frame in which the window manager resized the window already knows why: a window maximized by the window manager does not get
/// its full-screen size saved as its normal size.
pub fn apply(raw: &mut egui::RawInput) {
    let m = STATE.with(|s| {
        let mut s = s.borrow_mut();
        #[cfg(target_os = "linux")]
        if let Some(now) = s.x.as_ref().and_then(|c| c.maximized()) { s.maximized = Some(now); }
        s.maximized
    });
    let Some(m) = m else { return };
    if let Some(v) = raw.viewports.get_mut(&raw.viewport_id) { v.maximized = Some(m); }
}

#[cfg(target_os = "linux")]
mod x11 {
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    use x11rb::connection::Connection;
    use x11rb::protocol::xproto::{AtomEnum, ConnectionExt};
    use x11rb::rust_connection::RustConnection;

    pub struct Conn { c: RustConnection, win: u32, state: u32, horz: u32, vert: u32 }

    impl Conn {
        pub fn new(frame: &eframe::Frame) -> Option<Self> {
            let win = match frame.window_handle().ok()?.as_raw() {
                RawWindowHandle::Xlib(h) => h.window as u32,
                RawWindowHandle::Xcb(h) => h.window.get(),
                _ => return None,
            };
            let (c, _) = x11rb::connect(None).ok()?;
            let atom = |n: &str| c.intern_atom(false, n.as_bytes()).ok()?.reply().ok().map(|r| r.atom);
            let (state, horz, vert) = (atom("_NET_WM_STATE")?, atom("_NET_WM_STATE_MAXIMIZED_HORZ")?, atom("_NET_WM_STATE_MAXIMIZED_VERT")?);
            c.flush().ok()?;
            Some(Self { c, win, state, horz, vert })
        }

        pub fn maximized(&self) -> Option<bool> {
            let r = self.c.get_property(false, self.win, self.state, AtomEnum::ATOM, 0, 64).ok()?.reply().ok()?;
            let atoms: Vec<u32> = r.value32()?.collect();
            Some(atoms.contains(&self.horz) && atoms.contains(&self.vert))
        }
    }
}
