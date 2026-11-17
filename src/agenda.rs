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
mod tests;
