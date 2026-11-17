// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! The panel's camera: a process of its own (camera-app, utils/camera-app) draws the camera
//! into two zones, the tile's #camera/feed (ZONE_TILE: a still every few seconds) and the full
//! view's #camera-view/picture (ZONE_LIVE: live video while the view is open). This module is
//! the view's state and the panel's end of camera-app's control socket:
//!
//! ```text
//! panel ──connect──► $XDG_RUNTIME_DIR/har-camera (camera_app listens)
//!   ◄── "on" / "off"       the camera delivers or not: the LIVE chips (Event::On)
//!   ──► "live" / "still"   the full view opened / closed (set_live)
//!   end of the connection: no camera app (Event::Connected(false)): no chip, no zones, the
//!   panel's own "Camera offline"
//! ```
//!
//! A connection that asked for live keeps the camera live until it says still or closes: a
//! panel that dies leaves no camera streaming at the live rate. The thread connects again every
//! RETRY and says the mode again after a reconnect. The lines are camera-app's protocol.rs.
//! Nothing here needs a device: its tests run on the host (smarthome_app_host_test).

use everyware_ui_core::button::{Icon, IconButton, IconButtonKind};
use everyware_ui_core::overlay::Overlay;
use everyware_ui_core::transitions::{self, Change, ComponentKind};
use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// The tile's zone: the tag camera-app names in har_zone (its config.rs Zone::Tile).
pub const ZONE_TILE: &str = "#zone/camera";
/// The full view's zone (camera-app Zone::Live).
pub const ZONE_LIVE: &str = "#zone/camera-live";
/// The control socket's name in the runtime directory (camera-app config.rs).
const SOCKET: &str = "har-camera";
const RETRY: Duration = Duration::from_secs(2);
const MAX_LINE: usize = 16;

/// What the camera app said.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Event {
    /// Connected to the camera app, or the connection ended.
    Connected(bool),
    /// The camera delivers pictures (on) or not.
    On(bool),
}

/// The camera as the panel knows it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Link {
    /// The camera app runs and draws the zones.
    pub connected: bool,
    /// Its camera delivers pictures: the LIVE chips.
    pub on: bool,
}

impl Link {
    pub fn apply(&mut self, event: Event) {
        match event {
            Event::Connected(c) => {
                self.connected = c;
                // a camera app that is gone delivers nothing
                if !c {
                    self.on = false;
                }
            }
            Event::On(on) => self.on = self.connected && on,
        }
    }
}

/// Where the closed view waits: below the screen, as a closed sheet waits behind its edge.
pub const CLOSED_Y: f32 = 1080.0 + 40.0;

/// The full view (#camera-view): the whole screen, the camera live, closed by its button. It
/// opens and closes as the library's sheets do: the Overlay's state, the view's y following it
/// with the open and close transitions (display only in harry: no new tree, no layout). Its
/// zone stays on while camera-app is connected, so neither opening nor closing switches the
/// zone mid-animation; the camera goes back to stills once the close has played (`settled`),
/// and the view slides out on live frames.
#[derive(Debug)]
pub struct CameraView {
    pub overlay: Overlay,
    pub close: IconButton,
    pub link: Link,
    /// When the view was closed, until the camera is told to go back to stills.
    closing: Option<Instant>,
}

impl Default for CameraView {
    fn default() -> Self {
        CameraView {
            overlay: Overlay::new(),
            close: IconButton::new(IconButtonKind::Scrim, Icon::Close),
            link: Link::default(),
            closing: None,
        }
    }
}

impl CameraView {
    pub fn is_open(&self) -> bool {
        self.overlay.open
    }

    /// Opens the view; the caller asks camera-app for live video (`set_live(true)`).
    pub fn open(&mut self) {
        self.overlay.open();
        self.closing = None;
    }

    /// Closes the view at `now`; the camera stays live while it slides out.
    pub fn close(&mut self, now: Instant) {
        self.overlay.close();
        self.closing = Some(now);
    }

    /// True once, when the close has played at `now`: the caller tells camera-app to go back to
    /// stills (`set_live(false)`).
    pub fn settled(&mut self, now: Instant) -> bool {
        let played = transitions::transition(ComponentKind::Overlay, Change::Close).map_or(Duration::ZERO, |t| t.delay + t.duration);
        match self.closing {
            Some(at) if now.saturating_duration_since(at) >= played => {
                self.closing = None;
                true
            }
            _ => false,
        }
    }

    /// The view's y: on the screen while open, below it while closed.
    pub fn y(&self) -> f32 {
        self.overlay.slide(0.0, CLOSED_Y)
    }

    /// The tile's and the view's zones are on: the camera app is there to draw them.
    pub fn zones(&self) -> bool {
        self.link.connected
    }
}

/// The line of a status word, or None for anything else.
pub fn parse(line: &[u8]) -> Option<Event> {
    match line {
        b"on" => Some(Event::On(true)),
        b"off" => Some(Event::On(false)),
        _ => None,
    }
}

/// The full view's mode the camera app was asked for, said again after a reconnect.
static LIVE: AtomicBool = AtomicBool::new(false);
/// The connection's writing end, while connected.
static WRITER: Mutex<Option<UnixStream>> = Mutex::new(None);

/// The control socket next to the panel's Wayland socket: the runtime directory, without one
/// /data/local/tmp (camera-app does the same).
pub fn socket_path(runtime_dir: Option<&Path>) -> PathBuf {
    runtime_dir.unwrap_or(Path::new("/data/local/tmp")).join(SOCKET)
}

/// Asks the camera app for live video (the full view opened) or stills (closed and settled).
/// Never blocks for long (a write timeout); without a connection the mode goes when the thread
/// connects.
pub fn set_live(live: bool) {
    let Ok(mut w) = WRITER.lock() else { return };
    // under the writer's lock: a reconnect sends the mode under it too, so the last word the
    // camera app hears is the last mode asked for
    LIVE.store(live, Ordering::SeqCst);
    send(&mut w, live);
}

fn send(w: &mut Option<UnixStream>, live: bool) {
    if let Some(s) = w.as_mut() {
        if let Err(e) = s.write_all(if live { b"live\n" } else { b"still\n" }) {
            log::warn!("smarthome: camera: {e}");
            *w = None;
        }
    }
}

/// The thread of the control socket `path` (socket_path): connects, hands the camera app's
/// words to `on_event`, connects again when the connection ends. `on_event` returns false when
/// the app is ending.
pub fn spawn(path: PathBuf, on_event: impl Fn(Event) -> bool + Send + 'static) {
    let spawned = std::thread::Builder::new().name("camera-link".to_string()).spawn(move || loop {
        let Ok(stream) = UnixStream::connect(&path) else {
            std::thread::sleep(RETRY);
            continue;
        };
        let writer = stream.try_clone().and_then(|w| w.set_write_timeout(Some(Duration::from_millis(100))).map(|_| w));
        let Ok(writer) = writer else {
            std::thread::sleep(RETRY);
            continue;
        };
        log::info!("smarthome: camera: connected to {}", path.display());
        if let Ok(mut w) = WRITER.lock() {
            *w = Some(writer);
            if LIVE.load(Ordering::SeqCst) {
                send(&mut w, true);
            }
        }
        if !on_event(Event::Connected(true)) {
            return;
        }
        let ended = read(stream, &on_event);
        if let Ok(mut w) = WRITER.lock() {
            *w = None;
        }
        log::info!("smarthome: camera: the camera app is gone");
        if !ended || !on_event(Event::Connected(false)) {
            return;
        }
        std::thread::sleep(RETRY);
    });
    if let Err(e) = spawned {
        log::warn!("smarthome: camera: no link thread ({e}): the camera tile stays offline");
    }
}

/// The camera app's lines until the connection ends (true) or the app is ending (false); an
/// unknown or long line ends the connection.
fn read(mut stream: UnixStream, on_event: &impl Fn(Event) -> bool) -> bool {
    let (mut line, mut buf) = (Vec::with_capacity(MAX_LINE), [0u8; 64]);
    loop {
        let n = match stream.read(&mut buf) {
            Ok(0) | Err(_) => return true,
            Ok(n) => n,
        };
        for &b in &buf[..n] {
            if b != b'\n' {
                if line.len() == MAX_LINE {
                    log::warn!("smarthome: camera: a long line, connection closed");
                    return true;
                }
                line.push(b);
                continue;
            }
            let Some(event) = parse(&line) else {
                log::warn!("smarthome: camera: an unknown line ({} bytes), connection closed", line.len());
                return true;
            };
            line.clear();
            if !on_event(event) {
                return false;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_chip_is_on_only_while_connected_and_on() {
        let mut l = Link::default();
        l.apply(Event::On(true));
        assert!(!l.on, "no connection");
        l.apply(Event::Connected(true));
        l.apply(Event::On(true));
        assert!(l.on);
        l.apply(Event::Connected(false));
        assert_eq!(l, Link { connected: false, on: false });
        assert_eq!(parse(b"on"), Some(Event::On(true)));
        assert_eq!(parse(b"off"), Some(Event::On(false)));
        assert_eq!(parse(b"live"), None);
    }

    #[test]
    fn the_view_slides_and_goes_to_stills_once_the_close_has_played() {
        let t0 = Instant::now();
        let mut v = CameraView::default();
        assert_eq!((v.is_open(), v.y()), (false, CLOSED_Y));
        v.open();
        assert_eq!((v.is_open(), v.y()), (true, 0.0));
        assert!(v.overlay.transition().is_some(), "it slides in");
        assert!(!v.settled(t0), "nothing to settle while open");
        v.close(t0);
        assert_eq!(v.y(), CLOSED_Y);
        assert!(!v.settled(t0 + Duration::from_millis(100)), "still sliding out, still live");
        assert!(v.settled(t0 + Duration::from_secs(1)));
        assert!(!v.settled(t0 + Duration::from_secs(2)), "once");
        // opened again before the close played: no stills
        v.close(t0);
        v.open();
        assert!(!v.settled(t0 + Duration::from_secs(1)));
    }

    #[test]
    fn the_socket_is_next_to_the_wayland_socket() {
        assert_eq!(socket_path(Some(Path::new("/data/vendor/everyware/run"))), Path::new("/data/vendor/everyware/run/har-camera"));
        assert_eq!(socket_path(None), Path::new("/data/local/tmp/har-camera"));
    }

    #[test]
    fn the_link_hears_the_words_and_says_the_mode() {
        use std::os::unix::net::UnixListener;
        use std::sync::mpsc::channel;
        let dir = std::env::temp_dir().join(format!("smarthome-camera-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = socket_path(Some(&dir));
        let listener = UnixListener::bind(&path).unwrap();
        let (tx, rx) = channel();
        spawn(path, move |e| tx.send(e).is_ok());
        let (mut camera, _) = listener.accept().unwrap();
        let next = || rx.recv_timeout(Duration::from_secs(5)).unwrap();
        assert_eq!(next(), Event::Connected(true));
        camera.write_all(b"on\n").unwrap();
        assert_eq!(next(), Event::On(true));
        set_live(true);
        let mut got = [0u8; 5];
        camera.read_exact(&mut got).unwrap();
        assert_eq!(&got, b"live\n");
        set_live(false);
        let mut got = [0u8; 6];
        camera.read_exact(&mut got).unwrap();
        assert_eq!(&got, b"still\n");
        camera.write_all(b"zoom\n").unwrap();
        assert_eq!(next(), Event::Connected(false), "an unknown word ends the connection");
    }
}
