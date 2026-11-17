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
//! the panel sends nothing else. Nothing here needs a device: its tests run on the host
//! (smarthome_app_host_test).

use std::io::Write;
use std::os::unix::fs::{FileTypeExt, OpenOptionsExt};

/// The zone of the game's picture: the tag the game names in har_zone (doom-app --zone).
pub const ZONE_TAG: &str = "#zone/game";

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

/// Whether a game reads the FIFO now: it runs, and its picture comes into the zone.
pub fn running() -> bool {
    open(&fifo()).is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

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
