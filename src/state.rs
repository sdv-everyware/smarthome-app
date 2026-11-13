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
use everyware_ui::design::Sizes;
use everyware_ui::sheet::{Edge, Layout, Sheet};
use everyware_ui::pressable::Pressable;
use everyware_ui::segmented::Segmented;
use everyware_ui::skeleton::{Skeleton, SkeletonKind, SkeletonSize};
use everyware_ui::screens::Effect;
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
    /// opened by a news story. Its layout is PANEL_LAYOUT (State::new).
    pub panel: Sheet,
    /// The settings (#prefs, the library's #sheet at the right edge), opened by Settings.
    pub prefs: PrefsSheet,
    /// What the panel's browser shows (browser.rs).
    pub browser: BrowserPanel,
    /// The pages of the news feed's stories, in the list's order (a tap opens one).
    pub stories: Vec<crate::browser::Page>,
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
}

impl State {
    /// The state at the start: everything default, the panel with the app's layout, the
    /// design's sizes.
    pub fn new(sizes: Sizes) -> Self {
        let prefs = PrefsSheet { about: about(), timezone: system_time_zone(), ..Default::default() };
        State { panel: Sheet::new(PANEL_LAYOUT), prefs, sizes, tabs: Tabs::new(SCREENS.map(|(label, _)| label)), ..Default::default() }
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
pub const PANEL_LAYOUT: Layout = Layout::Panel(Edge::Right);
/// The settings' layout: a panel at the right edge, as the news panel.
pub const PREFS_LAYOUT: Layout = Layout::Panel(Edge::Right);
/// The time zones a tap on the row steps through, until the row opens the list of all of them.
pub const TIME_ZONES: [&str; 6] = ["Europe/Berlin", "Europe/London", "America/New_York", "Asia/Tokyo", "Australia/Sydney", "Etc/UTC"];

/// The settings sheet (#prefs, design/prefs-patch.js): the time zone, the clock's format, the
/// weather's unit, the way to the Wi-Fi sheet and what the panel is. The values the system
/// applies are the System bundle's (SDV user_preferences); the sheet shows the ones in force.
#[derive(Debug)]
pub struct PrefsSheet {
    pub sheet: Sheet,
    pub close: IconButton,
    /// The time zone's row (#prefs/timezone) and the name in force (#prefs/timezone/value).
    pub timezone_row: Pressable,
    pub timezone: String,
    /// The clock shows 24 hours (#prefs/hour24).
    pub hour24: Toggle,
    /// The weather's unit (#prefs/units): °C or °F.
    pub units: Segmented,
    /// The Wi-Fi row (#prefs/wifi): opens the Wi-Fi sheet.
    pub wifi_row: Pressable,
    /// What the panel is (#prefs/about), read once at the start (about()).
    pub about: String,
}

impl Default for PrefsSheet {
    fn default() -> Self {
        PrefsSheet {
            sheet: Sheet::new(PREFS_LAYOUT),
            close: IconButton::new(IconButtonKind::Tonal, Icon::Close),
            timezone_row: Pressable::default(),
            timezone: TIME_ZONES[0].to_string(),
            hour24: Toggle::new(true),
            units: Segmented::new(["°C", "°F"]),
            wifi_row: Pressable::default(),
            about: String::new(),
        }
    }
}

impl PrefsSheet {
    /// The next of TIME_ZONES after the one in force (the first if it is not one of them).
    pub fn next_time_zone(&self) -> &'static str {
        let at = TIME_ZONES.iter().position(|z| *z == self.timezone).map_or(0, |i| (i + 1) % TIME_ZONES.len());
        TIME_ZONES[at]
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
    /// The right column: the news feed, a scrolling list (news.rs).
    pub news: ListView,
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
            talk: Button::new(ButtonKind::Primary, "Talk").compact(),
            news: crate::news::empty(),
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

/// Row height and gap of the list in the design (#list-item 72 high, 8 between), and the
/// slot's height (6 rows).
const ROW_PITCH: f32 = 72.0 + 8.0;
const LIST_H: f32 = 6.0 * 72.0 + 5.0 * 8.0;


/// The password field's index in the form.
pub const PASSWORD: usize = 0;

impl WifiSheet {
    /// The sheet as the Wi-Fi indicator opens it: empty, nothing in focus. The keyboard comes
    /// up once the password field is tapped.
    /// The sheet as the Wi-Fi indicator opens it, with the networks known so far; a scan is
    /// asked for by the caller.
    pub fn opened(networks: Vec<Network>, state: Option<WifiState>) -> Self {
        let mut sheet = WifiSheet::default();
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
            list: ListView::new([], LIST_H, ROW_PITCH),
            connect: Button::new(ButtonKind::Primary, "Connect"),
            cancel: Button::new(ButtonKind::Secondary, "Cancel"),
            rescan: Button::new(ButtonKind::Secondary, "Rescan"),
            reveal: Toggle::new(false),
        }
    }
}

pub const STATUS_ENTER: &str = "Enter the password of the network";


/// The rows of the sheet's two lists (#list-item kind=text, column: 72 high, 8 between) and
/// the lists' height (6 rows).
const DEVICE_ROW_PITCH: f32 = 72.0 + 8.0;
const DEVICE_LIST_H: f32 = 6.0 * 72.0 + 5.0 * 8.0;

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
            state_list: ListView::new([], DEVICE_LIST_H, DEVICE_ROW_PITCH),
            info_list: ListView::new([], DEVICE_LIST_H, DEVICE_ROW_PITCH),
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
    /// Opens the sheet of `device`, from the top of its lists, without the diagnostics.
    pub fn open(&mut self, device: DeviceId, devices: &Devices, dash: &Dashboard) {
        let container = self.container;
        *self = DeviceSheet { container, device, ..DeviceSheet::default() };
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
#[derive(Debug)]
pub struct BrowserPanel {
    pub page: Option<crate::browser::Page>,
    pub close: IconButton,
    /// The loader over the browser (#panel/loading): skeleton slots #panel/skeleton-1..4, a
    /// heading, text, a picture and text, sweeping while the page loads.
    pub skeletons: [Skeleton; 4],
}

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
pub const PAGE_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(15);

impl BrowserPanel {
    /// A page was asked for: the loader shows and sweeps, the browser stays hidden.
    pub fn loading(&mut self, page: crate::browser::Page, now: std::time::Instant) {
        self.page = Some(page);
        for s in &mut self.skeletons {
            s.stop();
            s.start(now);
        }
    }

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
fn property(name: &str) -> String {
    let name = std::ffi::CString::new(name).expect("no NUL in a property name");
    let mut value = [0 as libc::c_char; 92];
    // SAFETY: name is NUL terminated, value has room for PROP_VALUE_MAX bytes; bionic writes a
    // NUL terminated string into it.
    unsafe {
        libc::__system_property_get(name.as_ptr(), value.as_mut_ptr());
        std::ffi::CStr::from_ptr(value.as_ptr()).to_string_lossy().into_owned()
    }
}
