/*
 * smarthome-app: display config + command line.
 */

use clap::Parser as _;
use clap_derive::Parser;
use har_platform_api::looper::DisplayId;
use har_types::color_format::ColorFormat;
use std::collections::HashMap;

/// Root frame of the Figma design (1920x1080, the RB3's HDMI mode).
pub const ROOT_NODE: &str = "#smarthome-root";

#[derive(Parser, Debug)]
pub struct CommandLineArgs {
    /// Directory with the document (.dcf).
    #[arg(short, long, default_value = "/data/local/tmp/smarthome-app-data")]
    pub data_path: String,
    /// The design's file in data_path (`<name>.dcf`; a name without the extension is completed).
    #[arg(short = 'i', long, default_value = "smarthome.dcf")]
    pub document_id: String,
    /// A directory checked before data_path: a document there wins (a design pushed to the
    /// device for a test, over the one in the APEX).
    #[arg(long)]
    pub override_path: Option<String>,
    /// Display size "WxH" to map touch positions to, for panels that report their own units
    /// (RB3 HDMI touch: 0..32767). Unset: positions pass through.
    #[arg(long)]
    pub touch_size: Option<String>,
    /// Serve Wayland clients (other harry processes) and show the one bound to
    /// ZONE_CAMERA_TAG in the camera tile. Off: the tile keeps its design picture and no
    /// socket is opened.
    #[arg(long)]
    pub wayland_zones: bool,
    /// What redraws the dashboard for the camera tile's Wayland zone: `timer` every 33 ms (also
    /// when the zone is idle), `frames` once per client frame (an idle zone costs nothing).
    #[arg(long, value_enum, default_value_t = ZoneRedraw::Timer)]
    pub zone_redraw: ZoneRedraw,
    /// Most frames a second the zone's client may render (the bridge answers its frame
    /// callbacks at that rate). Unset: as fast as the master redraws.
    #[arg(long)]
    pub zone_fps: Option<u32>,
    /// Show the zone's client on a display plane of its own under the dashboard (a hole in
    /// the tile), when the display has one that takes its buffers: a client frame then costs
    /// a plane commit, not a redraw of the dashboard. Implies `--zone-redraw frames` and
    /// raises the dashboard's plane to zpos 1.
    #[arg(long)]
    pub zone_plane: bool,
}

/// See `CommandLineArgs::zone_redraw`.
#[derive(clap_derive::ValueEnum, Clone, Copy, Debug, PartialEq, Eq)]
pub enum ZoneRedraw {
    /// Re-apply the zone's image every 33 ms.
    Timer,
    /// Redraw only when the client sends a frame.
    Frames,
}

/// Socket the bridge serves: a bare name, resolved as har_gl_context::wayland_bridge does
/// (init socket, else $XDG_RUNTIME_DIR, else /data/local/tmp). A zone client connects to
/// /data/local/tmp/har-display by default, so the two meet without configuration.
/// HAR_WAYLAND_SOCKET overrides it on both sides.
pub fn wayland_socket_name() -> String {
    std::env::var("HAR_WAYLAND_SOCKET").unwrap_or_else(|_| "har-display".to_string())
}

/// --wayland-zones, readable where the UI model is built (UiModel::new has no arguments).
static WAYLAND_ZONES: std::sync::OnceLock<bool> = std::sync::OnceLock::new();

pub fn set_wayland_zones(on: bool) {
    let _ = WAYLAND_ZONES.set(on);
}

pub fn wayland_zones() -> bool {
    WAYLAND_ZONES.get().copied().unwrap_or(false)
}

/// The property by which the product turns the panel's browser on. The browser is experimental
/// and off by default; everyware's product/everyware.mk sets it with
/// EVERYWARE_EXPERIMENTAL_BROWSER=true.
pub const BROWSER_PROPERTY: &str = "ro.vendor.everyware.browser";

static BROWSER: std::sync::OnceLock<bool> = std::sync::OnceLock::new();

/// Whether the image has the panel's browser: a tap on a story opens its page, and an ordinary
/// Wayland window shows in the panel. Read once.
pub fn browser() -> bool {
    *BROWSER.get_or_init(|| crate::state::property(BROWSER_PROPERTY) == "true")
}

/// --zone-redraw, readable where the UI model formats the zone (no arguments there either).
static ZONE_REDRAW: std::sync::OnceLock<ZoneRedraw> = std::sync::OnceLock::new();

pub fn set_zone_redraw(mode: ZoneRedraw) {
    let _ = ZONE_REDRAW.set(mode);
}

pub fn zone_redraw() -> ZoneRedraw {
    ZONE_REDRAW.get().copied().unwrap_or(ZoneRedraw::Timer)
}

/// --zone-plane, readable where the display config is built.
static ZONE_PLANE: std::sync::OnceLock<bool> = std::sync::OnceLock::new();

pub fn set_zone_plane(on: bool) {
    let _ = ZONE_PLANE.set(on);
}

pub fn zone_plane() -> bool {
    ZONE_PLANE.get().copied().unwrap_or(false)
}

#[derive(Debug)]
pub struct Display {
    pub id: DisplayId,
    pub width: u32,
    pub height: u32,
    pub dpi: f32,
    pub root_node_id: String,
    pub display_params: HashMap<String, String>,
    pub input_devices: Option<Vec<String>>,
}

/// One display: the first connector of card0, the frame drawn 1:1 (dpi 160: Figma px ==
/// screen px).
pub fn get_display_config() -> Vec<Display> {
    let mut display_params = HashMap::new();
    display_params.insert("interface_id".to_string(), 1.to_string());
    display_params.insert("use_exact_size".to_string(), false.to_string());
    display_params.insert("use_preferred_mode".to_string(), false.to_string());
    display_params.insert("color_format".to_string(), ColorFormat::ABGR8888.to_string());
    // Zone planes go under the dashboard at zpos 0 (har_gl_context zone_plane).
    if zone_plane() {
        display_params.insert("z_order".to_string(), 1.to_string());
    }
    vec![Display {
        id: DisplayId::new(1),
        width: 1920,
        height: 1080,
        dpi: 160f32,
        root_node_id: ROOT_NODE.to_string(),
        display_params,
        input_devices: None,
    }]
}

/// "1920x1080" (--touch-size) -> (1920, 1080).
pub fn parse_size(s: &str) -> Option<(u32, u32)> {
    let (w, h) = s.split_once('x')?;
    Some((w.trim().parse().ok()?, h.trim().parse().ok()?))
}

impl CommandLineArgs {
    /// The directory the document is loaded from: the override when it holds the document,
    /// else data_path.
    pub fn document_dir(&self) -> String {
        match &self.override_path {
            Some(dir) if std::path::Path::new(dir).join(&self.document_id).is_file() => dir.clone(),
            _ => self.data_path.clone(),
        }
    }
}

pub fn get_commandline_args() -> CommandLineArgs {
    let mut args = CommandLineArgs::parse();
    // the file name harry opens: <name>.dcf, also when the name came without the extension
    args.document_id = everyware_ui::design::document_file(&args.document_id);
    args
}
