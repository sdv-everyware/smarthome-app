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
//! leading text: the start over the duration, "Until" over the end for an event that began on an
//! earlier day, "All day" over "Day 2 of 3"; a bar in the event's colour; the whole title over as
//! many lines as it takes, the place under it), and "Nothing scheduled. Enjoy the day." for a
//! today without events. Until the calendar was read once the list has no day, only a line that
//! it is being read or could not be read: the events are unknown, not none. A tap on an event
//! opens its details (`details`); the other lines take no tap. The days and the times are the
//! panel's: its time zone and its 12 or 24 hour clock, the settings the clock follows.
//! While no calendar is linked a card over the list says so, with the QR code of the setup page
//! when there is one (`qr_rgba`, a picture the app makes); an old agenda says its age (`status`).
//!
//! The texts come from a calendar, that is from anyone who sends an invitation: they are shown
//! as plain text (`plain`, `plain_lines`), and the bars' colours are snapped to Google's palettes
//! (`accent`), so the screen holds a picture for each of a few colours only. The panel is seen
//! by everyone in the room: the agenda's `calendar` is never shown (Google names a primary
//! calendar by the account's e-mail address); the card's only title is the design's "Agenda".

use everyware_catalog::calendar::{Agenda, AgendaEvent, CalendarLink, Rsvp};
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
/// The line while the calendar was not read yet, and after a read that failed.
pub const READING: &str = "Reading the calendar";
pub const NOT_READ: &str = "The calendar could not be read. Trying again.";
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
/// the first agenda it waits, as the weather does. Without the link to the house's data no code
/// shows: its one-time token is likely gone.
pub fn link_card(agenda: Option<&Agenda>, house_link: bool) -> Option<LinkCard> {
    let card = |title: &str, hint: &str, qr: Option<String>| Some(LinkCard { title: title.to_string(), hint: hint.to_string(), qr });
    let no_link = "no link to the house's data";
    let Some(agenda) = agenda else {
        return card("Waiting for the calendar", if house_link { "from the calendar bundle" } else { no_link }, None);
    };
    let url = Some(agenda.setup_url.clone()).filter(|u| !u.is_empty() && house_link);
    let hint = if !house_link { no_link } else if url.is_some() { "Your phone must be on the same Wi-Fi" } else { "The panel has no network address yet" };
    match agenda.link.enum_value() {
        Ok(CalendarLink::CALENDAR_CONNECTED) => None,
        Ok(CalendarLink::CALENDAR_NOT_CONNECTED) if url.is_some() => card("Scan to connect Google Calendar", hint, url),
        Ok(CalendarLink::CALENDAR_NOT_CONNECTED) => card("Connect Google Calendar", hint, None),
        Ok(CalendarLink::CALENDAR_EXPIRED) => card("Connect again", hint, url),
        // no OAuth client on the panel, or a state this panel does not know: nothing to scan
        Ok(CalendarLink::CALENDAR_NO_CLIENT) | Err(_) => card("Calendar not set up", "", None),
    }
}

/// An agenda older than this is said to be (the bundle reads the calendar every minute).
const STALE_SECS: i64 = 5 * 60;

/// The line beside the card's title: empty while the agenda is fresh; else how old it is, as
/// the weather says it ("Last read 14:02 · no link to the house's data"; "Last read Mon 14:02"
/// on another day). Before the first read the list says it (`READING`).
pub fn status(agenda: Option<&Agenda>, house_link: bool, now: Timestamp, tz: &TimeZone, hour24: bool) -> String {
    let Some(agenda) = agenda.filter(|a| a.link.enum_value() == Ok(CalendarLink::CALENDAR_CONNECTED)) else {
        return String::new();
    };
    let last = (agenda.updated > 0).then(|| format!("Last read {}", read_at(agenda.updated, now, tz, hour24)));
    match (house_link, last) {
        (false, Some(last)) => format!("{last} · no link to the house's data"),
        (false, None) => "no link to the house's data".to_string(),
        (true, Some(last)) if now.as_second().saturating_sub(agenda.updated) > STALE_SECS => last,
        (true, _) => String::new(),
    }
}

/// When the agenda was read: the time on the panel's today ("14:02"), the weekday before it on
/// another day of the last week ("Mon 14:02"), the date before that ("Nov 10 14:02").
fn read_at(updated: i64, now: Timestamp, tz: &TimeZone, hour24: bool) -> String {
    let read = zoned(updated, tz);
    let time = time_text(&read, hour24);
    match read.date().until(now.to_zoned(tz.clone()).date()).map(|span| span.get_days()) {
        Ok(0) => time,
        Ok(1..=6) => format!("{} {time}", weekday_short(read.date())),
        _ => format!("{} {time}", month_day(read.date())),
    }
}

/// The rows of the agenda and, for each row, the event it shows (an index into the agenda's
/// events; none for a subheader and the line of a free today).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct AgendaRows {
    pub items: Vec<ListItem>,
    pub events: Vec<Option<usize>>,
}

/// The rows of the agenda at `now` in the zone `tz`: the days with events from today on, today
/// always; an event that has ended is left out, one that began before today and goes on is
/// today's. A headline is shown whole: `headline_lines` says how many lines it takes in a row
/// (the app measures it), the row is that high and takes that much of the list. None of it while
/// no calendar is linked (the card shows); before the first read one line, `READING` or, after
/// a failed read, `NOT_READ`, and no day. Each row is known by its event's id or its day across a
/// new agenda (`ListItem::id`), so a finger on a row stays on it when titles repeat.
pub fn rows(agenda: &Agenda, now: Timestamp, tz: &TimeZone, hour24: bool, headline_lines: &dyn Fn(&str) -> usize) -> AgendaRows {
    let mut out = AgendaRows::default();
    if agenda.link.enum_value() != Ok(CalendarLink::CALENDAR_CONNECTED) {
        return out;
    }
    if agenda.updated == 0 {
        // not read yet: the events are unknown, not none (calendar.proto)
        out.items.push(note(if agenda.read_failed { NOT_READ } else { READING }));
        out.events.push(None);
        return out;
    }
    let today = now.to_zoned(tz.clone()).date();
    let mut days: Vec<(Date, Vec<usize>)> = vec![(today, Vec::new())];
    for (i, event) in agenda.events.iter().enumerate() {
        if ended(event, now, today) {
            continue;
        }
        let day = first_day(event, tz).max(today);
        match days.iter_mut().find(|(d, _)| *d == day) {
            Some((_, events)) => events.push(i),
            None => days.push((day, vec![i])),
        }
    }
    days.sort_by_key(|(d, _)| *d);
    for (day, mut events) in days {
        out.items.push(
            ListItem::new(ListItemKind::Subheader, day_label(day, today), "")
                .trailing_text(date_label(day))
                .id(format!("day/{day}"))
                .size(ListItemSize::Column)
                .pitch(SUBHEADER_PITCH),
        );
        out.events.push(None);
        if events.is_empty() {
            out.items.push(note(NOTHING_TODAY));
            out.events.push(None);
        }
        // a day's all-day events first, then by their start; equal ones keep Google's order
        events.sort_by_key(|&i| (!agenda.events[i].all_day, agenda.events[i].start));
        for i in events {
            out.items.push(row(&agenda.events[i], day, tz, hour24, headline_lines));
            out.events.push(Some(i));
        }
    }
    out
}

/// Between two rows.
const GAP: f32 = 8.0;

/// A line of the list that only tells something: it takes no tap and shows no pressed layer.
fn note(text: &str) -> ListItem {
    let item = ListItem::new(ListItemKind::Plain, text, "").lines(ListItemLines::One).size(ListItemSize::Column).id(format!("note/{text}")).passive();
    let pitch = item.height() + GAP;
    item.pitch(pitch)
}

/// The headline of an event: its title, or Google's "(No title)".
fn headline(event: &AgendaEvent) -> String {
    Some(plain(&event.title)).filter(|t| !t.is_empty()).unwrap_or_else(|| "(No title)".to_string())
}

fn row(event: &AgendaEvent, day: Date, tz: &TimeZone, hour24: bool, headline_lines: &dyn Fn(&str) -> usize) -> ListItem {
    let (lead, lead_sub) = leading(event, day, tz, hour24);
    let title = headline(event);
    let lines = ListItemLines::with_headline(headline_lines(&title));
    let item = ListItem::new(ListItemKind::LeadingText, title, plain(&event.location)).leading_text(lead, lead_sub).lines(lines).size(ListItemSize::Column);
    // known by its event across a new agenda (a daily "Stand-up" repeats its title)
    let item = if event.id.is_empty() { item } else { item.id(format!("event/{}", event.id)) };
    let item = match accent(event.color) {
        Some(rgb) => item.accent(rgb),
        None => item,
    };
    let pitch = item.height() + GAP;
    item.pitch(pitch)
}

/// The two lines at the start of an event's row on `day`: its start over its length; "Until"
/// over the end for one that began on an earlier day, "Until Wed" when it ends on another day (on
/// the first line: the second keeps the whole time of a 12-hour clock in the leading column, 80
/// wide); "All day" over "Day 2 of 3" for an all-day event of more days.
fn leading(event: &AgendaEvent, day: Date, tz: &TimeZone, hour24: bool) -> (String, String) {
    if event.all_day {
        let days = all_day_days(event);
        if days <= 1 {
            return ("All day".to_string(), String::new());
        }
        let n = utc_date(event.start).until(day).map_or(0, |span| span.get_days() as i64) + 1;
        return ("All day".to_string(), format!("Day {} of {days}", n.clamp(1, days)));
    }
    if first_day(event, tz) < day {
        let end = zoned(event.end, tz);
        let until = if end.date() == day { "Until".to_string() } else { format!("Until {}", weekday_short(end.date())) };
        return (until, time_text(&end, hour24));
    }
    (time_text(&zoned(event.start, tz), hour24), duration_text(event.end.saturating_sub(event.start)))
}

/// The days an all-day event takes (its end is the day after its last).
fn all_day_days(event: &AgendaEvent) -> i64 {
    (event.end.saturating_sub(event.start) / 86_400).max(1)
}

/// What the event's panel shows: the whole title, the day and the time, the place, the answers
/// to its invitation when the panel may give one, the meeting link as text (the panel opens
/// nothing) and the description, each plain text; a private event comes from the bundle as "Busy"
/// without the rest.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Details {
    pub title: String,
    pub when: String,
    pub place: String,
    /// Accept, Maybe and Decline show under the place (AgendaEvent.can_respond).
    pub respond: bool,
    pub link: String,
    pub description: String,
}

/// The details of `event` as the panel shows them on `today`.
pub fn details(event: &AgendaEvent, today: Date, tz: &TimeZone, hour24: bool) -> Details {
    Details {
        title: headline(event),
        when: when_text(event, today, tz, hour24),
        place: plain(&event.location),
        respond: event.can_respond,
        link: meeting_link(event).unwrap_or_default(),
        description: plain_lines(&event.description),
    }
}

/// The event's meeting link as plain text: https and no space, else none.
fn meeting_link(event: &AgendaEvent) -> Option<String> {
    let link = plain(&event.meeting_url);
    plain_https(&link).then_some(link)
}

/// Whether `uri` is an https link that reads as it opens: only the characters of RFC 3986 (no
/// backslash, which a browser takes for a `/`, no space, no control character) and no user part
/// (a meeting link has none, and `user@host` shows another host than it opens). The calendar
/// bundle keeps the same rule; the panel checks it again on what it shows.
pub fn plain_https(uri: &str) -> bool {
    let Some(rest) = uri.strip_prefix("https://") else { return false };
    let allowed = |c: char| c.is_ascii_alphanumeric() || "-._~:/?#[]@!$&'()*+,;=%".contains(c);
    let authority = rest.split(['/', '?', '#']).next().unwrap_or("");
    !authority.is_empty() && !authority.contains('@') && uri.chars().all(allowed)
}

/// What the panel of an event offers besides its details: a QR code of its meeting ("Join on
/// phone") and of a Google Maps search for its place ("Directions"), the answers to its
/// invitation and its deletion. What the account may change is the calendar bundle's word
/// (AgendaEvent.can_respond, can_delete); the bundle checks it again on every change.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Offers {
    /// Google's id of the event, for a change.
    pub id: String,
    /// The meeting link the phone opens.
    pub join: Option<String>,
    /// The Maps search of the place the phone opens.
    pub directions: Option<String>,
    /// The panel may answer the invitation: the account's answer now (RSVP_NEEDS_ACTION while
    /// it has not answered).
    pub respond: Option<Rsvp>,
    /// The panel may delete the event.
    pub delete: bool,
    /// One occurrence of a recurring event: a deletion deletes this occurrence only.
    pub recurring: bool,
}

/// What `event`'s panel offers.
pub fn offers(event: &AgendaEvent) -> Offers {
    let place = plain(&event.location);
    Offers {
        id: event.id.clone(),
        join: meeting_link(event),
        directions: (!place.is_empty()).then(|| maps_url(&place)),
        respond: event.can_respond.then(|| event.response.enum_value_or_default()),
        delete: event.can_delete,
        recurring: event.recurring,
    }
}

/// The host a link opens, as the QR step shows it under the code ("meet.google.com"): a link
/// that reads as it opens only (`plain_https`: no backslash, no user part), the port left out,
/// and only a host of ASCII letters, digits, dots and dashes (a name that could pass for another
/// in other letters gets no code). None: no code for it.
pub fn link_host(url: &str) -> Option<String> {
    if !plain_https(url) {
        return None;
    }
    let rest = url.strip_prefix("https://")?;
    let host = rest.split(['/', '?', '#']).next()?;
    let host = match host.rsplit_once(':') {
        Some((name, port)) if !port.is_empty() && port.bytes().all(|b| b.is_ascii_digit()) => name,
        _ => host,
    };
    let plain = !host.is_empty() && host.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'.' || b == b'-');
    plain.then(|| host.to_ascii_lowercase())
}

/// Google Maps' search of `place` (its URL API, `api=1`): the place URL-encoded into the query,
/// so nothing in it reaches another parameter or the path.
pub fn maps_url(place: &str) -> String {
    format!("https://www.google.com/maps/search/?api=1&query={}", url_encoded(place))
}

/// `text` percent-encoded byte by byte (RFC 3986): the unreserved characters stay, every other
/// byte of its UTF-8 is %XX.
fn url_encoded(text: &str) -> String {
    let mut out = String::with_capacity(text.len() * 3);
    for b in text.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~') {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

/// The day and the time of an event: "Tue, Nov 17 · 08:30 to 09:00", "Tue, Nov 17 · 22:00 to
/// Wed, Nov 18 · 06:00", "Tue, Nov 17 · All day", "Tue, Nov 17 to Thu, Nov 19 · All day · Day 2
/// of 3" (the day of today while it runs).
pub fn when_text(event: &AgendaEvent, today: Date, tz: &TimeZone, hour24: bool) -> String {
    if event.all_day {
        let (first, days) = (utc_date(event.start), all_day_days(event));
        if days <= 1 {
            return format!("{} · All day", day_text(first));
        }
        let last = utc_date(event.end.saturating_sub(86_400));
        let n = first.until(today).map_or(-1, |span| span.get_days() as i64) + 1;
        let running = if (1..=days).contains(&n) { format!(" · Day {n} of {days}") } else { String::new() };
        return format!("{} to {} · All day{running}", day_text(first), day_text(last));
    }
    let (start, end) = (zoned(event.start, tz), zoned(event.end.max(event.start), tz));
    let from = format!("{} · {}", day_text(start.date()), time_text(&start, hour24));
    if event.end <= event.start {
        from
    } else if end.date() == start.date() {
        format!("{from} to {}", time_text(&end, hour24))
    } else {
        format!("{from} to {} · {}", day_text(end.date()), time_text(&end, hour24))
    }
}

/// "Tue, Nov 17".
fn day_text(day: Date) -> String {
    format!("{}, {}", weekday_short(day), month_day(day))
}

/// "Nov 17".
fn month_day(day: Date) -> String {
    let month = MONTHS[(day.month() - 1) as usize];
    format!("{}{} {}", &month[..1], month[1..].to_lowercase(), day.day())
}

/// "Tue".
fn weekday_short(day: Date) -> String {
    let name = WEEKDAYS[day.weekday().to_monday_zero_offset() as usize];
    format!("{}{}", &name[..1], name[1..3].to_lowercase())
}

/// Whether the event is over at `now`: a timed one at its end, an all-day one after its last
/// day (its dates are the calendar's, not the zone's).
fn ended(event: &AgendaEvent, now: Timestamp, today: Date) -> bool {
    if event.all_day {
        return utc_date(event.end.max(event.start.saturating_add(86_400))) <= today;
    }
    if event.end > event.start { event.end <= now.as_second() } else { event.start < now.as_second() }
}

/// The local day an event starts on; an all-day event's date is the same in every zone.
fn first_day(event: &AgendaEvent, tz: &TimeZone) -> Date {
    if event.all_day { utc_date(event.start) } else { zoned(event.start, tz).date() }
}

/// The time `secs` in `tz`; a time out of jiff's range is the epoch (the bundle sends none).
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

/// At most this many combining marks on one letter (more stack over the rows around it).
const MARKS_PER_LETTER: usize = 2;

/// A character that has no glyph of its own and changes or hides the text around it: format
/// characters (Unicode category Cf: direction marks and overrides, zero-width spaces, the soft
/// hyphen, the byte order mark, tags), except the zero-width joiner and non-joiner that some
/// scripts need. (A list of the ranges, the app has no Unicode tables.)
fn invisible(c: char) -> bool {
    matches!(c,
        '\u{00ad}' | '\u{0600}'..='\u{0605}' | '\u{061c}' | '\u{06dd}' | '\u{070f}' | '\u{08e2}' | '\u{180e}'
        | '\u{200b}' | '\u{200e}' | '\u{200f}' | '\u{202a}'..='\u{202e}' | '\u{2060}'..='\u{2064}' | '\u{2066}'..='\u{206f}'
        | '\u{feff}' | '\u{fff9}'..='\u{fffb}' | '\u{110bd}' | '\u{110cd}' | '\u{13430}'..='\u{1343f}'
        | '\u{1bca0}'..='\u{1bca3}' | '\u{1d173}'..='\u{1d17a}' | '\u{e0001}' | '\u{e0020}'..='\u{e007f}')
}

/// A combining mark (the blocks of combining diacritics; a mark of another block counts as a
/// letter, so a script that needs its marks keeps them).
fn combining(c: char) -> bool {
    matches!(c, '\u{0300}'..='\u{036f}' | '\u{1ab0}'..='\u{1aff}' | '\u{1dc0}'..='\u{1dff}' | '\u{20d0}'..='\u{20ff}' | '\u{fe20}'..='\u{fe2f}')
}

/// A text from the calendar as one plain line: control characters, line breaks and spaces are
/// one space, invisible characters are gone (`invisible`), a letter keeps at most two combining
/// marks.
pub fn plain(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let (mut marks, mut space) = (0, false);
    for c in text.chars() {
        if invisible(c) {
            continue;
        }
        if c.is_control() || c.is_whitespace() {
            (space, marks) = (!out.is_empty(), 0);
            continue;
        }
        if combining(c) {
            marks += 1;
            if marks > MARKS_PER_LETTER {
                continue;
            }
        } else {
            marks = 0;
        }
        if space {
            out.push(' ');
            space = false;
        }
        out.push(c);
    }
    out
}

/// A text of several lines from the calendar (a description): each line plain, no more than one
/// empty line in a row, none at the start or the end.
pub fn plain_lines(text: &str) -> String {
    let mut lines: Vec<String> = Vec::new();
    for line in text.lines().map(plain) {
        if line.is_empty() && lines.last().is_none_or(|l| l.is_empty()) {
            continue;
        }
        lines.push(line);
    }
    while lines.last().is_some_and(|l| l.is_empty()) {
        lines.pop();
    }
    lines.join("\n")
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
        e.id = format!("{title}@{start}");
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

    /// A linked agenda, read at 08:00 UTC on 10 November.
    fn agenda(events: Vec<AgendaEvent>) -> Agenda {
        let mut a = Agenda::new();
        a.link = CalendarLink::CALENDAR_CONNECTED.into();
        a.calendar = "someone@example.com".to_string();
        a.events = events;
        a.updated = at("2026-11-10T08:00:00Z").as_second();
        a
    }

    /// One line a headline.
    fn one(_: &str) -> usize {
        1
    }

    fn rows_of(a: &Agenda, now: Timestamp, tz: &TimeZone, hour24: bool) -> Vec<ListItem> {
        rows(a, now, tz, hour24, &one).items
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
        let r = rows_of(&a, now, &tz, false);
        assert_eq!(
            shown(&r),
            [
                "TODAY|NOV 10", "8:30 AM|30 min|Stand-up|Room 2", "12:00 PM|1h 30|Lunch|",
                "TOMORROW|NOV 11", "9:00 AM|1h|Dentist|",
                "THURSDAY|NOV 12", "7:00 PM|2h|Cinema|",
            ]
        );
        assert_eq!((r[0].pitch, r[1].pitch), (Some(SUBHEADER_PITCH), Some(ROW_PITCH)));
        assert_eq!(r[1].variant(), "state=default, kind=leading-text, lines=two, size=column");
        assert_eq!(r[0].variant(), "state=default, kind=subheader, lines=one, size=column");
        assert_eq!((r[1].accent, r[2].accent), (Some(0x039be5), None), "no colour: the default accent");
        // the same agenda on a 24-hour clock
        let r = rows_of(&a, now, &tz, true);
        assert_eq!((r[1].leading_title.as_str(), r[6].leading_title.as_str()), ("08:30", "19:00"));
    }

    #[test]
    fn each_row_knows_its_event_and_a_subheader_none() {
        let a = agenda(vec![event("A", "2026-11-11T08:00:00Z", 60), event("B", "2026-11-10T10:00:00Z", 60)]);
        let r = rows(&a, at("2026-11-10T09:00:00Z"), &TimeZone::UTC, true, &one);
        // TODAY, B (index 1), TOMORROW, A (index 0)
        assert_eq!(r.events, [None, Some(1), None, Some(0)]);
        assert_eq!(r.items.len(), r.events.len());
        let free = rows(&agenda(vec![]), at("2026-11-10T09:00:00Z"), &TimeZone::UTC, true, &one);
        assert_eq!(free.events, [None, None], "the subheader and the free day's line show no event");
    }

    #[test]
    fn a_long_title_takes_more_lines_and_the_row_more_of_the_list() {
        let long = "A title long enough to take three lines in the row";
        let a = agenda(vec![event(long, "2026-11-10T10:00:00Z", 60), event("Short", "2026-11-10T11:00:00Z", 60)]);
        let lines = |t: &str| if t.len() > 40 { 3 } else { 1 };
        let r = rows(&a, at("2026-11-10T09:00:00Z"), &TimeZone::UTC, true, &lines).items;
        assert_eq!(r[1].variant(), "state=default, kind=leading-text, lines=four, size=column");
        assert_eq!((r[1].height(), r[1].pitch), (116.0, Some(116.0 + 8.0)));
        assert_eq!((r[2].lines, r[2].pitch), (ListItemLines::Two, Some(ROW_PITCH)));
        assert_eq!(r[1].title, long, "the whole title");
    }

    #[test]
    fn midnight_is_the_zone_s() {
        // 23:30 UTC on the 10th: still the 10th in New York, the 11th in Berlin
        let now = at("2026-11-10T12:00:00Z");
        let a = agenda(vec![event("Late call", "2026-11-10T23:30:00Z", 45)]);
        let berlin = shown(&rows_of(&a, now, &TimeZone::posix(CET).unwrap(), true));
        assert_eq!(berlin, ["TODAY|NOV 10", NOTHING_TODAY, "TOMORROW|NOV 11", "00:30|45 min|Late call|"]);
        let new_york = shown(&rows_of(&a, now, &TimeZone::posix(NEW_YORK).unwrap(), false));
        assert_eq!(new_york, ["TODAY|NOV 10", "6:30 PM|45 min|Late call|"]);
        assert_eq!(shown(&rows_of(&a, now, &TimeZone::UTC, false))[1], "11:30 PM|45 min|Late call|");
        // today is the zone's too: at midnight UTC it is the 11th in Berlin and in UTC, still the
        // 10th in New York
        let late = at("2026-11-11T00:00:00Z");
        assert_eq!(shown(&rows_of(&agenda(vec![]), late, &TimeZone::posix(CET).unwrap(), true))[0], "TODAY|NOV 11");
        assert_eq!(shown(&rows_of(&agenda(vec![]), late, &TimeZone::UTC, true))[0], "TODAY|NOV 11");
        assert_eq!(shown(&rows_of(&agenda(vec![]), late, &TimeZone::posix(NEW_YORK).unwrap(), true))[0], "TODAY|NOV 10");
    }

    #[test]
    fn an_event_across_midnight_starts_on_its_day_and_runs_until_its_end_on_the_next() {
        let tz = TimeZone::posix(CET).unwrap();
        // 23:00 to 01:00 in Berlin
        let a = agenda(vec![event("Party", "2026-11-10T22:00:00Z", 120)]);
        assert_eq!(shown(&rows_of(&a, at("2026-11-10T20:00:00Z"), &tz, true))[1], "23:00|2h|Party|");
        // at 00:30 it is the 11th: today's, until 01:00
        assert_eq!(shown(&rows_of(&a, at("2026-11-10T23:30:00Z"), &tz, true)), ["TODAY|NOV 11", "Until|01:00|Party|"]);
        // a night shift from 22:00 to 10:00 sits above the morning's events, until its end
        let b = agenda(vec![event("Night shift", "2026-11-09T21:00:00Z", 12 * 60), event("Meeting", "2026-11-10T07:30:00Z", 30)]);
        assert_eq!(shown(&rows_of(&b, at("2026-11-10T07:00:00Z"), &tz, false))[1..], ["Until|10:00 AM|Night shift|", "8:30 AM|30 min|Meeting|"]);
        // one that ends on a later day says which, on the first line: the second keeps the whole
        // time, AM or PM too
        let c = agenda(vec![event("Trip", "2026-11-09T08:00:00Z", 3 * 1440 + 270)]);
        assert_eq!(shown(&rows_of(&c, at("2026-11-10T08:00:00Z"), &TimeZone::UTC, true))[1], "Until Thu|12:30|Trip|");
        assert_eq!(shown(&rows_of(&c, at("2026-11-10T08:00:00Z"), &TimeZone::UTC, false))[1], "Until Thu|12:30 PM|Trip|");
    }

    #[test]
    fn a_day_of_daylight_saving_time() {
        let tz = TimeZone::posix(CET).unwrap();
        // Sunday 25 October 2026: 03:00 summer time is 02:00 winter time (01:00 UTC)
        let now = at("2026-10-25T08:00:00Z");
        // 02:30 twice: before the change (00:30 UTC) and after it (01:30 UTC); two hours that
        // are three on the wall clock; an event in the evening
        let a = agenda(vec![event("First", "2026-10-25T00:30:00Z", 0), event("Second", "2026-10-25T01:30:00Z", 0), event("Night", "2026-10-24T23:00:00Z", 120), event("Evening", "2026-10-25T18:00:00Z", 60)]);
        let r = shown(&rows_of(&a, at("2026-10-24T22:30:00Z"), &tz, true));
        assert_eq!(r, ["TODAY|OCT 25", "01:00|2h|Night|", "02:30||First|", "02:30||Second|", "19:00|1h|Evening|"]);
        // today across the change is still the 25th
        assert_eq!(shown(&rows_of(&a, now, &tz, true))[..2], ["TODAY|OCT 25", "19:00|1h|Evening|"]);
        assert_eq!(shown(&rows_of(&a, at("2026-10-25T22:59:59Z"), &tz, true))[0], "TODAY|OCT 25");
        assert_eq!(shown(&rows_of(&a, at("2026-10-25T23:00:00Z"), &tz, true))[0], "TODAY|OCT 26");
    }

    #[test]
    fn an_all_day_event_keeps_its_date_in_every_zone() {
        // 20:00 on the 10th in New York is already the 11th in UTC: the birthday on the 11th is
        // tomorrow there, today in Berlin (02:00 on the 11th)
        let now = at("2026-11-11T01:00:00Z");
        let a = agenda(vec![all_day("Birthday", "2026-11-11", 1), all_day("Trip", "2026-11-12", 3), event("Breakfast", "2026-11-11T07:00:00Z", 30)]);
        let ny = shown(&rows_of(&a, now, &TimeZone::posix(NEW_YORK).unwrap(), false));
        assert_eq!(ny, [
            "TODAY|NOV 10", NOTHING_TODAY,
            "TOMORROW|NOV 11", "All day||Birthday|", "2:00 AM|30 min|Breakfast|",
            "THURSDAY|NOV 12", "All day|Day 1 of 3|Trip|",
        ]);
        let berlin = shown(&rows_of(&a, now, &TimeZone::posix(CET).unwrap(), true));
        assert_eq!(&berlin[..3], ["TODAY|NOV 11", "All day||Birthday|", "08:00|30 min|Breakfast|"]);
        // in the middle of its run and on its last day the trip is today's, with its day; the day
        // after, it is over
        assert_eq!(shown(&rows_of(&a, at("2026-11-13T10:00:00Z"), &TimeZone::UTC, true))[1], "All day|Day 2 of 3|Trip|");
        assert_eq!(shown(&rows_of(&a, at("2026-11-14T10:00:00Z"), &TimeZone::UTC, true))[1], "All day|Day 3 of 3|Trip|");
        assert_eq!(shown(&rows_of(&a, at("2026-11-15T10:00:00Z"), &TimeZone::UTC, true))[1], NOTHING_TODAY);
    }

    #[test]
    fn an_ended_event_goes_and_equal_starts_keep_google_s_order() {
        let now = at("2026-11-10T09:00:00Z");
        let a = agenda(vec![event("Over", "2026-11-10T07:00:00Z", 60), event("Zulu", "2026-11-10T10:00:00Z", 30), event("Alpha", "2026-11-10T10:00:00Z", 60), event("Reminder", "2026-11-10T10:00:00Z", 0)]);
        assert_eq!(shown(&rows_of(&a, now, &TimeZone::UTC, true)), ["TODAY|NOV 10", "10:00|30 min|Zulu|", "10:00|1h|Alpha|", "10:00||Reminder|"]);
    }

    #[test]
    fn an_empty_today_says_so_and_the_week_follows() {
        let now = at("2026-11-10T09:00:00Z");
        let r = rows_of(&agenda(vec![event("Yoga", "2026-11-13T17:00:00Z", 60)]), now, &TimeZone::UTC, true);
        assert_eq!(shown(&r), ["TODAY|NOV 10", NOTHING_TODAY, "FRIDAY|NOV 13", "17:00|1h|Yoga|"]);
        assert_eq!(r[1].variant(), "state=default, kind=plain, lines=one, size=column");
        assert!(!r[1].takes_taps(), "the free day's line takes no tap and shows no press");
        assert!(r[3].takes_taps());
        assert_eq!(shown(&rows_of(&agenda(vec![]), now, &TimeZone::UTC, true)), ["TODAY|NOV 10", NOTHING_TODAY]);
    }

    #[test]
    fn an_agenda_not_read_yet_shows_no_day() {
        let now = at("2026-11-10T09:00:00Z");
        // linked, not read yet: the events are unknown, not none
        let mut a = agenda(vec![]);
        a.updated = 0;
        let r = rows(&a, now, &TimeZone::UTC, true, &one);
        assert_eq!((shown(&r.items), r.events), (vec![READING.to_string()], vec![None]));
        assert!(!r.items[0].takes_taps());
        // the reads fail: it says so, still no day
        a.read_failed = true;
        assert_eq!(shown(&rows_of(&a, now, &TimeZone::UTC, true)), [NOT_READ]);
        assert!(!rows_of(&a, now, &TimeZone::UTC, true)[0].takes_taps());
        // read once, the reads fail after it: the days as they were read
        a.updated = now.as_second() - 600;
        assert_eq!(shown(&rows_of(&a, now, &TimeZone::UTC, true)), ["TODAY|NOV 10", NOTHING_TODAY]);
    }

    #[test]
    fn rows_are_known_by_their_event_and_their_day() {
        // a daily stand-up: the same title today and tomorrow, two events
        let mut today = event("Stand-up", "2026-11-10T09:00:00Z", 15);
        let mut tomorrow = event("Stand-up", "2026-11-11T09:00:00Z", 15);
        (today.id, tomorrow.id) = ("su_20261110".to_string(), "su_20261111".to_string());
        let r = rows_of(&agenda(vec![today, tomorrow]), at("2026-11-10T08:00:00Z"), &TimeZone::UTC, true);
        let ids: Vec<Option<&str>> = r.iter().map(|row| row.id.as_deref()).collect();
        assert_eq!(ids, [Some("day/2026-11-10"), Some("event/su_20261110"), Some("day/2026-11-11"), Some("event/su_20261111")]);
        assert_eq!((r[1].title.as_str(), r[3].title.as_str()), ("Stand-up", "Stand-up"));
        // an event without an id is known by its title
        let mut no_id = event("Dentist", "2026-11-10T10:00:00Z", 30);
        no_id.id.clear();
        assert_eq!(rows_of(&agenda(vec![no_id]), at("2026-11-10T08:00:00Z"), &TimeZone::UTC, true)[1].id, None);
    }

    #[test]
    fn a_private_event_shows_busy_and_no_place() {
        // the bundle sends "Busy" and no place, description or link for a private event
        let busy = event("Busy", "2026-11-10T10:00:00Z", 60);
        let r = rows_of(&agenda(vec![busy.clone()]), at("2026-11-10T09:00:00Z"), &TimeZone::UTC, true);
        assert_eq!(shown(&r)[1], "10:00|1h|Busy|");
        let d = details(&busy, at("2026-11-10T09:00:00Z").to_zoned(TimeZone::UTC).date(), &TimeZone::UTC, true);
        assert_eq!(d, Details { title: "Busy".into(), when: "Tue, Nov 10 · 10:00 to 11:00".into(), ..Default::default() });
        let r = rows_of(&agenda(vec![event("", "2026-11-10T10:00:00Z", 60)]), at("2026-11-10T09:00:00Z"), &TimeZone::UTC, true);
        assert_eq!(r[1].title, "(No title)");
    }

    #[test]
    fn the_calendar_is_never_shown() {
        // the agenda's calendar is the account's address here (agenda()): no row carries it
        let a = agenda(vec![event("Dentist", "2026-11-10T10:00:00Z", 60), all_day("Trip", "2026-11-11", 2)]);
        let r = rows_of(&a, at("2026-11-10T09:00:00Z"), &TimeZone::UTC, true);
        assert!(r.iter().all(|row| !format!("{row:?}").contains(&a.calendar)), "{r:?}");
        assert_eq!(shown(&r)[1], "10:00|1h|Dentist|");
    }

    #[test]
    fn the_details_of_an_event() {
        let tz = TimeZone::posix(CET).unwrap();
        let today = at("2026-11-17T06:00:00Z").to_zoned(tz.clone()).date();
        let mut e = event("Dentist", "2026-11-17T07:30:00Z", 30);
        e.location = "Main Street 12\nBerlin".to_string();
        e.description = "Bring the card.\n\n\n\nFloor 2, room 4\u{202e}.\n".to_string();
        e.meeting_url = "https://meet.google.com/abc-defg-hij".to_string();
        let d = details(&e, today, &tz, true);
        assert_eq!(d.when, "Tue, Nov 17 · 08:30 to 09:00");
        assert_eq!(d.place, "Main Street 12 Berlin");
        assert_eq!(d.description, "Bring the card.\n\nFloor 2, room 4.");
        assert_eq!(d.link, "https://meet.google.com/abc-defg-hij");
        // only an https link, as text
        e.meeting_url = "javascript:alert(1)".to_string();
        assert_eq!(details(&e, today, &tz, true).link, "");
        assert_eq!(details(&e, today, &tz, false).when, "Tue, Nov 17 · 8:30 AM to 9:00 AM");
    }

    #[test]
    fn what_an_event_offers() {
        let mut e = event("Dentist", "2026-11-17T07:30:00Z", 30);
        e.id = "abc_20261117T073000Z".to_string();
        e.location = "Main St 12 & 14, Zürich #2\n?x=1".to_string();
        e.meeting_url = "https://zoom.example/j/1?pwd=x".to_string();
        let o = offers(&e);
        assert_eq!(o.id, "abc_20261117T073000Z");
        assert_eq!(o.join.as_deref(), Some("https://zoom.example/j/1?pwd=x"));
        // the place as one plain line, every byte that is not unreserved encoded
        assert_eq!(o.directions.as_deref(), Some("https://www.google.com/maps/search/?api=1&query=Main%20St%2012%20%26%2014%2C%20Z%C3%BCrich%20%232%20%3Fx%3D1"));
        // nothing to change unless the bundle says so
        assert_eq!((o.respond, o.delete, o.recurring), (None, false, false));
        e.can_respond = true;
        e.response = Rsvp::RSVP_TENTATIVE.into();
        e.recurring = true;
        let o = offers(&e);
        assert_eq!((o.respond, o.recurring), (Some(Rsvp::RSVP_TENTATIVE), true));
        e.can_delete = true;
        assert!(offers(&e).delete);
        // no place, no https link: no code
        e.location = " \t".to_string();
        e.meeting_url = "http://insecure.example/j".to_string();
        let o = offers(&e);
        assert_eq!((o.join, o.directions), (None, None));
        // the details say whether the answers show
        let tz = TimeZone::UTC;
        assert!(details(&e, at("2026-11-17T06:00:00Z").to_zoned(tz.clone()).date(), &tz, true).respond);
    }

    #[test]
    fn the_host_a_link_opens() {
        assert_eq!(link_host("https://meet.google.com/abc-defg-hij").as_deref(), Some("meet.google.com"));
        assert_eq!(link_host("https://Zoom.US:443/j/1?pwd=x#a").as_deref(), Some("zoom.us"));
        // a user part or a backslash would show another host than the phone opens: no code
        assert_eq!(link_host("https://meet.google.com@evil.example/x"), None);
        assert_eq!(link_host("https://evil.example\\@meet.google.com/x"), None);
        assert_eq!(link_host("https://evil.example\\meet.google.com"), None);
        assert_eq!(link_host("https://user:pw@meet.google.com/x"), None);
        assert_eq!(link_host("https://meet.google.com/a<b"), None);
        assert_eq!(link_host("https://www.google.com/maps/search/?api=1&query=x").as_deref(), Some("www.google.com"));
        // not https, no host, a host in other letters, an address in brackets: no code
        for bad in ["http://meet.google.com/x", "javascript:alert(1)", "https://", "https:///x", "https://gооgle.com/x", "https://[::1]/x", "https://a b/x"] {
            assert_eq!(link_host(bad), None, "{bad}");
        }
    }

    #[test]
    fn a_meeting_link_that_reads_as_another_is_not_offered() {
        let mut e = event("Sync", "2026-11-17T07:30:00Z", 30);
        for bad in ["https://evil.example\\@meet.google.com/x", "https://meet.google.com@evil.example/x"] {
            e.meeting_url = bad.to_string();
            let o = offers(&e);
            assert_eq!(o.join, None, "{bad}");
            let tz = TimeZone::UTC;
            assert_eq!(details(&e, at("2026-11-17T06:00:00Z").to_zoned(tz.clone()).date(), &tz, true).link, "", "{bad}: not even as text");
        }
        e.meeting_url = "https://meet.google.com/abc-defg-hij".to_string();
        assert_eq!(offers(&e).join.as_deref(), Some("https://meet.google.com/abc-defg-hij"));
    }

    #[test]
    fn the_time_range_texts() {
        let tz = TimeZone::posix(CET).unwrap();
        let day = |s: &str| at(s).to_zoned(tz.clone()).date();
        let today = day("2026-11-17T12:00:00Z");
        assert_eq!(when_text(&event("Night", "2026-11-17T21:00:00Z", 9 * 60), today, &tz, true), "Tue, Nov 17 · 22:00 to Wed, Nov 18 · 07:00");
        assert_eq!(when_text(&event("Reminder", "2026-11-17T09:00:00Z", 0), today, &tz, true), "Tue, Nov 17 · 10:00");
        assert_eq!(when_text(&all_day("Holiday", "2026-11-17", 1), today, &tz, true), "Tue, Nov 17 · All day");
        let trip = all_day("Trip", "2026-11-16", 3);
        assert_eq!(when_text(&trip, today, &tz, true), "Mon, Nov 16 to Wed, Nov 18 · All day · Day 2 of 3");
        assert_eq!(when_text(&trip, day("2026-11-25T12:00:00Z"), &tz, true), "Mon, Nov 16 to Wed, Nov 18 · All day");
    }

    #[test]
    fn a_time_out_of_range_does_not_panic() {
        let mut e = event("Broken", "2026-11-10T10:00:00Z", 60);
        (e.start, e.end) = (i64::MIN, i64::MAX);
        let a = agenda(vec![e.clone(), all_day("Odd", "2026-11-10", 1)]);
        let now = at("2026-11-10T09:00:00Z");
        let _ = rows_of(&a, now, &TimeZone::UTC, true);
        e.all_day = true;
        let _ = rows_of(&agenda(vec![e.clone()]), now, &TimeZone::UTC, true);
        let _ = details(&e, now.to_zoned(TimeZone::UTC).date(), &TimeZone::UTC, true);
        (e.start, e.end) = (i64::MAX, i64::MIN);
        let _ = rows_of(&agenda(vec![e.clone()]), now, &TimeZone::UTC, true);
        e.all_day = false;
        let _ = details(&e, now.to_zoned(TimeZone::UTC).date(), &TimeZone::UTC, false);
        assert_eq!(duration_text(i64::MIN), "");
    }

    #[test]
    fn texts_from_the_calendar_are_one_plain_line() {
        assert_eq!(plain("Main Street 12\nBerlin"), "Main Street 12 Berlin");
        assert_eq!(plain("  a\t\tb \u{202e}gnp.exe\u{202c} "), "a b gnp.exe");
        assert_eq!(plain("\u{0007}"), "");
        // invisible characters go, the joiners some scripts need stay
        assert_eq!(plain("pay\u{200b}pal\u{2060}\u{feff}\u{00ad}\u{061c}\u{fff9}\u{e0041}"), "paypal");
        assert_eq!(plain("\u{0915}\u{094d}\u{200d}\u{0937}"), "\u{0915}\u{094d}\u{200d}\u{0937}");
        // at most two combining marks on a letter
        assert_eq!(plain("Z\u{0301}\u{0302}\u{0303}\u{0304}a\u{0301}"), "Z\u{0301}\u{0302}a\u{0301}");
        assert_eq!(plain_lines("\n\nOne\n\n\n\nTwo\u{0007}\n\n"), "One\n\nTwo");
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
        // linked and not read yet: no day, the line that it is being read
        assert_eq!(shown(&rows_of(&a, at("2026-11-10T09:00:00Z"), &TimeZone::UTC, true)), [READING], "a linked agenda not read yet");
        a.updated = at("2026-11-10T08:59:00Z").as_second();
        assert_eq!(shown(&rows_of(&a, at("2026-11-10T09:00:00Z"), &TimeZone::UTC, true)), ["TODAY|NOV 10", NOTHING_TODAY], "read: its days");
        // no address yet: no code to scan
        a.link = CalendarLink::CALENDAR_NOT_CONNECTED.into();
        a.setup_url.clear();
        assert_eq!(card(&a).map(|c| c.qr), Some(None));
        assert!(rows_of(&a, at("2026-11-10T09:00:00Z"), &TimeZone::UTC, true).is_empty(), "no rows behind the card");
        // a state this panel does not know, and the time before the first agenda
        a.link = protobuf::EnumOrUnknown::from_i32(9);
        assert_eq!(card(&a).map(|c| c.title), Some("Calendar not set up".to_string()));
        assert_eq!(link_card(None, true).map(|c| c.hint), Some("from the calendar bundle".to_string()));
        assert_eq!(link_card(None, false).map(|c| c.hint), Some("no link to the house's data".to_string()));
    }

    #[test]
    fn no_code_without_the_house_link() {
        let mut a = Agenda::new();
        a.link = CalendarLink::CALENDAR_NOT_CONNECTED.into();
        a.setup_url = "http://192.168.1.20:8765/setup?t=abc".to_string();
        let card = link_card(Some(&a), false).unwrap();
        assert_eq!((card.qr, card.hint.as_str()), (None, "no link to the house's data"));
    }

    #[test]
    fn an_old_agenda_says_how_old() {
        let tz = TimeZone::UTC;
        let now = at("2026-11-10T09:00:00Z");
        let mut a = agenda(vec![]);
        // not read yet: the list says it
        a.updated = 0;
        assert_eq!(status(Some(&a), true, now, &tz, true), "");
        a.updated = now.as_second() - 60;
        assert_eq!(status(Some(&a), true, now, &tz, true), "", "fresh");
        a.updated = now.as_second() - 10 * 60;
        assert_eq!(status(Some(&a), true, now, &tz, true), "Last read 08:50");
        assert_eq!(status(Some(&a), false, now, &tz, false), "Last read 8:50 AM · no link to the house's data");
        // read on another day: the weekday, a week ago and more the date
        a.updated = at("2026-11-09T14:02:00Z").as_second();
        assert_eq!(status(Some(&a), true, now, &tz, true), "Last read Mon 14:02");
        a.updated = at("2026-11-04T14:02:00Z").as_second();
        assert_eq!(status(Some(&a), true, now, &tz, false), "Last read Wed 2:02 PM");
        a.updated = at("2026-11-03T14:02:00Z").as_second();
        assert_eq!(status(Some(&a), true, now, &tz, true), "Last read Nov 3 14:02");
        // the day is the zone's: 23:30 UTC on the 9th is the 10th in Berlin, today there
        a.updated = at("2026-11-09T23:30:00Z").as_second();
        assert_eq!(status(Some(&a), true, now, &TimeZone::posix(CET).unwrap(), true), "Last read 00:30");
        a.updated = 0;
        assert_eq!(status(Some(&a), false, now, &tz, true), "no link to the house's data");
        assert_eq!(status(None, false, now, &tz, true), "", "the card says it");
        a.link = CalendarLink::CALENDAR_NOT_CONNECTED.into();
        assert_eq!(status(Some(&a), false, now, &tz, true), "", "the card says it");
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

    #[test]
    fn the_picture_is_the_code_of_the_url() {
        // read the picture back module by module and compare with the code of the URL made on its
        // own: the picture carries exactly that code (the encoder is qrcodegen's, tested there)
        for url in ["http://192.168.101.23:8765/setup?t=Q2xhdWRlIG1hZGUgdGhpcw", "http://10.0.0.2:8765/setup?t=x"] {
            let rgba = qr_rgba(url, QR_SIDE).unwrap();
            let len = Version::MAX.buffer_len();
            let (mut temp, mut out) = (vec![0u8; len], vec![0u8; len]);
            let qr = QrCode::encode_text(url, &mut temp, &mut out, QrCodeEcc::Medium, Version::MIN, Version::MAX, None, true).unwrap();
            let n = qr.size() as u32;
            let scale = QR_SIDE / (n + 2 * QUIET);
            let offset = (QR_SIDE - n * scale) / 2;
            for y in 0..n {
                for x in 0..n {
                    let (px, py) = (offset + x * scale + scale / 2, offset + y * scale + scale / 2);
                    assert_eq!(black(&rgba, QR_SIDE, px, py), qr.get_module(x as i32, y as i32), "module {x},{y} of {url}");
                }
            }
            // and another URL gives another picture
            assert_ne!(qr_rgba("http://10.0.0.3:8765/setup?t=y", QR_SIDE).unwrap(), rgba);
        }
    }
}
