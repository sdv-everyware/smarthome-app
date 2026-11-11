/*
 * smarthome-app: the devices behind the dashboard's tiles, as their sheet (#device) shows them.
 *
 * A device is what it does (its function: a thermostat, a light, a fan) and what it is (vendor,
 * model, firmware, where in the Matter fabric). The Matter bundle reports both (topic
 * matter-devices, through the dashboard bundle: screen_binder.rs); a tile is bound to the first
 * endpoint of its device type. The values the dashboard controls (the setpoint, the light, the
 * dimmer, the fan) follow the device; what no device reported yet shows as a dash.
 */

use crate::state::Dashboard;
use everyware_catalog::matter::{MatterDevices, MatterEndpoint, MatterNode};
use everyware_ui::symbol::SymbolIcon;

/// Shown for a value no one reported yet.
pub const UNKNOWN: &str = "—";

/// The devices of the dashboard, one per tile.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeviceId {
    /// The Home climate tile: a thermostat with a humidity sensor.
    Climate,
    /// The Entryway's lights: an on/off light.
    Entryway,
    /// The Bedroom's lights: a dimmable light.
    Bedroom,
    /// The Office fan.
    OfficeFan,
}

impl DeviceId {
    /// Every device, in the dashboard's order.
    pub const ALL: [DeviceId; 4] = [DeviceId::Climate, DeviceId::Entryway, DeviceId::Bedroom, DeviceId::OfficeFan];

    /// The tile header that opens the device's sheet (dashboard-patch.js).
    pub fn header(self) -> &'static str {
        match self {
            DeviceId::Climate => "#climate/header",
            DeviceId::Entryway => "#entryway/header",
            DeviceId::Bedroom => "#bedroom/header",
            DeviceId::OfficeFan => "#office/header",
        }
    }

    /// The Matter device type a tile shows (Matter Device Library).
    pub fn device_type(self) -> u32 {
        match self {
            DeviceId::Climate => 0x0301,
            DeviceId::Entryway => 0x0100,
            DeviceId::Bedroom => 0x0101,
            DeviceId::OfficeFan => 0x002B,
        }
    }

    /// The device whose tile header is `node`.
    pub fn of_header(node: &str) -> Option<DeviceId> {
        DeviceId::ALL.into_iter().find(|d| d.header() == node)
    }
}

/// What a device is and how it is doing (Basic Information, General Diagnostics, the fabric):
/// what the Matter bundle reports; None until then.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DeviceInfo {
    pub vendor: Option<String>,
    pub product: Option<String>,
    pub serial: Option<String>,
    pub firmware: Option<String>,
    /// "2 s ago": the last report.
    pub online: Option<String>,
    /// The node id and the endpoint in the fabric, "0x12 · 1".
    pub node_endpoint: Option<String>,
    pub uptime: Option<String>,
    pub last_reboot: Option<String>,
}

/// The thermostat's values only the device knows (Thermostat, Relative Humidity Measurement).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Thermostat {
    pub room_c: Option<f32>,
    pub cooling_c: Option<f32>,
    pub mode: Option<String>,
    pub running: Option<String>,
    pub heating_limits: Option<(f32, f32)>,
    pub cooling_limits: Option<(f32, f32)>,
    pub humidity: Option<f32>,
}

/// A light's values only the device knows (On/Off, Level Control).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Light {
    pub after_power_loss: Option<String>,
    pub on_since: Option<String>,
    pub transition_s: Option<f32>,
    pub level_range: Option<(u8, u8)>,
    pub level_at_power_on: Option<String>,
}

/// The fan's values only the device knows (Fan Control).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Fan {
    pub percent: Option<u8>,
    pub modes: Option<String>,
    pub airflow: Option<String>,
    pub rocking: Option<String>,
    pub wind: Option<String>,
}

/// Where a tile's device is in the fabric.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Binding {
    pub node_id: u64,
    pub endpoint: u32,
}

/// What is known of the devices beyond the dashboard's own state.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Devices {
    pub climate: (Thermostat, DeviceInfo),
    pub entryway: (Light, DeviceInfo),
    pub bedroom: (Light, DeviceInfo),
    pub office_fan: (Fan, DeviceInfo),
    /// The tiles' devices in the fabric, in DeviceId::ALL's order; None: no such device.
    pub bindings: [Option<Binding>; 4],
}

/// What the devices report of the values the dashboard controls: the dashboard follows them.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Reported {
    pub heating_setpoint_c: Option<f32>,
    pub entryway_on: Option<bool>,
    /// The bedroom's level, 0..100 (0 when off).
    pub bedroom_percent: Option<f32>,
    /// "Low", "Med", "High" (the fan's segments).
    pub fan_mode: Option<String>,
}

/// A device as its sheet shows it: the header, the state large, the rows.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DeviceView {
    pub name: String,
    /// The kind and the model: "Thermostat · EveryWare Virtual Home".
    pub subtitle: String,
    pub symbol: SymbolIcon,
    pub state: String,
    pub detail: String,
    /// The State list: what the device reports about its function.
    pub state_rows: Vec<(String, String)>,
    /// The Device list: what it is.
    pub info_rows: Vec<(String, String)>,
    /// Added to the Device list on "Show more".
    pub diagnostic_rows: Vec<(String, String)>,
}

fn or_dash(value: &Option<String>) -> String {
    value.clone().unwrap_or_else(|| UNKNOWN.to_string())
}

fn celsius(value: Option<f32>) -> String {
    value.map_or_else(|| UNKNOWN.to_string(), |c| format!("{c:.1} °C"))
}

fn range<T: std::fmt::Display>(value: Option<(T, T)>, unit: &str) -> String {
    value.map_or_else(|| UNKNOWN.to_string(), |(a, b)| format!("{a} – {b}{unit}"))
}

fn row(label: &str, value: String) -> (String, String) {
    (label.to_string(), value)
}

/// "1 h 2 min", "5 min", "40 s".
fn duration(s: u64) -> String {
    match s {
        0..=59 => format!("{s} s"),
        60..=3599 => format!("{} min", s / 60),
        _ => format!("{} h {} min", s / 3600, s % 3600 / 60),
    }
}

/// The node's online state as the Device list shows it.
fn online(node: &MatterNode, now: i64) -> String {
    match (node.online, now - node.last_seen) {
        (true, ago) if ago <= 1 => "Now".to_string(),
        (true, ago) => format!("{ago} s ago"),
        (false, _) if node.last_seen == 0 => "Offline".to_string(),
        (false, ago) => format!("Offline · last seen {} ago", duration(ago.max(0) as u64)),
    }
}

/// "OffLowMedHigh" as "Off · Low · Med · High".
fn mode_sequence(sequence: &str) -> String {
    let mut out = Vec::new();
    let mut word = String::new();
    for c in sequence.chars() {
        if c.is_uppercase() && !word.is_empty() {
            out.push(std::mem::take(&mut word));
        }
        word.push(c);
    }
    if !word.is_empty() {
        out.push(word);
    }
    out.join(" · ")
}

/// A fan mode of the cluster as the segment shows it.
pub fn fan_segment(mode: &str) -> Option<&'static str> {
    match mode {
        "Low" => Some("Low"),
        "Medium" => Some("Med"),
        "High" | "On" => Some("High"),
        _ => None,
    }
}

impl Devices {
    /// Takes a report of the Matter bundle: every tile to the first endpoint of its device type,
    /// its values and its node's. Returns what the dashboard follows.
    pub fn update(&mut self, report: &MatterDevices, now: i64) -> Reported {
        let mut reported = Reported::default();
        for (i, id) in DeviceId::ALL.into_iter().enumerate() {
            let found = report.nodes.iter().find_map(|n| n.endpoints.iter().find(|e| e.device_type == id.device_type()).map(|e| (n, e)));
            let Some((node, ep)) = found else {
                self.bindings[i] = None;
                continue;
            };
            self.bindings[i] = Some(Binding { node_id: node.node_id, endpoint: ep.id });
            let info = DeviceInfo {
                vendor: Some(node.vendor.clone()).filter(|v| !v.is_empty()),
                product: Some(node.product.clone()).filter(|v| !v.is_empty()),
                serial: Some(node.serial_number.clone()).filter(|v| !v.is_empty()),
                firmware: Some(node.software_version.clone()).filter(|v| !v.is_empty()),
                online: Some(online(node, now)),
                node_endpoint: Some(format!("{:#x} · {}", node.node_id, ep.id)),
                uptime: Some(duration(node.uptime_s)),
                last_reboot: Some(node.boot_reason.clone()).filter(|v| !v.is_empty()),
            };
            match id {
                DeviceId::Climate => {
                    self.climate = (thermostat(ep), info);
                    reported.heating_setpoint_c = ep.thermostat.as_ref().map(|t| t.heating_setpoint_c);
                }
                DeviceId::Entryway => {
                    self.entryway = (light(ep), info);
                    reported.entryway_on = ep.on_off.as_ref().map(|o| o.on);
                }
                DeviceId::Bedroom => {
                    self.bedroom = (light(ep), info);
                    let on = ep.on_off.as_ref().is_none_or(|o| o.on);
                    reported.bedroom_percent = ep.level.as_ref().filter(|l| l.level > 0).map(|l| if on { (l.level as f32 * 100.0 / 254.0).round() } else { 0.0 });
                }
                DeviceId::OfficeFan => {
                    let f = ep.fan.as_ref();
                    self.office_fan = (
                        Fan {
                            percent: f.map(|f| f.percent_current as u8),
                            modes: f.map(|f| mode_sequence(&f.mode_sequence)),
                            airflow: f.map(|f| f.airflow_direction.clone()).filter(|v| !v.is_empty()),
                            rocking: f.filter(|f| !f.rock_support.is_empty()).map(|f| if f.rock_setting.is_empty() { "Off".to_string() } else { f.rock_setting.clone() }),
                            wind: f.filter(|f| !f.wind_support.is_empty()).map(|f| if f.wind_setting.is_empty() { "Off".to_string() } else { f.wind_setting.clone() }),
                        },
                        info,
                    );
                    reported.fan_mode = f.and_then(|f| fan_segment(&f.mode)).map(str::to_string);
                }
            }
        }
        reported
    }

    /// The sheet of `id` with the dashboard's state `dash`.
    pub fn view(&self, id: DeviceId, dash: &Dashboard) -> DeviceView {
        let (name, kind, symbol, info) = match id {
            DeviceId::Climate => ("Home climate", "Thermostat", SymbolIcon::House, &self.climate.1),
            DeviceId::Entryway => ("Entryway", "On/off light", SymbolIcon::DoorOpen, &self.entryway.1),
            DeviceId::Bedroom => ("Bedroom", "Dimmable light", SymbolIcon::BedDouble, &self.bedroom.1),
            DeviceId::OfficeFan => ("Office fan", "Fan", SymbolIcon::Armchair, &self.office_fan.1),
        };
        let model = match (&info.vendor, &info.product) {
            (Some(v), Some(p)) => format!("{kind} · {v} {p}"),
            _ => kind.to_string(),
        };
        let (state, detail, state_rows) = match id {
            DeviceId::Climate => self.thermostat(dash),
            DeviceId::Entryway => {
                let (light, on) = (&self.entryway.0, dash.light.value);
                let state = if on { "On" } else { "Off" }.to_string();
                let detail = light.on_since.as_ref().filter(|_| on).map_or_else(String::new, |t| format!("On since {t}"));
                let rows = vec![
                    row("On/off", state.clone()),
                    row("After a power loss", or_dash(&light.after_power_loss)),
                    row("On since", if on { or_dash(&light.on_since) } else { UNKNOWN.to_string() }),
                ];
                (state, detail, rows)
            }
            DeviceId::Bedroom => {
                let (light, level) = (&self.bedroom.0, dash.dimmer.value.round());
                let on = level > 0.0;
                let state = format!("{level} %");
                let detail = match light.transition_s {
                    Some(s) if on => format!("On · fades in {s} s"),
                    _ => if on { "On" } else { "Off" }.to_string(),
                };
                let rows = vec![
                    row("Level", state.clone()),
                    row("On/off", if on { "On" } else { "Off" }.to_string()),
                    row("Transition", light.transition_s.map_or_else(|| UNKNOWN.to_string(), |s| format!("{s} s"))),
                    row("Level range", range(light.level_range, "")),
                    row("Level at power on", or_dash(&light.level_at_power_on)),
                ];
                (state, detail, rows)
            }
            DeviceId::OfficeFan => {
                let fan = &self.office_fan.0;
                let mode = dash.fan.value().to_string();
                let detail = fan.percent.map_or_else(String::new, |p| format!("{p} %"));
                let rows = vec![
                    row("Mode", mode.clone()),
                    row("Speed", fan.percent.map_or_else(|| UNKNOWN.to_string(), |p| format!("{p} %"))),
                    row("Modes", or_dash(&fan.modes)),
                    row("Airflow", or_dash(&fan.airflow)),
                    row("Rocking", or_dash(&fan.rocking)),
                    row("Wind", or_dash(&fan.wind)),
                ];
                (mode, detail, rows)
            }
        };
        DeviceView {
            name: name.to_string(),
            subtitle: model,
            symbol,
            state,
            detail,
            state_rows,
            info_rows: vec![
                row("Vendor", or_dash(&info.vendor)),
                row("Product", or_dash(&info.product)),
                row("Serial number", or_dash(&info.serial)),
                row("Firmware", or_dash(&info.firmware)),
                row("Online", or_dash(&info.online)),
                row("Node · endpoint", or_dash(&info.node_endpoint)),
            ],
            diagnostic_rows: vec![row("Uptime", or_dash(&info.uptime)), row("Last reboot", or_dash(&info.last_reboot))],
        }
    }

    fn thermostat(&self, dash: &Dashboard) -> (String, String, Vec<(String, String)>) {
        let t = &self.climate.0;
        let setpoint = dash.setpoint as f32;
        let state = t.room_c.map_or_else(|| format!("{setpoint:.0} °C"), |c| format!("{c:.1} °C"));
        let detail = match (&t.running, t.humidity) {
            (Some(running), Some(h)) => format!("{running} to {setpoint:.1} °C · {h:.0} % humidity"),
            _ => format!("Target {setpoint:.1} °C"),
        };
        let rows = vec![
            row("Room temperature", celsius(t.room_c)),
            row("Heating setpoint", celsius(Some(setpoint))),
            row("Cooling setpoint", celsius(t.cooling_c)),
            row("Mode", or_dash(&t.mode)),
            row("Running", or_dash(&t.running)),
            row("Heating limits", range(t.heating_limits.map(|(a, b)| (format!("{a:.1}"), format!("{b:.1}"))), " °C")),
            row("Cooling limits", range(t.cooling_limits.map(|(a, b)| (format!("{a:.1}"), format!("{b:.1}"))), " °C")),
            row("Humidity", t.humidity.map_or_else(|| UNKNOWN.to_string(), |h| format!("{h:.0} %"))),
        ];
        (state, detail, rows)
    }
}

fn thermostat(ep: &MatterEndpoint) -> Thermostat {
    let t = ep.thermostat.as_ref();
    let humidity = ep.humidity.as_ref().filter(|h| h.known).map(|h| h.percent);
    Thermostat {
        room_c: t.filter(|t| t.local_temperature_known).map(|t| t.local_temperature_c),
        cooling_c: t.map(|t| t.cooling_setpoint_c),
        mode: t.map(|t| t.system_mode.clone()).filter(|v| !v.is_empty()),
        running: t.map(|t| if t.running.is_empty() { "idle".to_string() } else { t.running.clone() }).map(|r| capitalize(&r)),
        heating_limits: t.map(|t| (t.min_heat_setpoint_c, t.max_heat_setpoint_c)),
        cooling_limits: t.map(|t| (t.min_cool_setpoint_c, t.max_cool_setpoint_c)),
        humidity,
    }
}

fn light(ep: &MatterEndpoint) -> Light {
    let o = ep.on_off.as_ref();
    let l = ep.level.as_ref();
    Light {
        after_power_loss: o.map(|o| match o.start_up.as_str() {
            "Previous" | "" => "As before".to_string(),
            other => other.to_string(),
        }),
        on_since: None,
        transition_s: l.map(|l| l.transition_ds as f32 / 10.0),
        level_range: l.map(|l| (l.min_level as u8, l.max_level as u8)),
        level_at_power_on: l.map(|l| if l.start_up_level == 0 { "As before".to_string() } else { format!("{} %", (l.start_up_level as f32 * 100.0 / 254.0).round()) }),
    }
}

fn capitalize(s: &str) -> String {
    let mut c = s.chars();
    c.next().map_or_else(String::new, |f| f.to_uppercase().chain(c).collect())
}
