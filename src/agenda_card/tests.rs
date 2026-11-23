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

//! The tests of agenda_card.rs (a child module: it sees the private items).

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
fn another_design_keeps_the_agenda_at_its_own_heights() {
    let mut c = card();
    let now = at("2026-11-10T08:00:00Z");
    c.set(linked(vec![event("a", "2026-11-10T09:00:00Z", 30), event("b", "2026-11-11T09:00:00Z", 30)]), "UTC", true, now);
    let t = Instant::now();
    c.on_list(Some(1), Gesture::down(0.0, 10.0), t);
    c.on_list(Some(1), Gesture::up(0.0, 10.0), t);
    assert!(c.event.is_open());
    let rows = titles(&c);
    c.resize(250.0, 300.0, "UTC", true, now);
    assert_eq!(titles(&c), rows, "the agenda stays");
    assert_eq!((c.list.view_h(), c.event.body.view_h()), (250.0, 300.0));
    assert!(!c.event.is_open(), "the panel of the other design is closed");
    assert_eq!(c.event_of_row(1), Some(0), "a tap still finds its event");
}

#[test]
fn another_design_keeps_a_command_on_its_way() {
    let mut c = opened(invitation());
    press(&mut c, "#event/rsvp/#segmented/segment-3").expect("a command");
    c.resize(250.0, 300.0, "UTC", true, at("2026-11-10T08:00:00Z"));
    assert!(c.event.pending.is_some() && !c.event.is_open());
    // its answer still ends the wait, and a failure is kept for the panel's next opening
    c.event.answered(&answer("respond", "inv_20261110T090000Z", Some(CalendarFailure::CALENDAR_FAILURE_READ_ONLY)), Instant::now());
    assert!(c.event.pending.is_none());
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
    assert!(c.event.is_open() && c.event.step() == EventStep::Details);
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
    assert_eq!((c.event.step(), c.event.confirm_texts()), (EventStep::Confirm, ("Delete this event?", "Only this occurrence.")));
    assert_eq!(c.event.confirm.label, "Delete");
    // Cancel: back to the details, nothing sent
    assert_eq!(press(&mut c, "#event/cancel"), None);
    assert_eq!(c.event.step(), EventStep::Details);
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
    assert_eq!(c.event.step(), EventStep::Details, "no confirmation for an event the account may not delete");
    assert_eq!(press(&mut c, "#event/rsvp/#segmented/segment-1"), None);
    assert_eq!(press(&mut c, "#event/join"), None);
    assert_eq!(c.event.step(), EventStep::Details, "no code without a meeting link");
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
    assert_eq!((c.event.step(), c.event.qr_title.as_str()), (EventStep::Qr, "Join on phone"));
    assert!(c.event.qr_picture().is_some());
    press(&mut c, "#event/back");
    assert_eq!(c.event.step(), EventStep::Details);
    press(&mut c, "#event/directions");
    assert_eq!((c.event.step(), c.event.qr_title.as_str()), (EventStep::Qr, "Directions"));
    assert_eq!(c.event.qr, "https://www.google.com/maps/search/?api=1&query=Main%20Street%2012");
    c.event.close();
    assert!(images::get(EVENT_QR_KEY).is_none());
}

#[test]
fn the_code_stays_while_its_page_slides_out_and_goes_with_the_panel() {
    let _one = EVENT_QR_TESTS.lock().unwrap_or_else(|e| e.into_inner());
    let mut e = event("meet", "2026-11-10T09:00:00Z", 30);
    e.meeting_url = "https://meet.google.com/abc-defg-hij".to_string();
    e.location = "Main Street 12".to_string();
    let mut c = opened(e);
    press(&mut c, "#event/join");
    let join = c.event.qr_picture().map(|p| p.id);
    assert!(join.is_some());
    // Back: the QR page is drawn while it slides out, its picture with it
    press(&mut c, "#event/back");
    assert_eq!(c.event.step(), EventStep::Details);
    assert_eq!(c.event.qr_picture().map(|p| p.id), join, "the picture stays after its step");
    assert!(images::get(EVENT_QR_KEY).is_some());
    // the same code again: nothing made again
    press(&mut c, "#event/join");
    assert_eq!(c.event.qr_picture().map(|p| p.id), join);
    // the next code replaces it under the same key
    press(&mut c, "#event/back");
    press(&mut c, "#event/directions");
    assert_eq!(c.event.qr, "https://www.google.com/maps/search/?api=1&query=Main%20Street%2012");
    assert!(c.event.qr_picture().is_some());
    // the panel closes: the picture goes
    c.event.close();
    assert!(c.event.qr_picture().is_none() && images::get(EVENT_QR_KEY).is_none());
}

/// The design's distance between two of the panel's pages (event-patch.js: the content's width).
const PAGE_W: f32 = 880.0;

fn page(c: &AgendaCard, step: EventStep) -> everyware_ui::screens::ScreenTarget {
    c.event.pages.page(step as usize, EVENT_EFFECT, PAGE_W)
}

#[test]
fn the_steps_slide_in_from_the_right_and_back_out() {
    let _one = EVENT_QR_TESTS.lock().unwrap_or_else(|e| e.into_inner());
    let mut e = event("mine", "2026-11-10T09:00:00Z", 30);
    e.meeting_url = "https://meet.google.com/abc-defg-hij".to_string();
    e.can_delete = true;
    let mut c = opened(e);
    // the panel opens on the details: nothing slides, the other pages wait on the right
    assert_eq!(c.event.pages.previous, None);
    assert_eq!((page(&c, EventStep::Details).offset, page(&c, EventStep::Details).offset_transition), (Some(0.0), None));
    assert_eq!((page(&c, EventStep::Qr).offset, page(&c, EventStep::Qr).opacity), (Some(PAGE_W), 0.0));
    // Join: the code comes from the right, the details leave to the left, in the pages' time
    press(&mut c, "#event/join");
    let (qr, details) = (page(&c, EventStep::Qr), page(&c, EventStep::Details));
    assert_eq!((c.event.step(), qr.offset, qr.opacity, details.offset), (EventStep::Qr, Some(0.0), 100.0, Some(-PAGE_W)));
    assert_eq!(qr.offset_transition.map(|t| t.duration.as_millis()), Some(300));
    // the details go when they are out (drawn while they slide)
    assert_eq!((details.opacity, details.opacity_transition.map(|t| t.delay.as_millis())), (0.0, Some(300)));
    // a tap on a page that does not show does nothing: only the step's own controls answer
    for other in ["#event/delete", "#event/join", "#event/cancel", "#event/confirm", "#event/disconnect", "#event/change-access"] {
        assert_eq!(press(&mut c, other), None);
        assert_eq!(c.event.step(), EventStep::Qr, "{other}");
    }
    // Back: the details come back from the left, the code leaves to the right
    press(&mut c, "#event/back");
    assert_eq!((c.event.step(), page(&c, EventStep::Details).offset, page(&c, EventStep::Qr).offset), (EventStep::Details, Some(0.0), Some(PAGE_W)));
    // Delete: the confirmation comes one width from the right over the pages between, which
    // stay hidden
    press(&mut c, "#event/delete");
    assert_eq!((c.event.step(), page(&c, EventStep::Confirm).offset, page(&c, EventStep::Details).offset), (EventStep::Confirm, Some(0.0), Some(-PAGE_W)));
    assert_eq!(page(&c, EventStep::Confirm).offset_transition.map(|t| t.duration.as_millis()), Some(300));
    for between in [EventStep::Qr, EventStep::Calendar] {
        assert_eq!((page(&c, between).opacity, page(&c, between).opacity_transition), (0.0, None), "{between:?}");
    }
    // Cancel: back to the details from the left
    press(&mut c, "#event/cancel");
    assert_eq!((c.event.step(), page(&c, EventStep::Details).offset, page(&c, EventStep::Confirm).offset), (EventStep::Details, Some(0.0), Some(PAGE_W)));
    // a panel closed on a step opens again on the details, nothing slides
    press(&mut c, "#event/delete");
    c.event.close();
    tap_row(&mut c, 1);
    assert_eq!((c.event.step(), c.event.pages.previous, page(&c, EventStep::Details).offset_transition), (EventStep::Details, None, None));
    c.event.close();
}

#[test]
fn the_calendar_s_link_opens_on_its_page_and_its_confirmation_slides_in() {
    let mut c = card();
    c.set(linked(vec![]), "UTC", true, at("2026-11-10T08:00:00Z"));
    c.on_account(Gesture::DOWN);
    c.on_account(Gesture::UP);
    // the panel opens on the link's page: it is there at once, nothing slides
    let link = page(&c, EventStep::Calendar);
    assert_eq!((c.event.step(), c.event.pages.previous), (EventStep::Calendar, None));
    assert_eq!((link.offset, link.opacity, link.offset_transition), (Some(0.0), 100.0, None));
    assert_eq!((page(&c, EventStep::Details).opacity, page(&c, EventStep::Confirm).offset), (0.0, Some(PAGE_W)));
    // Disconnect: the confirmation comes from the right, the link leaves to the left
    press(&mut c, "#event/disconnect");
    assert_eq!((c.event.step(), page(&c, EventStep::Confirm).offset, page(&c, EventStep::Calendar).offset), (EventStep::Confirm, Some(0.0), Some(-PAGE_W)));
    assert_eq!(page(&c, EventStep::Confirm).offset_transition.map(|t| t.duration.as_millis()), Some(300));
    // Cancel: the link comes back from the left
    press(&mut c, "#event/cancel");
    assert_eq!((c.event.step(), page(&c, EventStep::Calendar).offset, page(&c, EventStep::Confirm).offset), (EventStep::Calendar, Some(0.0), Some(PAGE_W)));
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
    assert!(c.event.is_open() && c.event.step() == EventStep::Calendar);
    assert_eq!(c.event.access, "The panel may read this calendar.");
    // Change access and Disconnect: both a confirmation, then the unlink
    assert_eq!(press(&mut c, "#event/change-access"), None);
    assert_eq!((c.event.step(), c.event.confirm.label.as_str(), c.event.confirm_texts().0), (EventStep::Confirm, "Change access", "Change the access?"));
    press(&mut c, "#event/cancel");
    assert_eq!(c.event.step(), EventStep::Calendar, "Cancel goes back to the link");
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
    // a's panel opened again says why (once), b's again says nothing
    c.event.close();
    tap_row(&mut c, 1);
    assert_eq!(c.event.error, failure_text("delete", CalendarFailure::CALENDAR_FAILURE_AGAIN));
    c.event.close();
    tap_row(&mut c, 1);
    assert!(c.event.error.is_empty(), "said once");
    c.event.close();
    tap_row(&mut c, 2);
    assert!(c.event.error.is_empty());
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
    assert_eq!((c.event.step(), c.event.qr_host.as_str()), (EventStep::Qr, "meet.google.com"));
    // a link whose host is not plain letters gets no code (it stays text in the details)
    let mut e = event("odd", "2026-11-10T09:00:00Z", 30);
    e.meeting_url = "https://gооgle.com/meet".to_string();
    let mut d = opened(e);
    press(&mut d, "#event/join");
    assert_eq!(d.event.step(), EventStep::Details);
    assert!(d.event.qr_picture().is_none());
    c.event.close();
}

#[test]
fn the_answers_and_the_destructive_buttons_have_their_tones() {
    let c = opened(invitation());
    assert_eq!((0..3).map(|i| c.event.rsvp.tone_of(i)).collect::<Vec<_>>(), [Tone::Success, Tone::Warning, Tone::Error]);
    assert_eq!(c.event.delete.tone, Tone::Error);
    assert_eq!((c.event.join.tone, c.event.directions.tone, c.event.back.tone, c.event.cancel.tone), (Tone::Default, Tone::Default, Tone::Default, Tone::Default));
    // the confirmation: Delete and Disconnect red, Change access as it is
    let mut e = event("mine", "2026-11-10T09:00:00Z", 30);
    e.can_delete = true;
    let mut c = opened(e);
    press(&mut c, "#event/delete");
    assert_eq!((c.event.confirm.label.as_str(), c.event.confirm.tone), ("Delete", Tone::Error));
    c.event.close();
    c.on_account(Gesture::DOWN);
    c.on_account(Gesture::UP);
    press(&mut c, "#event/change-access");
    assert_eq!((c.event.confirm.label.as_str(), c.event.confirm.tone), ("Change access", Tone::Default));
    press(&mut c, "#event/cancel");
    press(&mut c, "#event/disconnect");
    assert_eq!((c.event.confirm.label.as_str(), c.event.confirm.tone), ("Disconnect", Tone::Error));
}
