//! smarthome-app: the weather on this screen. The dashboard bundle sends the data (a WMO weather
//! code, temperatures, the chance of rain); what it looks like is decided here: which picture a
//! code gets, its words, the bars of the day. The pictures are this app's own component set
//! (#smarthome-weather, design/weather-pictures.js), not a component of the UI library: the same
//! kinds in both.

use everyware_ui::bar_chart::{BarChart, BARS};
use everyware_ui::{Change, Component, ComponentKind, Slotted};

/// A picture of #smarthome-weather: seven kinds of weather.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Picture {
    Clear,
    Partly,
    Cloudy,
    Fog,
    Rain,
    Snow,
    Thunder,
}

impl Picture {
    /// The picture for a WMO weather interpretation code (the bundle's `weather_code`).
    pub fn of(code: u32) -> Self {
        match code {
            0 | 1 => Picture::Clear,
            2 => Picture::Partly,
            3 => Picture::Cloudy,
            45 | 48 => Picture::Fog,
            51..=67 | 80..=82 => Picture::Rain,
            71..=77 | 85 | 86 => Picture::Snow,
            95..=99 => Picture::Thunder,
            _ => Picture::Cloudy,
        }
    }
}

impl Component for Picture {
    const KIND: ComponentKind = ComponentKind::App;

    fn variant(&self) -> String {
        let kind = match self {
            Picture::Clear => "clear",
            Picture::Partly => "partly",
            Picture::Cloudy => "cloudy",
            Picture::Fog => "fog",
            Picture::Rain => "rain",
            Picture::Snow => "snow",
            Picture::Thunder => "thunder",
        };
        format!("kind={kind}")
    }

    fn last_change(&self) -> Option<Change> {
        None
    }
}

impl Slotted for Picture {
    const SET: &'static str = "#smarthome-weather";
}

/// A WMO weather interpretation code as words, as the screen shows it.
pub fn words(code: u32) -> &'static str {
    match code {
        0 => "Clear",
        1 => "Mainly clear",
        2 => "Partly cloudy",
        3 => "Overcast",
        45 | 48 => "Fog",
        51 | 53 | 55 => "Drizzle",
        56 | 57 => "Freezing drizzle",
        61 | 63 | 65 => "Rain",
        66 | 67 => "Freezing rain",
        71 | 73 | 75 | 77 => "Snow",
        80..=82 => "Rain showers",
        85 | 86 => "Snow showers",
        95 => "Thunderstorm",
        96 | 99 => "Thunderstorm with hail",
        _ => "Unknown",
    }
}

/// The next hours as the bars of the library's bar chart: the temperatures (°C) as heights
/// that span the day's range with a degree of margin, so a flat day is a row of half-height
/// bars and a swing of ten degrees fills the chart; a bar is accented when rain is likely
/// (a precipitation probability of 50 % or more).
pub fn bars(hours: impl IntoIterator<Item = (f64, f64)>) -> BarChart {
    let data: Vec<(f64, f64)> = hours.into_iter().take(BARS).collect();
    let (mut lo, mut hi) = data.iter().map(|h| h.0).fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), t| (lo.min(t), hi.max(t)));
    lo -= 1.0;
    hi += 1.0;
    if hi - lo < 4.0 {
        // a flat day: four degrees around its temperature
        let mid = (lo + hi) / 2.0;
        lo = mid - 2.0;
        hi = mid + 2.0;
    }
    BarChart::new(data.into_iter().map(|(temperature, rain)| ((((temperature - lo) / (hi - lo)) * 100.0) as f32, rain >= 50.0)))
}
