//! The time zones the settings sheet offers (#prefs, step=timezone), as Car Settings offers
//! them: the 91 zones of frameworks/base/packages/SettingsLib/res/xml/timezones.xml, sorted by
//! their offset now, then by name (packages/apps/Car/Settings/src/com/android/car/settings/
//! datetime/TimeZonePickerScreenPreferenceController.java). A row shows the place and the
//! offset ("Amsterdam", "GMT+01:00 · Europe"); the offset is read from the device's tzdata, so
//! it follows daylight saving time. The chosen id goes to the System bundle unchanged. A zone in
//! force that the list lacks (Europe/Berlin) is not selected; the list opens on the first zone
//! with its offset.

use jiff::tz::TimeZoneDatabase;
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

/// The zones with their offset now, sorted as Car Settings sorts them; a zone the device's
/// tzdata lacks is left out. Empty when there is no tzdata at all.
pub fn zones() -> Vec<Zone> {
    let Some(db) = database() else {
        log::warn!("smarthome: no tzdata in {TZDATA:?}: no time zones to choose");
        return Vec::new();
    };
    let now = Timestamp::now();
    let mut zones: Vec<Zone> = ZONES
        .iter()
        .filter_map(|&id| match db.get(id) {
            Ok(tz) => Some(zone(id, tz.to_offset(now).seconds())),
            Err(e) => {
                log::warn!("smarthome: time zone {id} not in tzdata: {e}");
                None
            }
        })
        .collect();
    zones.sort_by(|a, b| a.offset.cmp(&b.offset).then_with(|| a.place.cmp(&b.place)));
    zones
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
    fn the_list_is_settingslib_s() {
        let mut ids = ZONES.to_vec();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), 91);
    }
}
