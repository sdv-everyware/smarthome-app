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

//! The agenda's state (#agenda, agenda.rs for what it shows): the calendar bundle's last agenda,
//! its rows in the list (made for the time zone, the clock's format and the minute they show),
//! the QR code of the setup page as a picture, and the panel of an event (#event, a library sheet
//! at the right edge) that a tap on its row opens.
//!
//! - The rows are made again at a new minute (an event ends, the day changes at midnight), at a
//!   new zone or clock format, and with each agenda; the list is set only when they changed. On
//!   a new day the list scrolls back to its top, so TODAY shows.
//! - The QR code's picture is made once per address and freed once no address is left (linked);
//!   the address carries a one-time token and is never logged.
//! - The event's panel closes on its scrim or its close button, and when a new agenda no longer
//!   has the event; while it is open, a new agenda brings the event's new details. Its body
//!   scrolls (a list of one row as high as the body: Android's drag and fling; ui-lib's `Scrolled`
//!   draws it on the fling's curve, so a finger that catches a fling stops it where it shows).

use crate::agenda::{self, Details, QR_KEY, QR_SIDE};
use everyware_catalog::calendar::{Agenda, CalendarLink};
use everyware_ui::button::{Icon, IconButton, IconButtonKind};
use everyware_ui::images;
use everyware_ui::list_item::ListItem;
use everyware_ui::list_view::{ListEvent, ListView};
use everyware_ui::sheet::{Edge, Layout, Sheet};
use everyware_ui::Gesture;
use jiff::civil::Date;
use jiff::tz::TimeZone;
use jiff::Timestamp;
use std::time::Instant;

/// How the event's panel shows: half the screen at the right edge (the design has #event in
/// that layout).
pub const EVENT_LAYOUT: Layout = Layout::Panel(Edge::Right);

/// How the app measures text (text_measure.rs on the device; the host tests count characters):
/// the lines a headline takes in a row, and how high the panel's body is for some details.
#[derive(Clone, Copy, Debug)]
pub struct Measure {
    pub headline_lines: fn(&str) -> usize,
    pub body_height: fn(&Details) -> f32,
}

/// The agenda card and the panel of its events.
#[derive(Debug)]
pub struct AgendaCard {
    /// None until the first agenda came.
    pub agenda: Option<Agenda>,
    pub list: ListView,
    /// The event of each row (an index into the agenda's events), none for a subheader.
    events: Vec<Option<usize>>,
    /// The zone of the rows: its id (the settings') and the zone, made again when the id changes.
    zone: Option<(String, TimeZone)>,
    zone_of: fn(&str) -> TimeZone,
    measure: Measure,
    /// The clock's format, the minute (Unix time / 60) and the day of the rows.
    hour24: bool,
    minute: i64,
    today: Option<Date>,
    /// The address whose QR code is registered (agenda::QR_KEY); empty: none.
    qr: String,
    pub event: EventPanel,
}

/// The panel of an event (#event): the library's sheet with the app's content (#event-content).
#[derive(Debug)]
pub struct EventPanel {
    pub sheet: Sheet,
    pub close: IconButton,
    /// The event shown, by its id; it stays while the panel slides out.
    pub id: Option<String>,
    pub details: Details,
    /// The body's scroll: one row as high as the body.
    pub body: ListView,
    /// How high the body's place is (the design's #event/scroll).
    view_h: f32,
}

impl EventPanel {
    fn new(view_h: f32) -> Self {
        EventPanel {
            sheet: Sheet::new(EVENT_LAYOUT),
            close: IconButton::new(IconButtonKind::Tonal, Icon::Close),
            id: None,
            details: Details::default(),
            body: ListView::new([], view_h, 0.0),
            view_h,
        }
    }

    /// Shows `details` from the top of the body, `height` high.
    fn show(&mut self, id: String, details: Details, height: f32) {
        self.id = Some(id);
        self.details = details;
        self.body = ListView::new([ListItem::default()], self.view_h, height.max(0.0));
        self.sheet.open();
    }

    /// The same event's new details: shown from the top of the body when they changed.
    fn update(&mut self, details: Details, height: f32) {
        if details != self.details {
            self.details = details;
            self.body = ListView::new([ListItem::default()], self.view_h, height.max(0.0));
        }
    }

    /// Closes the panel (it slides out with the event's details on it).
    pub fn close(&mut self) {
        self.sheet.close();
    }

    /// Whether it is open.
    pub fn is_open(&self) -> bool {
        self.sheet.is_open()
    }

    /// A gesture on the body: a drag or a fling scrolls it.
    pub fn on_body(&mut self, gesture: Gesture, now: Instant) {
        let _ = self.body.on(None, gesture, now);
    }
}

impl Default for AgendaCard {
    fn default() -> Self {
        AgendaCard::new(0.0, 0.0, Measure { headline_lines: |_| 1, body_height: |_| 0.0 }, |_| TimeZone::UTC)
    }
}

impl AgendaCard {
    /// No agenda yet: the list `list_h` high and the event's body `body_h` (the design's), text
    /// measured by `measure`, zones from `zone_of`.
    pub fn new(list_h: f32, body_h: f32, measure: Measure, zone_of: fn(&str) -> TimeZone) -> Self {
        AgendaCard {
            agenda: None,
            list: ListView::new([], list_h, agenda::ROW_PITCH),
            events: Vec::new(),
            zone: None,
            zone_of,
            measure,
            hour24: true,
            minute: 0,
            today: None,
            qr: String::new(),
            event: EventPanel::new(body_h),
        }
    }

    /// A new agenda from the calendar bundle: its rows, the QR code of its setup address while it
    /// has one, the open event's new details or, when the event is gone, the panel closed.
    pub fn set(&mut self, agenda: Agenda, zone: &str, hour24: bool, now: Timestamp) {
        self.show_qr(agenda::link_card(Some(&agenda), true).and_then(|card| card.qr));
        self.agenda = Some(agenda);
        self.refresh(zone, hour24, now);
        if let Some(id) = self.event.id.clone().filter(|_| self.event.is_open()) {
            match self.event_by_id(&id) {
                Some(i) => {
                    let details = self.details_of(i);
                    let height = (self.measure.body_height)(&details);
                    self.event.update(details, height);
                }
                None => self.event.close(),
            }
        }
    }

    /// After every action: the rows follow a new minute, another zone or the other clock.
    pub fn follow(&mut self, zone: &str, hour24: bool, now: Timestamp) {
        let same_zone = self.zone.as_ref().is_some_and(|(id, _)| id == zone);
        if !same_zone || hour24 != self.hour24 || now.as_second().div_euclid(60) != self.minute {
            self.refresh(zone, hour24, now);
        }
    }

    /// A gesture on the list: a drag or a fling scrolls it, a tap on an event's row opens its
    /// panel (a subheader takes no tap, the line of a free day opens nothing).
    pub fn on_list(&mut self, row: Option<usize>, gesture: Gesture, now: Instant) {
        if let Some(ListEvent::Chosen(row)) = self.list.on(row, gesture, now) {
            if let Some(i) = self.event_of_row(row) {
                self.open(i);
            }
        }
    }

    /// The event of row `row`, an index into the agenda's events.
    pub fn event_of_row(&self, row: usize) -> Option<usize> {
        self.events.get(row).copied().flatten()
    }

    /// The QR code's picture while there is one.
    pub fn qr_picture(&self) -> Option<customizations::ImageContentData> {
        if self.qr.is_empty() { None } else { images::get(QR_KEY) }
    }

    /// The zone the rows are made in.
    pub fn time_zone(&self) -> TimeZone {
        self.zone.as_ref().map_or(TimeZone::UTC, |(_, tz)| tz.clone())
    }

    fn open(&mut self, i: usize) {
        let Some(id) = self.agenda.as_ref().and_then(|a| a.events.get(i)).map(|e| e.id.clone()) else { return };
        let details = self.details_of(i);
        let height = (self.measure.body_height)(&details);
        self.event.show(id, details, height);
    }

    fn details_of(&self, i: usize) -> Details {
        let tz = self.time_zone();
        let today = self.today.unwrap_or_else(|| Timestamp::now().to_zoned(tz.clone()).date());
        match self.agenda.as_ref().and_then(|a| a.events.get(i)) {
            Some(event) => agenda::details(event, today, &tz, self.hour24),
            None => Details::default(),
        }
    }

    /// The index of the event with `id` in a linked agenda.
    fn event_by_id(&self, id: &str) -> Option<usize> {
        let agenda = self.agenda.as_ref().filter(|a| a.link.enum_value() == Ok(CalendarLink::CALENDAR_CONNECTED))?;
        agenda.events.iter().position(|e| e.id == id)
    }

    /// The rows for `now`; the list is set only when they changed (harry builds the rows of a list
    /// that is set again), and scrolls back to its top on a new day.
    fn refresh(&mut self, zone: &str, hour24: bool, now: Timestamp) {
        if self.zone.as_ref().is_none_or(|(id, _)| id != zone) {
            self.zone = Some((zone.to_string(), (self.zone_of)(zone)));
        }
        (self.hour24, self.minute) = (hour24, now.as_second().div_euclid(60));
        let tz = self.time_zone();
        let today = now.to_zoned(tz.clone()).date();
        let Some(agenda) = &self.agenda else { return };
        let rows = agenda::rows(agenda, now, &tz, hour24, &self.measure.headline_lines);
        self.events = rows.events;
        if rows.items.as_slice() != self.list.rows() {
            self.list.set_rows(rows.items);
        }
        if self.today.is_some_and(|d| d != today) {
            self.list.scroll_to(0);
        }
        self.today = Some(today);
    }

    /// Makes the QR code of `url` the card's picture, once per address; none frees it. The
    /// address carries a one-time token: it is not logged.
    fn show_qr(&mut self, url: Option<String>) {
        match url {
            Some(url) if url == self.qr => {}
            Some(url) => match agenda::qr_rgba(&url, QR_SIDE).map(|rgba| images::register_rgba(QR_KEY, rgba, QR_SIDE, QR_SIDE)) {
                Some(Ok(())) => self.qr = url,
                failed => {
                    match failed {
                        Some(Err(err)) => log::warn!("smarthome: the agenda's QR code: {err}"),
                        _ => log::warn!("smarthome: the agenda's setup address ({} bytes) does not fit a QR code", url.len()),
                    }
                    // no picture of an older address stays: its token is gone
                    images::unregister(QR_KEY);
                    self.qr.clear();
                }
            },
            None if !self.qr.is_empty() => {
                images::unregister(QR_KEY);
                self.qr.clear();
            }
            None => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use everyware_catalog::calendar::AgendaEvent;

    const CET: &str = "CET-1CEST,M3.5.0,M10.5.0/3";

    fn zone_of(id: &str) -> TimeZone {
        if id == "Europe/Berlin" { TimeZone::posix(CET).unwrap() } else { TimeZone::UTC }
    }

    /// A headline of more than 30 characters takes two lines; a body is 30 px a line of text.
    fn measure() -> Measure {
        Measure {
            headline_lines: |t| if t.chars().count() > 30 { 2 } else { 1 },
            body_height: |d| 30.0 * (1 + d.description.lines().count()) as f32,
        }
    }

    fn card() -> AgendaCard {
        AgendaCard::new(400.0, 600.0, measure(), zone_of)
    }

    fn at(s: &str) -> Timestamp {
        s.parse().unwrap()
    }

    fn event(id: &str, start: &str, minutes: i64) -> AgendaEvent {
        let mut e = AgendaEvent::new();
        e.id = id.to_string();
        e.title = format!("Event {id}");
        e.start = at(start).as_second();
        e.end = e.start + minutes * 60;
        e
    }

    /// A linked agenda, read at 08:00 UTC on 10 November.
    fn linked(events: Vec<AgendaEvent>) -> Agenda {
        let mut a = Agenda::new();
        a.link = CalendarLink::CALENDAR_CONNECTED.into();
        a.events = events;
        a.updated = at("2026-11-10T08:00:00Z").as_second();
        a
    }

    fn unlinked(url: &str) -> Agenda {
        let mut a = Agenda::new();
        a.link = CalendarLink::CALENDAR_NOT_CONNECTED.into();
        a.setup_url = url.to_string();
        a
    }

    fn titles(card: &AgendaCard) -> Vec<String> {
        card.list.rows().iter().map(|r| r.leading_title.clone() + "|" + &r.title).collect()
    }

    #[test]
    fn the_qr_picture_lives_as_long_as_its_address() {
        let mut c = card();
        let now = at("2026-11-10T09:00:00Z");
        c.set(unlinked("http://10.0.0.2:8765/setup?t=one"), "UTC", true, now);
        let first = c.qr_picture().expect("a picture for the address");
        // the same address again: the same picture, nothing made again
        c.set(unlinked("http://10.0.0.2:8765/setup?t=one"), "UTC", true, now);
        assert_eq!(c.qr_picture().map(|p| p.id), Some(first.id));
        // a new token: the picture changes in its slot
        c.set(unlinked("http://10.0.0.2:8765/setup?t=two"), "UTC", true, now);
        assert_eq!(c.qr, "http://10.0.0.2:8765/setup?t=two");
        assert!(images::get(QR_KEY).is_some());
        // no address yet: the old code goes
        c.set(unlinked(""), "UTC", true, now);
        assert!(c.qr_picture().is_none() && images::get(QR_KEY).is_none());
        // linked: none
        c.set(unlinked("http://10.0.0.2:8765/setup?t=three"), "UTC", true, now);
        assert!(c.qr_picture().is_some());
        c.set(linked(vec![]), "UTC", true, now);
        assert!(c.qr_picture().is_none() && images::get(QR_KEY).is_none());
        // an address too long for a code: none, and no older one left
        c.set(unlinked("http://10.0.0.2:8765/setup?t=four"), "UTC", true, now);
        c.set(unlinked(&format!("http://10.0.0.2:8765/setup?t={}", "x".repeat(4000))), "UTC", true, now);
        assert!(c.qr_picture().is_none() && images::get(QR_KEY).is_none());
    }

    #[test]
    fn the_rows_follow_the_minute_the_zone_and_the_clock() {
        let mut c = card();
        // 22:30 UTC: the event at 23:15 is today's in UTC, tomorrow's in Berlin
        let now = at("2026-11-10T22:30:00Z");
        c.set(linked(vec![event("a", "2026-11-10T22:15:00Z", 30), event("b", "2026-11-10T23:15:00Z", 30)]), "UTC", true, now);
        assert_eq!(titles(&c), ["|TODAY", "22:15|Event a", "23:15|Event b"]);
        // the same minute: nothing made again
        c.follow("UTC", true, at("2026-11-10T22:30:40Z"));
        assert_eq!(titles(&c), ["|TODAY", "22:15|Event a", "23:15|Event b"]);
        // the next minute: a ends at 22:45
        c.follow("UTC", true, at("2026-11-10T22:45:00Z"));
        assert_eq!(titles(&c), ["|TODAY", "23:15|Event b"]);
        // the other clock
        c.follow("UTC", false, at("2026-11-10T22:45:10Z"));
        assert_eq!(titles(&c), ["|TODAY", "11:15 PM|Event b"]);
        // another zone: at 23:05 UTC it is the 11th in Berlin, b at 00:15
        c.follow("Europe/Berlin", true, at("2026-11-10T23:05:00Z"));
        assert_eq!(titles(&c), ["|TODAY", "00:15|Event b"]);
        assert_eq!(c.list.rows()[0].trailing_text, "NOV 11");
    }

    #[test]
    fn a_new_day_scrolls_the_list_back_to_today() {
        let mut c = card();
        let events = (0..12).map(|i| event(&format!("e{i}"), "2026-11-11T10:00:00Z", 30)).collect();
        c.set(linked(events), "UTC", true, at("2026-11-10T23:58:00Z"));
        c.list.scroll_to(3);
        assert!(c.list.scroll_at(Instant::now()) > 0.0);
        // the same day: the place stays
        c.follow("UTC", true, at("2026-11-10T23:59:00Z"));
        assert!(c.list.scroll_at(Instant::now()) > 0.0);
        // midnight: TODAY is the 11th and the list is at its top
        c.follow("UTC", true, at("2026-11-11T00:00:00Z"));
        assert_eq!(c.list.rows()[0].trailing_text, "NOV 11");
        assert_eq!(c.list.scroll_at(Instant::now()), 0.0);
    }

    #[test]
    fn a_new_agenda_never_scrolls_the_list() {
        let mut c = card();
        let now = at("2026-11-10T08:00:00Z");
        let events = |n: usize| (0..n).map(|i| event(&format!("e{i}"), "2026-11-10T09:00:00Z", 30)).collect();
        c.set(linked(events(3)), "UTC", true, now);
        assert_eq!((c.list.scroll_at(Instant::now()), c.list.offset(0), c.list.offset(1)), (0.0, 0.0, agenda::SUBHEADER_PITCH));
        c.set(linked(events(20)), "UTC", true, now);
        c.set(linked(events(2)), "UTC", true, now);
        assert_eq!(c.list.scroll_at(Instant::now()), 0.0, "TODAY stays at the top");
    }

    #[test]
    fn a_tap_on_an_event_opens_its_panel_and_a_subheader_nothing() {
        let mut c = card();
        let now = at("2026-11-10T08:00:00Z");
        let mut e = event("a", "2026-11-10T09:00:00Z", 30);
        e.description = "One\nTwo".to_string();
        c.set(linked(vec![e, event("b", "2026-11-11T09:00:00Z", 30)]), "UTC", true, now);
        assert_eq!((c.event_of_row(0), c.event_of_row(1), c.event_of_row(2), c.event_of_row(3)), (None, Some(0), None, Some(1)));
        let tap = |c: &mut AgendaCard, row: usize| {
            let t = Instant::now();
            c.on_list(Some(row), Gesture::down(0.0, 10.0), t);
            c.on_list(Some(row), Gesture::up(0.0, 10.0), t);
        };
        tap(&mut c, 0);
        assert!(!c.event.is_open(), "a subheader opens nothing");
        tap(&mut c, 1);
        assert!(c.event.is_open());
        assert_eq!(c.event.id.as_deref(), Some("a"));
        assert_eq!(c.event.details.title, "Event a");
        assert_eq!(c.event.details.when, "Tue, Nov 10 · 09:00 to 09:30");
        assert_eq!(c.event.body.content_h(), 90.0, "the body as high as it was measured");
        c.event.close();
        assert!(!c.event.is_open());
        assert_eq!(c.event.details.title, "Event a", "the details stay while it slides out");
    }

    #[test]
    fn a_finger_on_a_row_stays_on_its_event_when_titles_repeat() {
        let mut c = card();
        let standup = |id: &str, start: &str| {
            let mut e = event(id, start, 15);
            e.title = "Stand-up".to_string();
            e
        };
        c.set(linked(vec![standup("mon", "2026-11-10T09:00:00Z"), standup("tue", "2026-11-11T09:00:00Z")]), "UTC", true, at("2026-11-10T08:00:00Z"));
        assert_eq!(titles(&c), ["|TODAY", "09:00|Stand-up", "|TOMORROW", "09:00|Stand-up"]);
        // the finger goes down on tomorrow's
        let t = Instant::now();
        c.on_list(Some(3), Gesture::down(0.0, 10.0), t);
        // a new agenda before the lift: today's is over, another stand-up came today
        c.set(linked(vec![standup("extra", "2026-11-10T11:00:00Z"), standup("tue", "2026-11-11T09:00:00Z")]), "UTC", true, at("2026-11-10T09:20:00Z"));
        assert_eq!(titles(&c), ["|TODAY", "11:00|Stand-up", "|TOMORROW", "09:00|Stand-up"]);
        assert!(!c.list.rows()[1].press.pressed && c.list.rows()[3].press.pressed, "the press stays on tomorrow's");
        c.on_list(Some(3), Gesture::up(0.0, 10.0), t);
        assert_eq!(c.event.id.as_deref(), Some("tue"));
    }

    #[test]
    fn an_agenda_not_read_yet_says_so_and_its_line_takes_no_tap() {
        let mut c = card();
        let mut a = linked(vec![]);
        a.updated = 0;
        c.set(a.clone(), "UTC", true, at("2026-11-10T08:00:00Z"));
        assert_eq!(titles(&c), ["|Reading the calendar"]);
        let t = Instant::now();
        c.on_list(Some(0), Gesture::down(0.0, 10.0), t);
        assert!(!c.list.rows()[0].press.pressed, "no pressed layer");
        c.on_list(Some(0), Gesture::up(0.0, 10.0), t);
        assert!(!c.event.is_open());
        a.read_failed = true;
        c.set(a, "UTC", true, at("2026-11-10T08:01:00Z"));
        assert_eq!(titles(&c), ["|The calendar could not be read. Trying again."]);
        // read: the day and its events
        c.set(linked(vec![event("a", "2026-11-10T09:00:00Z", 30)]), "UTC", true, at("2026-11-10T08:02:00Z"));
        assert_eq!(titles(&c), ["|TODAY", "09:00|Event a"]);
    }

    #[test]
    fn a_new_agenda_without_the_event_closes_its_panel() {
        let mut c = card();
        let now = at("2026-11-10T08:00:00Z");
        c.set(linked(vec![event("a", "2026-11-10T09:00:00Z", 30)]), "UTC", true, now);
        let t = Instant::now();
        c.on_list(Some(1), Gesture::down(0.0, 10.0), t);
        c.on_list(Some(1), Gesture::up(0.0, 10.0), t);
        assert!(c.event.is_open());
        // the same event, renamed: the panel stays and shows the new title
        let mut renamed = event("a", "2026-11-10T09:00:00Z", 30);
        renamed.title = "Renamed".to_string();
        c.set(linked(vec![event("x", "2026-11-10T10:00:00Z", 30), renamed]), "UTC", true, now);
        assert!(c.event.is_open());
        assert_eq!(c.event.details.title, "Renamed");
        // gone: closed
        c.set(linked(vec![event("x", "2026-11-10T10:00:00Z", 30)]), "UTC", true, now);
        assert!(!c.event.is_open());
        // and an agenda no longer linked closes it too
        c.on_list(Some(1), Gesture::down(0.0, 10.0), t);
        c.on_list(Some(1), Gesture::up(0.0, 10.0), t);
        assert!(c.event.is_open());
        c.set(unlinked(""), "UTC", true, now);
        assert!(!c.event.is_open());
    }

    #[test]
    fn the_body_scrolls_as_far_as_it_reaches() {
        let mut c = card();
        let now = at("2026-11-10T08:00:00Z");
        let mut e = event("a", "2026-11-10T09:00:00Z", 30);
        e.description = (0..40).map(|i| format!("line {i}")).collect::<Vec<_>>().join("\n");
        c.set(linked(vec![e]), "UTC", true, now);
        let t = Instant::now();
        c.on_list(Some(1), Gesture::down(0.0, 10.0), t);
        c.on_list(Some(1), Gesture::up(0.0, 10.0), t);
        // 41 lines of 30: 1230 high in a place of 600
        assert_eq!(c.event.body.max_scroll(), 630.0);
        c.event.on_body(Gesture::down(0.0, 500.0), t);
        c.event.on_body(Gesture::Move { x: 0.0, y: 100.0, inside: true }, t);
        assert_eq!(c.event.body.scroll_at(t), 400.0 - everyware_ui::scroller::TOUCH_SLOP);
        // a fast lift: it flings to an end within the body
        c.event.on_body(Gesture::Up { x: 0.0, y: 100.0, inside: true, velocity: -3000.0, velocity_x: 0.0 }, t);
        let fling = c.event.body.scroll().fling.expect("a fling");
        let end = c.event.body.scroll_at(fling.ends_at());
        assert!(end > 400.0 && end <= 630.0, "{end}");
        // a finger that catches it stops it where the fling is, which is where it is drawn
        let caught = t + std::time::Duration::from_millis(100);
        let there = c.event.body.scroll_at(caught);
        c.event.on_body(Gesture::down(0.0, 300.0), caught);
        assert_eq!(c.event.body.scroll_at(caught + std::time::Duration::from_secs(1)), there);
    }
}
