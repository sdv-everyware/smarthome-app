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

//! The panel's game: a game runs in a process of its own (doom-app) and shows its picture in
//! the zone ZONE_TAG, the frame #game/picture of the dialog #game. The dialog's buttons are the
//! game's keys: while a button is held its key is down, so moving and firing work at once (two
//! fingers, two buttons). The keys go to the game as lines in a FIFO the game reads:
//!
//! ```text
//! #game/left held ──► "down left\n" ──► FIFO har-game-keys ──► doom-app ──► DG_GetKey
//! #game/left let go ─► "up left\n"
//! ```
//!
//! One line is one write below PIPE_BUF, so it arrives whole. The game reads only these twelve
//! lines (doom-app's keys.rs); it pauses itself while the zone is hidden (the dialog closed), so
//! the panel sends nothing else. Whether a game runs is whether it reads the FIFO (`running`):
//! the dialog asks when it opens and every second while it is open.
//!
//! The direction pad follows a sliding thumb (`Pad`): a finger that rolls from Left onto Up lets
//! Left go and holds Up, as on a game pad. The keys lie over the picture (design/game-patch.js):
//! scrim icon buttons, white 15 % as over the camera. Nothing here needs a device: its tests run on the host
//! (smarthome_app_host_test).

use everyware_ui_core::button::{Icon, IconButton, IconButtonKind};
use everyware_ui_core::sheet::Rect;
use everyware_ui_core::Gesture;
use std::io::Write;
use std::os::unix::fs::{FileTypeExt, OpenOptionsExt};

/// The zone of the game's picture: the tag the game names in har_zone (doom-app --zone).
pub const ZONE_TAG: &str = "#zone/game";

/// The property by which the image says it has the game: doom-app's product/doom.mk sets it with
/// EVERYWARE_GAME=true, when it puts doom_app and its WAD into the image. Without it the status
/// row has no game button. A development run on an image without it: `setprop` it (as root,
/// before the panel starts; a ro. property can be set once).
pub const PROPERTY: &str = "ro.vendor.everyware.game";

/// Whether the image has the game, by the property's value.
pub fn in_image(property: &str) -> bool {
    property == "true"
}

/// A key of the game, one per button of the dialog.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Key {
    Up,
    Down,
    Left,
    Right,
    Fire,
    Use,
}

impl Key {
    /// Every key, in the order of `Held`.
    pub const ALL: [Key; 6] = [Key::Up, Key::Down, Key::Left, Key::Right, Key::Fire, Key::Use];

    /// The key's word on the line.
    pub fn word(self) -> &'static str {
        match self {
            Key::Up => "up",
            Key::Down => "down",
            Key::Left => "left",
            Key::Right => "right",
            Key::Fire => "fire",
            Key::Use => "use",
        }
    }
}

/// The direction pad: up, down, left, right (Key's order), scrim icon buttons in a 3 x 3 grid
/// over the picture's bottom left (design/game-patch.js). A finger holds the button it is on: the touch stays with the button it
/// started on (ui-lib Input), and the pad finds the button under the finger from its places on
/// the screen (`rects`, from the design, while the dialog is open).
#[derive(Debug)]
pub struct Pad {
    pub buttons: [IconButton; 4],
    /// Where the buttons are on the screen; zero rectangles: unknown (a finger then holds only
    /// the button it started on).
    pub rects: [Rect; 4],
    /// Per button a finger started on: the button that finger holds now, if any.
    rolled: [Option<usize>; 4],
}

/// The pad's nodes, in Key's order.
pub const PAD_NODES: [&str; 4] = ["#game/up", "#game/down", "#game/left", "#game/right"];

impl Default for Pad {
    fn default() -> Self {
        let button = |icon| IconButton::new(IconButtonKind::Scrim, icon);
        Pad {
            buttons: [button(Icon::ChevronUp), button(Icon::ChevronDown), button(Icon::ChevronLeft), button(Icon::ChevronRight)],
            rects: [Rect { x: 0.0, y: 0.0, w: 0.0, h: 0.0 }; 4],
            rolled: [None; 4],
        }
    }
}

impl Pad {
    /// The pad's button of a node.
    pub fn index(node: &str) -> Option<usize> {
        PAD_NODES.iter().position(|n| *n == node)
    }

    /// A gesture of a finger that started on the button `start`: the finger holds the button it
    /// is on, also after sliding onto another one; off the pad (the middle, outside) it holds
    /// none. A button is held while any finger is on it.
    pub fn on(&mut self, start: usize, gesture: Gesture) {
        self.rolled[start] = match gesture {
            Gesture::Down { .. } => Some(start),
            Gesture::Move { inside: true, .. } => Some(start),
            Gesture::Move { x, y, inside: false } => at(&self.rects, x, y),
            Gesture::Up { .. } | Gesture::Cancel => None,
        };
        self.press();
    }

    /// Every button lets go.
    pub fn release(&mut self) {
        self.rolled = [None; 4];
        self.press();
    }

    /// Whether each button is held, in Key's order: a finger is on it.
    pub fn held(&self) -> [bool; 4] {
        std::array::from_fn(|i| self.rolled.contains(&Some(i)))
    }

    /// The buttons' pressed layers follow what is held.
    fn press(&mut self) {
        let held = self.held();
        for (b, held) in self.buttons.iter_mut().zip(held) {
            if b.press.pressed != held {
                b.on(if held { Gesture::DOWN } else { Gesture::Cancel });
            }
        }
    }
}

/// The index of the rectangle that holds (x, y), if one does.
pub fn at(rects: &[Rect], x: f32, y: f32) -> Option<usize> {
    rects.iter().position(|r| r.w > 0.0 && r.h > 0.0 && x >= r.x && x < r.x + r.w && y >= r.y && y < r.y + r.h)
}

/// Whether a game reads the keys, as the panel finds the FIFO (`reach`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Reach {
    /// No game: no FIFO, or nobody reads it.
    #[default]
    NotRunning,
    /// A game reads the FIFO and the panel may write it.
    Runs,
    /// A game made the FIFO and reads it, but the panel may not open it (an enforcing board
    /// without a rule for the FIFO): its picture shows, its keys do not arrive.
    KeysBlocked,
}

impl Reach {
    /// Whether the game's picture belongs in the zone.
    pub fn runs(self) -> bool {
        self != Reach::NotRunning
    }
}

/// What opening the FIFO to write says: opened (None), or the error's errno.
pub fn reach(errno: Option<i32>) -> Reach {
    match errno {
        None => Reach::Runs,
        Some(libc::EACCES) | Some(libc::EPERM) => Reach::KeysBlocked,
        Some(_) => Reach::NotRunning,
    }
}

/// What the dialog says under the picture until the game's first frame covers it.
pub fn status(reach: Reach) -> &'static str {
    match reach {
        Reach::Runs => "Starting the game",
        Reach::KeysBlocked => "The game runs, but the panel may not send it keys",
        Reach::NotRunning => "The game is not running",
    }
}

/// The line that says a key went down or up: "down fire\n", "up left\n".
pub fn line(key: Key, down: bool) -> String {
    format!("{} {}\n", if down { "down" } else { "up" }, key.word())
}

/// Which keys the game was told are down.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Held([bool; 6]);

impl Held {
    /// The changes from what the game was told to `now` (each key and whether it is down now),
    /// as (key, down) in the order of `now`; afterwards the game is taken to know `now`.
    pub fn follow(&mut self, now: impl IntoIterator<Item = (Key, bool)>) -> Vec<(Key, bool)> {
        let mut changes = Vec::new();
        for (key, down) in now {
            let i = Key::ALL.iter().position(|k| *k == key).unwrap_or(0);
            if self.0[i] != down {
                self.0[i] = down;
                changes.push((key, down));
            }
        }
        changes
    }

    /// Every key up: the changes for the keys that were down.
    pub fn release(&mut self) -> Vec<(Key, bool)> {
        self.follow(Key::ALL.map(|k| (k, false)))
    }
}

/// The FIFO's name next to the panel's Wayland socket.
const FIFO: &str = "har-game-keys";

/// Where the FIFO is: HAR_GAME_KEYS, else next to the panel's Wayland socket (the runtime
/// directory XDG_RUNTIME_DIR, without one /data/local/tmp, as wayland_bridge resolves it). The
/// game finds it the same way.
pub fn fifo_path(keys: Option<&str>, runtime_dir: Option<&str>) -> String {
    match (keys, runtime_dir) {
        (Some(path), _) if !path.is_empty() => path.to_string(),
        (_, Some(dir)) if dir.starts_with('/') => format!("{}/{FIFO}", dir.trim_end_matches('/')),
        _ => format!("/data/local/tmp/{FIFO}"),
    }
}

/// The FIFO of this process's environment.
fn fifo() -> String {
    fifo_path(std::env::var("HAR_GAME_KEYS").ok().as_deref(), std::env::var("XDG_RUNTIME_DIR").ok().as_deref())
}

/// Opens the FIFO to write: never blocks (no game reading: ENXIO, no FIFO: ENOENT) and follows
/// no link planted in the directory.
fn open(path: &str) -> std::io::Result<std::fs::File> {
    let file = std::fs::OpenOptions::new().write(true).custom_flags(libc::O_NONBLOCK | libc::O_NOFOLLOW).open(path)?;
    if !file.metadata()?.file_type().is_fifo() {
        return Err(std::io::Error::other("not a FIFO"));
    }
    Ok(file)
}

/// Sends the changes to the game, all in one opening of the FIFO. An error when no game reads.
pub fn send(changes: &[(Key, bool)]) -> std::io::Result<()> {
    if changes.is_empty() {
        return Ok(());
    }
    let mut fifo = open(&fifo())?;
    for (key, down) in changes {
        fifo.write_all(line(*key, *down).as_bytes())?;
    }
    Ok(())
}

/// Whether a game reads the FIFO now (`reach`).
pub fn running() -> Reach {
    reach(open(&fifo()).err().map(|e| e.raw_os_error().unwrap_or(0)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use everyware_ui_core::Component;

    #[test]
    fn a_line_per_key_and_way() {
        assert_eq!(line(Key::Fire, true), "down fire\n");
        assert_eq!(line(Key::Left, false), "up left\n");
        assert_eq!(Key::ALL.map(Key::word), ["up", "down", "left", "right", "fire", "use"]);
        assert!(Key::ALL.iter().all(|k| line(*k, true).len() < 16), "one short write each");
    }

    #[test]
    fn only_the_changes_go_to_the_game() {
        let mut held = Held::default();
        assert_eq!(held.follow([(Key::Left, true), (Key::Fire, false)]), vec![(Key::Left, true)]);
        // moving and firing at once: the second finger adds a key, the first stays
        assert_eq!(held.follow([(Key::Left, true), (Key::Fire, true)]), vec![(Key::Fire, true)]);
        assert_eq!(held.follow([(Key::Left, false), (Key::Fire, true)]), vec![(Key::Left, false)]);
        assert_eq!(held.follow([(Key::Left, false), (Key::Fire, true)]), vec![]);
    }

    #[test]
    fn closing_lets_every_key_go() {
        let mut held = Held::default();
        held.follow([(Key::Up, true), (Key::Use, true)]);
        assert_eq!(held.release(), vec![(Key::Up, false), (Key::Use, false)]);
        assert_eq!(held.release(), vec![]);
        assert_eq!(held, Held::default());
    }

    /// The pad as game-patch.js draws it, its 72 px cells at (100, 200).
    fn pad() -> Pad {
        let cell = |cx: f32, cy: f32| Rect { x: 100.0 + cx * 72.0, y: 200.0 + cy * 72.0, w: 72.0, h: 72.0 };
        Pad { rects: [cell(1.0, 0.0), cell(1.0, 2.0), cell(0.0, 1.0), cell(2.0, 1.0)], ..Pad::default() }
    }

    #[test]
    fn a_thumb_rolls_from_one_direction_to_the_next() {
        let mut p = pad();
        let left = Pad::index("#game/left").unwrap();
        p.on(left, Gesture::down(130.0, 300.0));
        assert_eq!(p.held(), [false, false, true, false]);
        p.on(left, Gesture::Move { x: 140.0, y: 310.0, inside: true });
        assert_eq!(p.held(), [false, false, true, false], "still on Left");
        // onto Up: Left lets go, Up is held
        p.on(left, Gesture::Move { x: 200.0, y: 230.0, inside: false });
        assert_eq!(p.held(), [true, false, false, false]);
        // the middle of the pad: no direction
        p.on(left, Gesture::Move { x: 208.0, y: 308.0, inside: false });
        assert_eq!(p.held(), [false; 4]);
        // onto Right, then the lift
        p.on(left, Gesture::Move { x: 270.0, y: 300.0, inside: false });
        assert_eq!(p.held(), [false, false, false, true]);
        p.on(left, Gesture::Up { x: 270.0, y: 300.0, inside: false, velocity: 0.0, velocity_x: 0.0 });
        assert_eq!(p.held(), [false; 4]);
    }

    #[test]
    fn a_button_stays_held_while_any_thumb_is_on_it() {
        let mut p = pad();
        let (left, up) = (Pad::index("#game/left").unwrap(), Pad::index("#game/up").unwrap());
        // thumb B holds Up, thumb A rolls from Left onto Up and off it again
        p.on(up, Gesture::DOWN);
        p.on(left, Gesture::down(130.0, 300.0));
        p.on(left, Gesture::Move { x: 200.0, y: 230.0, inside: false });
        assert_eq!(p.held(), [true, false, false, false]);
        p.on(left, Gesture::Move { x: 130.0, y: 300.0, inside: false });
        assert_eq!(p.held(), [true, false, true, false], "B still holds Up");
        assert!(p.buttons[up].press.pressed, "its pressed layer too");
        p.on(up, Gesture::Up { x: 0.0, y: 0.0, inside: true, velocity: 0.0, velocity_x: 0.0 });
        assert_eq!(p.held(), [false, false, true, false]);
        assert!(!p.buttons[up].press.pressed);
    }

    #[test]
    fn the_fifo_says_whether_a_game_runs() {
        assert_eq!(reach(None), Reach::Runs);
        assert_eq!(reach(Some(libc::EACCES)), Reach::KeysBlocked, "an enforcing board without a rule");
        assert_eq!(reach(Some(libc::EPERM)), Reach::KeysBlocked);
        assert_eq!(reach(Some(libc::ENXIO)), Reach::NotRunning, "nobody reads");
        assert_eq!(reach(Some(libc::ENOENT)), Reach::NotRunning, "no FIFO");
        assert!(Reach::KeysBlocked.runs() && Reach::Runs.runs() && !Reach::NotRunning.runs());
    }

    #[test]
    fn two_thumbs_and_a_pad_without_places() {
        let mut p = pad();
        let (up, down) = (Pad::index("#game/up").unwrap(), Pad::index("#game/down").unwrap());
        p.on(up, Gesture::DOWN);
        p.on(down, Gesture::DOWN);
        assert_eq!(p.held(), [true, true, false, false]);
        p.on(down, Gesture::Cancel);
        assert_eq!(p.held(), [true, false, false, false]);
        p.release();
        assert_eq!(p.held(), [false; 4]);
        // no places known: a finger holds its button while on it, none off it
        let mut p = Pad::default();
        p.on(up, Gesture::DOWN);
        p.on(up, Gesture::Move { x: 0.0, y: 0.0, inside: false });
        assert_eq!(p.held(), [false; 4]);
        assert_eq!(Pad::index("#game/fire"), None);
        assert_eq!((status(Reach::Runs), status(Reach::NotRunning)), ("Starting the game", "The game is not running"));
        assert_eq!(status(Reach::KeysBlocked), "The game runs, but the panel may not send it keys");
        assert_eq!(Pad::default().buttons[0].variant(), "kind=scrim, state=default, icon=chevron-up, size=regular", "the design's");
        assert!(in_image("true"));
        assert!(!in_image("") && !in_image("false") && !in_image("1"));
    }

    #[test]
    fn the_fifo_lies_next_to_the_wayland_socket() {
        assert_eq!(fifo_path(None, None), "/data/local/tmp/har-game-keys");
        assert_eq!(fifo_path(None, Some("/data/vendor/everyware/run")), "/data/vendor/everyware/run/har-game-keys");
        assert_eq!(fifo_path(None, Some("/run/")), "/run/har-game-keys");
        assert_eq!(fifo_path(Some("/tmp/keys"), Some("/run")), "/tmp/keys");
        assert_eq!(fifo_path(Some(""), None), "/data/local/tmp/har-game-keys");
        assert_eq!(fifo_path(None, Some("relative")), "/data/local/tmp/har-game-keys", "as the bridge: only an absolute runtime directory");
    }

    #[test]
    fn without_a_game_nothing_blocks() {
        let dir = std::env::temp_dir().join(format!("game-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("keys");
        let path_str = path.to_str().unwrap();
        // no FIFO
        assert!(open(path_str).is_err());
        // a FIFO nobody reads: ENXIO at once, not a wait for a reader
        let c = std::ffi::CString::new(path_str).unwrap();
        // SAFETY: a valid C string; mkfifo only creates the file.
        assert_eq!(unsafe { libc::mkfifo(c.as_ptr(), 0o600) }, 0);
        assert_eq!(open(path_str).unwrap_err().raw_os_error(), Some(libc::ENXIO));
        // a reader: the lines arrive whole
        let reader = std::fs::OpenOptions::new().read(true).custom_flags(libc::O_NONBLOCK).open(&path).unwrap();
        let mut fifo = open(path_str).unwrap();
        fifo.write_all(line(Key::Use, true).as_bytes()).unwrap();
        drop(fifo);
        let mut got = String::new();
        std::io::Read::read_to_string(&mut &reader, &mut got).unwrap();
        assert_eq!(got, "down use\n");
        // a plain file is no FIFO
        let plain = dir.join("plain");
        std::fs::write(&plain, b"").unwrap();
        assert!(open(plain.to_str().unwrap()).is_err());
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
