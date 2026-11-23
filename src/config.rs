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
    /// Serve Wayland clients (other harry processes) and show them in their zones: camera-app's
    /// tile and full view (camera.rs), the game. Off: no socket is opened and the camera tile
    /// says "Camera offline" (no camera app can draw it).
    #[arg(long)]
    pub wayland_zones: bool,
    /// What redraws the dashboard for the camera tile's and the browser's Wayland zones: `timer`
    /// every 33 ms (also when the zone is idle), `frames` once per client frame (an idle zone
    /// costs nothing). The game's zone is always `frames` (ui_model.rs GameZoneCustomizer).
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
/// and off by default; everyware's product/everyware.mk sets it with EVERYWARE_EXPERIMENTAL_BROWSER=true,
/// which also builds the panel with its browser code (the cfg browser).
#[cfg(browser)]
pub const BROWSER_PROPERTY: &str = "ro.vendor.everyware.browser";

#[cfg(browser)]
static BROWSER: std::sync::OnceLock<bool> = std::sync::OnceLock::new();

/// Whether the image has the panel's browser: an ordinary Wayland window shows in the panel. Read
/// once.
#[cfg(browser)]
pub fn browser() -> bool {
    *BROWSER.get_or_init(|| crate::state::property(BROWSER_PROPERTY) == "true")
}

static GAME: std::sync::OnceLock<bool> = std::sync::OnceLock::new();

/// Whether the image has the game (game::PROPERTY): the status row shows the game button. Read
/// once.
pub fn game() -> bool {
    *GAME.get_or_init(|| crate::game::in_image(&crate::state::property(crate::game::PROPERTY)))
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
    choose_document(&mut args);
    args
}

/// How the panel ends in order (main.rs sets it): the reducer and the looper stop.
static QUIT: std::sync::OnceLock<Box<dyn Fn() + Send + Sync>> = std::sync::OnceLock::new();

pub fn set_quit(quit: impl Fn() + Send + Sync + 'static) {
    let _ = QUIT.set(Box::new(quit));
}

/// Ends the panel in order; its service starts it again.
pub fn quit() {
    match QUIT.get() {
        Some(quit) => quit(),
        None => log::warn!("smarthome: no way to end in order yet"),
    }
}

/// The first look: the design in data_path itself.
pub const CLASSIC: &str = "classic";

/// The looks of the panel: other screens for the same app (design/looks), each a document of
/// its own in `<data_path>/looks/<name>/`. The one in force is named in the file `look` of the
/// override directory (the panel's writable directory); the settings write it (`set_look`) and
/// the panel starts again with it.
#[derive(Debug, Default)]
pub struct Looks {
    /// "classic" first, then the directories of `<data_path>/looks` that hold the document.
    pub names: Vec<String>,
    /// The look in force, an index into `names`.
    pub current: usize,
    /// Where the choice is kept; None: nowhere (no override directory), the look cannot change.
    file: Option<std::path::PathBuf>,
}

static LOOKS: std::sync::OnceLock<Looks> = std::sync::OnceLock::new();

pub fn looks() -> &'static Looks {
    LOOKS.get_or_init(Looks::default)
}

/// The names of the looks under `dir` (a look: a directory with the document), sorted.
fn looks_in(dir: &std::path::Path, document: &str) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| e.path().join(document).is_file())
        .filter_map(|e| e.file_name().into_string().ok())
        .collect();
    names.sort();
    names
}

/// Points the arguments at the document of the look in force. A look other than the first
/// has no override: the override directory holds a test copy of the first one.
fn choose_document(args: &mut CommandLineArgs) {
    let looks_dir = std::path::Path::new(&args.data_path).join("looks");
    let mut names = vec![CLASSIC.to_string()];
    names.extend(looks_in(&looks_dir, &args.document_id));
    let file = args.override_path.as_ref().map(|p| std::path::Path::new(p).join("look"));
    let chosen = file.as_ref().and_then(|f| std::fs::read_to_string(f).ok()).map(|s| s.trim().to_string()).unwrap_or_default();
    let current = names.iter().position(|n| *n == chosen).unwrap_or(0);
    if current > 0 {
        args.data_path = looks_dir.join(&names[current]).to_string_lossy().into_owned();
        args.override_path = None;
    }
    let _ = LOOKS.set(Looks { names, current, file });
}

impl Looks {
    /// The look after the one in force, around.
    pub fn next(&self) -> Option<&str> {
        (self.names.len() > 1 && self.file.is_some()).then(|| self.names[(self.current + 1) % self.names.len()].as_str())
    }

    /// The name of the look in force as the settings show it: "Classic".
    pub fn label(&self) -> String {
        let name = self.names.get(self.current).map_or(CLASSIC, |n| n.as_str());
        let mut chars = name.chars();
        chars.next().map_or_else(String::new, |c| c.to_uppercase().chain(chars).collect())
    }

    /// Keeps `name` as the look for the next start.
    pub fn set_look(&self, name: &str) -> std::io::Result<()> {
        let file = self.file.as_ref().ok_or_else(|| std::io::Error::other("no directory for the choice"))?;
        std::fs::write(file, format!("{name}\n"))
    }
}
