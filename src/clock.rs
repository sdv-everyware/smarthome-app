//! The wall clock of the panel in local time: the system clock (set by chrony) in the time
//! zone of the system (`persist.sys.timezone`, which bionic's localtime_r reads with the
//! platform's tzdata). Android's chrono build has no clock feature (no chrono::Local).

/// What the screen shows of the time.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Clock {
    /// "FRI, 6 NOV" (#home/date)
    pub date: String,
    /// "10:33" (#home/time) on a 24-hour clock; time_text() for the other
    pub time: String,
    /// The hour (0..23) and the minute, for a 12-hour clock (time_text).
    pub hour: i32,
    pub min: i32,
    /// ":08" (#home/sec, next to the time)
    pub sec: String,
}

const DAYS: [&str; 7] = ["SUN", "MON", "TUE", "WED", "THU", "FRI", "SAT"];
const MONTHS: [&str; 12] = ["JAN", "FEB", "MAR", "APR", "MAY", "JUN", "JUL", "AUG", "SEP", "OCT", "NOV", "DEC"];

/// The local time now.
pub fn now() -> Clock {
    // SAFETY: time(NULL) only reads the clock; localtime_r writes into the tm we own and
    // returns NULL on failure, which leaves the default (empty) clock.
    unsafe {
        let t = libc::time(std::ptr::null_mut());
        let mut tm: libc::tm = std::mem::zeroed();
        if libc::localtime_r(&t, &mut tm).is_null() {
            return Clock::default();
        }
        format(tm.tm_wday, tm.tm_mday, tm.tm_mon, tm.tm_hour, tm.tm_min, tm.tm_sec)
    }
}

/// Seconds since the kernel started (CLOCK_BOOTTIME): the clock of the kernel log and of
/// init's ro.boottime.*, which chrony's setting of the wall clock does not move.
pub fn since_boot() -> f64 {
    // SAFETY: clock_gettime writes into the timespec we own.
    unsafe {
        let mut ts: libc::timespec = std::mem::zeroed();
        libc::clock_gettime(libc::CLOCK_BOOTTIME, &mut ts);
        ts.tv_sec as f64 + ts.tv_nsec as f64 / 1e9
    }
}

/// The texts from the fields of a `struct tm` (weekday 0 = Sunday, month 0 = January).
fn format(wday: i32, mday: i32, mon: i32, hour: i32, min: i32, sec: i32) -> Clock {
    let day = DAYS.get(wday as usize).copied().unwrap_or("");
    let month = MONTHS.get(mon as usize).copied().unwrap_or("");
    Clock {
        date: format!("{day}, {mday} {month}"),
        time: format!("{hour:02}:{min:02}"),
        hour,
        min,
        sec: format!(":{sec:02}"),
    }
}

impl Clock {
    /// The time as the settings want it: "21:05" on a 24-hour clock, "9:05" on a 12-hour one
    /// (its AM or PM is sec_text's: the time's field has no room for it).
    pub fn time_text(&self, hour24: bool) -> String {
        if hour24 || self.time.is_empty() {
            return self.time.clone();
        }
        let h = match self.hour % 12 { 0 => 12, h => h };
        format!("{h}:{:02}", self.min)
    }

    /// Next to the time: the seconds on a 24-hour clock, AM or PM on a 12-hour one.
    pub fn sec_text(&self, hour24: bool) -> String {
        if hour24 || self.time.is_empty() {
            return self.sec.clone();
        }
        (if self.hour < 12 { " AM" } else { " PM" }).to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_texts_of_a_time() {
        let c = format(5, 6, 10, 9, 5, 7);
        assert_eq!(c.date, "FRI, 6 NOV");
        assert_eq!(c.time, "09:05");
        assert_eq!(c.sec, ":07");
    }

    #[test]
    fn a_12_hour_clock() {
        assert_eq!(format(5, 6, 10, 9, 5, 7).time_text(false), "9:05");
        assert_eq!(format(5, 6, 10, 9, 5, 7).sec_text(false), " AM");
        assert_eq!(format(5, 6, 10, 21, 5, 7).sec_text(false), " PM");
        assert_eq!(format(5, 6, 10, 0, 30, 0).time_text(false), "12:30");
        assert_eq!(format(5, 6, 10, 12, 0, 0).sec_text(false), " PM");
        assert_eq!(format(5, 6, 10, 21, 5, 7).time_text(true), "21:05");
        assert_eq!(format(5, 6, 10, 21, 5, 7).sec_text(true), ":07");
    }
}
