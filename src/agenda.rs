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

//! The agenda of the dashboard's right column (#agenda): the calendar bundle's agenda (topic
//! `agenda`, through the dashboard bundle) as rows of ui-lib's list item. A subheader per day
//! ("TODAY", "TOMORROW", then the weekday; the date at the end), under it the day's events (kind
//! leading text: the start over the duration, a bar in the event's colour, the title over the
//! place), and "Nothing scheduled. Enjoy the day." for a today without
//! events. The days and the times are the panel's: its time zone and its 12 or 24 hour clock,
//! the settings the clock follows. While no calendar is linked a card over the list says so,
//! with the QR code of the setup page when there is one (`qr_rgba`, a picture the app makes).
//!
//! The texts come from a calendar, that is from anyone who sends an invitation: they are shown
//! as plain text on one line (`plain`), and the bars' colours are snapped to Google's palettes
//! (`accent`), so the screen holds a picture for each of a few colours only. The panel is seen
//! by everyone in the room: the agenda's `calendar` is never shown (Google names a primary
//! calendar by the account's e-mail address); the card's only title is the design's "Agenda".

use everyware_catalog::calendar::{Agenda, AgendaEvent, CalendarLink};
use everyware_ui_core::list_item::{ListItem, ListItemKind, ListItemLines, ListItemSize};
use jiff::civil::Date;
use jiff::tz::TimeZone;
use jiff::{Timestamp, Zoned};
use qrcodegen_no_heap::{QrCode, QrCodeEcc, Version};

/// What a row takes of the list: an event 72 high, a subheader 48, 8 between them (the
/// design's, dashboard-patch.js).
pub const ROW_PITCH: f32 = 72.0 + 8.0;
pub const SUBHEADER_PITCH: f32 = 48.0 + 8.0;
/// The line of a today without events.
pub const NOTHING_TODAY: &str = "Nothing scheduled. Enjoy the day.";
/// The key of the QR code's picture (everyware_ui::images).
pub const QR_KEY: &str = "agenda/qr";
/// The QR code's frame in the design (#agenda/qr), square: the picture is made at this size, so
/// harry draws it 1:1 and every module stays sharp.
pub const QR_SIDE: u32 = 264;
/// The white around the code, in modules: the QR standard's quiet zone.
const QUIET: u32 = 4;

const WEEKDAYS: [&str; 7] = ["MONDAY", "TUESDAY", "WEDNESDAY", "THURSDAY", "FRIDAY", "SATURDAY", "SUNDAY"];
const MONTHS: [&str; 12] = ["JAN", "FEB", "MAR", "APR", "MAY", "JUN", "JUL", "AUG", "SEP", "OCT", "NOV", "DEC"];

/// The colours of Google Calendar's events: the palette of its API (colorId 1 to 11) and the
/// one its apps show today. A colour of the agenda takes the nearest of them.
const PALETTE: [u32; 22] = [
    0xa4bdfc, 0x7ae7bf, 0xdbadff, 0xff887c, 0xfbd75b, 0xffb878, 0x46d6db, 0xe1e1e1, 0x5484ed, 0x51b749, 0xdc2127,
    0x7986cb, 0x33b679, 0x8e24aa, 0xe67c73, 0xf6bf26, 0xf4511e, 0x039be5, 0x616161, 0x3f51b5, 0x0b8043, 0xd50000,
];

/// The card over the list while no calendar is linked: what it says and the address its QR code
/// carries (none: no code, the texts alone).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LinkCard {
    pub title: String,
    pub hint: String,
    pub qr: Option<String>,
}

/// The card for the agenda's link, or none while a calendar is linked (the list shows). Before
/// the first agenda it waits, as the weather does.
pub fn link_card(agenda: Option<&Agenda>, house_link: bool) -> Option<LinkCard> {
    let card = |title: &str, hint: &str, qr: Option<String>| Some(LinkCard { title: title.to_string(), hint: hint.to_string(), qr });
    let Some(agenda) = agenda else {
        return card("Waiting for the calendar", if house_link { "from the calendar bundle" } else { "no link to the house's data" }, None);
    };
    let url = Some(agenda.setup_url.clone()).filter(|u| !u.is_empty());
    let same_wifi = "Your phone must be on the same Wi-Fi";
    let no_address = "The panel has no network address yet";
    match agenda.link.enum_value() {
        Ok(CalendarLink::CALENDAR_CONNECTED) => None,
        Ok(CalendarLink::CALENDAR_NOT_CONNECTED) if url.is_some() => card("Scan to connect Google Calendar", same_wifi, url),
        Ok(CalendarLink::CALENDAR_NOT_CONNECTED) => card("Connect Google Calendar", no_address, None),
        Ok(CalendarLink::CALENDAR_EXPIRED) if url.is_some() => card("Connect again", same_wifi, url),
        Ok(CalendarLink::CALENDAR_EXPIRED) => card("Connect again", no_address, None),
        // no OAuth client on the panel, or a state this panel does not know: nothing to scan
        Ok(CalendarLink::CALENDAR_NO_CLIENT) | Err(_) => card("Calendar not set up", "", None),
    }
}

/// The rows of the agenda at `now` in the zone `tz`: the days with events from today on, today
/// always; an event that has ended is left out, one that began before today and goes on is
/// today's. None of it while no calendar is linked (the card shows).
pub fn rows(agenda: &Agenda, now: Timestamp, tz: &TimeZone, hour24: bool) -> Vec<ListItem> {
    if agenda.link.enum_value() != Ok(CalendarLink::CALENDAR_CONNECTED) {
        return Vec::new();
    }
    let today = now.to_zoned(tz.clone()).date();
    let mut days: Vec<(Date, Vec<&AgendaEvent>)> = vec![(today, Vec::new())];
    for event in &agenda.events {
        if ended(event, now, today) {
            continue;
        }
        let day = first_day(event, tz).max(today);
        match days.iter_mut().find(|(d, _)| *d == day) {
            Some((_, events)) => events.push(event),
            None => days.push((day, vec![event])),
        }
    }
    days.sort_by_key(|(d, _)| *d);
    let mut out = Vec::new();
    for (day, mut events) in days {
        out.push(
            ListItem::new(ListItemKind::Subheader, day_label(day, today), "")
                .trailing_text(date_label(day))
                .size(ListItemSize::Column)
                .pitch(SUBHEADER_PITCH),
        );
        if events.is_empty() {
            out.push(ListItem::new(ListItemKind::Plain, NOTHING_TODAY, "").lines(ListItemLines::One).size(ListItemSize::Column));
        }
        // a day's all-day events first, then by their start
        events.sort_by_key(|e| (!e.all_day, e.start));
        out.extend(events.into_iter().map(|e| row(e, tz, hour24)));
    }
    out
}

fn row(event: &AgendaEvent, tz: &TimeZone, hour24: bool) -> ListItem {
    let (start, length) = if event.all_day {
        let days = (event.end - event.start) / 86_400;
        ("All day".to_string(), if days > 1 { format!("{days} days") } else { String::new() })
    } else {
        (time_text(&zoned(event.start, tz), hour24), duration_text(event.end - event.start))
    };
    let title = Some(plain(&event.title)).filter(|t| !t.is_empty()).unwrap_or_else(|| "(No title)".to_string());
    let item = ListItem::new(ListItemKind::LeadingText, title, plain(&event.location)).leading_text(start, length).size(ListItemSize::Column);
    match accent(event.color) {
        Some(rgb) => item.accent(rgb),
        None => item,
    }
}

/// Whether the event is over at `now`: a timed one at its end, an all-day one after its last
/// day (its dates are the calendar's, not the zone's).
fn ended(event: &AgendaEvent, now: Timestamp, today: Date) -> bool {
    if event.all_day {
        return utc_date(event.end.max(event.start + 86_400)) <= today;
    }
    if event.end > event.start { event.end <= now.as_second() } else { event.start < now.as_second() }
}

/// The local day an event starts on; an all-day event's date is the same in every zone.
fn first_day(event: &AgendaEvent, tz: &TimeZone) -> Date {
    if event.all_day { utc_date(event.start) } else { zoned(event.start, tz).date() }
}

fn zoned(secs: i64, tz: &TimeZone) -> Zoned {
    Timestamp::from_second(secs).unwrap_or(Timestamp::UNIX_EPOCH).to_zoned(tz.clone())
}

fn utc_date(secs: i64) -> Date {
    zoned(secs, &TimeZone::UTC).date()
}

/// "TODAY", "TOMORROW", else the weekday: "WEDNESDAY".
pub fn day_label(day: Date, today: Date) -> String {
    match today.until(day).map(|span| span.get_days()) {
        Ok(0) => "TODAY".to_string(),
        Ok(1) => "TOMORROW".to_string(),
        _ => WEEKDAYS[day.weekday().to_monday_zero_offset() as usize].to_string(),
    }
}

/// "NOV 10".
pub fn date_label(day: Date) -> String {
    format!("{} {}", MONTHS[(day.month() - 1) as usize], day.day())
}

/// The start as the clock shows the time: "08:30" on a 24-hour clock, "8:30 AM" on the other.
pub fn time_text(at: &Zoned, hour24: bool) -> String {
    let (h, m) = (at.hour(), at.minute());
    if hour24 {
        return format!("{h:02}:{m:02}");
    }
    let h12 = match h % 12 { 0 => 12, h => h };
    format!("{h12}:{m:02} {}", if h < 12 { "AM" } else { "PM" })
}

/// "30 min", "1h", "1h 30", "1d 2h"; nothing for an event without a length.
pub fn duration_text(secs: i64) -> String {
    let min = secs.max(0) / 60;
    let (d, h, m) = (min / 1440, min % 1440 / 60, min % 60);
    match (d, h, m) {
        (0, 0, 0) => String::new(),
        (0, 0, m) => format!("{m} min"),
        (0, h, 0) => format!("{h}h"),
        (0, h, m) => format!("{h}h {m:02}"),
        (d, 0, _) => format!("{d}d"),
        (d, h, _) => format!("{d}d {h}h"),
    }
}

/// A text from the calendar as one plain line: control characters, line breaks and the
/// characters that turn the direction of text are spaces, runs of space are one.
pub fn plain(text: &str) -> String {
    let bidi = |c: char| matches!(c, '\u{200e}' | '\u{200f}' | '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}');
    text.split(|c: char| c.is_control() || c.is_whitespace() || bidi(c)).filter(|w| !w.is_empty()).collect::<Vec<_>>().join(" ")
}

/// The bar's colour: the nearest of Google's colours; none (0) keeps the design's accent.
pub fn accent(color: u32) -> Option<u32> {
    if color == 0 {
        return None;
    }
    let channels = |c: u32| [(c >> 16) & 0xff, (c >> 8) & 0xff, c & 0xff].map(|v| v as i32);
    let want = channels(color);
    PALETTE.iter().copied().min_by_key(|&p| channels(p).iter().zip(want).map(|(a, b)| (a - b) * (a - b)).sum::<i32>())
}

/// The QR code of `text` as an RGBA picture `side` x `side`: black modules on white, at least
/// the quiet zone of four modules around them, each module a whole number of pixels (the
/// picture is drawn 1:1, so the code stays sharp), the code in the middle. None when the text
/// does not fit a QR code or the code not the picture.
pub fn qr_rgba(text: &str, side: u32) -> Option<Vec<u8>> {
    let len = Version::MAX.buffer_len();
    let (mut temp, mut out) = (vec![0u8; len], vec![0u8; len]);
    let qr = QrCode::encode_text(text, &mut temp, &mut out, QrCodeEcc::Medium, Version::MIN, Version::MAX, None, true).ok()?;
    let modules = qr.size() as u32;
    let scale = side / (modules + 2 * QUIET);
    if scale == 0 {
        return None;
    }
    let offset = (side - modules * scale) / 2;
    let mut rgba = vec![255u8; (side * side * 4) as usize];
    for y in 0..modules * scale {
        for x in 0..modules * scale {
            if qr.get_module((x / scale) as i32, (y / scale) as i32) {
                let i = (((offset + y) * side + offset + x) * 4) as usize;
                rgba[i..i + 3].fill(0);
            }
        }
    }
    Some(rgba)
}

#[cfg(test)]
mod tests {
    use super::*;
    use everyware_ui_core::Component;

    const CET: &str = "CET-1CEST,M3.5.0,M10.5.0/3";
    const NEW_YORK: &str = "EST5EDT,M3.2.0,M11.1.0";

    fn at(s: &str) -> Timestamp {
        s.parse().unwrap()
    }

    fn event(title: &str, start: &str, minutes: i64) -> AgendaEvent {
        let mut e = AgendaEvent::new();
        e.title = title.to_string();
        e.start = at(start).as_second();
        e.end = e.start + minutes * 60;
        e
    }

    fn all_day(title: &str, date: &str, days: i64) -> AgendaEvent {
        let mut e = event(title, &format!("{date}T00:00:00Z"), days * 1440);
        e.all_day = true;
        e
    }

    fn agenda(events: Vec<AgendaEvent>) -> Agenda {
        let mut a = Agenda::new();
        a.link = CalendarLink::CALENDAR_CONNECTED.into();
        a.calendar = "someone@gmail.com".to_string();
        a.events = events;
        a
    }

    /// The rows as "label|trailing" for a subheader and "lead|lead sub|title|place" for an event.
    fn shown(rows: &[ListItem]) -> Vec<String> {
        rows.iter()
            .map(|r| match r.kind {
                ListItemKind::Subheader => format!("{}|{}", r.title, r.trailing_text),
                ListItemKind::LeadingText => format!("{}|{}|{}|{}", r.leading_title, r.leading_subtitle, r.title, r.subtitle),
                _ => r.title.clone(),
            })
            .collect()
    }

    #[test]
    fn days_with_their_events_under_a_subheader_each() {
        let tz = TimeZone::posix(CET).unwrap();
        let now = at("2026-11-10T07:00:00Z"); // Tuesday 08:00 in Berlin
        let mut standup = event("Stand-up", "2026-11-10T07:30:00Z", 30);
        standup.location = "Room 2".to_string();
        standup.color = 0x039be5;
        let a = agenda(vec![standup, event("Lunch", "2026-11-10T11:00:00Z", 90), event("Dentist", "2026-11-11T08:00:00Z", 60), event("Cinema", "2026-11-12T18:00:00Z", 120)]);
        let r = rows(&a, now, &tz, false);
        assert_eq!(
            shown(&r),
            [
                "TODAY|NOV 10", "8:30 AM|30 min|Stand-up|Room 2", "12:00 PM|1h 30|Lunch|",
                "TOMORROW|NOV 11", "9:00 AM|1h|Dentist|",
                "THURSDAY|NOV 12", "7:00 PM|2h|Cinema|",
            ]
        );
        assert_eq!((r[0].pitch, r[1].pitch), (Some(SUBHEADER_PITCH), None));
        assert_eq!(r[1].variant(), "state=default, kind=leading-text, lines=two, size=column");
        assert_eq!(r[0].variant(), "state=default, kind=subheader, lines=one, size=column");
        assert_eq!((r[1].accent, r[2].accent), (Some(0x039be5), None));
        // the same agenda on a 24-hour clock
        let r = rows(&a, now, &tz, true);
        assert_eq!((r[1].leading_title.as_str(), r[6].leading_title.as_str()), ("08:30", "19:00"));
    }

    #[test]
    fn midnight_is_the_zone_s() {
        // 23:30 UTC on the 10th: still the 10th in New York, the 11th in Berlin
        let now = at("2026-11-10T12:00:00Z");
        let a = agenda(vec![event("Late call", "2026-11-10T23:30:00Z", 45)]);
        let berlin = shown(&rows(&a, now, &TimeZone::posix(CET).unwrap(), true));
        assert_eq!(berlin, ["TODAY|NOV 10", NOTHING_TODAY, "TOMORROW|NOV 11", "00:30|45 min|Late call|"]);
        let new_york = shown(&rows(&a, now, &TimeZone::posix(NEW_YORK).unwrap(), false));
        assert_eq!(new_york, ["TODAY|NOV 10", "6:30 PM|45 min|Late call|"]);
        assert_eq!(shown(&rows(&a, now, &TimeZone::UTC, false))[1], "11:30 PM|45 min|Late call|");
        // today is the zone's too: at midnight UTC it is the 11th in Berlin and in UTC, still the
        // 10th in New York
        let late = at("2026-11-11T00:00:00Z");
        assert_eq!(shown(&rows(&agenda(vec![]), late, &TimeZone::posix(CET).unwrap(), true))[0], "TODAY|NOV 11");
        assert_eq!(shown(&rows(&agenda(vec![]), late, &TimeZone::UTC, true))[0], "TODAY|NOV 11");
        assert_eq!(shown(&rows(&agenda(vec![]), late, &TimeZone::posix(NEW_YORK).unwrap(), true))[0], "TODAY|NOV 10");
    }

    #[test]
    fn an_all_day_event_keeps_its_date_in_every_zone() {
        // 20:00 on the 10th in New York is already the 11th in UTC: the birthday on the 11th is
        // tomorrow there, today in Berlin (05:00 on the 11th)
        let now = at("2026-11-11T01:00:00Z");
        let a = agenda(vec![all_day("Birthday", "2026-11-11", 1), all_day("Trip", "2026-11-12", 3), event("Breakfast", "2026-11-11T07:00:00Z", 30)]);
        let ny = shown(&rows(&a, now, &TimeZone::posix(NEW_YORK).unwrap(), false));
        assert_eq!(ny, [
            "TODAY|NOV 10", NOTHING_TODAY,
            "TOMORROW|NOV 11", "All day||Birthday|", "2:00 AM|30 min|Breakfast|",
            "THURSDAY|NOV 12", "All day|3 days|Trip|",
        ]);
        let berlin = shown(&rows(&a, now, &TimeZone::posix(CET).unwrap(), true));
        assert_eq!(&berlin[..3], ["TODAY|NOV 11", "All day||Birthday|", "08:00|30 min|Breakfast|"]);
        // on its last day a trip of three days is today's; the day after, it is over
        assert_eq!(shown(&rows(&a, at("2026-11-14T10:00:00Z"), &TimeZone::UTC, true))[1], "All day|3 days|Trip|");
        assert_eq!(shown(&rows(&a, at("2026-11-15T10:00:00Z"), &TimeZone::UTC, true))[1], NOTHING_TODAY);
    }

    #[test]
    fn an_ended_event_goes_and_one_that_goes_on_is_today_s() {
        let tz = TimeZone::UTC;
        let now = at("2026-11-10T09:00:00Z");
        let a = agenda(vec![event("Over", "2026-11-10T07:00:00Z", 60), event("Night shift", "2026-11-09T22:00:00Z", 12 * 60), event("Reminder", "2026-11-10T10:00:00Z", 0)]);
        assert_eq!(shown(&rows(&a, now, &tz, true)), ["TODAY|NOV 10", "22:00|12h|Night shift|", "10:00||Reminder|"]);
    }

    #[test]
    fn an_empty_today_says_so_and_the_week_follows() {
        let now = at("2026-11-10T09:00:00Z");
        let r = rows(&agenda(vec![event("Yoga", "2026-11-13T17:00:00Z", 60)]), now, &TimeZone::UTC, true);
        assert_eq!(shown(&r), ["TODAY|NOV 10", NOTHING_TODAY, "FRIDAY|NOV 13", "17:00|1h|Yoga|"]);
        assert_eq!(r[1].variant(), "state=default, kind=plain, lines=one, size=column");
        assert_eq!(shown(&rows(&agenda(vec![]), now, &TimeZone::UTC, true)), ["TODAY|NOV 10", NOTHING_TODAY]);
    }

    #[test]
    fn a_private_event_shows_busy_and_no_place() {
        // the bundle sends "Busy" and no place for a private event
        let r = rows(&agenda(vec![event("Busy", "2026-11-10T10:00:00Z", 60)]), at("2026-11-10T09:00:00Z"), &TimeZone::UTC, true);
        assert_eq!(shown(&r)[1], "10:00|1h|Busy|");
        let r = rows(&agenda(vec![event("", "2026-11-10T10:00:00Z", 60)]), at("2026-11-10T09:00:00Z"), &TimeZone::UTC, true);
        assert_eq!(r[1].title, "(No title)");
    }

    #[test]
    fn the_calendar_is_never_shown() {
        // the agenda's calendar is the account's address here (agenda()): no row carries it
        let a = agenda(vec![event("Dentist", "2026-11-10T10:00:00Z", 60), all_day("Trip", "2026-11-11", 2)]);
        let r = rows(&a, at("2026-11-10T09:00:00Z"), &TimeZone::UTC, true);
        assert!(r.iter().all(|row| !format!("{row:?}").contains('@')), "{r:?}");
        assert_eq!(shown(&r)[1], "10:00|1h|Dentist|");
    }

    #[test]
    fn texts_from_the_calendar_are_one_plain_line() {
        assert_eq!(plain("Main Street 12\nBerlin"), "Main Street 12 Berlin");
        assert_eq!(plain("  a\t\tb \u{202e}gnp.exe\u{202c} "), "a b gnp.exe");
        assert_eq!(plain("\u{0007}"), "");
    }

    #[test]
    fn durations() {
        assert_eq!(duration_text(30 * 60), "30 min");
        assert_eq!(duration_text(60 * 60), "1h");
        assert_eq!(duration_text(90 * 60), "1h 30");
        assert_eq!(duration_text(65 * 60), "1h 05");
        assert_eq!(duration_text(25 * 3600), "1d 1h");
        assert_eq!(duration_text(48 * 3600), "2d");
        assert_eq!(duration_text(0), "");
        assert_eq!(duration_text(-60), "");
    }

    #[test]
    fn times_on_both_clocks() {
        let t = |s: &str, h24| time_text(&at(s).to_zoned(TimeZone::UTC), h24);
        assert_eq!((t("2026-11-10T00:15:00Z", false), t("2026-11-10T12:00:00Z", false)), ("12:15 AM".to_string(), "12:00 PM".to_string()));
        assert_eq!((t("2026-11-10T08:30:00Z", false), t("2026-11-10T20:05:00Z", true)), ("8:30 AM".to_string(), "20:05".to_string()));
        assert_eq!(t("2026-11-10T08:30:00Z", true), "08:30");
    }

    #[test]
    fn colours_snap_to_google_s() {
        assert_eq!(accent(0), None);
        assert_eq!(accent(0x039be5), Some(0x039be5));
        assert_eq!(accent(0xa4bdfc), Some(0xa4bdfc));
        // a calendar's own colour: the nearest
        assert_eq!(accent(0x0a8142), Some(0x0b8043));
        assert_eq!(accent(0xff0000), Some(0xd50000));
    }

    #[test]
    fn each_link_state() {
        let mut a = Agenda::new();
        let card = |a: &Agenda| link_card(Some(a), true);
        a.link = CalendarLink::CALENDAR_NO_CLIENT.into();
        a.setup_url = "http://192.168.1.20:8765/setup?t=abc".to_string();
        assert_eq!(card(&a), Some(LinkCard { title: "Calendar not set up".into(), hint: String::new(), qr: None }));
        a.link = CalendarLink::CALENDAR_NOT_CONNECTED.into();
        assert_eq!(card(&a), Some(LinkCard { title: "Scan to connect Google Calendar".into(), hint: "Your phone must be on the same Wi-Fi".into(), qr: Some(a.setup_url.clone()) }));
        a.link = CalendarLink::CALENDAR_EXPIRED.into();
        assert_eq!(card(&a).map(|c| (c.title, c.qr.is_some())), Some(("Connect again".to_string(), true)));
        a.link = CalendarLink::CALENDAR_CONNECTED.into();
        assert_eq!(card(&a), None);
        assert_eq!(rows(&a, at("2026-11-10T09:00:00Z"), &TimeZone::UTC, true).len(), 2, "a linked agenda has its rows");
        // no address yet: no code to scan
        a.link = CalendarLink::CALENDAR_NOT_CONNECTED.into();
        a.setup_url.clear();
        assert_eq!(card(&a).map(|c| c.qr), Some(None));
        assert!(rows(&a, at("2026-11-10T09:00:00Z"), &TimeZone::UTC, true).is_empty(), "no rows behind the card");
        // a state this panel does not know, and the time before the first agenda
        a.link = protobuf::EnumOrUnknown::from_i32(9);
        assert_eq!(card(&a).map(|c| c.title), Some("Calendar not set up".to_string()));
        assert_eq!(link_card(None, true).map(|c| c.hint), Some("from the calendar bundle".to_string()));
        assert_eq!(link_card(None, false).map(|c| c.hint), Some("no link to the house's data".to_string()));
    }

    /// Whether the pixel at (x, y) of a QR picture is black.
    fn black(rgba: &[u8], side: u32, x: u32, y: u32) -> bool {
        rgba[((y * side + x) * 4) as usize] == 0
    }

    #[test]
    fn the_qr_code_is_sharp_and_has_its_quiet_zone() {
        let url = "http://192.168.101.23:8765/setup?t=Q2xhdWRlIG1hZGUgdGhpcw";
        let rgba = qr_rgba(url, QR_SIDE).unwrap();
        assert_eq!(rgba.len(), (QR_SIDE * QR_SIDE * 4) as usize);
        assert!(rgba.chunks(4).all(|p| p[3] == 255), "opaque");
        // where the code starts: the first black pixel of the top left finder pattern
        let start = (0..QR_SIDE).find(|&i| black(&rgba, QR_SIDE, i, i)).unwrap();
        // the finder pattern is 7 modules: its black edge, then the white ring one module in
        let module = (start..QR_SIDE).take_while(|&x| black(&rgba, QR_SIDE, x, start)).count() as u32 / 7;
        assert!(module >= 2, "at least 2 pixels a module: {module}");
        assert!(start >= QUIET * module, "the quiet zone: {start} px for modules of {module}");
        assert!(black(&rgba, QR_SIDE, start + module / 2, start + module / 2) && !black(&rgba, QR_SIDE, start + module + module / 2, start + module + module / 2));
        // the code in the middle: as much white on the other side (a pixel more when the rest is odd)
        let end = (0..QR_SIDE).rev().find(|&i| black(&rgba, QR_SIDE, i, start)).unwrap();
        assert!((QR_SIDE - 1 - end).abs_diff(start) <= 1, "{start} px left, {} right", QR_SIDE - 1 - end);
        // nothing black in the quiet zone, on any side
        for i in 0..QR_SIDE {
            for j in (0..start).chain(QR_SIDE - start..QR_SIDE) {
                assert!(!black(&rgba, QR_SIDE, i, j) && !black(&rgba, QR_SIDE, j, i), "black at {i},{j}");
            }
        }
        // the longest address the agenda carries (256 bytes) still fits, a picture too small does not
        assert!(qr_rgba(&"a".repeat(256), QR_SIDE).is_some());
        assert!(qr_rgba(url, 20).is_none());
        assert!(qr_rgba(&"a".repeat(4000), QR_SIDE).is_none());
    }
}
