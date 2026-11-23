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
//! - The panel's steps (`EventStep`, pages that slide: ui-lib Pages): the details with Accept,
//!   Maybe and Decline when the account may answer, Join on phone and Directions (a QR code
//!   each, for the phone to open: the panel opens nothing) and Delete when it may delete (after
//!   a confirmation); and the calendar's link, from the header's button while a calendar is
//!   linked, with Change access and Disconnect (each after a confirmation). A change is a
//!   command to the calendar bundle; the panel waits for its answer with its controls off and
//!   says why it failed.

use crate::agenda::{self, Details, Offers, QR_KEY, QR_SIDE};
use everyware_catalog::calendar::{Agenda, CalendarLink, Rsvp};
use everyware_catalog::calendar_service::{CalendarCommand, CalendarDelete, CalendarDisconnect, CalendarFailure, CalendarRespond, CalendarResult};
use everyware_ui::button::{Button, ButtonEvent, ButtonKind, Icon, IconButton, IconButtonKind};
use everyware_ui::images;
use everyware_ui::list_item::ListItem;
use everyware_ui::list_view::{ListEvent, ListView};
use everyware_ui::part_of;
use everyware_ui::screens::{Effect, Pages};
use everyware_ui::segmented::{Segmented, SegmentedEvent};
use everyware_ui::tone::Tone;
use everyware_ui::sheet::{Edge, Layout, Sheet};
use everyware_ui::Gesture;
use jiff::civil::Date;
use jiff::tz::TimeZone;
use jiff::Timestamp;
use std::time::{Duration, Instant};

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
    /// The header's button (#agenda/account) that opens the calendar's link, while one is linked.
    pub account: IconButton,
}

/// The panel of an event (#event): the library's sheet with the app's content (#event-content),
/// a step of it at a time (its pages, `EventStep`): the details with what the event offers, a
/// QR code for the phone, a confirmation, and the calendar's link (opened from the agenda's
/// header, #agenda/account). A change goes to the calendar bundle as a command (`on` returns it,
/// the caller sends it); until its answer (`answered`) the panel waits, its controls off.
#[derive(Debug)]
pub struct EventPanel {
    pub sheet: Sheet,
    pub close: IconButton,
    /// The event shown, by its id; it stays while the panel slides out. None in the calendar's step.
    pub id: Option<String>,
    pub details: Details,
    /// What the event offers (agenda::offers).
    pub offers: Offers,
    /// The body's scroll: one row as high as the body.
    pub body: ListView,
    /// How high the body's place is (the design's #event/scroll).
    view_h: f32,
    /// Which step shows and which showed before (`EventStep`'s order): the pages slide
    /// (`EVENT_EFFECT`).
    pub pages: Pages,
    /// The details' controls: the answers, the codes for the phone, the deletion.
    pub rsvp: Segmented,
    pub join: Button,
    pub directions: Button,
    pub delete: Button,
    /// The QR step: what the code is for, its address (a picture under EVENT_QR_KEY), the host
    /// the phone will open (under the code), Back.
    pub qr_title: String,
    qr: String,
    pub qr_host: String,
    pub back: Button,
    /// The confirmation step: what it confirms, Cancel and the action.
    pub confirming: Option<Confirm>,
    pub cancel: Button,
    pub confirm: Button,
    /// The calendar's step: the access Google granted, in words, and its two actions.
    pub access: String,
    pub change_access: Button,
    pub disconnect: Button,
    /// The command on its way, until its answer.
    pub pending: Option<Pending>,
    /// An answer to an invitation that went through, while the agenda does not show it yet (the
    /// read after the change failed): the event, the answer and when; it stays lit until the
    /// agenda has it, `KEEP_ANSWER` at most.
    sent_answer: Option<(String, Rsvp, Instant)>,
    /// What the panel waits for (`status_line`), and why the last command of the panel shown
    /// failed.
    status: String,
    pub error: String,
    /// Why a change failed while its panel was not shown, by its key (the event's id, empty for
    /// the calendar's link): said when that panel opens again. The newest `MISSED` at most.
    missed: Vec<(String, String)>,
}

/// Failures kept for panels not shown, at most.
const MISSED: usize = 8;

/// The steps of the event's panel: #event-content's pages in the design's order
/// (#event/page-details, -qr, -calendar, -confirm). A higher step comes from the right: the
/// confirmation is last, as both the details and the calendar's link go forward to it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum EventStep {
    #[default]
    Details = 0,
    Qr = 1,
    Calendar = 2,
    Confirm = 3,
}

/// The steps by their page.
pub const EVENT_STEPS: [EventStep; 4] = [EventStep::Details, EventStep::Qr, EventStep::Calendar, EventStep::Confirm];

/// How the steps change: the next one slides in from the right, Back and Cancel slide it out
/// (Material 3's shared axis X, as the settings' pages); a step over pages between them moves
/// one width all the same.
pub const EVENT_EFFECT: Effect = Effect::Slide;

/// What the confirmation step confirms.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Confirm {
    /// The deletion of the event shown.
    Delete,
    /// The calendar unlinked.
    Disconnect,
    /// The calendar unlinked, to link it again with another access (the same command).
    ChangeAccess,
}

/// A command on its way to the calendar bundle.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Pending {
    /// "respond", "delete" or "disconnect" (the answer names it).
    pub command: &'static str,
    /// The event it is for; empty for a disconnect.
    pub event_id: String,
    /// The answer sent, for "respond".
    response: Option<Rsvp>,
    since: Instant,
}

/// The calendar bundle's answer to a command of the panel (IScreenLink.publishCalendarResult).
#[derive(Clone, Debug, PartialEq)]
pub struct CalendarAnswer {
    pub command: String,
    pub event_id: String,
    pub result: CalendarResult,
}

/// The key of the QR step's picture (everyware_ui::images).
pub const EVENT_QR_KEY: &str = "event/qr";
/// The QR step's frame in the design (#event/qr), square.
pub const EVENT_QR_SIDE: u32 = 480;
/// The answers, in the segments' order, and the tone each is lit in when it is the answer:
/// green, yellow, red.
pub const RSVP: [(Rsvp, &str, Tone); 3] =
    [(Rsvp::RSVP_ACCEPTED, "Accept", Tone::Success), (Rsvp::RSVP_TENTATIVE, "Maybe", Tone::Warning), (Rsvp::RSVP_DECLINED, "Decline", Tone::Error)];
/// An answer waits this long, then the panel stops waiting: the dashboard bundle gives up after
/// 150 s and then answers itself.
pub const ANSWER_WAIT: Duration = Duration::from_secs(160);
/// An answer that went through stays lit this long at most while the agenda does not show it
/// (a read interval and a half: the next read brings it).
pub const KEEP_ANSWER: Duration = Duration::from_secs(90);
/// The line of a panel whose controls are off for a change of another panel.
pub const OTHER_CHANGE: &str = "Another change is on its way.";

impl EventPanel {
    fn new(view_h: f32) -> Self {
        EventPanel {
            sheet: Sheet::new(EVENT_LAYOUT),
            close: IconButton::new(IconButtonKind::Tonal, Icon::Close),
            id: None,
            details: Details::default(),
            offers: Offers::default(),
            body: ListView::new([], view_h, 0.0),
            view_h,
            pages: Pages::default(),
            rsvp: Segmented::new(RSVP.map(|(_, label, _)| label)).tones(RSVP.map(|(_, _, tone)| tone)),
            join: Button::new(ButtonKind::Secondary, "Join on phone"),
            directions: Button::new(ButtonKind::Secondary, "Directions"),
            // red: it cannot be undone
            delete: Button::new(ButtonKind::Secondary, "Delete").tone(Tone::Error),
            qr_title: String::new(),
            qr: String::new(),
            qr_host: String::new(),
            back: Button::new(ButtonKind::Secondary, "Back"),
            confirming: None,
            cancel: Button::new(ButtonKind::Secondary, "Cancel"),
            confirm: Button::new(ButtonKind::Primary, "Delete"),
            access: String::new(),
            change_access: Button::new(ButtonKind::Secondary, "Change access"),
            disconnect: Button::new(ButtonKind::Secondary, "Disconnect"),
            pending: None,
            sent_answer: None,
            missed: Vec::new(),
            status: String::new(),
            error: String::new(),
        }
    }

    /// Shows the event `id`: its details from the top of the body, `height` high, and what it
    /// offers. The panel opens on them; nothing slides.
    fn show(&mut self, id: String, details: Details, offers: Offers, height: f32) {
        self.id = Some(id);
        self.details = details;
        self.body = ListView::new([ListItem::default()], self.view_h, height.max(0.0));
        self.offers = offers;
        self.pages.reset();
        let key = self.offers.id.clone();
        self.error = self.take_missed(&key);
        self.follow_answer();
        self.sheet.open();
    }

    /// The same event's new details: shown from the top of the body when they changed.
    fn update(&mut self, details: Details, offers: Offers, height: f32) {
        if details != self.details {
            self.details = details;
            self.body = ListView::new([ListItem::default()], self.view_h, height.max(0.0));
        }
        self.offers = offers;
        // an offer gone leaves its step (another device changed the event, say)
        let gone = match self.step() {
            EventStep::Qr => self.qr_title == JOIN && self.offers.join.is_none() || self.qr_title == DIRECTIONS && self.offers.directions.is_none(),
            EventStep::Confirm => self.confirming == Some(Confirm::Delete) && !self.offers.delete && !self.waits_for("delete"),
            _ => false,
        };
        if gone {
            self.go(EventStep::Details);
        }
        // the agenda shows the answer that went through: it is the account's now
        if self.sent_answer.as_ref().is_some_and(|(id, sent, _)| *id == self.offers.id && self.offers.respond == Some(*sent)) {
            self.sent_answer = None;
        }
        self.follow_answer();
    }

    /// The calendar's link (from the agenda's header): what Google granted, Change access and
    /// Disconnect. The panel opens on its page; nothing slides.
    pub fn open_calendar(&mut self, can_change: bool) {
        self.id = None;
        self.access = if can_change {
            "The panel may read and change this calendar: anyone here can answer its invitations and delete its events."
        } else {
            "The panel may read this calendar."
        }
        .to_string();
        self.error = self.take_missed("");
        self.pages = Pages::at(EventStep::Calendar as usize);
        self.sheet.open();
    }

    /// Closes the panel (it slides out with the step it shows); the QR picture goes.
    pub fn close(&mut self) {
        self.sheet.close();
        self.show_qr(None);
    }

    /// Whether it is open.
    pub fn is_open(&self) -> bool {
        self.sheet.is_open()
    }

    /// The step that shows.
    pub fn step(&self) -> EventStep {
        EVENT_STEPS.get(self.pages.selected).copied().unwrap_or_default()
    }

    /// A gesture on the body: a drag or a fling scrolls it.
    pub fn on_body(&mut self, gesture: Gesture, now: Instant) {
        let _ = self.body.on(None, gesture, now);
    }

    /// A gesture on the open panel: the scrim and close close it, a drag on the body scrolls it,
    /// the step's controls do what they say. A change is the command returned, for the caller to
    /// send (`sent`); the panel waits for its answer from then on.
    pub fn on(&mut self, node: &str, gesture: Gesture, now: Instant) -> Option<CalendarCommand> {
        let scrim = node == "#event/scrim" && matches!(gesture, Gesture::Up { inside: true, .. });
        if scrim || (node == "#event/close" && self.close.on(gesture) == Some(ButtonEvent::Clicked)) {
            self.close();
            return None;
        }
        let waiting = self.pending.is_some();
        let tapped = |button: &mut Button| button.on(gesture) == Some(ButtonEvent::Clicked);
        // the step's own controls only: the page that slides out takes no tap
        match (self.step(), node) {
            (EventStep::Details, "#event/scroll") => self.on_body(gesture, now),
            (EventStep::Details, "#event/join") if tapped(&mut self.join) => {
                let url = self.offers.join.clone();
                self.qr_step(JOIN, url);
            }
            (EventStep::Details, "#event/directions") if tapped(&mut self.directions) => {
                let url = self.offers.directions.clone();
                self.qr_step(DIRECTIONS, url);
            }
            (EventStep::Details, "#event/delete") if tapped(&mut self.delete) && self.offers.delete && !waiting => self.ask(Confirm::Delete),
            (EventStep::Details, _) => {
                let segment = part_of("#event/rsvp", node).and_then(|p| p.strip_prefix("#segmented/segment-")).and_then(|n| n.parse::<usize>().ok());
                if let Some(i) = segment.and_then(|n| n.checked_sub(1)) {
                    if self.offers.respond.is_some() && !waiting {
                        if let Some(SegmentedEvent::Selected(i)) = self.rsvp.on(i, gesture) {
                            return self.respond(RSVP[i].0, now);
                        }
                    }
                }
            }
            (EventStep::Qr, "#event/back") if tapped(&mut self.back) => self.go(EventStep::Details),
            (EventStep::Confirm, "#event/cancel") if tapped(&mut self.cancel) && !waiting => {
                let back = if self.confirming == Some(Confirm::Delete) { EventStep::Details } else { EventStep::Calendar };
                self.go(back);
            }
            (EventStep::Confirm, "#event/confirm") if tapped(&mut self.confirm) && !waiting => return self.confirmed(now),
            (EventStep::Calendar, "#event/change-access") if tapped(&mut self.change_access) && !waiting => self.ask(Confirm::ChangeAccess),
            (EventStep::Calendar, "#event/disconnect") if tapped(&mut self.disconnect) && !waiting => self.ask(Confirm::Disconnect),
            _ => {}
        }
        None
    }

    /// The command did not go out (no link to the house's data): nothing waits for it.
    pub fn sent(&mut self, went_out: bool) {
        if let Some(pending) = self.pending.take().filter(|_| !went_out) {
            self.fail(NO_LINK, &pending);
        }
    }

    /// The calendar bundle's answer at `now`: the panel stops waiting, says why a change failed
    /// (on the panel the change was made from only), and closes after a deletion or an unlink of
    /// what it shows. Another command's answer (a late one) changes nothing.
    pub fn answered(&mut self, answer: &CalendarAnswer, now: Instant) {
        let ours = self.pending.as_ref().is_some_and(|p| p.command == answer.command && p.event_id == answer.event_id);
        if !ours {
            log::info!("smarthome: a calendar answer no command waits for ({})", answer.command);
            return;
        }
        let pending = self.pending.take().expect("ours");
        let here = self.shows(&pending);
        if answer.result.ok {
            self.status.clear();
            self.enable(true);
            match answer.command.as_str() {
                // the agenda after it came first; the panel of what is gone closes, not another
                "delete" | "disconnect" if here => self.close(),
                "respond" => {
                    if let Some(sent) = pending.response {
                        self.sent_answer = Some((pending.event_id.clone(), sent, now));
                    }
                    self.follow_answer();
                }
                _ => {}
            }
        } else {
            log::warn!("smarthome: calendar {} not done: {:?} {}", answer.command, answer.result.failure, answer.result.error);
            self.fail(failure_text(answer.command.as_str(), answer.result.failure.enum_value_or_default()), &pending);
        }
    }

    /// After every action: an answer that does not come (the dashboard bundle gone) is waited for
    /// `ANSWER_WAIT` at most, without the link to the house's data not at all; an answer that went
    /// through stays lit `KEEP_ANSWER` at most.
    pub fn follow(&mut self, house_link: bool, now: Instant) {
        let late = self.pending.as_ref().is_some_and(|p| now.saturating_duration_since(p.since) > ANSWER_WAIT);
        if let Some(pending) = self.pending.take_if(|_| late || !house_link) {
            self.fail(if house_link { NO_ANSWER } else { NO_LINK }, &pending);
        }
        if self.sent_answer.as_ref().is_some_and(|(_, _, at)| now.saturating_duration_since(*at) > KEEP_ANSWER) {
            self.sent_answer = None;
            self.follow_answer();
        }
    }

    /// What the panel waits for, when the change is the one of what it shows; the controls of
    /// another panel are off with a line that says why.
    pub fn status_line(&self) -> &str {
        match &self.pending {
            Some(p) if self.shows(p) => &self.status,
            Some(_) => OTHER_CHANGE,
            None => "",
        }
    }

    /// Whether the panel shows what `pending` is for: the event it changes, or the calendar's
    /// link (its step and its confirmations) for an unlink.
    fn shows(&self, pending: &Pending) -> bool {
        match pending.command {
            "disconnect" => self.id.is_none(),
            _ => self.id.as_deref() == Some(pending.event_id.as_str()),
        }
    }

    fn respond(&mut self, response: Rsvp, now: Instant) -> Option<CalendarCommand> {
        let id = self.offers.id.clone();
        let mut respond = CalendarRespond::new();
        respond.event_id = id.clone();
        respond.response = response.into();
        let mut command = CalendarCommand::new();
        command.set_respond(respond);
        self.wait("respond", id, Some(response), "Sending the answer…", now);
        Some(command)
    }

    fn confirmed(&mut self, now: Instant) -> Option<CalendarCommand> {
        let mut command = CalendarCommand::new();
        match self.confirming? {
            Confirm::Delete => {
                if !self.offers.delete {
                    return None;
                }
                let mut delete = CalendarDelete::new();
                delete.event_id = self.offers.id.clone();
                command.set_delete(delete);
                self.wait("delete", self.offers.id.clone(), None, "Deleting the event…", now);
            }
            Confirm::Disconnect | Confirm::ChangeAccess => {
                command.set_disconnect(CalendarDisconnect::new());
                self.wait("disconnect", String::new(), None, "Disconnecting the calendar…", now);
            }
        }
        Some(command)
    }

    /// The confirmation of `what`.
    fn ask(&mut self, what: Confirm) {
        self.confirming = Some(what);
        (self.confirm.label, self.confirm.tone) = match what {
            // red: a deletion and an unlink cannot be undone from the panel
            Confirm::Delete => ("Delete".to_string(), Tone::Error),
            Confirm::Disconnect => ("Disconnect".to_string(), Tone::Error),
            Confirm::ChangeAccess => ("Change access".to_string(), Tone::Default),
        };
        self.error.clear();
        self.go(EventStep::Confirm);
    }

    /// The title and the line of the confirmation step.
    pub fn confirm_texts(&self) -> (&'static str, &'static str) {
        match self.confirming {
            Some(Confirm::Delete) if self.offers.recurring => ("Delete this event?", "Only this occurrence."),
            Some(Confirm::Delete) => ("Delete this event?", ""),
            Some(Confirm::Disconnect) => ("Disconnect the calendar?", "The panel ends its access to the Google account and forgets it. The agenda stays empty until a calendar is linked again."),
            Some(Confirm::ChangeAccess) => ("Change the access?", "The panel ends its access and shows the setup code. Scan it to link the calendar again and choose read only, or read and change."),
            None => ("", ""),
        }
    }

    /// The QR step for `url`, titled `title`, with the host the phone will open under the code;
    /// a code only for an https link with a plain host (agenda::link_host), none otherwise.
    fn qr_step(&mut self, title: &str, url: Option<String>) {
        let Some((url, host)) = url.and_then(|url| agenda::link_host(&url).map(|host| (url, host))) else { return };
        self.show_qr(Some(url));
        if !self.qr.is_empty() {
            self.qr_title = title.to_string();
            self.qr_host = host;
            self.go(EventStep::Qr);
        }
    }

    /// Makes the QR code of `url` the step's picture, once per address; none frees it. A meeting
    /// link may hold its passcode: it is not logged.
    fn show_qr(&mut self, url: Option<String>) {
        match url {
            Some(url) if url == self.qr => {}
            Some(url) => match agenda::qr_rgba(&url, EVENT_QR_SIDE).map(|rgba| images::register_rgba(EVENT_QR_KEY, rgba, EVENT_QR_SIDE, EVENT_QR_SIDE)) {
                Some(Ok(())) => self.qr = url,
                failed => {
                    match failed {
                        Some(Err(err)) => log::warn!("smarthome: the event's QR code: {err}"),
                        _ => log::warn!("smarthome: an event's address ({} bytes) does not fit a QR code", url.len()),
                    }
                    images::unregister(EVENT_QR_KEY);
                    self.qr.clear();
                }
            },
            None if !self.qr.is_empty() => {
                images::unregister(EVENT_QR_KEY);
                self.qr.clear();
            }
            None => {}
        }
    }

    /// The QR step's picture while there is one.
    pub fn qr_picture(&self) -> Option<customizations::ImageContentData> {
        if self.qr.is_empty() { None } else { images::get(EVENT_QR_KEY) }
    }

    /// Slides to `step`. The QR code's picture stays: its page is drawn while it slides out; it
    /// goes when the panel closes or the next code replaces it.
    fn go(&mut self, step: EventStep) {
        self.pages.show(step as usize);
    }

    fn wait(&mut self, command: &'static str, event_id: String, response: Option<Rsvp>, status: &str, now: Instant) {
        self.pending = Some(Pending { command, event_id, response, since: now });
        self.status = status.to_string();
        self.error.clear();
        self.enable(false);
    }

    fn waits_for(&self, command: &str) -> bool {
        self.pending.as_ref().is_some_and(|p| p.command == command)
    }

    /// `pending` failed: the controls on again; why, on its panel when it is the one shown, or
    /// kept for when that panel opens again.
    fn fail(&mut self, why: &str, pending: &Pending) {
        self.status.clear();
        if self.shows(pending) {
            self.error = why.to_string();
        } else {
            let key = if pending.command == "disconnect" { String::new() } else { pending.event_id.clone() };
            self.missed.retain(|(k, _)| *k != key);
            self.missed.push((key, why.to_string()));
            if self.missed.len() > MISSED {
                self.missed.remove(0);
            }
        }
        self.enable(true);
        // the answer shown is the account's again, not the one the finger chose
        self.follow_answer();
    }

    /// The failure kept for the panel of `key` (an event's id, empty for the calendar's link),
    /// taken: it is said once; none: empty.
    fn take_missed(&mut self, key: &str) -> String {
        match self.missed.iter().position(|(k, _)| k == key) {
            Some(i) => self.missed.remove(i).1,
            None => String::new(),
        }
    }

    /// The controls that start a change, on or off while one runs.
    fn enable(&mut self, on: bool) {
        self.rsvp.press.set_enabled(on);
        for button in [&mut self.delete, &mut self.confirm, &mut self.cancel, &mut self.change_access, &mut self.disconnect] {
            button.press.set_enabled(on);
        }
    }

    /// The segment of the account's answer lit, none while it has not answered; left alone while
    /// an answer is on its way (the finger's choice shows), and the answer that went through while
    /// the agenda does not show it yet.
    fn follow_answer(&mut self) {
        if self.waits_for("respond") {
            return;
        }
        let account = self.offers.respond.unwrap_or_default();
        let shown = match &self.sent_answer {
            Some((id, sent, _)) if *id == self.offers.id => *sent,
            _ => account,
        };
        match RSVP.iter().position(|(r, _, _)| *r == shown) {
            Some(i) => self.rsvp.select(i),
            None => self.rsvp.clear(),
        }
    }
}

const JOIN: &str = "Join on phone";
const DIRECTIONS: &str = "Directions";
const NO_LINK: &str = "No link to the house's data. Try again later.";
const NO_ANSWER: &str = "No answer from the calendar. Try again.";

/// Why a command failed, for the panel.
pub fn failure_text(command: &str, failure: CalendarFailure) -> &'static str {
    use CalendarFailure::*;
    match (command, failure) {
        ("disconnect", CALENDAR_FAILURE_AGAIN) => "Google could not be reached. The calendar stays linked; try again.",
        (_, CALENDAR_FAILURE_READ_ONLY) => "The panel may only read this calendar.",
        (_, CALENDAR_FAILURE_GONE) => "This event is not in the calendar any more.",
        (_, CALENDAR_FAILURE_NOT_ALLOWED) => "This change is not possible for this event.",
        (_, CALENDAR_FAILURE_REFUSED) => "Google refused the change.",
        (_, CALENDAR_FAILURE_NOT_LINKED) => "No calendar is linked.",
        (_, CALENDAR_FAILURE_AGAIN) => "Google could not be reached. Try again.",
        (_, CALENDAR_FAILURE_NONE) => "Something went wrong. Try again.",
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
            account: IconButton::new(IconButtonKind::Tonal, Icon::Settings).compact(),
        }
    }

    /// Another design: the list `list_h` high and the event's body `body_h`. The agenda stays,
    /// the list is at its top again and an open event's panel is closed. A command on its way
    /// keeps waiting for its answer, and what the panel still has to say stays.
    pub fn resize(&mut self, list_h: f32, body_h: f32, zone: &str, hour24: bool, now: Timestamp) {
        self.list = ListView::new([], list_h, agenda::ROW_PITCH);
        let before = std::mem::replace(&mut self.event, EventPanel::new(body_h));
        (self.event.pending, self.event.sent_answer, self.event.missed) = (before.pending, before.sent_answer, before.missed);
        self.refresh(zone, hour24, now);
    }

    /// Whether a calendar is linked (the header's button shows).
    pub fn linked(&self) -> bool {
        self.agenda.as_ref().is_some_and(|a| a.link.enum_value() == Ok(CalendarLink::CALENDAR_CONNECTED))
    }

    /// A gesture on the header's button: a tap opens the calendar's link in the panel while one is
    /// linked.
    pub fn on_account(&mut self, gesture: Gesture) {
        if self.account.on(gesture) == Some(ButtonEvent::Clicked) && self.linked() {
            let can_change = self.agenda.as_ref().is_some_and(|a| a.can_change);
            self.event.open_calendar(can_change);
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
                    let offers = self.offers_of(i);
                    self.event.update(details, offers, height);
                }
                None => self.event.close(),
            }
        } else if self.event.is_open() && self.event.step() == EventStep::Calendar && !self.linked() {
            // unlinked (from here or elsewhere): the link's step has nothing to show
            self.event.close();
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
        let offers = self.offers_of(i);
        self.event.show(id, details, offers, height);
    }

    fn offers_of(&self, i: usize) -> Offers {
        self.agenda.as_ref().and_then(|a| a.events.get(i)).map(agenda::offers).unwrap_or_default()
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
mod tests;
