//! The panel's browser: Cog in the Debian root (vendor/sdv-everyware/everyware docs/project/
//! browser.md), a Wayland client of this app shown in #panel/browser (ui_model.rs
//! ZONE_BROWSER_TAG). A tap on a news story opens its page there.
//!
//! The page goes to the browser as one line in a FIFO; a relay inside the root (uid browser,
//! run-browser-rb3.sh) reads it and runs `cogctl open`, which reaches Cog over a D-Bus session
//! bus that only exists inside the root:
//!
//! ```text
//! smarthome_app ──"https://…\n"──► FIFO har-browser-open ──► relay ──cogctl──► cog
//!   (root, writes)                 (0660 root:browser)       (uid browser, D-Bus in the root)
//! ```
//!
//! The URL comes from the news bundle, that is from the web: only http and https reach the
//! FIFO (and about:blank, sent by the app itself), no control characters or spaces (a line
//! is a message), at most MAX_URL bytes (one write below PIPE_BUF is atomic). The relay
//! checks the scheme again.
//!
//! Back the other way, a second FIFO says how the page is doing, for the loader over the
//! browser (state.rs BrowserPanel): har-browser-events.sh reads Cog's output in the root and
//! writes one word a line. The writer is the browser's user, so the app takes nothing else
//! from it: three known words, short lines, everything else dropped.
//!
//! ```text
//! cog ──stdout──► har-browser-events (uid browser) ──"committed\n"──► FIFO har-browser-events
//!                                                                     (0660 root:browser)
//!     ──► events thread here ──► HomeAction::Browser(BrowserEvent) ──► the reducer
//! ```

use crate::actions::HomeAction;
use harry::reducer::Actions;
use std::io::{Read, Write};
use std::os::unix::fs::{FileTypeExt, OpenOptionsExt};
use std::sync::mpsc::Sender;
use std::time::Duration;

/// The page that leaves the browser empty and quiet: sent when the panel closes, so a video
/// stops playing (the hidden zone stops the frames, not the page or its sound).
pub const BLANK: &str = "about:blank";
/// The longest URL sent (Hacker News' own limit is lower; PIPE_BUF is 4096).
const MAX_URL: usize = 2048;

/// A page the browser can open: what the panel's title bar says, and where it is.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Page {
    pub title: String,
    pub url: String,
}

impl Page {
    /// A news story's page: its article, or its discussion on Hacker News when it has no link
    /// (Ask HN).
    pub fn of(story: &everyware_dashboard_grpc::news::NewsStory) -> Page {
        let url = if story.url.is_empty() {
            format!("https://news.ycombinator.com/item?id={}", story.id)
        } else {
            story.url.clone()
        };
        Page { title: story.title.clone(), url }
    }
}

/// An address the browser may get from outside: http or https, one line, not too long.
pub fn is_web(url: &str) -> bool {
    let lower = url.get(..8).unwrap_or(url).to_ascii_lowercase();
    (lower.starts_with("https://") || lower.starts_with("http://"))
        && url.len() <= MAX_URL
        && !url.chars().any(|c| c.is_control() || c.is_whitespace())
}

/// The host of a web address ("example.org"), or the address itself (about:blank).
pub fn host(url: &str) -> &str {
    let rest = url.split_once("://").map_or(url, |(_, rest)| rest);
    rest.split(['/', '?', '#']).next().unwrap_or(rest)
}

/// The FIFO next to the Wayland socket (the master's runtime directory, /run/har in the root).
fn fifo_path() -> String {
    std::env::var("HAR_BROWSER_FIFO").unwrap_or_else(|_| "/data/local/tmp/har-browser-open".to_string())
}

/// Asks the browser to open `url`. Never blocks: without a relay (no browser started) the
/// page is dropped with a warning. Called from the reducer, as house_link's sends.
pub fn open(url: &str) {
    if url != BLANK && !is_web(url) {
        log::warn!("smarthome: browser: not a web address ({} bytes), not sent", url.len());
        return;
    }
    let path = fifo_path();
    // O_NONBLOCK: open fails with ENXIO when no relay reads, instead of waiting for one;
    // O_NOFOLLOW: a link planted in the directory does not redirect the write.
    let sent = std::fs::OpenOptions::new()
        .write(true)
        .custom_flags(libc::O_NONBLOCK | libc::O_NOFOLLOW)
        .open(&path)
        .and_then(|mut fifo| {
            if !fifo.metadata()?.file_type().is_fifo() {
                return Err(std::io::Error::other("not a FIFO"));
            }
            fifo.write_all(format!("{url}\n").as_bytes())
        });
    match sent {
        // the host only: logcat is readable over adb, the full address is the household's reading
        Ok(()) => log::info!("smarthome: browser: open a page on {}", host(url)),
        Err(e) => log::warn!("smarthome: browser: {path}: {e} (is the browser's relay running?)"),
    }
}

/// What Cog reported about the page it was asked for (har-browser-events.sh).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BrowserEvent {
    /// The first answer came and the page replaced the old one (WebKit's load committed).
    Committed,
    /// The page and what it loads are in.
    Loaded,
    /// The page could not be loaded (Cog shows its error page).
    Failed,
}

impl BrowserEvent {
    /// The word of one line, or None for anything else.
    pub fn parse(line: &[u8]) -> Option<BrowserEvent> {
        match line {
            b"committed" => Some(BrowserEvent::Committed),
            b"loaded" => Some(BrowserEvent::Loaded),
            b"failed" => Some(BrowserEvent::Failed),
            _ => None,
        }
    }
}

/// The longest line taken from the events FIFO; a longer one is dropped whole.
const MAX_EVENT: usize = 32;

/// The events FIFO next to the other one.
fn events_path() -> String {
    std::env::var("HAR_BROWSER_EVENTS").unwrap_or_else(|_| "/data/local/tmp/har-browser-events".to_string())
}

/// Reads the browser's events on a thread of its own and hands them to the reducer. Waits for
/// the FIFO (the browser may start after the app); opened read-write, so a browser that
/// restarts is no end of file.
pub fn spawn_events(actions: Sender<Actions<HomeAction>>) {
    std::thread::spawn(move || loop {
        let path = events_path();
        let fifo = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .custom_flags(libc::O_NOFOLLOW)
            .open(&path)
            .and_then(|f| if f.metadata()?.file_type().is_fifo() { Ok(f) } else { Err(std::io::Error::other("not a FIFO")) });
        let Ok(mut fifo) = fifo else {
            std::thread::sleep(Duration::from_secs(2));
            continue;
        };
        log::info!("smarthome: browser: events from {path}");
        let (mut line, mut buf, mut too_long) = (Vec::with_capacity(MAX_EVENT), [0u8; 256], false);
        while let Ok(n) = fifo.read(&mut buf) {
            if n == 0 {
                break;
            }
            for &b in &buf[..n] {
                if b == b'\n' {
                    match BrowserEvent::parse(&line) {
                        Some(event) if !too_long => {
                            if actions.send(Actions::CustomAction(HomeAction::Browser(event))).is_err() {
                                return;
                            }
                        }
                        _ => log::warn!("smarthome: browser: an unknown event dropped ({} bytes)", line.len()),
                    }
                    line.clear();
                    too_long = false;
                } else if line.len() < MAX_EVENT {
                    line.push(b);
                } else {
                    too_long = true;
                }
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn web_addresses_only() {
        assert!(is_web("https://example.org/a?b=c#d"));
        assert!(is_web("HTTP://example.org"));
        assert!(!is_web("javascript:alert(1)"));
        assert!(!is_web("file:///etc/passwd"));
        assert!(!is_web("https://example.org/\nhttps://evil.example"));
        assert!(!is_web("https://example.org/ a"));
        assert!(!is_web(&format!("https://{}", "a".repeat(MAX_URL))));
        assert!(!is_web(""));
    }

    #[test]
    fn three_words_only() {
        assert_eq!(BrowserEvent::parse(b"committed"), Some(BrowserEvent::Committed));
        assert_eq!(BrowserEvent::parse(b"loaded"), Some(BrowserEvent::Loaded));
        assert_eq!(BrowserEvent::parse(b"failed"), Some(BrowserEvent::Failed));
        assert_eq!(BrowserEvent::parse(b"loaded "), None);
        assert_eq!(BrowserEvent::parse(b"https://evil.example"), None);
        assert_eq!(BrowserEvent::parse(b""), None);
    }

    #[test]
    fn host_of() {
        assert_eq!(host("https://example.org/a?b"), "example.org");
        assert_eq!(host("http://example.org"), "example.org");
        assert_eq!(host("about:blank"), "about:blank");
    }
}
