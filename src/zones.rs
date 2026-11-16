//! The time zones the settings sheet offers (#prefs, the page of the time zone), as Car
//! Settings offers them: the 91 zones of frameworks/base/packages/SettingsLib/res/xml/
//! timezones.xml, sorted by their offset now, then by name (packages/apps/Car/Settings/src/com/
//! android/car/settings/datetime/TimeZonePickerScreenPreferenceController.java). A row shows the
//! place and the offset ("Amsterdam", "GMT+01:00 · Europe"); the offset is read from the
//! device's tzdata with the time it holds until (the next change of daylight saving time of one
//! of the zones), so the list is read again when it opens after such a change. The chosen id
//! goes to the System bundle unchanged. A zone in force that the list lacks (Europe/Berlin) is
//! not selected; the list opens on the first zone with its offset.

use jiff::tz::{TimeZone, TimeZoneDatabase};
use jiff::Timestamp;

/// The zone ids of SettingsLib's timezones.xml, in its order.
const ZONES: [&str; 91] = [
    "Pacific/Midway", "Pacific/Honolulu", "America/Anchorage", "America/Los_Angeles", "America/Tijuana",
    "America/Phoenix", "America/Chihuahua", "America/Denver", "America/Costa_Rica", "America/Chicago",
    "America/Mexico_City", "America/Regina", "America/Bogota", "America/New_York", "America/Caracas",
    "America/Barbados", "America/Halifax", "America/Manaus", "America/Santiago", "America/St_Johns",
    "America/Recife", "America/Sao_Paulo", "America/Argentina/Buenos_Aires", "America/Godthab",
    "America/Montevideo", "Atlantic/South_Georgia", "Atlantic/Azores", "Atlantic/Cape_Verde", "Etc/UTC",
    "Africa/Casablanca", "Europe/London", "Europe/Amsterdam", "Europe/Belgrade", "Europe/Brussels",
    "Europe/Madrid", "Europe/Sarajevo", "Europe/Warsaw", "Africa/Windhoek", "Africa/Brazzaville",
    "Asia/Amman", "Europe/Athens", "Europe/Istanbul", "Asia/Beirut", "Africa/Cairo", "Europe/Helsinki",
    "Asia/Jerusalem", "Europe/Minsk", "Africa/Harare", "Asia/Baghdad", "Europe/Moscow", "Asia/Kuwait",
    "Africa/Nairobi", "Asia/Tehran", "Asia/Baku", "Asia/Tbilisi", "Asia/Yerevan", "Asia/Dubai", "Asia/Kabul",
    "Asia/Karachi", "Asia/Oral", "Asia/Yekaterinburg", "Asia/Kolkata", "Asia/Colombo", "Asia/Kathmandu",
    "Asia/Almaty", "Asia/Yangon", "Asia/Krasnoyarsk", "Asia/Bangkok", "Asia/Jakarta", "Asia/Shanghai",
    "Asia/Hong_Kong", "Asia/Irkutsk", "Asia/Kuala_Lumpur", "Australia/Perth", "Asia/Taipei", "Asia/Seoul",
    "Asia/Tokyo", "Asia/Yakutsk", "Australia/Adelaide", "Australia/Darwin", "Australia/Brisbane",
    "Australia/Hobart", "Australia/Sydney", "Asia/Vladivostok", "Pacific/Guam", "Asia/Magadan",
    "Pacific/Noumea", "Pacific/Majuro", "Pacific/Auckland", "Pacific/Fiji", "Pacific/Tongatapu",
];

/// Where bionic reads the time zone data (the System bundle checks a zone against the same files).
const TZDATA: [&str; 2] = ["/apex/com.android.tzdata/etc/tz/tzdata", "/system/usr/share/zoneinfo/tzdata"];

/// A zone of the list.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Zone {
    /// The IANA id, what the System bundle sets ("Europe/Berlin").
    pub id: &'static str,
    /// The place ("Berlin", "Buenos Aires", "UTC").
    pub place: String,
    /// The offset now and the region ("GMT+01:00 · Europe").
    pub detail: String,
    /// The offset now, in seconds east of UTC.
    pub offset: i32,
}

/// The zones of the list with the time their offsets hold.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Zones {
    /// The zones, sorted as Car Settings sorts them.
    pub list: Vec<Zone>,
    /// When the offsets were read.
    pub read: Timestamp,
    /// The next change of an offset of one of the zones (daylight saving time), if any.
    pub until: Option<Timestamp>,
}

impl Zones {
    /// Whether the offsets are the ones of `now`: not before they were read (the clock was set
    /// back), not at or past the next change.
    pub fn hold_at(&self, now: Timestamp) -> bool {
        now >= self.read && self.until.is_none_or(|until| now < until)
    }
}

/// The zones with their offset now, sorted as Car Settings sorts them; a zone the device's
/// tzdata lacks is left out. Empty when there is no tzdata at all.
pub fn zones() -> Zones {
    let Some(db) = database() else {
        log::warn!("smarthome: no tzdata in {TZDATA:?}: no time zones to choose");
        return Zones::default();
    };
    let now = Timestamp::now();
    let found: Vec<(&'static str, TimeZone)> = ZONES
        .iter()
        .filter_map(|&id| match db.get(id) {
            Ok(tz) => Some((id, tz)),
            Err(e) => {
                log::warn!("smarthome: time zone {id} not in tzdata: {e}");
                None
            }
        })
        .collect();
    let mut list: Vec<Zone> = found.iter().map(|(id, tz)| zone(id, tz.to_offset(now).seconds())).collect();
    list.sort_by(|a, b| a.offset.cmp(&b.offset).then_with(|| a.place.cmp(&b.place)));
    Zones { list, read: now, until: next_change(found.iter().map(|(_, tz)| tz), now) }
}

/// The first transition of any of `zones` after `now`: the offsets of the list hold until then.
fn next_change<'a>(zones: impl Iterator<Item = &'a TimeZone>, now: Timestamp) -> Option<Timestamp> {
    zones.filter_map(|tz| tz.following(now).next()).map(|t| t.timestamp()).min()
}

/// The zone `id` from the device's tzdata; UTC when it has no tzdata or not that zone (a warning
/// in the log).
pub fn time_zone(id: &str) -> TimeZone {
    match database().map(|db| db.get(id)) {
        Some(Ok(tz)) => tz,
        Some(Err(e)) => {
            log::warn!("smarthome: time zone {id} not in tzdata ({e}): UTC");
            TimeZone::UTC
        }
        None => {
            log::warn!("smarthome: no tzdata in {TZDATA:?}: UTC");
            TimeZone::UTC
        }
    }
}

/// The time zone database of the device, if it has one.
fn database() -> Option<TimeZoneDatabase> {
    TZDATA.iter().find_map(|p| TimeZoneDatabase::from_concatenated_path(p).ok())
}

/// Where the list opens for the zone `id` in force: its row, selected; else, for a zone the
/// list lacks, the first row with its offset now, not selected (past every row: the last row);
/// else (no tzdata, or a zone tzdata lacks) the top.
pub fn opening_row(zones: &[Zone], id: &str) -> (Option<usize>, usize) {
    if let Some(row) = zones.iter().position(|z| z.id == id) {
        return (Some(row), row);
    }
    let offset = database().and_then(|db| db.get(id).ok()).map(|tz| tz.to_offset(Timestamp::now()).seconds());
    (None, row_for_offset(zones, offset))
}

/// The row a list opens on for a zone with `offset` now that the list lacks: the first row
/// with it or after it; past every row (Pacific/Kiritimati, +14) the last row; no offset known,
/// the top.
fn row_for_offset(zones: &[Zone], offset: Option<i32>) -> usize {
    offset.map_or(0, |o| first_with(zones, o).unwrap_or(zones.len().saturating_sub(1)))
}

/// The first row whose offset is `offset`, or the first one after it.
fn first_with(zones: &[Zone], offset: i32) -> Option<usize> {
    zones.iter().position(|z| z.offset >= offset)
}

/// A zone with its offset: the place from the id's last part, the region from its first.
fn zone(id: &'static str, offset: i32) -> Zone {
    let place = if id == "Etc/UTC" { "UTC".to_string() } else { id.rsplit('/').next().unwrap_or(id).replace('_', " ") };
    let region = id.split('/').next().unwrap_or(id);
    let (sign, abs) = if offset < 0 { ('-', -offset) } else { ('+', offset) };
    let detail = format!("GMT{sign}{:02}:{:02} · {region}", abs / 3600, abs % 3600 / 60);
    Zone { id, place, detail, offset }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_row_shows_the_place_and_the_offset() {
        assert_eq!(zone("Europe/Amsterdam", 3600).detail, "GMT+01:00 · Europe");
        assert_eq!(zone("Asia/Kolkata", 19800).detail, "GMT+05:30 · Asia");
        assert_eq!(zone("America/St_Johns", -12600).detail, "GMT-03:30 · America");
        assert_eq!(zone("America/Argentina/Buenos_Aires", -10800).place, "Buenos Aires");
        assert_eq!(zone("Etc/UTC", 0).place, "UTC");
    }

    #[test]
    fn a_zone_the_list_lacks_opens_on_its_offset() {
        let zones = [zone("Europe/London", 0), zone("Europe/Amsterdam", 3600), zone("Europe/Athens", 7200)];
        assert_eq!(first_with(&zones, 3600), Some(1));
        assert_eq!(first_with(&zones, 1800), Some(1));
        assert_eq!(first_with(&zones, 10800), None);
        assert_eq!(opening_row(&zones, "Europe/Athens"), (Some(2), 2));
        assert_eq!(row_for_offset(&zones, Some(3600)), 1);
        assert_eq!(row_for_offset(&zones, Some(50400)), 2);
        assert_eq!(row_for_offset(&zones, None), 0);
        assert_eq!(row_for_offset(&[], Some(3600)), 0);
    }

    #[test]
    fn the_offsets_hold_until_the_next_change_of_daylight_saving_time() {
        let cet = TimeZone::posix("CET-1CEST,M3.5.0,M10.5.0/3").unwrap();
        let utc = TimeZone::UTC;
        let now: Timestamp = "2026-10-01T00:00:00Z".parse().unwrap();
        // the last Sunday of October, 03:00 summer time: 01:00 UTC
        let change: Timestamp = "2026-10-25T01:00:00Z".parse().unwrap();
        assert_eq!(next_change([&cet, &utc].into_iter(), now), Some(change));
        assert_eq!(next_change([&utc].into_iter(), now), None);

        let zones = Zones { list: Vec::new(), read: now, until: Some(change) };
        assert!(zones.hold_at(now));
        assert!(zones.hold_at("2026-10-25T00:59:59Z".parse().unwrap()));
        assert!(!zones.hold_at(change));
        assert!(!zones.hold_at("2026-11-16T00:00:00Z".parse().unwrap()));
        // the clock set back (the RTC before the network's time came)
        assert!(!zones.hold_at("2026-09-30T23:59:59Z".parse().unwrap()));
        assert!(Zones { until: None, ..zones }.hold_at("2030-01-01T00:00:00Z".parse().unwrap()));
    }

    #[test]
    fn the_list_is_settingslib_s() {
        let mut ids = ZONES.to_vec();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), 91);
    }
}
