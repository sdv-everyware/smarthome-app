/*
 * smarthome-app: actions, the only way to change the state.
 */

use crate::clock::Clock;
use crate::house_binder;
use crate::house_link;
use crate::devices::{DeviceId, Devices, Reported};
use crate::state::{Dashboard, DeviceSheet, Network, Security, State, Step, WifiSheet, SETPOINT, STATUS_ENTER};
use everyware_dashboard_grpc::wifi::{WifiCommand, WifiConnect, WifiConnection, WifiNetworks, WifiScan, WifiState};
use harry::side_effects::SideEffects;
use harry::state::StateAction;
use everyware_ui::part_of;
use everyware_ui::button::{ButtonEvent, IconButton};
use everyware_ui::dropdown::DropdownEvent;
use everyware_ui::dock::KeyboardDock;
use everyware_ui::form::FormEvent;
use everyware_ui::list_view::ListEvent;
use everyware_ui::segmented::SegmentedEvent;
use everyware_ui::slider::SliderEvent;
use everyware_ui::toggle::ToggleEvent;
use everyware_ui::Gesture;
use everyware_dashboard_grpc::dashboard::{MatterCommandResult, WifiCommandResult};
use everyware_dashboard_grpc::matter::MatterDevices;
use everyware_dashboard_grpc::matter_service::{MatterCommand, MatterMoveToLevel, MatterSetFanMode, MatterSetOnOff, MatterSetSetpoint};
use everyware_dashboard_grpc::news::{NewsFeed, NewsThumbnail};
use everyware_dashboard_grpc::weather::Weather;
use std::time::Instant;

/// WPA2/WPA3 passphrases are at least 8 characters.
const MIN_LEN: usize = 8;

#[derive(Clone, Debug, PartialEq)]
pub enum HomeAction {
    /// Once a second from the ticker thread in main.rs.
    Tick(Clock),
    /// A gesture on a node (ui_event_listener.rs, everyware_ui::Input).
    Gesture(String, Gesture),
    /// The weather of the house, from the dashboard bundle (house_link.rs).
    Weather(Weather),
    /// The news feed, from the news bundle (house_link.rs).
    News(NewsFeed),
    /// The thumbnail of a story of the feed (house_link.rs).
    NewsThumbnail(NewsThumbnail),
    /// The dashboard bundle's heartbeats started or stopped (house_link.rs).
    HouseLink(bool),
    /// The networks around, from the Wi-Fi bundle (house_link.rs).
    WifiNetworks(WifiNetworks),
    /// The Wi-Fi connection, from the Wi-Fi bundle (house_link.rs).
    WifiState(WifiState),
    /// The Wi-Fi bundle's answer to a command of the screen (house_link.rs).
    WifiResult(WifiCommandResult),
    /// The Matter devices of the house, from the Matter bundle (house_link.rs).
    MatterDevices(MatterDevices),
    /// The Matter bundle's answer to a command of the screen (house_link.rs).
    MatterResult(MatterCommandResult),
    /// How the page in the panel's browser is doing (browser.rs events).
    Browser(crate::browser::BrowserEvent),
}

impl StateAction<State, ()> for HomeAction {
    fn apply(&self, state: &mut State, _side_effects: &mut Vec<SideEffects<()>>) {
        if let HomeAction::Gesture(node, gesture) = self {
            log::debug!("smarthome: gesture {gesture:?} on {node}");
        }
        match self {
            HomeAction::Tick(clock) => {
                state.clock = clock.clone();
                // a page that never reports (no events FIFO, a stuck load): show what there is
                if state.browser.loading_since().is_some_and(|t| t.elapsed() > crate::state::PAGE_TIMEOUT) {
                    log::warn!("smarthome: browser: no word from the page in {:?}, showing it", crate::state::PAGE_TIMEOUT);
                    state.browser.shown();
                }
            }
            HomeAction::Browser(event) => {
                // the page replaced the old one (or failed, and Cog shows why): the loader goes
                if state.browser.is_loading() {
                    log::info!("smarthome: browser: {event:?} after {:?}", state.browser.loading_since().map(|t| t.elapsed()).unwrap_or_default());
                    state.browser.shown();
                }
            }
            HomeAction::Weather(weather) => state.weather = Some(weather.clone()),
            HomeAction::News(feed) => {
                let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs() as i64);
                state.dash.news.set_rows(crate::news::rows(feed, now));
                state.stories = feed.stories.iter().map(crate::browser::Page::of).collect();
                crate::news::forget_pictures(feed);
            }
            HomeAction::NewsThumbnail(thumbnail) => crate::news::set_picture(thumbnail),
            HomeAction::HouseLink(up) => state.house_link = *up,
            HomeAction::WifiNetworks(networks) => {
                state.wifi.set_networks(networks.networks.iter().map(Network::from).collect());
            }
            HomeAction::WifiState(wifi_state) => on_wifi_state(&mut state.wifi, wifi_state),
            HomeAction::WifiResult(result) => on_wifi_result(&mut state.wifi, result),
            HomeAction::MatterDevices(report) => {
                let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs() as i64);
                let reported = state.devices.update(report, now);
                follow_devices(&mut state.dash, &mut state.matter_pending, &reported);
            }
            HomeAction::MatterResult(result) => on_matter_result(&mut state.matter_pending, &state.devices, result),
            HomeAction::Gesture(node, gesture) if state.panel.is_open() => {
                // the panel while it is open: a tap on the scrim or on close closes it, the card
                // takes the rest (the browser's zone takes its own touches in the looper)
                let scrim = node == "#panel/scrim" && matches!(gesture, Gesture::Up { inside: true, .. });
                if scrim || (node == "#panel/close" && clicked(&mut state.browser.close, *gesture)) {
                    state.panel.close();
                    // the hidden zone stops the browser's frames, not its page: a video would
                    // play on, heard and not seen
                    if state.browser.page.take().is_some() {
                        crate::browser::open(crate::browser::BLANK);
                    }
                    state.browser.shown();
                }
            }
            HomeAction::Gesture(node, gesture) if node == "#settings" => {
                // Settings opens the panel with the browser as it is (empty after a close)
                if clicked(&mut state.dash.settings, *gesture) {
                    state.panel.open();
                }
            }
            HomeAction::Gesture(node, gesture) if state.device.container.is_open() => {
                on_device_sheet(&mut state.device, &state.devices, &state.dash, node, *gesture)
            }
            HomeAction::Gesture(node, gesture) if DeviceId::of_header(node).is_some() => {
                // a tile's header opens its device's sheet
                let index = DeviceId::ALL.iter().position(|d| d.header() == node).expect("of_header found it");
                if state.dash.headers[index].on(*gesture) {
                    state.device.open(DeviceId::ALL[index], &state.devices, &state.dash);
                }
            }
            HomeAction::Gesture(node, gesture) if node == "#status/wifi" => {
                // the Wi-Fi indicator opens the sheet; the sheet starts over every time
                if state.dash.wifi.on(*gesture) == Some(ButtonEvent::Clicked) && !state.wifi.form.is_open() {
                    state.wifi = WifiSheet::opened(state.wifi.networks.clone(), state.wifi.state.clone());
                    house_binder::send_wifi_command(wifi_scan());
                }
            }
            HomeAction::Gesture(node, gesture) if state.wifi.form.is_open() => {
                on_sheet(&mut state.wifi, &mut state.osk, node, *gesture)
            }
            HomeAction::Gesture(node, gesture) if node == "#news/list" || node.starts_with("#news/list/") => {
                // a tap on a story opens its page in the panel's browser
                if let Some(page) = on_news(&mut state.dash, node, *gesture).and_then(|row| state.stories.get(row)) {
                    crate::browser::open(&page.url);
                    state.browser.loading(page.clone(), Instant::now());
                    state.panel.open();
                }
            }
            HomeAction::Gesture(node, gesture) => {
                if let Some(control) = on_dashboard(&mut state.dash, node, *gesture) {
                    send_control(&mut state.matter_pending, &state.devices, control);
                }
            }
        }
        // the screen's keyboard follows the focus (every form of the screen, or'ed)
        state.osk.follow(state.wifi.form.focused().is_some());
        // an open sheet shows the device as it is now (the dashboard or a bundle changed it)
        if state.device.container.is_open() {
            state.device.refresh(&state.devices, &state.dash);
        }
    }
}

fn clicked(button: &mut IconButton, gesture: Gesture) -> bool {
    button.on(gesture) == Some(ButtonEvent::Clicked)
}

/// A control of a device the dashboard changed: a command for the device.
#[derive(Clone, Debug, PartialEq)]
enum Control {
    Setpoint(i32),
    Light(bool),
    /// 0..100.
    Dimmer(f32),
    /// "Low", "Med", "High".
    Fan(String),
}

impl Control {
    fn device(&self) -> DeviceId {
        match self {
            Control::Setpoint(_) => DeviceId::Climate,
            Control::Light(_) => DeviceId::Entryway,
            Control::Dimmer(_) => DeviceId::Bedroom,
            Control::Fan(_) => DeviceId::OfficeFan,
        }
    }
}

/// How long a sent command keeps the device's reports off its control at most (the Matter
/// bundle answers a command within 20 s).
const PENDING: std::time::Duration = std::time::Duration::from_secs(25);

fn index_of(id: DeviceId) -> usize {
    DeviceId::ALL.iter().position(|d| *d == id).expect("a device of the dashboard")
}

/// The command for `control` to its device, if the tile has one in the fabric; the control
/// keeps what the finger set until the device answered.
fn send_control(pending: &mut [Option<Instant>; 4], devices: &Devices, control: Control) {
    use everyware_dashboard_grpc::matter_service::matter_command::Command;
    let index = index_of(control.device());
    let Some(b) = devices.bindings[index] else {
        log::info!("smarthome: {control:?}: no Matter device for this tile, the screen only");
        return;
    };
    let command = match &control {
        Control::Setpoint(c) => {
            let mut r = MatterSetSetpoint::new();
            (r.node_id, r.endpoint, r.heating, r.celsius) = (b.node_id, b.endpoint, true, *c as f32);
            Command::SetSetpoint(r)
        }
        Control::Light(on) => {
            let mut r = MatterSetOnOff::new();
            (r.node_id, r.endpoint, r.on) = (b.node_id, b.endpoint, *on);
            Command::SetOnOff(r)
        }
        Control::Dimmer(percent) if *percent < 1.0 => {
            let mut r = MatterSetOnOff::new();
            (r.node_id, r.endpoint, r.on) = (b.node_id, b.endpoint, false);
            Command::SetOnOff(r)
        }
        Control::Dimmer(percent) => {
            let mut r = MatterMoveToLevel::new();
            (r.node_id, r.endpoint) = (b.node_id, b.endpoint);
            r.level = ((percent * 254.0 / 100.0).round() as u32).clamp(1, 254);
            Command::MoveToLevel(r)
        }
        Control::Fan(mode) => {
            let mut r = MatterSetFanMode::new();
            (r.node_id, r.endpoint) = (b.node_id, b.endpoint);
            r.mode = if mode == "Med" { "Medium".to_string() } else { mode.clone() };
            Command::SetFanMode(r)
        }
    };
    let mut message = MatterCommand::new();
    message.command = Some(command);
    if house_link::send_matter_command(message) {
        log::info!("smarthome: {control:?} to node {:#x} ep{}", b.node_id, b.endpoint);
        pending[index] = Some(Instant::now());
    }
}

/// The dashboard follows what the devices report, but not a control with a command in flight
/// nor one under the finger.
fn follow_devices(d: &mut Dashboard, pending: &mut [Option<Instant>; 4], reported: &Reported) {
    for p in pending.iter_mut() {
        if p.is_some_and(|t| t.elapsed() > PENDING) {
            *p = None;
        }
    }
    let free = |id: DeviceId| pending[index_of(id)].is_none();
    if let Some(c) = reported.heating_setpoint_c.filter(|_| free(DeviceId::Climate)) {
        d.setpoint = (c.round() as i32).clamp(*SETPOINT.start(), *SETPOINT.end());
    }
    if let Some(on) = reported.entryway_on.filter(|_| free(DeviceId::Entryway)) {
        d.light.set(on);
    }
    if let Some(percent) = reported.bedroom_percent.filter(|_| free(DeviceId::Bedroom) && !d.dimmer.dragging()) {
        d.dimmer.value = percent;
    }
    if let Some(mode) = reported.fan_mode.as_deref().filter(|_| free(DeviceId::OfficeFan)) {
        if let Some(i) = d.fan.labels.iter().position(|l| l == mode) {
            d.fan.select(i);
        }
    }
}

/// A command's answer: its control follows the device again (with the next report).
fn on_matter_result(pending: &mut [Option<Instant>; 4], devices: &Devices, result: &MatterCommandResult) {
    let ok = result.result.as_ref().is_some_and(|r| r.ok);
    if ok {
        log::info!("smarthome: Matter {} node {:#x} ep{}: done", result.command, result.node_id, result.endpoint);
    } else {
        let error = result.result.as_ref().map(|r| r.error.clone()).unwrap_or_default();
        log::warn!("smarthome: Matter {} node {:#x} ep{}: {error}", result.command, result.node_id, result.endpoint);
    }
    for (i, b) in devices.bindings.iter().enumerate() {
        if b.is_some_and(|b| b.node_id == result.node_id && b.endpoint == result.endpoint) {
            pending[i] = None;
        }
    }
}

/// The news feed: a drag or a fling scrolls it, a tap on a story selects it (ui-lib ListView,
/// as the Wi-Fi list) and returns its row.
fn on_news(d: &mut Dashboard, node: &str, gesture: Gesture) -> Option<usize> {
    let row = node.strip_prefix("#news/list/").and_then(|r| r.parse().ok());
    let ListEvent::Chosen(row) = d.news.on(row, gesture, Instant::now())?;
    d.news.select(Some(row));
    log::info!("smarthome: news story {row} chosen");
    Some(row)
}

/// The dashboard's controls; a control of a device returns the command it means.
fn on_dashboard(d: &mut Dashboard, node: &str, gesture: Gesture) -> Option<Control> {
    match node {
        "#status/presence" => d.presence.on_field(gesture),
        _ if node.starts_with("#dropdown/item/") => {
            // the open list's items, by their names in the component (ui-lib dropdown)
            if let Ok(n) = node["#dropdown/item/".len()..].parse::<usize>() {
                if let Some(DropdownEvent::Selected(i)) = d.presence.on_item(n - 1, gesture) {
                    log::info!("smarthome: presence {}", d.presence.items[i]);
                }
            }
        }
        "#climate/minus" => {
            if clicked(&mut d.temp_minus, gesture) {
                d.setpoint = (d.setpoint - 1).max(*SETPOINT.start());
                return Some(Control::Setpoint(d.setpoint));
            }
        }
        "#climate/plus" => {
            if clicked(&mut d.temp_plus, gesture) {
                d.setpoint = (d.setpoint + 1).min(*SETPOINT.end());
                return Some(Control::Setpoint(d.setpoint));
            }
        }
        "#climate/humidity-adjust" => {
            if clicked(&mut d.humidity_adjust, gesture) {
                log::info!("smarthome: adjust humidity");
            }
        }
        "#camera/open" => {
            if clicked(&mut d.open_feed, gesture) {
                log::info!("smarthome: open the camera feed");
            }
        }
        "#entryway/light" => {
            if let Some(ToggleEvent::Toggled(on)) = d.light.on(gesture) {
                log::info!("smarthome: entryway lights {}", if on { "on" } else { "off" });
                return Some(Control::Light(on));
            }
        }
        "#bedroom/dimmer" => {
            // the value follows the finger (SliderEvent::Moving) and is in the slider; the
            // device gets it when the finger is up
            if let Some(SliderEvent::Changed(percent)) = d.dimmer.on(gesture) {
                return Some(Control::Dimmer(percent));
            }
        }
        _ if part_of("#office/fan", node).is_some() => {
            // the fan's segments, "#office/fan/#segmented/segment-<i>" (the part with its slot)
            if let Some(n) = part_of("#office/fan", node)
                .and_then(|p| p.strip_prefix("#segmented/segment-"))
                .and_then(|n| n.parse::<usize>().ok())
            {
                if let Some(SegmentedEvent::Selected(_)) = n.checked_sub(1).and_then(|i| d.fan.on(i, gesture)) {
                    log::info!("smarthome: office fan {}", d.fan.value());
                    return Some(Control::Fan(d.fan.value().to_string()));
                }
            }
        }
        "#door/details" => {
            if clicked(&mut d.door_details, gesture) {
                log::info!("smarthome: front door details");
            }
        }
        "#intercom/talk" => {
            if d.talk.on(gesture) == Some(ButtonEvent::Clicked) {
                log::info!("smarthome: Talk");
            }
        }
        _ => {}
    }
    None
}

/// The device sheet while it is open: ✕ and a tap on the scrim close it, "Show more" adds the
/// diagnostics, the lists scroll; a tap on the card between its parts does nothing.
fn on_device_sheet(sheet: &mut DeviceSheet, devices: &Devices, dash: &Dashboard, node: &str, gesture: Gesture) {
    for (list_node, list) in [("#device/state-list", &mut sheet.state_list), ("#device/info-list", &mut sheet.info_list)] {
        if node == list_node || node.strip_prefix(list_node).is_some_and(|rest| rest.starts_with('/')) {
            let row = node.strip_prefix(list_node).and_then(|r| r.strip_prefix('/')).and_then(|r| r.parse().ok());
            list.on(row, gesture, Instant::now());
            return;
        }
    }
    match node {
        "#device/close" => {
            if sheet.close.on(gesture) == Some(ButtonEvent::Clicked) {
                sheet.container.close();
            }
        }
        "#device/more" => {
            if sheet.more.on(gesture) == Some(ButtonEvent::Clicked) {
                sheet.toggle_diagnostics(devices, dash);
            }
        }
        // the scrim takes taps only while the sheet is open (TapTarget in the UI model)
        "#device/scrim" if matches!(gesture, Gesture::Up { inside: true, .. }) => sheet.container.close(),
        _ => {}
    }
}

/// The Wi-Fi sheet while it is open.
fn on_sheet(wifi: &mut WifiSheet, osk: &mut KeyboardDock, node: &str, gesture: Gesture) {
    // a tap on the scrim closes the sheet, as for every sheet
    if node == "#wifi/scrim" {
        if matches!(gesture, Gesture::Up { inside: true, .. }) {
            wifi.form.close();
        }
        return;
    }
    // The list: a tap on a row chooses the network (open ones join at once, the others ask
    // for the password); a drag or a fling scrolls it (ui-lib ListView).
    if node == "#wifi/networks" || node.starts_with("#wifi/networks/") {
        // the row under the finger, or none (a touch on the slot between or below the rows)
        let row = node.strip_prefix("#wifi/networks/").and_then(|r| r.parse().ok());
        if let Some(ListEvent::Chosen(row)) = wifi.list.on(row, gesture, Instant::now()) {
            choose(wifi, row);
        }
        return;
    }
    // The form (ui-lib) owns the field and the focus and types through the screen's keyboard;
    // the sheet decides what an edit or Done means.
    if let Some(event) = wifi.form.on(node, gesture, Instant::now(), &mut osk.keyboard) {
        match event {
            FormEvent::Edited(_) => wifi.password_mut().helper = STATUS_ENTER.into(),
            FormEvent::Submitted(_) => connect(wifi),
            FormEvent::Focused(_) => {}
        }
        return;
    }
    match node {
        "#wifi/connect" => {
            if wifi.connect.on(gesture) == Some(ButtonEvent::Clicked) {
                connect(wifi);
            }
        }
        "#wifi/cancel" => {
            if wifi.cancel.on(gesture) == Some(ButtonEvent::Clicked) {
                match wifi.step {
                    // from the password back to the list (as Android does)
                    Step::Password => wifi.back_to_networks(),
                    // the form closes with its transition
                    Step::Networks => wifi.form.close(),
                }
            }
        }
        "#wifi/rescan" => {
            if wifi.rescan.on(gesture) == Some(ButtonEvent::Clicked) {
                log::info!("smarthome: Wi-Fi rescan");
                house_binder::send_wifi_command(wifi_scan());
            }
        }
        "#wifi/reveal" => {
            if let Some(ToggleEvent::Toggled(shown)) = wifi.reveal.on(gesture) {
                wifi.password_mut().secure = !shown;
            }
        }
        _ => {}
    }
}

/// The Wi-Fi bundle's state of the connection: the sheet shows a failure on the password
/// card and goes back to the list once joined.
fn on_wifi_state(wifi: &mut WifiSheet, wifi_state: &WifiState) {
    wifi.state = Some(wifi_state.clone());
    if !wifi.form.is_open() || wifi.step != Step::Password {
        return;
    }
    match wifi_state.connection.enum_value_or_default() {
        WifiConnection::WIFI_FAILED => {
            let error = if wifi_state.error.is_empty() { "could not join".to_string() } else { wifi_state.error.clone() };
            wifi.password_mut().error = Some(format!("{error}: try again"));
        }
        WifiConnection::WIFI_CONNECTED if wifi.chosen().is_some_and(|n| n.ssid == wifi_state.ssid) => {
            log::info!("smarthome: joined {:?}", wifi_state.ssid);
            wifi.back_to_networks();
        }
        WifiConnection::WIFI_CONNECTING => wifi.password_mut().helper = format!("Joining {}…", wifi_state.ssid),
        _ => {}
    }
}

/// The answer to a connect: a failure shows on the password card (the state says the same
/// when the attempt started; a call that did not get through only says it here). Success
/// needs nothing: the state CONNECTED takes the sheet back to the list.
fn on_wifi_result(wifi: &mut WifiSheet, result: &WifiCommandResult) {
    log::info!("smarthome: Wi-Fi {} {:?}: ok {} {:?}", result.command, result.ssid, result.result.ok, result.result.error);
    if result.command != "connect" || result.result.ok || !wifi.form.is_open() || wifi.step != Step::Password {
        return;
    }
    if wifi.chosen().is_some_and(|n| n.ssid == result.ssid) {
        let error = if result.result.error.is_empty() { "could not join" } else { result.result.error.as_str() };
        wifi.password_mut().error = Some(format!("{error}: try again"));
    }
}

/// A network was tapped in the list: a saved or an open one is joined at once, the others
/// ask for the password.
fn choose(wifi: &mut WifiSheet, row: usize) {
    let Some(network) = wifi.networks.get(row).cloned() else { return };
    if network.connected {
        return;
    }
    if network.security.secured() && !network.saved {
        log::info!("smarthome: Wi-Fi network {:?} chosen, asking for the password", network.ssid);
        wifi.list.select(Some(row));
        wifi.step = Step::Password;
    } else {
        log::info!("smarthome: joining {:?}", network.ssid);
        wifi.list.select(Some(row));
        house_binder::send_wifi_command(wifi_connect(&network.ssid, "", network.security));
    }
}

/// Connect with the password typed: the Wi-Fi bundle joins, the state comes back.
fn connect(wifi: &mut WifiSheet) {
    let len = wifi.password().text.chars().count();
    if len < MIN_LEN {
        // the field turns red until the next key
        wifi.password_mut().error = Some(format!("At least {MIN_LEN} characters ({len} so far)"));
        return;
    }
    let Some(network) = wifi.chosen().cloned() else { return };
    log::info!("smarthome: joining {:?} with a password of {len} characters", network.ssid);
    house_binder::send_wifi_command(wifi_connect(&network.ssid, &wifi.password().text, network.security));
    wifi.password_mut().helper = format!("Joining {}…", network.ssid);
}

/// The command for the Wi-Fi bundle.
fn wifi_connect(ssid: &str, psk: &str, security: Security) -> WifiCommand {
    let mut connect = WifiConnect::new();
    connect.ssid = ssid.to_string();
    connect.psk = psk.to_string();
    connect.security = security.into_proto().into();
    let mut command = WifiCommand::new();
    command.set_connect(connect);
    command
}

/// A scan: the Wi-Fi bundle answers on the networks topic.
pub fn wifi_scan() -> WifiCommand {
    let mut command = WifiCommand::new();
    command.set_scan(WifiScan::new());
    command
}
