/*
 * smarthome-app: application state. Only the reducer thread changes it (actions.rs).
 */

use everyware_ui::button::{Button, ButtonKind, Icon, IconButton, IconButtonKind};
use everyware_ui::dropdown::Dropdown;
use everyware_ui::dock::KeyboardDock;
use everyware_ui::form::Form;
use everyware_ui::input_field::InputField;
use everyware_ui::list_item::{ListItem, ListItemKind, ListItemLines, ListItemSize};
use everyware_ui::list_view::ListView;
use everyware_ui::design::{Frame, Sizes};
use everyware_ui::sheet::{Edge, Layout, Sheet};
use everyware_ui::pressable::Pressable;
use protobuf::MessageField;
use user_preferences_api::change_notifier::OnSettingsChangeRequest;
use user_preferences_api::setting::setting::Value;
use user_preferences_api::setting::Setting;
use user_preferences_api::settings_group::SettingsGroupId;
use user_preferences_api::user_preferences_management_service::RequestSettingsChangeRequest;
use everyware_ui::segmented::Segmented;
#[cfg(browser)]
use everyware_ui::skeleton::{Skeleton, SkeletonKind, SkeletonSize};
use everyware_ui::screens::{Effect, Pages};
use everyware_ui::slider::Slider;
use everyware_ui::tabs::Tabs;
use everyware_ui::toggle::Toggle;
use crate::devices::{DeviceId, DeviceView, Devices};
use everyware_catalog::weather::Weather;
use crate::clock::Clock;
use everyware_catalog::wifi::{WifiNetwork, WifiSecurity, WifiState};

#[derive(Debug, Default)]
pub struct State {
    /// The local time, set by the ticker every second (clock.rs).
    pub clock: Clock,
    pub wifi: WifiSheet,
    /// The screen's keyboard: open while a field (the sheet's password) has the focus.
    pub osk: KeyboardDock,
    pub dash: Dashboard,
    /// What is known of the devices beyond the dashboard's state (the Matter bundle's).
    pub devices: Devices,
    /// The sheet of a device (#device), opened from its tile's header.
    pub device: DeviceSheet,
    /// The panel (#panel, an instance of the library's #sheet): the browser with a title bar,
    /// built with the cfg browser. Nothing on the dashboard opens it. Its layout is PANEL_LAYOUT
    /// (State::new).
    #[cfg(browser)]
    pub panel: Sheet,
    /// The settings (#prefs, the library's #sheet at the right edge), opened by Settings.
    pub prefs: PrefsSheet,
    /// The game (#game, the library's #sheet as a dialog), opened by the status row's game
    /// button: the game's picture and its keys (game.rs).
    pub game: GamePanel,
    /// The camera (camera.rs): camera-app's link and the full view (#camera-view), opened by the
    /// tile's open button.
    pub camera: crate::camera::CameraView,
    /// What the panel's browser shows (browser.rs).
    #[cfg(browser)]
    pub browser: BrowserPanel,
    /// The agenda in the dashboard's right column (#agenda, agenda_card.rs) and its event panel.
    pub agenda: crate::agenda_card::AgendaCard,
    /// The design's sizes (ui-lib design.rs): a dialog's card is as high as its content.
    pub sizes: Sizes,
    /// The outdoor weather from the weather bundle (screen_binder.rs); None until the first
    /// message.
    pub weather: Option<Weather>,
    /// The dashboard bundle is linked: its command sink is subscribed and its process alive.
    pub house_link: bool,
    /// A Matter command in flight per tile (DeviceId::ALL's order): its control keeps what the
    /// finger set until the device answered (actions.rs follow_devices).
    pub matter_pending: [Option<std::time::Instant>; 4],
    /// The header's tabs (#home/tabs): which screen the panel shows (SCREENS).
    pub tabs: Tabs,
    /// The lists' heights, from the design.
    pub heights: ListHeights,
}

impl State {
    /// The state at the start: everything default, the panel with the app's layout, the
    /// design's sizes.
    pub fn new(sizes: Sizes) -> Self {
        let prefs = PrefsSheet { about: about(), timezone: system_time_zone(), ..Default::default() };
        let heights = ListHeights::of(&sizes);
        let mut state = State { prefs, sizes, heights, tabs: Tabs::new(SCREENS.map(|(label, _)| label)), ..Default::default() };
        #[cfg(browser)]
        {
            state.panel = Sheet::new(PANEL_LAYOUT);
        }
        state.agenda = crate::agenda_card::AgendaCard::new(heights.agenda, heights.event_body, agenda_measure(), crate::zones::time_zone);
        state.prefs.build_zones(heights.zones);
        state
    }
}

/// The lists' heights as the design draws their slots (ui-lib Sizes): a list is as high as its
/// slot, so a list the designer makes higher scrolls and shows its scrollbar that high, and the
/// code types no height. A slot the design lacks gives no height (a warning in the log).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ListHeights {
    /// The agenda (#agenda/list, on the dashboard) and its event panel's body (#event/scroll, in
    /// #event-content).
    pub agenda: f32,
    pub event_body: f32,
    /// The Wi-Fi networks (#wifi/networks, in #wifi-content step=networks).
    pub wifi: f32,
    /// The device sheet's two lists (#device/state-list, #device/info-list, in #device-content).
    pub device_state: f32,
    pub device_info: f32,
    /// The time zones (#prefs/zones, in #prefs-content).
    pub zones: f32,
}

impl ListHeights {
    /// The heights of the design `sizes` holds.
    pub fn of(sizes: &Sizes) -> Self {
        let height = |name: &str, frame: Option<Frame>| match frame {
            Some(f) => f.height,
            None => {
                log::warn!("smarthome: the design has no {name}: the list has no height");
                0.0
            }
        };
        ListHeights {
            agenda: height("#agenda/list", sizes.frame("#agenda/list")),
            event_body: height("#event/scroll", sizes.part("#event-content", "state=default", "#event/scroll")),
            wifi: height("#wifi/networks", sizes.part("#wifi-content", "step=networks", "#wifi/networks")),
            device_state: height("#device/state-list", sizes.part("#device-content", "state=default", "#device/state-list")),
            device_info: height("#device/info-list", sizes.part("#device-content", "state=default", "#device/info-list")),
            zones: height("#prefs/zones", sizes.part("#prefs-content", "state=default", "#prefs/zones")),
        }
    }
}

/// The screens behind the header's tabs, in the tabs' order: the label and the screen's node.
pub const SCREENS: [(&str, &str); 3] = [("Overview", "#overview"), ("Rooms", "#rooms"), ("Automations", "#automations")];
/// How the screens change on a tap on a tab (ui-lib screens.rs).
pub const SCREEN_EFFECT: Effect = Effect::Fade;

/// The screen, for the sheets' geometry (ui-lib sheet.rs).
pub const SCREEN: (f32, f32) = (1920.0, 1080.0);
/// How each sheet shows (ui-lib sheet.rs): a dialog in the middle, or half the screen along an
/// edge. The design has a variant for every layout.
pub const WIFI_LAYOUT: Layout = Layout::Dialog;
pub const DEVICE_LAYOUT: Layout = Layout::Dialog;
#[cfg(browser)]
pub const PANEL_LAYOUT: Layout = Layout::Panel(Edge::Right);
/// The settings' layout: a panel at the right edge, as the browser's panel.
pub const PREFS_LAYOUT: Layout = Layout::Panel(Edge::Right);
/// The game's layout: a dialog in the middle.
pub const GAME_LAYOUT: Layout = Layout::Dialog;
/// The System bundle's settings the sheet shows (everyware bundles/system): the group `time`
/// with the time zone (an IANA name in UTF-8 bytes) and the clock's format, the group `units`
/// with the unit of temperatures (an enum: the index of the sheet's segment, °C 0, °F 1).
/// The names are the System bundle's (everyware bundles/system/src/lib.rs, same constants); a
/// changed name must change there too. The messages are Google's, no proto of ours:
/// system/software_defined_vehicle/automotive_services/user_preferences/vsidl/v1/
/// (change_notifier.proto, user_preferences_management_service.proto, setting.proto).
pub const TIMEZONE: &str = "timezone";
pub const HOUR_CYCLE_24: &str = "hour_cycle_24";
pub const TEMPERATURE: &str = "temperature";

/// The time zone list's rows (design/prefs-patch.js #prefs/zones): 72 high, 8 between them.
const ZONE_PITCH: f32 = 80.0;
/// Rows above the current zone when the list opens, so it shows in the middle of the list.
const ZONES_ABOVE: usize = 4;

/// What the settings sheet shows: its pages in the design's order (#prefs/page-main,
/// #prefs/page-timezone).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PrefsStep {
    /// The settings.
    #[default]
    Main = 0,
    /// The time zone list.
    TimeZone = 1,
}

/// How the settings' pages change: one page deeper slides in from the right, Back slides it
/// out (Android's Settings, Material 3's shared axis).
pub const PREFS_EFFECT: Effect = Effect::Slide;

/// The settings sheet (#prefs, design/prefs-patch.js): the time zone, the clock's format, the
/// weather's unit, the way to the Wi-Fi sheet and what the panel is. The values the system
/// applies are the System bundle's (SDV user_preferences); the sheet shows the ones in force.
#[derive(Debug)]
pub struct PrefsSheet {
    pub sheet: Sheet,
    /// Which page shows and which showed before (PrefsStep's order).
    pub pages: Pages,
    pub close: IconButton,
    /// The time zone's row (#prefs/timezone) and the name in force (#prefs/timezone/value): the
    /// System bundle's, the system's until its settings came; the row opens the time zone list.
    pub timezone_row: Pressable,
    pub timezone: String,
    /// The time zone list (the page PrefsStep::TimeZone): back to the settings, the zones
    /// (zones.rs: read at the start, again when the list opens after a change of daylight
    /// saving time) and their rows, the zone in force selected.
    pub back: Button,
    pub zones: crate::zones::Zones,
    pub zone_list: ListView,
    /// The clock shows 24 hours (#prefs/hour24).
    pub hour24: Toggle,
    /// The weather's unit (#prefs/units): °C or °F, applied by the presenter.
    pub units: Segmented,
    /// The Wi-Fi row (#prefs/wifi): opens the Wi-Fi sheet.
    pub wifi_row: Pressable,
    /// What the panel is (#prefs/about), read once at the start (about()).
    pub about: String,
    /// The settings groups that came from user_preferences, by name, with their ids: a change
    /// goes back with the id of its group.
    pub groups: std::collections::BTreeMap<String, SettingsGroupId>,
}

impl Default for PrefsSheet {
    fn default() -> Self {
        PrefsSheet {
            sheet: Sheet::new(PREFS_LAYOUT),
            pages: Pages::default(),
            close: IconButton::new(IconButtonKind::Tonal, Icon::Close),
            timezone_row: Pressable::default(),
            timezone: String::new(),
            back: Button::new(ButtonKind::Ghost, "‹ Back").compact(),
            zones: Default::default(),
            zone_list: ListView::new([], 0.0, ZONE_PITCH),
            hour24: Toggle::new(true),
            units: Segmented::new(["°C", "°F"]),
            wifi_row: Pressable::default(),
            about: String::new(),
            groups: Default::default(),
        }
    }
}

/// The rows of the time zone list: the place and the offset, as Car Settings shows them.
fn zone_rows(zones: &crate::zones::Zones) -> impl Iterator<Item = ListItem> + '_ {
    zones.list.iter().map(|z| {
        ListItem::new(ListItemKind::Plain, z.place.clone(), z.detail.clone()).lines(ListItemLines::Two).size(ListItemSize::Panel)
    })
}

impl PrefsSheet {
    /// Opens the sheet; until the System bundle's settings came, with the system's time zone.
    pub fn open(&mut self) {
        if !self.groups.contains_key("time") {
            self.timezone = system_time_zone();
        }
        self.pages.reset();
        self.sheet.open();
    }

    /// The page that shows.
    pub fn step(&self) -> PrefsStep {
        if self.pages.selected == PrefsStep::TimeZone as usize { PrefsStep::TimeZone } else { PrefsStep::Main }
    }

    /// Slides to a page.
    pub fn show(&mut self, step: PrefsStep) {
        self.pages.show(step as usize);
    }

    /// Builds the time zone list, `height` high (the design's), with the offsets of now and the
    /// zone in force marked. Done once at the start: the rows are in harry's tree from the first
    /// frame, so a tap on the time zone only slides the list in. Rows that appear with the tap
    /// make harry build its whole tree and layout again in that frame (about 160 ms on the
    /// RB3), which cut the slide's start off.
    pub fn build_zones(&mut self, height: f32) {
        self.zones = crate::zones::zones();
        self.zone_list = ListView::new(zone_rows(&self.zones), height, ZONE_PITCH);
        self.mark_zone();
    }

    /// Reads the zones again when their offsets changed since they were read (daylight saving
    /// time, or the clock was set): the same rows with the new offsets and order. Rows of the
    /// same variant only change their texts, which harry sets in the tree it keeps.
    fn refresh_zones(&mut self) {
        if self.zones.hold_at(jiff::Timestamp::now()) {
            return;
        }
        self.zones = crate::zones::zones();
        self.zone_list.set_rows(zone_rows(&self.zones));
    }

    /// Marks the zone in force in the list and returns the row the list opens on; a zone the
    /// list lacks is not marked, the list opens on its offset (zones::opening_row).
    fn mark_zone(&mut self) -> usize {
        let (selected, row) = crate::zones::opening_row(&self.zones.list, &self.timezone);
        self.zone_list.select(selected);
        row
    }

    /// Shows the time zone list, the zone in force marked and in view, with the offsets of now.
    pub fn open_zones(&mut self, height: f32) {
        if self.zones.list.is_empty() {
            self.build_zones(height);
        } else {
            self.refresh_zones();
        }
        let row = self.mark_zone();
        self.zone_list.scroll_to(row.saturating_sub(ZONES_ABOVE));
        self.show(PrefsStep::TimeZone);
    }

    /// The zone of a row of the list.
    pub fn zone_at(&self, row: usize) -> Option<&'static str> {
        self.zones.list.get(row).map(|z| z.id)
    }

    /// A settings group with its values in force: the sheet shows them, a switch the user just
    /// flipped included (the owner applied it, or refused and sent the old value). A change
    /// user_preferences still holds for an owner that is not there (pending) is shown as made.
    pub fn on_settings(&mut self, group: &OnSettingsChangeRequest) {
        let Some(id) = group.settings_group_id.as_ref().cloned() else { return };
        for setting in group.active_settings.iter().chain(&group.pending_changes) {
            match (setting.key.as_str(), &setting.value) {
                (TIMEZONE, Some(Value::Blob(name))) => self.timezone = String::from_utf8_lossy(name).into_owned(),
                (HOUR_CYCLE_24, Some(Value::Bool(on))) => self.hour24.set(*on),
                (TEMPERATURE, Some(Value::Enum(unit))) if (0..=1).contains(unit) => self.units.select(*unit as usize),
                (key, value) => log::debug!("smarthome: setting {key} = {value:?} not shown"),
            }
        }
        self.groups.insert(id.name.clone(), id);
    }

    /// The request for one setting of the group it belongs to; None before the group came.
    pub fn change(&self, key: &str, value: Value) -> Option<RequestSettingsChangeRequest> {
        let group = match key {
            TIMEZONE | HOUR_CYCLE_24 => "time",
            TEMPERATURE => "units",
            _ => return None,
        };
        let id = self.groups.get(group)?.clone();
        Some(RequestSettingsChangeRequest {
            settings_group_id: MessageField::some(id),
            settings: vec![Setting { key: key.to_string(), value: Some(value), ..Default::default() }],
            ..Default::default()
        })
    }
}

/// The game's dialog (#game, design/game-patch.js): the game's picture in the zone
/// #game/picture, a direction pad left of it, Fire and Use right of it, close at the top. A
/// button held is its key down (game.rs); the game pauses itself while the dialog is closed.
#[derive(Debug)]
pub struct GamePanel {
    pub sheet: Sheet,
    pub close: IconButton,
    /// The direction pad: up, down, left, right (game::Key's order), following a sliding thumb.
    pub pad: crate::game::Pad,
    pub fire: Button,
    pub use_key: Button,
    /// The keys the game was told are down.
    pub held: crate::game::Held,
    /// Whether a game reads the keys (game::running): asked when the dialog opens and every
    /// second while it is open. Without one the zone is off: no frozen last frame over the status.
    pub reach: crate::game::Reach,
    /// What shows under the picture until the game's first frame (#game/status).
    pub status: String,
    /// A key could not be sent since the dialog opened (said once in the log).
    pub unsent: bool,
}

impl Default for GamePanel {
    fn default() -> Self {
        GamePanel {
            sheet: Sheet::new(GAME_LAYOUT),
            close: IconButton::new(IconButtonKind::Tonal, Icon::Close),
            pad: crate::game::Pad::default(),
            fire: Button::new(ButtonKind::Primary, "Fire").compact(),
            use_key: Button::new(ButtonKind::Secondary, "Use").compact(),
            held: Default::default(),
            reach: Default::default(),
            status: String::new(),
            unsent: false,
        }
    }
}

impl GamePanel {
    /// The keys as the buttons hold them now, in game::Key's order.
    pub fn keys(&self) -> [(crate::game::Key, bool); 6] {
        use crate::game::Key;
        let [up, down, left, right] = self.pad.held();
        [
            (Key::Up, up),
            (Key::Down, down),
            (Key::Left, left),
            (Key::Right, right),
            (Key::Fire, self.fire.press.pressed),
            (Key::Use, self.use_key.press.pressed),
        ]
    }

    /// Opens the dialog; `reach`: whether a game reads the keys (game::running). The pad's
    /// buttons take their places on the screen in the open card (`sizes`: the design's).
    pub fn open(&mut self, reach: crate::game::Reach, sizes: &Sizes) {
        self.unsent = false;
        self.follow(reach);
        self.sheet.open();
        let card = self.sheet.with_content_height(sizes.height("#game-content", "state=default")).card_position(SCREEN, SCREEN.1);
        let (x0, y0) = (card.0 + everyware_ui::sheet::CARD_PAD, card.1 + everyware_ui::sheet::CARD_PAD);
        for (rect, node) in self.pad.rects.iter_mut().zip(crate::game::PAD_NODES) {
            *rect = match sizes.part("#game-content", "state=default", node) {
                Some(f) => everyware_ui::sheet::Rect { x: x0 + f.x, y: y0 + f.y, w: f.width, h: f.height },
                None => everyware_ui::sheet::Rect { x: 0.0, y: 0.0, w: 0.0, h: 0.0 },
            };
        }
    }

    /// Whether a game reads the keys now: the zone and the status follow.
    pub fn follow(&mut self, reach: crate::game::Reach) {
        if reach != self.reach || self.status.is_empty() {
            if reach == crate::game::Reach::KeysBlocked {
                log::warn!("smarthome: game: it runs, but the panel may not open its FIFO (SELinux?): no keys reach it");
            }
            self.reach = reach;
            self.status = crate::game::status(reach).to_string();
        }
    }

    /// Closes the dialog: every button lets go. Returns the keys to send up.
    pub fn close(&mut self) -> Vec<(crate::game::Key, bool)> {
        self.pad.release();
        self.fire.on(everyware_ui::Gesture::Cancel);
        self.use_key.on(everyware_ui::Gesture::Cancel);
        self.sheet.close();
        self.held.release()
    }
}

/// The dashboard's controls: library components in the slots of #smarthome-root
/// (design/dashboard-patch.js). Local state until the bundles drive them.
#[derive(Debug)]
pub struct Dashboard {
    /// Status row: who is home, and the Wi-Fi indicator that opens the sheet.
    pub presence: Dropdown,
    pub wifi: IconButton,
    /// Climate: the setpoint in °C between - and +, the humidity button.
    pub temp_minus: IconButton,
    pub temp_plus: IconButton,
    pub setpoint: i32,
    pub humidity_adjust: IconButton,
    /// The camera's open-feed button.
    pub open_feed: IconButton,
    /// Entryway lights on/off, the bedroom dimmer 0..100.
    pub light: Toggle,
    pub dimmer: Slider,
    /// The Office fan's mode.
    pub fan: Segmented,
    /// The tiles' headers (#climate/header, ...), which open their device's sheet: one press
    /// per header, in DeviceId::ALL's order.
    pub headers: [Pressable; 4],
    pub door_details: IconButton,
    pub settings: IconButton,
    pub talk: Button,
    /// The status row's game button, which opens the game (#status/game).
    pub game: IconButton,
}

/// The setpoint's range in °C.
pub const SETPOINT: std::ops::RangeInclusive<i32> = 16..=30;

impl Default for Dashboard {
    fn default() -> Self {
        Dashboard {
            presence: Dropdown::new(["At home", "Away", "Asleep"]),
            wifi: IconButton::new(IconButtonKind::Glass, Icon::Wifi).compact(),
            temp_minus: IconButton::new(IconButtonKind::Tonal, Icon::Minus),
            temp_plus: IconButton::new(IconButtonKind::Tonal, Icon::Plus),
            setpoint: 22,
            humidity_adjust: IconButton::new(IconButtonKind::Tonal, Icon::Sliders).compact(),
            open_feed: IconButton::new(IconButtonKind::Scrim, Icon::Maximize),
            light: Toggle::new(true),
            // The track on the display: the dimmer's slot at x 763 (Smart HOme.fig, Bedroom
            // lights / Dimmer control), the track inset 20, 203 long (#slider).
            dimmer: Slider::new(65.0, 763.0 + 20.0, 203.0),
            fan: Segmented::new(["Low", "Med", "High"]),
            headers: [Pressable::default(); 4],
            door_details: IconButton::new(IconButtonKind::Tonal, Icon::ChevronRight),
            settings: IconButton::new(IconButtonKind::Tonal, Icon::Settings),
            game: IconButton::new(IconButtonKind::Glass, Icon::Gamepad).compact(),
            talk: Button::new(ButtonKind::Primary, "Talk").compact(),
        }
    }
}

/// The Wi-Fi sheet (#wifi) over the dashboard: the networks around, then the password of
/// the chosen one.
#[derive(Debug)]
pub struct WifiSheet {
    /// The sheet: closed or open, with the password field (ui-lib form; the variant and the
    /// transitions are its). The keyboard is the screen's (State::osk).
    pub form: Form,
    /// Which card the sheet shows (the set's own property, after the form's state).
    pub step: Step,
    /// The networks of the last scan, strongest first (the Wi-Fi bundle's).
    pub networks: Vec<Network>,
    /// The connection, as the Wi-Fi bundle last reported it.
    pub state: Option<WifiState>,
    /// The list #wifi/networks: one row (ui-lib list item) per network, scrolling.
    pub list: ListView,
    /// The buttons of the sheet (Figma slots #wifi/connect, #wifi/cancel, #wifi/rescan, #wifi/reveal).
    pub connect: Button,
    pub cancel: Button,
    pub rescan: Button,
    /// Show the password instead of dots.
    pub reveal: Toggle,
}

/// The cards of the Wi-Fi sheet.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Step {
    /// The list of networks.
    Networks,
    /// The password of the chosen network.
    Password,
}

impl Step {
    /// The value of the set's `step` property.
    pub fn name(self) -> &'static str {
        match self {
            Step::Networks => "networks",
            Step::Password => "password",
        }
    }
}

/// How a Wi-Fi network is protected.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Security {
    Open,
    Wpa2,
    Wpa3,
    Enterprise,
}

impl Security {
    /// The name on the row.
    pub fn label(self) -> &'static str {
        match self {
            Security::Open => "Open",
            Security::Wpa2 => "WPA2-Personal",
            Security::Wpa3 => "WPA3-Personal",
            Security::Enterprise => "WPA-Enterprise",
        }
    }

    /// Needs a password.
    pub fn secured(self) -> bool {
        self != Security::Open
    }
}

impl Security {
    /// From the Wi-Fi bundle's message.
    pub fn from_proto(security: WifiSecurity) -> Self {
        match security {
            WifiSecurity::WIFI_SECURITY_OPEN => Security::Open,
            WifiSecurity::WIFI_SECURITY_WPA2 => Security::Wpa2,
            WifiSecurity::WIFI_SECURITY_WPA3 => Security::Wpa3,
            WifiSecurity::WIFI_SECURITY_ENTERPRISE => Security::Enterprise,
        }
    }

    /// For the Wi-Fi bundle's command.
    pub fn into_proto(self) -> WifiSecurity {
        match self {
            Security::Open => WifiSecurity::WIFI_SECURITY_OPEN,
            Security::Wpa2 => WifiSecurity::WIFI_SECURITY_WPA2,
            Security::Wpa3 => WifiSecurity::WIFI_SECURITY_WPA3,
            Security::Enterprise => WifiSecurity::WIFI_SECURITY_ENTERPRISE,
        }
    }
}

/// A Wi-Fi network of the last scan, as the Wi-Fi bundle reports it (topic wifi-networks).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Network {
    pub ssid: String,
    pub security: Security,
    /// 1..=4 bars.
    pub signal: u8,
    pub saved: bool,
    pub connected: bool,
}

impl From<&WifiNetwork> for Network {
    fn from(n: &WifiNetwork) -> Self {
        Network {
            ssid: n.ssid.clone(),
            security: Security::from_proto(n.security.enum_value_or_default()),
            signal: n.signal_bars.clamp(1, 4) as u8,
            saved: n.saved,
            connected: n.connected,
        }
    }
}

impl Network {

    /// The row of the list: the signal as the leading glyph, a lock for a secured network,
    /// the detail line as Android shows it.
    pub fn row(&self) -> ListItem {
        let signal = match self.signal {
            0 | 1 => "wifi-zero",
            2 => "wifi-low",
            3 => "wifi-high",
            _ => "wifi",
        };
        let detail = if self.connected {
            "Connected".to_string()
        } else if self.saved {
            format!("{} · Saved", self.security.label())
        } else {
            self.security.label().to_string()
        };
        ListItem::new(ListItemKind::Icon, self.ssid.clone(), detail)
            .leading(signal)
            .trailing(self.security.secured().then_some("lock"))
            .wide()
    }
}

/// Row height and gap of the list in the design (#list-item 72 high, 8 between).
const ROW_PITCH: f32 = 72.0 + 8.0;


/// The password field's index in the form.
pub const PASSWORD: usize = 0;

impl WifiSheet {
    /// The sheet as the Wi-Fi indicator opens it: empty, nothing in focus. The keyboard comes
    /// up once the password field is tapped.
    /// The sheet as the Wi-Fi indicator opens it, with the networks known so far; a scan is
    /// asked for by the caller.
    pub fn opened(networks: Vec<Network>, state: Option<WifiState>, list_h: f32) -> Self {
        let mut sheet = WifiSheet { list: ListView::new([], list_h, ROW_PITCH), ..WifiSheet::default() };
        sheet.set_networks(networks);
        sheet.state = state;
        sheet.form.open();
        sheet
    }

    /// A new scan: the rows follow, the chosen one stays chosen. The same networks again
    /// (the bundle repeats itself on every event) change nothing.
    pub fn set_networks(&mut self, networks: Vec<Network>) {
        if networks == self.networks {
            return;
        }
        let chosen = self.chosen().map(|n| n.ssid.clone());
        self.list.set_rows(networks.iter().map(Network::row));
        self.networks = networks;
        self.list.select(chosen.and_then(|ssid| self.networks.iter().position(|n| n.ssid == ssid)));
    }

    /// The variant of #wifi/content (#wifi-content): the step. The sheet's own variant is the
    /// form's (`form.sheet`).
    pub fn step_variant(&self) -> String {
        format!("step={}", self.step.name())
    }

    /// The network the user chose (the password card's name).
    pub fn chosen(&self) -> Option<&Network> {
        self.list.selected().and_then(|row| self.networks.get(row))
    }

    /// Back to the list: the password is not kept around.
    pub fn back_to_networks(&mut self) {
        self.step = Step::Networks;
        self.list.select(None);
        self.form.blur();
        self.form.clear();
        let password = self.password_mut();
        password.helper = STATUS_ENTER.into();
        password.secure = true;
        self.reveal = Toggle::new(false);
    }

    /// The password field; its helper line carries the hint and the errors.
    pub fn password(&self) -> &InputField {
        self.form.field(PASSWORD)
    }

    /// The password field, to change.
    pub fn password_mut(&mut self) -> &mut InputField {
        self.form.field_mut(PASSWORD)
    }
}

impl Default for WifiSheet {
    fn default() -> Self {
        let mut password = InputField::new("Password", STATUS_ENTER);
        password.secure = true;
        WifiSheet {
            form: Form::new(WIFI_LAYOUT, [("#wifi/password", password)]),
            step: Step::Networks,
            networks: Vec::new(),
            state: None,
            list: ListView::new([], 0.0, ROW_PITCH),
            connect: Button::new(ButtonKind::Primary, "Connect"),
            cancel: Button::new(ButtonKind::Secondary, "Cancel"),
            rescan: Button::new(ButtonKind::Secondary, "Rescan"),
            reveal: Toggle::new(false),
        }
    }
}

pub const STATUS_ENTER: &str = "Enter the password of the network";


/// The rows of the sheet's two lists (#list-item kind=text, column: 72 high, 8 between).
const DEVICE_ROW_PITCH: f32 = 72.0 + 8.0;

/// The sheet of a device (#device, the library's #sheet with #device-content in it) over the
/// dashboard: the device's state large, its State and Device lists; "Show more" adds the
/// diagnostics.
#[derive(Debug)]
pub struct DeviceSheet {
    /// The sheet itself: closed or open, its layout, the card's place (ui-lib sheet.rs).
    pub container: Sheet,
    /// Whose sheet it is.
    pub device: DeviceId,
    /// What it shows.
    pub view: DeviceView,
    /// The diagnostics are in the Device list.
    pub diagnostics: bool,
    pub state_list: ListView,
    pub info_list: ListView,
    /// The buttons (slots #device/close, #device/more).
    pub close: IconButton,
    pub more: Button,
}

impl Default for DeviceSheet {
    fn default() -> Self {
        DeviceSheet {
            container: Sheet::new(DEVICE_LAYOUT),
            device: DeviceId::Climate,
            view: DeviceView::default(),
            diagnostics: false,
            state_list: ListView::new([], 0.0, DEVICE_ROW_PITCH),
            info_list: ListView::new([], 0.0, DEVICE_ROW_PITCH),
            close: IconButton::new(IconButtonKind::Tonal, Icon::Close),
            more: Button::new(ButtonKind::Ghost, "Show more"),
        }
    }
}

/// A label and its value as a row of the sheet's lists.
fn value_row((label, value): &(String, String)) -> ListItem {
    ListItem::new(ListItemKind::Text, label.clone(), "").lines(ListItemLines::One).size(ListItemSize::Column).trailing_text(value.clone())
}

impl DeviceSheet {
    /// Opens the sheet of `device`, from the top of its lists (as high as `heights` says),
    /// without the diagnostics.
    pub fn open(&mut self, device: DeviceId, devices: &Devices, dash: &Dashboard, heights: &ListHeights) {
        let container = self.container;
        *self = DeviceSheet {
            container,
            device,
            state_list: ListView::new([], heights.device_state, DEVICE_ROW_PITCH),
            info_list: ListView::new([], heights.device_info, DEVICE_ROW_PITCH),
            ..DeviceSheet::default()
        };
        self.refresh(devices, dash);
        self.container.open();
    }

    /// The device's values as they are now (the lists keep their scroll).
    pub fn refresh(&mut self, devices: &Devices, dash: &Dashboard) {
        self.view = devices.view(self.device, dash);
        self.state_list.set_rows(self.view.state_rows.iter().map(value_row));
        let diagnostics = if self.diagnostics { self.view.diagnostic_rows.as_slice() } else { &[] };
        self.info_list.set_rows(self.view.info_rows.iter().chain(diagnostics).map(value_row));
    }

    /// "Show more" adds the diagnostics to the Device list, "Show less" takes them away.
    pub fn toggle_diagnostics(&mut self, devices: &Devices, dash: &Dashboard) {
        self.diagnostics = !self.diagnostics;
        self.more.label = if self.diagnostics { "Show less" } else { "Show more" }.to_string();
        self.refresh(devices, dash);
    }
}

/// The panel's browser: the page it was sent, whether it is still loading, and the title
/// bar's close button (slot #panel/close). The page itself is Cog's (browser.rs); the app knows
/// what it asked for and what Cog reported back (BrowserEvent).
#[cfg(browser)]
#[derive(Debug)]
pub struct BrowserPanel {
    pub page: Option<crate::browser::Page>,
    pub close: IconButton,
    /// The loader over the browser (#panel/loading): skeleton slots #panel/skeleton-1..4, a
    /// heading, text, a picture and text, sweeping while the page loads.
    pub skeletons: [Skeleton; 4],
}

#[cfg(browser)]
impl Default for BrowserPanel {
    fn default() -> Self {
        let wide = |kind| Skeleton::new(kind, SkeletonSize::Wide);
        BrowserPanel {
            page: None,
            close: IconButton::new(IconButtonKind::Tonal, Icon::Close),
            skeletons: [wide(SkeletonKind::Heading), wide(SkeletonKind::Text), wide(SkeletonKind::Media), wide(SkeletonKind::Text)],
        }
    }
}

/// How long the loader waits for the page before it shows whatever the browser has.
#[cfg(browser)]
pub const PAGE_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(15);

#[cfg(browser)]
impl BrowserPanel {
    /// The page shows (Cog committed it, finished, failed, or the loader gave up): the loader
    /// fades out and the browser's zone comes in.
    pub fn shown(&mut self) {
        for s in &mut self.skeletons {
            s.stop();
        }
    }

    /// Whether the loader is up.
    pub fn is_loading(&self) -> bool {
        self.skeletons[0].is_loading()
    }

    /// Since when the loader is up.
    pub fn loading_since(&self) -> Option<std::time::Instant> {
        self.skeletons[0].since
    }

    /// The title bar: the site while loading, the story's title once the page shows.
    pub fn title(&self) -> String {
        match &self.page {
            Some(page) if self.is_loading() => crate::browser::host(&page.url).to_string(),
            Some(page) => page.title.clone(),
            None => String::new(),
        }
    }
}

/// How the agenda measures its texts on the device (text_measure.rs): a headline in its row (the
/// list item's title of kind leading text, size column: Inter Medium 17/22, 342 wide), and the
/// event panel's body (design/event-patch.js: the same sizes, widths and gap).
pub fn agenda_measure() -> crate::agenda_card::Measure {
    use crate::text_measure::{height, lines, Font};
    const HEADLINE: Font = Font { family: "Inter", weight: 500.0, size: 17.0, line_height: 22.0 };
    const HEADLINE_W: f32 = 490.0 - 20.0 - 128.0;
    const CONTENT_W: f32 = 880.0;
    const GAP: f32 = 16.0;
    const TITLE: Font = Font { family: "Inter", weight: 600.0, size: 28.0, line_height: 36.0 };
    const WHEN: Font = Font { family: "Inter", weight: 400.0, size: 19.0, line_height: 26.0 };
    const LINK: Font = Font { family: "Inter", weight: 400.0, size: 17.0, line_height: 24.0 };
    const TEXT: Font = Font { family: "Inter", weight: 400.0, size: 19.0, line_height: 28.0 };
    fn headline_lines(title: &str) -> usize {
        lines(title, &HEADLINE, HEADLINE_W)
    }
    fn body_height(d: &crate::agenda::Details) -> f32 {
        let parts = [(&d.title, TITLE, CONTENT_W - 72.0 - 16.0), (&d.when, WHEN, CONTENT_W), (&d.place, WHEN, CONTENT_W), (&d.link, LINK, CONTENT_W), (&d.description, TEXT, CONTENT_W)];
        // an empty part is out of the column (display gone), with its gap
        let shown: Vec<f32> = parts.iter().filter(|(t, _, _)| !t.is_empty()).map(|(t, font, w)| height(t, font, *w)).collect();
        shown.iter().sum::<f32>() + GAP * shown.len().saturating_sub(1) as f32
    }
    crate::agenda_card::Measure { headline_lines, body_height }
}

/// The system's time zone (persist.sys.timezone; empty: bionic uses UTC).
pub fn system_time_zone() -> String {
    Some(property("persist.sys.timezone")).filter(|tz| !tz.is_empty()).unwrap_or_else(|| "Etc/UTC".to_string())
}

/// What the panel is, for the settings' About: the system's release and build (build props).
pub fn about() -> String {
    let prop = property;
    format!("Android {}, build {}\n{}", prop("ro.build.version.release"), prop("ro.build.id"), prop("ro.product.model"))
}

/// A system property's value, empty when it is not set.
pub(crate) fn property(name: &str) -> String {
    let name = std::ffi::CString::new(name).expect("no NUL in a property name");
    let mut value = [0 as libc::c_char; 92];
    // SAFETY: name is NUL terminated, value has room for PROP_VALUE_MAX bytes; bionic writes a
    // NUL terminated string into it.
    unsafe {
        libc::__system_property_get(name.as_ptr(), value.as_mut_ptr());
        std::ffi::CStr::from_ptr(value.as_ptr()).to_string_lossy().into_owned()
    }
}
