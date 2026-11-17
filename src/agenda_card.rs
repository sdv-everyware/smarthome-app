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
//! - The panel's steps (`EventStep`): the details with Accept, Maybe and Decline when the
//!   account may answer, Join on phone and Directions (a QR code each, for the phone to open:
//!   the panel opens nothing) and Delete when it may delete (after a confirmation); and the
//!   calendar's link, from the header's button while a calendar is linked, with Change access and
//!   Disconnect (each after a confirmation). A change is a command to the calendar bundle; the
//!   panel waits for its answer with its controls off and says why it failed.

use crate::agenda::{self, Details, Offers, QR_KEY, QR_SIDE};
use everyware_catalog::calendar::{Agenda, CalendarLink, Rsvp};
use everyware_catalog::calendar_service::{CalendarCommand, CalendarDelete, CalendarDisconnect, CalendarFailure, CalendarRespond, CalendarResult};
use everyware_ui::button::{Button, ButtonEvent, ButtonKind, Icon, IconButton, IconButtonKind};
use everyware_ui::images;
use everyware_ui::list_item::ListItem;
use everyware_ui::list_view::{ListEvent, ListView};
use everyware_ui::part_of;
use everyware_ui::segmented::{Segmented, SegmentedEvent};
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
/// a step of it at a time (its variant, `EventStep`): the details with what the event offers, a
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
    pub step: EventStep,
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
}

/// The steps of the event's panel: #event-content's variants.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum EventStep {
    #[default]
    Details,
    Qr,
    Confirm,
    Calendar,
}

impl EventStep {
    /// The content's variant.
    pub fn variant(self) -> &'static str {
        match self {
            EventStep::Details => "step=details",
            EventStep::Qr => "step=qr",
            EventStep::Confirm => "step=confirm",
            EventStep::Calendar => "step=calendar",
        }
    }
}

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
/// The answers, in the segments' order.
pub const RSVP: [(Rsvp, &str); 3] = [(Rsvp::RSVP_ACCEPTED, "Accept"), (Rsvp::RSVP_TENTATIVE, "Maybe"), (Rsvp::RSVP_DECLINED, "Decline")];
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
            step: EventStep::Details,
            rsvp: Segmented::new(RSVP.map(|(_, label)| label)),
            join: Button::new(ButtonKind::Secondary, "Join on phone"),
            directions: Button::new(ButtonKind::Secondary, "Directions"),
            delete: Button::new(ButtonKind::Secondary, "Delete"),
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
            status: String::new(),
            error: String::new(),
        }
    }

    /// Shows the event `id`: its details from the top of the body, `height` high, and what it
    /// offers.
    fn show(&mut self, id: String, details: Details, offers: Offers, height: f32) {
        self.id = Some(id);
        self.details = details;
        self.body = ListView::new([ListItem::default()], self.view_h, height.max(0.0));
        self.offers = offers;
        self.go(EventStep::Details);
        self.error.clear();
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
        let gone = match self.step {
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
    /// Disconnect.
    pub fn open_calendar(&mut self, can_change: bool) {
        self.id = None;
        self.access = if can_change {
            "The panel may read and change this calendar: anyone here can answer its invitations and delete its events."
        } else {
            "The panel may read this calendar."
        }
        .to_string();
        self.error.clear();
        self.go(EventStep::Calendar);
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
        match (self.step, node) {
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
        if !went_out && self.pending.take().is_some() {
            self.fail(NO_LINK, true);
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
            self.fail(failure_text(answer.command.as_str(), answer.result.failure.enum_value_or_default()), here);
        }
    }

    /// After every action: an answer that does not come (the dashboard bundle gone) is waited for
    /// `ANSWER_WAIT` at most, without the link to the house's data not at all; an answer that went
    /// through stays lit `KEEP_ANSWER` at most.
    pub fn follow(&mut self, house_link: bool, now: Instant) {
        let late = self.pending.as_ref().is_some_and(|p| now.saturating_duration_since(p.since) > ANSWER_WAIT);
        if let Some(pending) = self.pending.take_if(|_| late || !house_link) {
            let here = self.shows(&pending);
            self.fail(if house_link { NO_ANSWER } else { NO_LINK }, here);
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
        self.confirm.label = match what {
            Confirm::Delete => "Delete",
            Confirm::Disconnect => "Disconnect",
            Confirm::ChangeAccess => "Change access",
        }
        .to_string();
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

    fn go(&mut self, step: EventStep) {
        if step != EventStep::Qr {
            self.show_qr(None);
        }
        self.step = step;
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

    /// A command failed: the controls on again; why, on the panel it was made from (`here`).
    fn fail(&mut self, why: &str, here: bool) {
        self.status.clear();
        if here {
            self.error = why.to_string();
        }
        self.enable(true);
        // the answer shown is the account's again, not the one the finger chose
        self.follow_answer();
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
        match RSVP.iter().position(|(r, _)| *r == shown) {
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
        } else if self.event.is_open() && self.event.step == EventStep::Calendar && !self.linked() {
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

    /// A tap (a down and an up) on `node` of the open panel: the command it returns.
    fn press(c: &mut AgendaCard, node: &str) -> Option<CalendarCommand> {
        let t = Instant::now();
        let down = c.event.on(node, Gesture::DOWN, t);
        assert_eq!(down, None, "a down sends nothing");
        c.event.on(node, Gesture::UP, t)
    }

    /// The panel of the agenda's first event, opened by a tap on its row.
    fn opened(e: AgendaEvent) -> AgendaCard {
        let mut c = card();
        let mut agenda = linked(vec![e]);
        agenda.can_change = true;
        c.set(agenda, "UTC", true, at("2026-11-10T08:00:00Z"));
        let t = Instant::now();
        c.on_list(Some(1), Gesture::down(0.0, 10.0), t);
        c.on_list(Some(1), Gesture::up(0.0, 10.0), t);
        assert!(c.event.is_open() && c.event.step == EventStep::Details);
        c
    }

    fn invitation() -> AgendaEvent {
        let mut e = event("inv_20261110T090000Z", "2026-11-10T09:00:00Z", 30);
        e.can_respond = true;
        e.response = Rsvp::RSVP_NEEDS_ACTION.into();
        e
    }

    fn answer(command: &str, event_id: &str, failure: Option<CalendarFailure>) -> CalendarAnswer {
        let mut result = CalendarResult::new();
        match failure {
            None => result.ok = true,
            Some(f) => result.failure = f.into(),
        }
        CalendarAnswer { command: command.into(), event_id: event_id.into(), result }
    }

    #[test]
    fn an_invitation_is_answered_and_the_panel_waits_for_the_answer() {
        let mut c = opened(invitation());
        // not answered yet: no segment lit; the body has room for the answers
        assert!(c.event.details.respond);
        assert_eq!((0..3).map(|i| c.event.rsvp.target(i)).collect::<Vec<_>>(), [0.0, 0.0, 0.0]);
        let command = press(&mut c, "#event/rsvp/#segmented/segment-2").expect("a command");
        assert_eq!((command.respond().event_id.as_str(), command.respond().response.enum_value_or_default()), ("inv_20261110T090000Z", Rsvp::RSVP_TENTATIVE));
        // waiting: Maybe lit, the controls off, a line says so; another tap sends nothing
        assert_eq!(c.event.rsvp.target(1), 100.0);
        assert!(!c.event.rsvp.press.enabled && !c.event.status_line().is_empty());
        assert_eq!(press(&mut c, "#event/rsvp/#segmented/segment-1"), None);
        // the agenda after the change comes first, then the answer
        let mut e = invitation();
        e.response = Rsvp::RSVP_TENTATIVE.into();
        let mut agenda = linked(vec![e]);
        agenda.can_change = true;
        c.set(agenda, "UTC", true, at("2026-11-10T08:01:00Z"));
        c.event.answered(&answer("respond", "inv_20261110T090000Z", None), Instant::now());
        assert!(c.event.pending.is_none() && c.event.rsvp.press.enabled && c.event.status_line().is_empty() && c.event.error.is_empty());
        assert_eq!(c.event.rsvp.target(1), 100.0, "the account's answer now");
    }

    #[test]
    fn a_failed_answer_says_why_and_shows_the_account_s_answer_again() {
        let mut e = invitation();
        e.response = Rsvp::RSVP_ACCEPTED.into();
        let mut c = opened(e);
        assert_eq!(c.event.rsvp.target(0), 100.0, "Accept is the account's answer");
        press(&mut c, "#event/rsvp/#segmented/segment-3").expect("a command");
        // an answer of another command changes nothing
        c.event.answered(&answer("delete", "other", None), Instant::now());
        assert!(c.event.pending.is_some());
        c.event.answered(&answer("respond", "inv_20261110T090000Z", Some(CalendarFailure::CALENDAR_FAILURE_READ_ONLY)), Instant::now());
        assert_eq!(c.event.error, "The panel may only read this calendar.");
        assert!(c.event.status_line().is_empty() && c.event.rsvp.press.enabled);
        assert_eq!((c.event.rsvp.target(0), c.event.rsvp.target(2)), (100.0, 0.0));
        // every failure has its words
        for f in [CalendarFailure::CALENDAR_FAILURE_GONE, CalendarFailure::CALENDAR_FAILURE_NOT_ALLOWED, CalendarFailure::CALENDAR_FAILURE_REFUSED, CalendarFailure::CALENDAR_FAILURE_AGAIN, CalendarFailure::CALENDAR_FAILURE_NOT_LINKED, CalendarFailure::CALENDAR_FAILURE_NONE] {
            assert!(!failure_text("respond", f).is_empty());
        }
    }

    #[test]
    fn an_answer_that_does_not_come_is_not_waited_for_ever() {
        let mut c = opened(invitation());
        press(&mut c, "#event/rsvp/#segmented/segment-1");
        c.event.follow(true, Instant::now());
        assert!(c.event.pending.is_some());
        c.event.follow(true, Instant::now() + ANSWER_WAIT + Duration::from_secs(1));
        assert!(c.event.pending.is_none() && c.event.error.contains("No answer"));
        // the link to the house's data gone: at once
        press(&mut c, "#event/rsvp/#segmented/segment-2");
        c.event.follow(false, Instant::now());
        assert!(c.event.pending.is_none() && c.event.error.contains("No link"));
        // a command that did not go out
        press(&mut c, "#event/rsvp/#segmented/segment-3");
        c.event.sent(false);
        assert!(c.event.pending.is_none() && c.event.error.contains("No link"));
    }

    #[test]
    fn a_deletion_is_confirmed_first() {
        let mut e = event("mine_20261110T090000Z", "2026-11-10T09:00:00Z", 30);
        e.can_delete = true;
        e.recurring = true;
        let mut c = opened(e);
        assert!(!c.event.details.respond, "no answers on the account's own event");
        assert_eq!(press(&mut c, "#event/rsvp/#segmented/segment-1"), None);
        assert_eq!(press(&mut c, "#event/delete"), None, "Delete asks first");
        assert_eq!((c.event.step, c.event.confirm_texts()), (EventStep::Confirm, ("Delete this event?", "Only this occurrence.")));
        assert_eq!(c.event.confirm.label, "Delete");
        // Cancel: back to the details, nothing sent
        assert_eq!(press(&mut c, "#event/cancel"), None);
        assert_eq!(c.event.step, EventStep::Details);
        press(&mut c, "#event/delete");
        let command = press(&mut c, "#event/confirm").expect("a command");
        assert_eq!(command.delete().event_id, "mine_20261110T090000Z");
        assert!(!c.event.confirm.press.enabled && !c.event.cancel.press.enabled);
        // the agenda without the event closes the panel; the answer after it changes nothing more
        let mut agenda = linked(vec![]);
        agenda.can_change = true;
        c.set(agenda, "UTC", true, at("2026-11-10T08:01:00Z"));
        assert!(!c.event.is_open());
        c.event.answered(&answer("delete", "mine_20261110T090000Z", None), Instant::now());
        assert!(c.event.pending.is_none() && !c.event.is_open());
    }

    #[test]
    fn nothing_is_offered_that_the_event_does_not_allow() {
        let mut c = opened(event("plain", "2026-11-10T09:00:00Z", 30));
        assert_eq!(press(&mut c, "#event/delete"), None);
        assert_eq!(c.event.step, EventStep::Details, "no confirmation for an event the account may not delete");
        assert_eq!(press(&mut c, "#event/rsvp/#segmented/segment-1"), None);
        assert_eq!(press(&mut c, "#event/join"), None);
        assert_eq!(c.event.step, EventStep::Details, "no code without a meeting link");
    }

    /// The tests that make the event's QR picture: one at a time (the picture's key is global).
    static EVENT_QR_TESTS: std::sync::Mutex<()> = std::sync::Mutex::new(());

    #[test]
    fn join_and_directions_show_a_code_for_the_phone() {
        let _one = EVENT_QR_TESTS.lock().unwrap_or_else(|e| e.into_inner());
        let mut e = event("meet", "2026-11-10T09:00:00Z", 30);
        e.meeting_url = "https://meet.google.com/abc-defg-hij".to_string();
        e.location = "Main Street 12".to_string();
        let mut c = opened(e);
        press(&mut c, "#event/join");
        assert_eq!((c.event.step, c.event.qr_title.as_str()), (EventStep::Qr, "Join on phone"));
        assert!(c.event.qr_picture().is_some());
        press(&mut c, "#event/back");
        assert_eq!(c.event.step, EventStep::Details);
        assert!(c.event.qr_picture().is_none() && images::get(EVENT_QR_KEY).is_none(), "the picture goes with its step");
        press(&mut c, "#event/directions");
        assert_eq!((c.event.step, c.event.qr_title.as_str()), (EventStep::Qr, "Directions"));
        assert_eq!(c.event.qr, "https://www.google.com/maps/search/?api=1&query=Main%20Street%2012");
        c.event.close();
        assert!(images::get(EVENT_QR_KEY).is_none());
    }

    #[test]
    fn the_calendar_s_link_from_the_header() {
        let mut c = card();
        // no calendar linked: the button does nothing
        c.set(unlinked("http://10.0.0.2:8765/setup?t=one"), "UTC", true, at("2026-11-10T08:00:00Z"));
        c.on_account(Gesture::DOWN);
        c.on_account(Gesture::UP);
        assert!(!c.linked() && !c.event.is_open());
        let agenda = linked(vec![]);
        c.set(agenda, "UTC", true, at("2026-11-10T08:00:00Z"));
        c.on_account(Gesture::DOWN);
        c.on_account(Gesture::UP);
        assert!(c.event.is_open() && c.event.step == EventStep::Calendar);
        assert_eq!(c.event.access, "The panel may read this calendar.");
        // Change access and Disconnect: both a confirmation, then the unlink
        assert_eq!(press(&mut c, "#event/change-access"), None);
        assert_eq!((c.event.step, c.event.confirm.label.as_str(), c.event.confirm_texts().0), (EventStep::Confirm, "Change access", "Change the access?"));
        press(&mut c, "#event/cancel");
        assert_eq!(c.event.step, EventStep::Calendar, "Cancel goes back to the link");
        press(&mut c, "#event/disconnect");
        assert_eq!(c.event.confirm_texts().0, "Disconnect the calendar?");
        let command = press(&mut c, "#event/confirm").expect("a command");
        assert!(command.has_disconnect());
        // Google could not be reached: the calendar stays linked, and the panel says so
        c.event.answered(&answer("disconnect", "", Some(CalendarFailure::CALENDAR_FAILURE_AGAIN)), Instant::now());
        assert!(c.event.is_open() && c.event.error.contains("stays linked"));
        press(&mut c, "#event/confirm").expect("again");
        c.event.answered(&answer("disconnect", "", None), Instant::now());
        assert!(!c.event.is_open());
        // with write access the link says what that means
        let mut agenda = linked(vec![]);
        agenda.can_change = true;
        c.set(agenda, "UTC", true, at("2026-11-10T08:00:00Z"));
        c.on_account(Gesture::DOWN);
        c.on_account(Gesture::UP);
        assert!(c.event.access.contains("anyone here"));
        // unlinked meanwhile (the agenda says so): the link's step closes
        c.set(unlinked("http://10.0.0.2:8765/setup?t=two"), "UTC", true, at("2026-11-10T08:02:00Z"));
        assert!(!c.event.is_open());
    }

    /// Two events of the same day, the account may delete both and answer neither.
    fn two_of_mine() -> AgendaCard {
        let mine = |id: &str, start: &str| {
            let mut e = event(id, start, 30);
            e.can_delete = true;
            e
        };
        let mut c = card();
        let mut agenda = linked(vec![mine("a", "2026-11-10T09:00:00Z"), mine("b", "2026-11-10T10:00:00Z")]);
        agenda.can_change = true;
        c.set(agenda, "UTC", true, at("2026-11-10T08:00:00Z"));
        c
    }

    fn tap_row(c: &mut AgendaCard, row: usize) {
        let t = Instant::now();
        c.on_list(Some(row), Gesture::down(0.0, 10.0), t);
        c.on_list(Some(row), Gesture::up(0.0, 10.0), t);
    }

    #[test]
    fn a_late_answer_closes_only_the_panel_it_belongs_to() {
        let mut c = two_of_mine();
        tap_row(&mut c, 1);
        assert_eq!(c.event.id.as_deref(), Some("a"));
        press(&mut c, "#event/delete");
        press(&mut c, "#event/confirm").expect("a command");
        // the owner closes the panel and opens the other event while the deletion runs
        c.event.close();
        tap_row(&mut c, 2);
        assert_eq!(c.event.id.as_deref(), Some("b"));
        assert_eq!(c.event.status_line(), OTHER_CHANGE, "its controls are off, and the line says why");
        assert!(!c.event.delete.press.enabled);
        // a's deletion fails: b's panel shows no error of a, and its controls come back
        c.event.answered(&answer("delete", "a", Some(CalendarFailure::CALENDAR_FAILURE_AGAIN)), Instant::now());
        assert!(c.event.is_open() && c.event.error.is_empty() && c.event.delete.press.enabled && c.event.status_line().is_empty());
        // again, and this time it goes through: b's panel stays open
        c.event.close();
        tap_row(&mut c, 1);
        press(&mut c, "#event/delete");
        press(&mut c, "#event/confirm").expect("a command");
        c.event.close();
        tap_row(&mut c, 2);
        c.event.answered(&answer("delete", "a", None), Instant::now());
        assert!(c.event.is_open() && c.event.id.as_deref() == Some("b"));
        // an unlink's answer closes only the calendar's link, not an event's panel
        c.on_account(Gesture::DOWN);
        c.on_account(Gesture::UP);
        press(&mut c, "#event/disconnect");
        press(&mut c, "#event/confirm").expect("a command");
        c.event.close();
        tap_row(&mut c, 2);
        c.event.answered(&answer("disconnect", "", None), Instant::now());
        assert!(c.event.is_open() && c.event.id.as_deref() == Some("b"));
    }

    #[test]
    fn an_answer_that_went_through_stays_lit_until_the_agenda_has_it() {
        let mut c = opened(invitation());
        press(&mut c, "#event/rsvp/#segmented/segment-1").expect("a command");
        // the read after the change failed: the answer comes, the agenda is the old one
        let t = Instant::now();
        c.event.answered(&answer("respond", "inv_20261110T090000Z", None), t);
        assert_eq!(c.event.rsvp.chosen(), Some(0), "Accept stays lit, not the old 'not answered'");
        let mut agenda = linked(vec![invitation()]);
        agenda.can_change = true;
        c.set(agenda.clone(), "UTC", true, at("2026-11-10T08:01:00Z"));
        assert_eq!(c.event.rsvp.chosen(), Some(0), "an agenda without it yet changes nothing");
        // no agenda with it for a read interval and a half: the agenda's answer again
        c.event.follow(true, t + KEEP_ANSWER + Duration::from_secs(1));
        assert_eq!(c.event.rsvp.chosen(), None);
        // the next time the agenda has it: it is the account's answer
        press(&mut c, "#event/rsvp/#segmented/segment-3").expect("a command");
        c.event.answered(&answer("respond", "inv_20261110T090000Z", None), t);
        let mut e = invitation();
        e.response = Rsvp::RSVP_DECLINED.into();
        agenda.events = vec![e];
        c.set(agenda, "UTC", true, at("2026-11-10T08:02:00Z"));
        c.event.follow(true, t + KEEP_ANSWER + Duration::from_secs(1));
        assert_eq!(c.event.rsvp.chosen(), Some(2));
    }

    #[test]
    fn the_code_says_which_host_the_phone_opens() {
        let _one = EVENT_QR_TESTS.lock().unwrap_or_else(|e| e.into_inner());
        let mut e = event("meet", "2026-11-10T09:00:00Z", 30);
        e.meeting_url = "https://meet.google.com/abc-defg-hij?pwd=1".to_string();
        let mut c = opened(e);
        press(&mut c, "#event/join");
        assert_eq!((c.event.step, c.event.qr_host.as_str()), (EventStep::Qr, "meet.google.com"));
        // a link whose host is not plain letters gets no code (it stays text in the details)
        let mut e = event("odd", "2026-11-10T09:00:00Z", 30);
        e.meeting_url = "https://gооgle.com/meet".to_string();
        let mut d = opened(e);
        press(&mut d, "#event/join");
        assert_eq!(d.event.step, EventStep::Details);
        assert!(d.event.qr_picture().is_none());
        c.event.close();
    }
}
