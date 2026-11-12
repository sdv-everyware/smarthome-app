/*
 * smarthome-app: state -> UI model, after every action.
 */

use everyware_catalog::weather::Weather;
use everyware_ui::symbol::Symbol;
use everyware_ui::{list, scrollbar, slot, Component, Meter, Position, TapTarget, Variant};
use crate::weather::{bars, words, Picture};
use crate::state::{State, SCREEN};
use everyware_ui::transitions::{self, Change, ComponentKind};
use everyware_ui::dock::KEYBOARD_DOCK_H;
use crate::ui_model::UiModel;
use harry::presentation::Presenter;

pub struct UiModelPresenter;

impl Presenter<State, UiModel> for UiModelPresenter {
    fn present(model: &mut UiModel, state: &State) {
        model.title = "Hello, Smarthome".to_string();
        model.date = state.clock.date.clone();
        model.time = state.clock.time.clone();
        model.sec = state.clock.sec.clone();

        // dashboard: each control in its slot
        let d = &state.dash;
        model.presence = slot("#status/presence", &d.presence);
        model.news = list("#news/list", &d.news);
        model.news_scrollbar = scrollbar("#news/scrollbar", &d.news);
        model.wifi_indicator = slot("#status/wifi", &d.wifi);
        model.climate_minus = slot("#climate/minus", &d.temp_minus);
        model.climate_plus = slot("#climate/plus", &d.temp_plus);
        model.climate_setpoint = format!("{}°", d.setpoint);
        model.climate_target = format!("Target · {}° indoors", d.setpoint);
        model.humidity_adjust = slot("#climate/humidity-adjust", &d.humidity_adjust);
        model.camera_open = slot("#camera/open", &d.open_feed);
        model.camera_image = crate::camera::picture(&model.camera_image, state.camera);
        model.entryway_light = slot("#entryway/light", &d.light);
        model.entryway_state = if d.light.value { "On" } else { "Off" }.to_string();
        model.bedroom_dimmer = slot("#bedroom/dimmer", &d.dimmer);
        model.bedroom_level = format!("{}%", d.dimmer.value.round());
        model.office_fan = slot("#office/fan", &d.fan);
        model.office_state = format!("On · {}", d.fan.value());
        model.door_details = slot("#door/details", &d.door_details);
        model.settings = slot("#settings", &d.settings);
        model.talk = slot("#intercom/talk", &d.talk);

        present_weather(model, state.weather.as_ref(), state.house_link);

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

        // the panel: the card from behind its edge, the scrim fading, with the sheet's transition
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
            Meter::new(0.0, transitions::transition(ComponentKind::Skeleton, Change::Hide))
        };
        model.panel_skeleton_1 = slot("#panel/skeleton-1", &browser.skeletons[0]);
        model.panel_skeleton_2 = slot("#panel/skeleton-2", &browser.skeletons[1]);
        model.panel_skeleton_3 = slot("#panel/skeleton-3", &browser.skeletons[2]);
        model.panel_skeleton_4 = slot("#panel/skeleton-4", &browser.skeletons[3]);

        // the screen's keyboard: the slot with the keys moves with the dock (ui-lib dock.rs)
        model.osk_keys_position = Position::new(state.osk.keys_position(SCREEN), state.osk.transition());
        model.osk_keys = slot("#osk/keys", &state.osk.keyboard);
    }
}

/// The weather widget. Before the first message it says so; the chart and the icon keep the
/// design's preview until then. Without the house link the values stay and the details line
/// says the data is not live.
fn present_weather(model: &mut UiModel, weather: Option<&Weather>, house_link: bool) {
    let Some(w) = weather else {
        model.weather_place = String::new();
        model.weather_temp = "–".to_string();
        model.weather_condition = "Waiting for the weather".to_string();
        model.weather_details = if house_link { "from the weather bundle" } else { "no link to the house's data" }.to_string();
        return;
    };
    model.weather_place = w.place.clone();
    model.weather_icon = slot("#weather/icon", &Picture::of(w.code));
    model.weather_temp = format!("{:.0}°", w.temperature_c);
    model.weather_condition = words(w.code).to_string();
    model.weather_details = if house_link {
        format!("Feels {:.0}° · {:.0} % · {:.0} km/h", w.apparent_c, w.humidity_percent, w.wind_kmh)
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
