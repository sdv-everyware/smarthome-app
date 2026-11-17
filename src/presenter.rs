/*
 * smarthome-app: state -> UI model, after every action.
 */

use everyware_catalog::weather::Weather;
use everyware_ui::symbol::Symbol;
use customizations::render_customizations::DisplayState;
use everyware_ui::{list, scrollbar, slot, Component, Meter, Position, ScreenView, Scrolled, TapTarget, Variant};
use crate::weather::{bars, words, Picture};
use crate::state::{PrefsStep, State, PREFS_EFFECT, SCREEN, SCREENS, SCREEN_EFFECT};
use everyware_ui::dock::KEYBOARD_DOCK_H;
use crate::ui_model::UiModel;
use harry::presentation::Presenter;

pub struct UiModelPresenter;

impl Presenter<State, UiModel> for UiModelPresenter {
    fn present(model: &mut UiModel, state: &State) {
        model.title = "Hello, Smarthome".to_string();
        model.date = state.clock.date.clone();
        model.time = state.clock.time_text(state.prefs.hour24.value);
        model.sec = state.clock.sec_text(state.prefs.hour24.value);

        // dashboard: each control in its slot
        let d = &state.dash;
        model.presence = slot("#status/presence", &d.presence);
        model.wifi_indicator = slot("#status/wifi", &d.wifi);
        model.game_button = slot("#status/game", &d.game);
        model.game_button_display = if crate::config::game() { DisplayState::Visible } else { DisplayState::Gone };
        model.climate_minus = slot("#climate/minus", &d.temp_minus);
        model.climate_plus = slot("#climate/plus", &d.temp_plus);
        model.climate_setpoint = format!("{}°", d.setpoint);
        model.climate_target = format!("Target · {}° indoors", d.setpoint);
        model.humidity_adjust = slot("#climate/humidity-adjust", &d.humidity_adjust);
        model.camera_open = slot("#camera/open", &d.open_feed);
        present_camera(model, state);
        model.entryway_light = slot("#entryway/light", &d.light);
        model.entryway_state = if d.light.value { "On" } else { "Off" }.to_string();
        model.bedroom_dimmer = slot("#bedroom/dimmer", &d.dimmer);
        model.bedroom_level = format!("{}%", d.dimmer.value.round());
        model.office_fan = slot("#office/fan", &d.fan);
        model.office_state = format!("On · {}", d.fan.value());
        model.door_details = slot("#door/details", &d.door_details);
        model.settings = slot("#settings", &d.settings);
        model.talk = slot("#intercom/talk", &d.talk);

        // the header's tabs and the screen of the chosen tab, with the screens' effect
        model.home_tabs = slot("#home/tabs", &state.tabs);
        let screen = |i: usize| {
            let x = state.sizes.frame(SCREENS[i].1).map_or(0.0, |f| f.x);
            ScreenView::new(state.tabs.screen(i, SCREEN_EFFECT, SCREEN.0), x)
        };
        model.overview = screen(0);
        model.rooms = screen(1);
        model.automations = screen(2);

        present_weather(model, state.weather.as_ref(), state.house_link, state.prefs.units.value() == "°F");

        // the agenda (agenda.rs): the days' rows and their scrollbar, how old the agenda is when it is
        // not fresh; while no calendar is linked the card over them says so, with the setup page's
        // QR code when there is one and the house's data is linked
        let agenda = &state.agenda;
        let now = jiff::Timestamp::now();
        model.agenda = list("#agenda/list", &agenda.list);
        model.agenda_scrollbar = scrollbar("#agenda/scrollbar", &agenda.list);
        model.agenda_status = crate::agenda::status(agenda.agenda.as_ref(), state.house_link, now, &agenda.time_zone(), state.prefs.hour24.value);
        let card = crate::agenda::link_card(agenda.agenda.as_ref(), state.house_link);
        model.agenda_link = Meter::new(if card.is_some() { 100.0 } else { 0.0 }, None);
        model.agenda_qr = card.as_ref().and_then(|c| c.qr.as_ref()).and_then(|_| agenda.qr_picture());
        let card = card.unwrap_or(crate::agenda::LinkCard { title: String::new(), hint: String::new(), qr: None });
        model.agenda_link_title = card.title;
        model.agenda_link_hint = card.hint;

        // while the keyboard dock is open the dialogs centre in the screen above it
        let free_height = if state.osk.open() { SCREEN.1 - KEYBOARD_DOCK_H } else { SCREEN.1 };
        let wifi = &state.wifi;
        // the card and the scrim move with the form's transition (ui-lib sheet.rs; the sheet's
        // variant is never set, see ui_model.rs); the step is the content's variant
        let slide = wifi.form.transition();
        model.wifi_step = Variant(wifi.step_variant());
        // the card hugs the step's content: its height from the design puts the card in the middle
        let sheet = wifi.form.sheet.with_content_height(state.sizes.height("#wifi-content", &wifi.step_variant()));
        model.wifi_card = Position::new(sheet.card_position(SCREEN, free_height), slide);
        model.wifi_scrim = Meter::new(wifi.form.sheet.scrim(), slide);
        model.wifi_scrim_taps = TapTarget(wifi.form.is_open());
        model.wifi_ssid = wifi.chosen().map(|n| n.ssid.clone()).unwrap_or_default();
        model.wifi_networks = list("#wifi/networks", &wifi.list);
        model.wifi_scrollbar = scrollbar("#wifi/scrollbar", &wifi.list);
        model.wifi_rescan = slot("#wifi/rescan", &wifi.rescan);
        model.wifi_password = slot("#wifi/password", wifi.password());
        model.wifi_connect = slot("#wifi/connect", &wifi.connect);
        model.wifi_cancel = slot("#wifi/cancel", &wifi.cancel);
        model.wifi_reveal = slot("#wifi/reveal", &wifi.reveal);

        // the device sheet: the card slides and the scrim fades with the sheet's transition
        // (no variant change, no new tree)
        let device = &state.device;
        let slide = device.container.transition();
        let container = device.container.with_content_height(state.sizes.height("#device-content", "state=default"));
        model.device_card = Position::new(container.card_position(SCREEN, free_height), slide);
        model.device_scrim = Meter::new(device.container.scrim(), slide);
        model.device_scrim_taps = TapTarget(device.container.is_open());
        model.device_symbol = slot("#device/symbol", &Symbol::new(device.view.symbol));
        model.device_title = device.view.name.clone();
        model.device_subtitle = device.view.subtitle.clone();
        model.device_close = slot("#device/close", &device.close);
        model.device_state = device.view.state.clone();
        model.device_state_detail = device.view.detail.clone();
        model.device_state_list = list("#device/state-list", &device.state_list);
        model.device_state_scrollbar = scrollbar("#device/state-list-scrollbar", &device.state_list);
        model.device_info_list = list("#device/info-list", &device.info_list);
        model.device_info_scrollbar = scrollbar("#device/info-list-scrollbar", &device.info_list);
        model.device_more = slot("#device/more", &device.more);

        #[cfg(browser)]
        present_browser(model, state, free_height);

        // the settings: as the panel, the card from behind the right edge and the scrim
        let prefs = &state.prefs;
        let slide = prefs.sheet.transition();
        model.prefs_card = Position::new(prefs.sheet.card_position(SCREEN, free_height), slide);
        model.prefs_scrim = Meter::new(prefs.sheet.scrim(), slide);
        model.prefs_scrim_taps = TapTarget(prefs.sheet.is_open());
        model.prefs_close = slot("#prefs/close", &prefs.close);
        // the pages lie side by side in the design: each slides by the first one's width from
        // the first one's place
        let first = state.sizes.part("#prefs-content", "state=default", "#prefs/page-main");
        let (x, width) = first.map_or((0.0, 0.0), |f| (f.x, f.width));
        let page = |step: PrefsStep| ScreenView::new(prefs.pages.page(step as usize, PREFS_EFFECT, width), x);
        model.prefs_page_main = page(PrefsStep::Main);
        model.prefs_page_timezone = page(PrefsStep::TimeZone);
        model.prefs_back = slot("#prefs/back", &prefs.back);
        model.prefs_zones = list("#prefs/zones", &prefs.zone_list);
        model.prefs_zones_scrollbar = scrollbar("#prefs/zones-scrollbar", &prefs.zone_list);
        model.prefs_timezone = prefs.timezone.clone();
        model.prefs_hour24 = slot("#prefs/hour24", &prefs.hour24);
        model.prefs_units = slot("#prefs/units", &prefs.units);
        model.prefs_wifi = match state.wifi.state.as_ref() {
            Some(link) if !link.ssid.is_empty() => link.ssid.clone(),
            _ => "Not connected".to_string(),
        };
        model.prefs_about = prefs.about.clone();

        // an agenda event's panel: as the settings, the card from behind the right edge and the
        // scrim; an empty part out of the column; the body where the finger scrolled it
        let event = &state.agenda.event;
        let slide = event.sheet.transition();
        model.event_card = Position::new(event.sheet.card_position(SCREEN, free_height), slide);
        model.event_scrim = Meter::new(event.sheet.scrim(), slide);
        model.event_scrim_taps = TapTarget(event.is_open());
        model.event_close = slot("#event/close", &event.close);
        let d = &event.details;
        let shown = |text: &str| if text.is_empty() { DisplayState::Gone } else { DisplayState::Visible };
        model.event_title = d.title.clone();
        model.event_when = d.when.clone();
        (model.event_place, model.event_place_display) = (d.place.clone(), shown(&d.place));
        (model.event_link, model.event_link_display) = (d.link.clone(), shown(&d.link));
        (model.event_description, model.event_description_display) = (d.description.clone(), shown(&d.description));
        model.event_body = Scrolled::new(0.0, &event.body);

        present_game(model, state, free_height);

        // the screen's keyboard: the slot with the keys moves with the dock (ui-lib dock.rs)
        model.osk_keys_position = Position::new(state.osk.keys_position(SCREEN), state.osk.transition());
        model.osk_keys = slot("#osk/keys", &state.osk.keyboard);
    }
}

/// The weather widget. Before the first message it says so; the chart and the icon keep the
/// design's preview until then. Without the house link the values stay and the details line
/// says the data is not live.
fn present_weather(model: &mut UiModel, weather: Option<&Weather>, house_link: bool, fahrenheit: bool) {
    // the settings' unit (°C or °F); the bundle sends °C
    let t = |c: f64| if fahrenheit { c * 9.0 / 5.0 + 32.0 } else { c };
    let Some(w) = weather else {
        model.weather_place = String::new();
        model.weather_temp = "–".to_string();
        model.weather_condition = "Waiting for the weather".to_string();
        model.weather_details = if house_link { "from the weather bundle" } else { "no link to the house's data" }.to_string();
        return;
    };
    model.weather_place = w.place.clone();
    model.weather_icon = slot("#weather/icon", &Picture::of(w.code));
    model.weather_temp = format!("{:.0}°", t(w.temperature_c));
    model.weather_condition = words(w.code).to_string();
    model.weather_details = if house_link {
        format!("Feels {:.0}° · {:.0} % · {:.0} km/h", t(w.apparent_c), w.humidity_percent, w.wind_kmh)
    } else {
        format!("Last reading {} · no link to the house's data", w.time)
    };
    let chart = bars(w.hourly.iter().map(|h| (h.temperature_c, h.precipitation_percent)));
    model.weather_chart = slot("#weather/chart", &chart);
    // the times under the bars: the first, the middle and the last hour
    let label = |i: usize| w.hourly.get(i).map(|h| format!("{:02}:00", h.hour)).unwrap_or_default();
    model.weather_t1 = label(0);
    model.weather_t2 = label(12);
    model.weather_t3 = label(23);
}

/// The camera (camera.rs): the tile's zone, chip and offline state, and the full view with its
/// zone while it is open.
fn present_camera(model: &mut UiModel, state: &State) {
    let camera = &state.camera;
    let shown = |b: bool| if b { DisplayState::Visible } else { DisplayState::Gone };
    // camera-app draws the zones while it is connected; without it the panel says offline
    let zones = crate::config::wayland_zones() && camera.zones();
    model.camera_zone = zones;
    model.camera_live = shown(camera.link.on);
    model.camera_offline = shown(!zones);
    // the view slides with the library's overlay transitions; its zone stays on (camera.rs)
    model.camera_view = Position::new((0.0, camera.y()), camera.overlay.transition());
    model.camera_view_zone = zones;
    model.camera_view_offline = shown(!zones);
    model.camera_view_live = shown(camera.link.on);
    model.camera_view_close = slot("#camera-view/close", &camera.close);
}

/// The game's dialog: the card from below, the scrim, the zone of the game's picture while the
/// dialog is open (closed, the game pauses), the keys' buttons.
fn present_game(model: &mut UiModel, state: &State, free_height: f32) {
    let game = &state.game;
    let slide = game.sheet.transition();
    // the card hugs the content: its height from the design puts the card in the middle
    let sheet = game.sheet.with_content_height(state.sizes.height("#game-content", "state=default"));
    model.game_card = Position::new(sheet.card_position(SCREEN, free_height), slide);
    model.game_scrim = Meter::new(game.sheet.scrim(), slide);
    model.game_scrim_taps = TapTarget(game.sheet.is_open());
    model.game_close = slot("#game/close", &game.close);
    model.game_status = game.status.clone();
    // the zone only while a game runs: a game that ended leaves no frozen frame over the status
    model.game_zone = crate::config::wayland_zones() && game.sheet.is_open() && game.reach.runs();
    let [up, down, left, right] = &game.pad.buttons;
    model.game_up = slot("#game/up", up);
    model.game_down = slot("#game/down", down);
    model.game_left = slot("#game/left", left);
    model.game_right = slot("#game/right", right);
    model.game_fire = slot("#game/fire", &game.fire);
    model.game_use = slot("#game/use", &game.use_key);
}

/// The browser's panel (built with the cfg browser): the card from behind its edge, the scrim
/// fading, with the sheet's transition; the browser's zone and its loader.
#[cfg(browser)]
fn present_browser(model: &mut UiModel, state: &State, free_height: f32) {
    use everyware_ui::transitions::{Change, ComponentKind};
    let panel = &state.panel;
    let slide = panel.transition();
    model.panel_card = Position::new(panel.card_position(SCREEN, free_height), slide);
    model.panel_scrim = Meter::new(panel.scrim(), slide);
    model.panel_scrim_taps = TapTarget(panel.is_open());
    let browser = &state.browser;
    model.panel_title = browser.title();
    model.panel_close = slot("#panel/close", &browser.close);
    // the browser's zone only once its page shows: while it loads Cog gets no frame
    // callbacks and draws nothing, the loader stands in its place (no white flash)
    model.browser_zone = crate::config::wayland_zones() && panel.is_open() && !browser.is_loading();
    model.panel_loading = if browser.is_loading() {
        Meter::new(100.0, None)
    } else {
        Meter::new(0.0, everyware_ui::transitions::transition(ComponentKind::Skeleton, Change::Hide))
    };
    model.panel_skeleton_1 = slot("#panel/skeleton-1", &browser.skeletons[0]);
    model.panel_skeleton_2 = slot("#panel/skeleton-2", &browser.skeletons[1]);
    model.panel_skeleton_3 = slot("#panel/skeleton-3", &browser.skeletons[2]);
    model.panel_skeleton_4 = slot("#panel/skeleton-4", &browser.skeletons[3]);
}
