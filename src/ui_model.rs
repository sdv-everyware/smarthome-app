/*
 * smarthome-app: the UI model, one field per Figma layer the app writes to or taps.
 * Layer names: Figma file "Smart HOme ui" (key in vendor/sdv-everyware/everyware/scripts/figma-key);
 * the library components (#button, #input-field, #keyboard, ...) come from
 * vendor/sdv-everyware/ui-lib/design/components-patch.js, #wifi from keyboard-patch.js,
 * the dashboard's slots and the texts the app sets from dashboard-patch.js.
 */

use customizations::render_customizations::DisplayState;
use customizations::{Customizer, DesignDocument, DesignDocumentRoot, UpdatePolicy};
use derive_customizations::DesignDocument;
use har_platform_api::looper::DisplayId;
use har_ui_model_derive::HarUiModel;
use har_ui_model_derive::NoDesignTokensOrVariables;
use customizations::render_customizations::List;
use customizations::AnimationOverride;
use everyware_ui::{FollowOpacityCustomizer, FollowPositionCustomizer, Meter, Position, ScreenCustomizer, ScreenView, Scrolled, ScrolledCustomizer, Slot, SlotCustomizer, TapTarget, TapTargetCustomizer, Variant};
use customizations::render_customizations::RenderCustomization;
use customizations::{CustomizerOutput, ExternalImageContent, ImageContentData};
use std::collections::HashMap;
use std::time::Duration;

/// Zone tag of the camera tile: the content tag a Wayland client names in har_zone, not the
/// layer's name. A harry client built for a zone binds to it (its display params, zone_tag).
pub const ZONE_CAMERA_TAG: &str = "#zone/camera";
/// Zone tag of the panel's browser (#panel/browser, experimental, built with the cfg browser):
/// with it every ordinary Wayland app (xdg-shell, the browser) binds to it (main.rs,
/// BridgeConfig::xdg_default_tag).
#[cfg(browser)]
pub const ZONE_BROWSER_TAG: &str = "#zone/browser";

/// Turns a node into a zone: every ~33 ms it re-applies ExternalImage(ZONE_CAMERA_TAG), so
/// harry puts an ExternalImage entry in the display list, the looper tells the Wayland bridge
/// the scene is visible and the bridge composites the client's frames there. With false the
/// node keeps its design fill.
#[derive(Clone, Debug, Default)]
pub struct ZoneCustomizer {}

impl Customizer<bool> for ZoneCustomizer {
    fn format(&mut self, enabled: bool) -> CustomizerOutput {
        zone(ZONE_CAMERA_TAG, enabled)
    }
}

/// The panel's browser (#panel/browser) as a zone, ZoneCustomizer's twin for ZONE_BROWSER_TAG.
/// The presenter turns it on only while the panel is open: without the tag in the display list
/// the looper reports the scene hidden and the browser gets no frame callbacks (idle).
#[cfg(browser)]
#[derive(Clone, Debug, Default)]
pub struct BrowserZoneCustomizer {}

#[cfg(browser)]
impl Customizer<bool> for BrowserZoneCustomizer {
    fn format(&mut self, enabled: bool) -> CustomizerOutput {
        zone(ZONE_BROWSER_TAG, enabled)
    }
}

/// A node as the zone `tag`, or its design fill.
fn zone(tag: &str, enabled: bool) -> CustomizerOutput {
    if enabled {
        let image = RenderCustomization::ExternalImage(ExternalImageContent::new(tag));
        // --zone-redraw. timer (default): every 33 ms, as the cluster's ego car (harry-app-core
        // EgoCarCustomizer). It costs ~90 % of a core with an idle zone (the whole dashboard
        // redrawn ~30 times a second). frames: applied once (new_finished), the bridge's
        // client frames drive the redraws and an idle zone costs ~9 %. Tried once-only
        // 2026-11-07 on the RB3 and Cog seemed to ignore touch (synthetic input only), so
        // timer stays the default until a finger test says otherwise.
        match crate::config::zone_redraw() {
            crate::config::ZoneRedraw::Timer => {
                CustomizerOutput::new_update_in(image, Duration::from_millis(33))
            }
            crate::config::ZoneRedraw::Frames => CustomizerOutput::new_finished(image),
        }
    } else {
        CustomizerOutput::new_finished(RenderCustomization::Display(DisplayState::Visible))
    }
}

/// A picture the app made for a plain frame of the design (the agenda's QR code), or none.
pub type Picture = Option<ImageContentData>;

/// A picture on its frame, or without one the frame out of its column (display gone: the
/// texts under it move up). harry draws a picture on a plain frame only.
#[derive(Clone, Debug, Default)]
pub struct PictureCustomizer {}

impl Customizer<Picture> for PictureCustomizer {
    fn format(&mut self, picture: Picture) -> CustomizerOutput {
        match picture {
            Some(image) => image.into(),
            None => CustomizerOutput::new_finished(RenderCustomization::Display(DisplayState::Gone)),
        }
    }
}

/// First value of a UI model field, before the presenter runs.
trait Initial {
    fn initial() -> Self;
}

impl Initial for String {
    fn initial() -> Self {
        String::new()
    }
}

impl Initial for () {
    fn initial() -> Self {}
}

impl Initial for DisplayState {
    fn initial() -> Self {
        DisplayState::Visible
    }
}

impl Initial for Variant {
    fn initial() -> Self {
        Variant("state=closed".to_string())
    }
}

impl Initial for AnimationOverride {
    fn initial() -> Self {
        AnimationOverride::DisableAnimation
    }
}

impl Initial for TapTarget {
    fn initial() -> Self {
        TapTarget(false)
    }
}

impl Initial for Picture {
    fn initial() -> Self {
        None
    }
}

impl Initial for Slot {
    fn initial() -> Self {
        List::new(Vec::new())
    }
}

/// The fields as `name: type = "layer" [interactive] [["Customizer"]] [= initial];`: one line per
/// layer, and new() gives every field its initial value (the type's, or the one given: a
/// variant whose set has more properties than the library's `state`). A field may have
/// attributes before it (`#[cfg(browser)]`): they go on the field and on its initial value.
/// The type is a single token (`tt`, not `ty`): a `ty` fragment reaches the derive macros
/// wrapped in an invisible group, which DesignDocument does not accept as a field type.
macro_rules! ui_model {
    (@init $ty:tt) => { <$ty as Initial>::initial() };
    (@init $ty:tt, $init:expr) => { $init };
    ($($(#[$attr:meta])* $field:ident: $ty:tt = $node:literal $($interactive:ident)? $([$customizer:literal])? $(= $init:expr)?;)*) => {
        #[derive(DesignDocument, Debug, HarUiModel, NoDesignTokensOrVariables)]
        #[DesignDocument(root)]
        pub struct UiModel {
            customizers: UiModelCustomizers,
            display_map: HashMap<DisplayId, String>,
            $($(#[$attr])* #[Design(node = $node $(, $interactive)? $(, customizer = $customizer)?)] pub $field: $ty,)*
        }

        impl UiModel {
            pub fn new(display_map: HashMap<DisplayId, String>) -> Self {
                UiModel {
                    customizers: Default::default(),
                    display_map,
                    $($(#[$attr])* $field: ui_model!(@init $ty $(, $init)?),)*
                }
            }
        }
    };
}

ui_model! {
    title: String = "#home/title";
    date: String = "#home/date";
    time: String = "#home/time";
    sec: String = "#home/sec";

    // status row: library components in their slots (everyware_ui): harry clones the
    // component into the slot and customizes the clone on its own; taps come by the slot's name
    presence: Slot = "#status/presence" ["SlotCustomizer"];
    wifi_indicator: Slot = "#status/wifi" ["SlotCustomizer"];

    // climate
    climate_minus: Slot = "#climate/minus" ["SlotCustomizer"];
    climate_plus: Slot = "#climate/plus" ["SlotCustomizer"];
    climate_setpoint: String = "#climate/setpoint";
    climate_target: String = "#climate/target";
    humidity_adjust: Slot = "#climate/humidity-adjust" ["SlotCustomizer"];

    // camera, rooms, door, settings, intercom
    camera_open: Slot = "#camera/open" ["SlotCustomizer"];
    entryway_light: Slot = "#entryway/light" ["SlotCustomizer"];
    entryway_state: String = "#entryway/state";
    bedroom_dimmer: Slot = "#bedroom/dimmer" ["SlotCustomizer"];
    bedroom_level: String = "#bedroom/level";
    office_fan: Slot = "#office/fan" ["SlotCustomizer"];
    office_state: String = "#office/state";
    // the tiles' headers: a tap opens the device's sheet (#device)
    _climate_header: () = "#climate/header" interactive;
    _entryway_header: () = "#entryway/header" interactive;
    _bedroom_header: () = "#bedroom/header" interactive;
    _office_header: () = "#office/header" interactive;
    door_details: Slot = "#door/details" ["SlotCustomizer"];
    settings: Slot = "#settings" ["SlotCustomizer"];
    // The header's tabs and the screens behind them (design/screens-patch.js; ui-lib tabs.rs,
    // screens.rs). Rooms and Automations are instances of sets with state=hidden/shown: shown
    // once, then the screens' effect fades (or moves) each screen; at opacity 0 a screen is not
    // drawn and takes no taps.
    home_tabs: Slot = "#home/tabs" ["SlotCustomizer"];
    overview: ScreenView = "#overview" ["ScreenCustomizer"] = ScreenView { opacity: Meter::new(100.0, None), x: None };
    rooms: ScreenView = "#rooms" ["ScreenCustomizer"] = ScreenView::default();
    rooms_variant: Variant = "#rooms" = Variant("state=shown".to_string());
    automations: ScreenView = "#automations" ["ScreenCustomizer"] = ScreenView::default();
    automations_variant: Variant = "#automations" = Variant("state=shown".to_string());
    // --wayland-zones: the tile's picture shows another harry process (a zone). A frame, not a
    // rectangle (dashboard-patch.js): harry draws an external image only on a plain frame. The
    // scrim and the caption with #camera/open are drawn after it, on top of the client's frames.
    camera_zone: bool = "#camera/feed" ["ZoneCustomizer"] = crate::config::wayland_zones();
    talk: Slot = "#intercom/talk" ["SlotCustomizer"];

    // weather (Open-Meteo): the widget in place of the energy one
    weather_place: String = "#weather/place";
    weather_icon: Slot = "#weather/icon" ["SlotCustomizer"];
    weather_temp: String = "#weather/temp";
    weather_condition: String = "#weather/condition";
    weather_details: String = "#weather/details";
    weather_chart: Slot = "#weather/chart" ["SlotCustomizer"];
    weather_t1: String = "#weather/t1";
    weather_t2: String = "#weather/t2";
    weather_t3: String = "#weather/t3";

    // The Wi-Fi sheet: #wifi, the library's #sheet (design/keyboard-patch.js) with the
    // content component #wifi-content swapped in. The app never sets a sheet's variant: harry
    // would take the variant from the Library and lose the instance's names and content. It
    // moves #wifi/card from behind the edge and fades #wifi/scrim (display-only, ui-lib
    // sheet.rs). The step is the content's variant, switched without an animation.
    wifi_step: Variant = "#wifi/content" = Variant("step=networks".to_string());
    wifi_step_animation: AnimationOverride = "#wifi/content";
    wifi_scrim: Meter = "#wifi/scrim" ["FollowOpacityCustomizer"] = Meter::new(0.0, None);
    wifi_scrim_taps: TapTarget = "#wifi/scrim" ["TapTargetCustomizer"];
    wifi_card: Position = "#wifi/card" ["FollowPositionCustomizer"] = Position::new(crate::state::WIFI_LAYOUT.closed(crate::state::SCREEN), None);
    _wifi_card_tap: () = "#wifi/card" interactive;
    wifi_password: Slot = "#wifi/password" ["SlotCustomizer"];
    wifi_connect: Slot = "#wifi/connect" ["SlotCustomizer"];
    wifi_cancel: Slot = "#wifi/cancel" ["SlotCustomizer"];
    // the networks step: the list of #network rows and Rescan; the chosen name on the password card
    // interactive: a drag may start between the rows or below the last one
    wifi_networks: Slot = "#wifi/networks" interactive ["SlotCustomizer"];
    // the right column: the agenda (agenda.rs), its rows (a drag may start between them) and
    // their scrollbar; over them the card that links the calendar, shown while none is linked
    // (opacity 0 in the design), its QR code a picture the app makes
    agenda: Slot = "#agenda/list" interactive ["SlotCustomizer"];
    agenda_scrollbar: Slot = "#agenda/scrollbar" ["SlotCustomizer"];
    agenda_link: Meter = "#agenda/link" ["FollowOpacityCustomizer"] = Meter::new(0.0, None);
    agenda_qr: Picture = "#agenda/qr" ["PictureCustomizer"];
    agenda_link_title: String = "#agenda/link-title";
    agenda_link_hint: String = "#agenda/link-hint";
    agenda_status: String = "#agenda/status";
    // the header's button that opens the calendar's link in the event's panel, while one is linked
    agenda_account: Slot = "#agenda/account" ["SlotCustomizer"];

    // An agenda event's panel: #event, the library's #sheet (design/event-patch.js) with
    // #event-content in it, at the right edge; the card and the scrim moved by the app, as #prefs.
    // A part with no text is out of the body's column (display gone); the body moves up as it
    // scrolls (a fling stepped frame by frame, ui-lib Scrolled), the drag starts on #event/scroll.
    event_scrim: Meter = "#event/scrim" ["FollowOpacityCustomizer"] = Meter::new(0.0, None);
    event_scrim_taps: TapTarget = "#event/scrim" ["TapTargetCustomizer"];
    event_card: Position = "#event/card" ["FollowPositionCustomizer"] = Position::new(crate::agenda_card::EVENT_LAYOUT.closed(crate::state::SCREEN), None);
    _event_card_tap: () = "#event/card" interactive;
    event_close: Slot = "#event/close" ["SlotCustomizer"];
    event_title: String = "#event/title";
    event_when: String = "#event/when";
    event_place: String = "#event/place";
    event_place_display: DisplayState = "#event/place";
    event_link: String = "#event/link";
    event_link_display: DisplayState = "#event/link";
    event_description: String = "#event/description";
    event_description_display: DisplayState = "#event/description";
    event_body: Scrolled = "#event/body" ["ScrolledCustomizer"] = Scrolled::default();
    _event_scroll_tap: () = "#event/scroll" interactive;
    // The panel's step is the content's variant (step=details, qr, confirm, calendar), switched
    // without an animation, as #wifi/content. The details: Accept, Maybe and Decline in the body's
    // column under the place, out of it (display gone) when the event takes no answer; the
    // bottom row Join on phone and Directions (a row that closes up), Delete at the right.
    event_step: Variant = "#event/content" = Variant("step=details".to_string());
    event_step_animation: AnimationOverride = "#event/content";
    event_rsvp: Slot = "#event/rsvp" ["SlotCustomizer"];
    event_rsvp_display: DisplayState = "#event/rsvp";
    event_join: Slot = "#event/join" ["SlotCustomizer"];
    event_join_display: DisplayState = "#event/join";
    event_directions: Slot = "#event/directions" ["SlotCustomizer"];
    event_directions_display: DisplayState = "#event/directions";
    event_delete: Slot = "#event/delete" ["SlotCustomizer"];
    event_delete_display: DisplayState = "#event/delete";
    // what the panel waits for and why a change failed, above the buttons of every step
    event_status: String = "#event/status";
    event_status_display: DisplayState = "#event/status";
    event_error: String = "#event/error";
    event_error_display: DisplayState = "#event/error";
    // the QR step: what the code is for, the code (a picture the app makes), Back
    event_qr_title: String = "#event/qr-title";
    event_qr: Picture = "#event/qr" ["PictureCustomizer"];
    event_back: Slot = "#event/back" ["SlotCustomizer"];
    // the confirmation step: the question, a line (out of the step when empty), Cancel, the action
    event_confirm_title: String = "#event/confirm-title";
    event_confirm_text: String = "#event/confirm-text";
    event_confirm_text_display: DisplayState = "#event/confirm-text";
    event_cancel: Slot = "#event/cancel" ["SlotCustomizer"];
    event_confirm: Slot = "#event/confirm" ["SlotCustomizer"];
    // the calendar's link: the access Google granted in words, Change access, Disconnect
    event_access: String = "#event/access";
    event_change_access: Slot = "#event/change-access" ["SlotCustomizer"];
    event_disconnect: Slot = "#event/disconnect" ["SlotCustomizer"];
    wifi_scrollbar: Slot = "#wifi/scrollbar" ["SlotCustomizer"];
    wifi_rescan: Slot = "#wifi/rescan" ["SlotCustomizer"];
    wifi_ssid: String = "#wifi/ssid";
    wifi_reveal: Slot = "#wifi/reveal" ["SlotCustomizer"];

    // The device sheet: #device, the library's #sheet with #device-content in it, as #wifi. The
    // scrim closes it on a tap, so it takes taps while the sheet is open (closed, it lies
    // transparent over the screen); a tap on the card between its parts stays on the card.
    device_scrim: Meter = "#device/scrim" ["FollowOpacityCustomizer"] = Meter::new(0.0, None);
    device_scrim_taps: TapTarget = "#device/scrim" ["TapTargetCustomizer"];
    device_card: Position = "#device/card" ["FollowPositionCustomizer"] = Position::new(crate::state::DEVICE_LAYOUT.closed(crate::state::SCREEN), None);
    _device_card_tap: () = "#device/card" interactive;
    device_symbol: Slot = "#device/symbol" ["SlotCustomizer"];
    device_title: String = "#device/title";
    device_subtitle: String = "#device/subtitle";
    device_close: Slot = "#device/close" ["SlotCustomizer"];
    device_state: String = "#device/state";
    device_state_detail: String = "#device/state-detail";
    // the lists: a drag may start between the rows
    device_state_list: Slot = "#device/state-list" interactive ["SlotCustomizer"];
    device_state_scrollbar: Slot = "#device/state-list-scrollbar" ["SlotCustomizer"];
    device_info_list: Slot = "#device/info-list" interactive ["SlotCustomizer"];
    device_info_scrollbar: Slot = "#device/info-list-scrollbar" ["SlotCustomizer"];
    device_more: Slot = "#device/more" ["SlotCustomizer"];

    // The panel of the browser (built with the cfg browser): #panel, the library's #sheet
    // (design/panel-patch.js) with #panel-content in it, at the right edge; the card and the
    // scrim moved by the app, as #wifi. Without the cfg the app binds none of it: the closed
    // sheet stays behind the edge.
    #[cfg(browser)]
    panel_scrim: Meter = "#panel/scrim" ["FollowOpacityCustomizer"] = Meter::new(0.0, None);
    #[cfg(browser)]
    panel_scrim_taps: TapTarget = "#panel/scrim" ["TapTargetCustomizer"];
    #[cfg(browser)]
    panel_card: Position = "#panel/card" ["FollowPositionCustomizer"] = Position::new(crate::state::PANEL_LAYOUT.closed(crate::state::SCREEN), None);
    #[cfg(browser)]
    _panel_card_tap: () = "#panel/card" interactive;
    // the title bar: the page's title, close; under it the browser's zone
    #[cfg(browser)]
    panel_title: String = "#panel/title";
    #[cfg(browser)]
    panel_close: Slot = "#panel/close" ["SlotCustomizer"];
    #[cfg(browser)]
    browser_zone: bool = "#panel/browser" ["BrowserZoneCustomizer"] = false;
    // the loader over the browser while the page loads: skeletons sweeping, then a fade
    #[cfg(browser)]
    panel_loading: Meter = "#panel/loading" ["FollowOpacityCustomizer"] = Meter::new(0.0, None);
    #[cfg(browser)]
    panel_skeleton_1: Slot = "#panel/skeleton-1" ["SlotCustomizer"];
    #[cfg(browser)]
    panel_skeleton_2: Slot = "#panel/skeleton-2" ["SlotCustomizer"];
    #[cfg(browser)]
    panel_skeleton_3: Slot = "#panel/skeleton-3" ["SlotCustomizer"];
    #[cfg(browser)]
    panel_skeleton_4: Slot = "#panel/skeleton-4" ["SlotCustomizer"];

    // The settings: #prefs, the library's #sheet (design/prefs-patch.js) with #prefs-content in
    // it, at the right edge; the card and the scrim moved by the app, as #panel.
    prefs_scrim: Meter = "#prefs/scrim" ["FollowOpacityCustomizer"] = Meter::new(0.0, None);
    prefs_scrim_taps: TapTarget = "#prefs/scrim" ["TapTargetCustomizer"];
    prefs_card: Position = "#prefs/card" ["FollowPositionCustomizer"] = Position::new(crate::state::PREFS_LAYOUT.closed(crate::state::SCREEN), None);
    _prefs_card_tap: () = "#prefs/card" interactive;
    prefs_close: Slot = "#prefs/close" ["SlotCustomizer"];
    // the content's two pages, the settings and the time zone list: they slide (state.rs Pages)
    prefs_page_main: ScreenView = "#prefs/page-main" ["ScreenCustomizer"] = ScreenView { opacity: Meter::new(100.0, None), x: None };
    prefs_page_timezone: ScreenView = "#prefs/page-timezone" ["ScreenCustomizer"] = ScreenView::default();
    prefs_back: Slot = "#prefs/back" ["SlotCustomizer"];
    // interactive: a drag may start between the rows
    prefs_zones: Slot = "#prefs/zones" interactive ["SlotCustomizer"];
    prefs_zones_scrollbar: Slot = "#prefs/zones-scrollbar" ["SlotCustomizer"];
    // the rows: a tap target with its value, or a control in a slot
    _prefs_timezone_tap: () = "#prefs/timezone" interactive;
    prefs_timezone: String = "#prefs/timezone/value";
    prefs_hour24: Slot = "#prefs/hour24" ["SlotCustomizer"];
    prefs_units: Slot = "#prefs/units" ["SlotCustomizer"];
    _prefs_wifi_tap: () = "#prefs/wifi" interactive;
    prefs_wifi: String = "#prefs/wifi/value";
    prefs_about: String = "#prefs/about";

    // The screen's keyboard: an instance of #keyboard-dock, the library's keyboard in its slot
    // (labels, pressed keys, taps). The app never sets the dock's variant (a variant switch
    // rebuilds the tree on every frame of the animation): the instance stays closed and the
    // slot moves, as a sheet's card does (ui-lib dock.rs keys_position).
    osk_keys_position: Position = "#osk/keys" ["FollowPositionCustomizer"] = Position::new(everyware_ui::dock::KeyboardDock::new().keys_position(crate::state::SCREEN), None);
    osk_keys: Slot = "#osk/keys" ["SlotCustomizer"];
}

impl DesignDocumentRoot for UiModel {
    fn get_root_node(&self, display_id: u32) -> Option<String> {
        self.display_map.get(&DisplayId::new(display_id)).cloned()
    }
}
