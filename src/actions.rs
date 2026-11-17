/*
 * smarthome-app: actions, the only way to change the state.
 */

use crate::clock::Clock;
use crate::screen_binder::{self, MatterCommandResult, WifiCommandResult};
use crate::devices::{DeviceId, Devices, Reported};
use crate::state::{Dashboard, DeviceSheet, GamePanel, Network, PrefsSheet, PrefsStep, Security, State, Step, WifiSheet, HOUR_CYCLE_24, SETPOINT, STATUS_ENTER, TEMPERATURE, TIMEZONE};
use user_preferences_api::setting::setting::Value;
use everyware_catalog::wifi::{WifiCommand, WifiConnect, WifiConnection, WifiNetworks, WifiScan, WifiState};
use harry::side_effects::SideEffects;
use harry::state::StateAction;
use everyware_ui::{part_of, row_of};
use everyware_ui::button::{ButtonEvent, IconButton};
use everyware_ui::dropdown::DropdownEvent;
use everyware_ui::dock::KeyboardDock;
use everyware_ui::form::FormEvent;
use everyware_ui::list_view::ListEvent;
use everyware_ui::segmented::SegmentedEvent;
use everyware_ui::slider::SliderEvent;
use everyware_ui::toggle::ToggleEvent;
use everyware_ui::Gesture;
use everyware_catalog::matter::MatterDevices;
use everyware_catalog::matter_service::{MatterCommand, MatterMoveToLevel, MatterSetFanMode, MatterSetOnOff, MatterSetSetpoint};
use everyware_catalog::calendar::Agenda;
use everyware_catalog::weather::Weather;
use std::time::Instant;

/// WPA2/WPA3 passphrases are at least 8 characters.
const MIN_LEN: usize = 8;

#[derive(Clone, Debug, PartialEq)]
pub enum HomeAction {
    /// Once a second from the ticker thread in main.rs.
    Tick(Clock),
    /// A gesture on a node (ui_event_listener.rs, everyware_ui::Input).
    Gesture(String, Gesture),
    /// The weather of the house, from the dashboard bundle (screen_binder.rs).
    Weather(Weather),
    /// The agenda and the state of the calendar's link, from the calendar bundle
    /// (screen_binder.rs).
    Agenda(Agenda),
    /// The dashboard bundle subscribed to the screen's commands, or its process died
    /// (screen_binder.rs).
    HouseLink(bool),
    /// The networks around, from the Wi-Fi bundle (screen_binder.rs).
    WifiNetworks(WifiNetworks),
    /// The Wi-Fi connection, from the Wi-Fi bundle (screen_binder.rs).
    WifiState(WifiState),
    /// The Wi-Fi bundle's answer to a command of the screen (screen_binder.rs).
    WifiResult(WifiCommandResult),
    /// The Matter devices of the house, from the Matter bundle (screen_binder.rs).
    MatterDevices(MatterDevices),
    /// The Matter bundle's answer to a command of the screen (screen_binder.rs).
    MatterResult(MatterCommandResult),
    /// A settings group of the device with its values in force (user_preferences).
    Settings(user_preferences_api::change_notifier::OnSettingsChangeRequest),
    /// How the page in the panel's browser is doing (browser.rs events).
    #[cfg(browser)]
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
                // the game's dialog follows a game that ends or starts while it is open
                if state.game.sheet.is_open() {
                    state.game.follow(crate::game::running());
                }
                #[cfg(browser)]
                browser_timeout(state);
            }
            #[cfg(browser)]
            HomeAction::Browser(event) => {
                // the page replaced the old one (or failed, and Cog shows why): the loader goes
                if state.browser.is_loading() {
                    log::info!("smarthome: browser: {event:?} after {:?}", state.browser.loading_since().map(|t| t.elapsed()).unwrap_or_default());
                    state.browser.shown();
                }
            }
            HomeAction::Weather(weather) => state.weather = Some(weather.clone()),
            HomeAction::Agenda(agenda) => state.agenda.set(agenda.clone(), &state.prefs.timezone, state.prefs.hour24.value, jiff::Timestamp::now()),
            HomeAction::HouseLink(up) => state.house_link = *up,
            HomeAction::WifiNetworks(networks) => {
                state.wifi.set_networks(networks.networks.iter().map(Network::from).collect());
            }
            HomeAction::WifiState(wifi_state) => on_wifi_state(&mut state.wifi, wifi_state),
            HomeAction::Settings(group) => state.prefs.on_settings(group),
            HomeAction::WifiResult(result) => on_wifi_result(&mut state.wifi, result),
            HomeAction::MatterDevices(report) => {
                let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs() as i64);
                let reported = state.devices.update(report, now);
                follow_devices(&mut state.dash, &mut state.matter_pending, &reported);
            }
            HomeAction::MatterResult(result) => on_matter_result(&mut state.matter_pending, &state.devices, result),
            #[cfg(browser)]
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
            HomeAction::Gesture(node, gesture) if state.prefs.sheet.is_open() => on_prefs(state, node, *gesture),
            HomeAction::Gesture(node, gesture) if state.game.sheet.is_open() => on_game(&mut state.game, node, *gesture),
            HomeAction::Gesture(node, gesture) if state.agenda.event.is_open() => on_event(&mut state.agenda.event, node, *gesture),
            HomeAction::Gesture(node, gesture) if node == "#settings" && !state.device.container.is_open() && !state.wifi.form.is_open() => {
                // Settings opens the settings sheet
                if clicked(&mut state.dash.settings, *gesture) {
                    state.prefs.open();
                }
            }
            HomeAction::Gesture(node, gesture) if node == "#status/game" && crate::config::game() && !state.device.container.is_open() && !state.wifi.form.is_open() => {
                // the game button opens the game; the game goes on where it paused
                if clicked(&mut state.dash.game, *gesture) {
                    state.game.open(crate::game::running(), &state.sizes);
                }
            }
            HomeAction::Gesture(node, gesture) if state.device.container.is_open() => {
                on_device_sheet(&mut state.device, &state.devices, &state.dash, node, *gesture)
            }
            HomeAction::Gesture(node, gesture) if DeviceId::of_header(node).is_some() => {
                // a tile's header opens its device's sheet
                let index = DeviceId::ALL.iter().position(|d| d.header() == node).expect("of_header found it");
                if state.dash.headers[index].on(*gesture) {
                    state.device.open(DeviceId::ALL[index], &state.devices, &state.dash, &state.heights);
                }
            }
            HomeAction::Gesture(node, gesture) if node == "#status/wifi" => {
                // the Wi-Fi indicator opens the sheet; the sheet starts over every time
                if state.dash.wifi.on(*gesture) == Some(ButtonEvent::Clicked) && !state.wifi.form.is_open() {
                    state.wifi = WifiSheet::opened(state.wifi.networks.clone(), state.wifi.state.clone(), state.heights.wifi);
                    screen_binder::send_wifi_command(wifi_scan());
                }
            }
            HomeAction::Gesture(node, gesture) if state.wifi.form.is_open() => {
                on_sheet(&mut state.wifi, &mut state.osk, node, *gesture)
            }
            HomeAction::Gesture(node, gesture) if node == "#agenda/list" || node.starts_with("#agenda/list/") => {
                // a drag or a fling scrolls the agenda; a tap on an event opens its panel
                state.agenda.on_list(row_of("#agenda/list", node), *gesture, Instant::now());
            }
            HomeAction::Gesture(node, gesture) if part_of("#home/tabs", node).is_some() => {
                on_tabs(state, node, *gesture);
            }
            HomeAction::Gesture(node, gesture) => {
                if let Some(control) = on_dashboard(&mut state.dash, node, *gesture) {
                    send_control(&mut state.matter_pending, &state.devices, control);
                }
            }
        }
        // the screen's keyboard follows the focus (every form of the screen, or'ed)
        state.osk.follow(state.wifi.form.focused().is_some());
        // the agenda's rows follow the clock (a new minute), the zone and the clock's format
        state.agenda.follow(&state.prefs.timezone, state.prefs.hour24.value, jiff::Timestamp::now());
        // an open sheet shows the device as it is now (the dashboard or a bundle changed it)
        if state.device.container.is_open() {
            state.device.refresh(&state.devices, &state.dash);
        }
    }
}

/// The header's tabs: `#home/tabs/#tabs/tab-<i>`, 1-based; the screens follow the choice
/// (presenter.rs).
fn on_tabs(state: &mut State, node: &str, gesture: Gesture) {
    let tab = part_of("#home/tabs", node).and_then(|p| p.strip_prefix("#tabs/tab-")).and_then(|n| n.parse::<usize>().ok());
    if let Some(i) = tab.and_then(|n| n.checked_sub(1)) {
        state.tabs.on(i, gesture);
    }
}

/// The settings sheet while it is open: the scrim and close close it, the rows act, the card
/// takes the rest. A switch shows the new value at once and asks the System bundle for it (SDV
/// user_preferences); the value the bundle applied comes back with the settings and replaces it.
fn on_prefs(state: &mut State, node: &str, gesture: Gesture) {
    let prefs = &mut state.prefs;
    let scrim = node == "#prefs/scrim" && matches!(gesture, Gesture::Up { inside: true, .. });
    if scrim || (node == "#prefs/close" && clicked(&mut prefs.close, gesture)) {
        prefs.sheet.close();
        return;
    }
    if prefs.step() == PrefsStep::TimeZone {
        on_zones(prefs, node, gesture);
        return;
    }
    match node {
        "#prefs/timezone" => {
            if prefs.timezone_row.on(gesture) {
                prefs.open_zones(state.heights.zones);
            }
        }
        "#prefs/hour24" => {
            if let Some(ToggleEvent::Toggled(on)) = prefs.hour24.on(gesture) {
                log::info!("smarthome: 24-hour clock {on}");
                ask(prefs.change(HOUR_CYCLE_24, Value::Bool(on)));
            }
        }
        "#prefs/wifi" => {
            if prefs.wifi_row.on(gesture) {
                // the settings make way for the Wi-Fi sheet, which starts over every time
                prefs.sheet.close();
                state.wifi = WifiSheet::opened(state.wifi.networks.clone(), state.wifi.state.clone(), state.heights.wifi);
                screen_binder::send_wifi_command(wifi_scan());
            }
        }
        _ => {
            // the unit's segments, "#prefs/units/#segmented/segment-<i>"
            if let Some(i) = part_of("#prefs/units", node)
                .and_then(|p| p.strip_prefix("#segmented/segment-"))
                .and_then(|n| n.parse::<usize>().ok())
                .and_then(|n| n.checked_sub(1))
            {
                if let Some(SegmentedEvent::Selected(_)) = prefs.units.on(i, gesture) {
                    log::info!("smarthome: temperature in {}", prefs.units.value());
                    ask(prefs.change(TEMPERATURE, Value::Enum(i as i32)));
                }
            }
        }
    }
}

/// The time zone list (as Car Settings' picker): a tap on a zone shows it at once, asks the
/// System bundle for it and goes back to the settings; Back goes back without a change; a drag
/// or a fling scrolls the list (ui-lib ListView).
fn on_zones(prefs: &mut PrefsSheet, node: &str, gesture: Gesture) {
    if node == "#prefs/back" {
        if prefs.back.on(gesture) == Some(ButtonEvent::Clicked) {
            prefs.show(PrefsStep::Main);
        }
        return;
    }
    if node == "#prefs/zones" || node.starts_with("#prefs/zones/") {
        let row = node.strip_prefix("#prefs/zones/").and_then(|r| r.parse().ok());
        if let Some(ListEvent::Chosen(row)) = prefs.zone_list.on(row, gesture, Instant::now()) {
            if let Some(id) = prefs.zone_at(row) {
                log::info!("smarthome: time zone {id}");
                prefs.timezone = id.to_string();
                ask(prefs.change(TIMEZONE, Value::Blob(id.as_bytes().to_vec())));
                prefs.show(PrefsStep::Main);
            }
        }
    }
}

/// A settings change to the dashboard bundle; none while the group has not come yet.
fn ask(request: Option<user_preferences_api::user_preferences_management_service::RequestSettingsChangeRequest>) {
    match request {
        Some(request) => {
            screen_binder::send_settings_change(request);
        }
        None => log::warn!("smarthome: the settings are not there yet, the change stays on the screen"),
    }
}

/// A page that never reports (no events FIFO, a stuck load): the browser shows what there is.
#[cfg(browser)]
fn browser_timeout(state: &mut State) {
    if state.browser.loading_since().is_some_and(|t| t.elapsed() > crate::state::PAGE_TIMEOUT) {
        log::warn!("smarthome: browser: no word from the page in {:?}, showing it", crate::state::PAGE_TIMEOUT);
        state.browser.shown();
    }
}

/// An event's panel while it is open: the scrim and close close it, a drag on the body scrolls
/// it, the card takes the rest.
fn on_event(panel: &mut crate::agenda_card::EventPanel, node: &str, gesture: Gesture) {
    let scrim = node == "#event/scrim" && matches!(gesture, Gesture::Up { inside: true, .. });
    if scrim || (node == "#event/close" && clicked(&mut panel.close, gesture)) {
        panel.close();
    } else if node == "#event/scroll" {
        panel.on_body(gesture, Instant::now());
    }
}

fn clicked(button: &mut IconButton, gesture: Gesture) -> bool {
    button.on(gesture) == Some(ButtonEvent::Clicked)
}

/// The game's dialog while it is open: the scrim and close close it and let every key go; a
/// key's button holds its key from the finger's down until it lifts or slides off, and a thumb
/// that slides across the pad holds the direction it is on (game.rs Pad). Two fingers hold two
/// keys when each reaches the panel as a touch of its own (not verified on the board: harry's
/// evdev reader reports one finger per input frame). The card takes the rest. Closed, the zone
/// is hidden and the game pauses.
fn on_game(game: &mut GamePanel, node: &str, gesture: Gesture) {
    let scrim = node == "#game/scrim" && matches!(gesture, Gesture::Up { inside: true, .. });
    let changes = if scrim || (node == "#game/close" && clicked(&mut game.close, gesture)) {
        game.close()
    } else {
        match (crate::game::Pad::index(node), node) {
            (Some(i), _) => game.pad.on(i, gesture),
            (None, "#game/fire") => {
                game.fire.on(gesture);
            }
            (None, "#game/use") => {
                game.use_key.on(gesture);
            }
            _ => return,
        }
        let keys = game.keys();
        game.held.follow(keys)
    };
    if let Err(e) = crate::game::send(&changes) {
        if !game.unsent {
            log::warn!("smarthome: game: the keys did not reach the game: {e} (is it running?)");
            game.unsent = true;
        }
    }
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
    use everyware_catalog::matter_service::matter_command::Command;
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
    if screen_binder::send_matter_command(message) {
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
    if result.result.ok {
        log::info!("smarthome: Matter {} node {:#x} ep{}: done", result.command, result.node_id, result.endpoint);
    } else {
        log::warn!("smarthome: Matter {} node {:#x} ep{}: {}", result.command, result.node_id, result.endpoint, result.result.error);
    }
    for (i, b) in devices.bindings.iter().enumerate() {
        if b.is_some_and(|b| b.node_id == result.node_id && b.endpoint == result.endpoint) {
            pending[i] = None;
        }
    }
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
                screen_binder::send_wifi_command(wifi_scan());
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
        screen_binder::send_wifi_command(wifi_connect(&network.ssid, "", network.security));
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
    screen_binder::send_wifi_command(wifi_connect(&network.ssid, &wifi.password().text, network.security));
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
