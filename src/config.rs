/*
 * smarthome-app: display config + command line.
 */

use clap::Parser as _;
use clap_derive::Parser;
use dc_bundle::design_compose_definition::{DesignComposeDefinition, DesignComposeDefinitionHeader};
use everyware_ui::design::Sizes;
use har_platform_api::commlib::api::GenericResponse;
use har_platform_api::commlib::responder::ChannelResponder;
use har_platform_api::error::Error;
use har_platform_api::looper::DisplayId;
use har_types::color_format::ColorFormat;
use harry::state::FigmaDocumentLoader;
use harry_app_utils::document_loader::LocalDiskFigmaLoader;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

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

pub fn get_commandline_args() -> CommandLineArgs {
    let mut args = CommandLineArgs::parse();
    // the file name harry opens: <name>.dcf, also when the name came without the extension
    args.document_id = everyware_ui::design::document_file(&args.document_id);
    choose_document(&mut args);
    args
}

/// The first look: the design in data_path itself.
pub const CLASSIC: &str = "classic";

/// The looks of the panel: other screens for the same app (design/looks), each a document of
/// its own in `<data_path>/looks/<name>/`. The settings take the next one (`change_look`): the
/// running panel swaps its document for that look's, and the file `look` of the override
/// directory (the panel's writable directory) names it for the next start.
#[derive(Debug, Default)]
pub struct Looks {
    /// "classic" first, then the directories of `<data_path>/looks` that hold the document.
    pub names: Vec<String>,
    /// The look in force, an index into `names`.
    current: AtomicUsize,
    /// The document's file name, the same in every look's directory (`smarthome.dcf`).
    document: String,
    /// Where the documents are: data_path, and the override directory that is checked first.
    data_path: String,
    override_path: Option<String>,
    /// Where the choice is kept; None: nowhere (no override directory), the look cannot change.
    file: Option<PathBuf>,
}

static LOOKS: OnceLock<Looks> = OnceLock::new();

pub fn looks() -> &'static Looks {
    LOOKS.get_or_init(Looks::default)
}

/// The names of the looks under `dir` (a look: a directory with the document), sorted.
fn looks_in(dir: &Path, document: &str) -> Vec<String> {
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

/// Finds the looks and points the arguments at the document of the one in force.
fn choose_document(args: &mut CommandLineArgs) {
    let mut names = vec![CLASSIC.to_string()];
    names.extend(looks_in(&Path::new(&args.data_path).join("looks"), &args.document_id));
    let file = args.override_path.as_ref().map(|p| Path::new(p).join("look"));
    let chosen = file.as_ref().and_then(|f| std::fs::read_to_string(f).ok()).map(|s| s.trim().to_string()).unwrap_or_default();
    let current = names.iter().position(|n| *n == chosen).unwrap_or(0);
    let looks = Looks {
        names,
        current: AtomicUsize::new(current),
        document: args.document_id.clone(),
        data_path: args.data_path.clone(),
        override_path: args.override_path.clone(),
        file,
    };
    args.document_id = looks.document_id(current);
    let _ = LOOKS.set(looks);
}

impl Looks {
    /// The look in force.
    pub fn current(&self) -> usize {
        self.current.load(Ordering::SeqCst)
    }

    /// The look after the one in force, around; None when the look cannot change.
    pub fn next(&self) -> Option<usize> {
        (self.names.len() > 1 && self.file.is_some()).then(|| (self.current() + 1) % self.names.len())
    }

    /// The name of the look in force as the settings show it: "Classic".
    pub fn label(&self) -> String {
        let name = self.names.get(self.current()).map_or(CLASSIC, |n| n.as_str());
        let mut chars = name.chars();
        chars.next().map_or_else(String::new, |c| c.to_uppercase().chain(chars).collect())
    }

    /// The document of `look` as harry names it: the file's path under the directory it is
    /// loaded from (`smarthome.dcf`, `looks/poster/smarthome.dcf`).
    pub fn document_id(&self, look: usize) -> String {
        match self.names.get(look) {
            Some(name) if look > 0 => format!("looks/{name}/{}", self.document),
            _ => self.document.clone(),
        }
    }

    /// The directory the document `id` is loaded from: the override when it holds a copy (a
    /// design pushed to the device for a test, over the one in the APEX), else data_path.
    fn dir_of(&self, id: &str) -> &str {
        match &self.override_path {
            Some(dir) if Path::new(dir).join(id).is_file() => dir,
            _ => &self.data_path,
        }
    }

    /// The file of the document `id`.
    pub fn path_of(&self, id: &str) -> PathBuf {
        Path::new(self.dir_of(id)).join(id)
    }

    /// `look` is in force from now on, and at the next start.
    pub fn set_current(&self, look: usize) {
        self.current.store(look, Ordering::SeqCst);
        let kept = match (&self.file, self.names.get(look)) {
            (Some(file), Some(name)) => std::fs::write(file, format!("{name}\n")),
            _ => Ok(()),
        };
        if let Err(e) = kept {
            log::warn!("smarthome: the look is not kept for the next start: {e}");
        }
    }
}

/// A document as harry takes it from a loader.
type Document = (DesignComposeDefinitionHeader, DesignComposeDefinition);

/// The document the app read for its sizes, by its id, until harry asks for it.
static READ: Mutex<Option<(String, Document)>> = Mutex::new(None);

fn read_from_disk(id: &str) -> Result<Document, Error> {
    LocalDiskFigmaLoader::new(looks().dir_of(id).to_string()).load_document(id)
}

/// Reads the document `id` from the directory `Looks::dir_of` names and gives the design's
/// sizes. The document is kept for harry's next load of this id (`DesignLoader`): parsing is
/// the cost of a design (15 MB, 28 000 nodes), and the file is parsed once for both.
pub fn read_design(id: &str) -> Result<Sizes, Error> {
    let (sizes, document) = load_design(id)?;
    keep_for_harry(id, document);
    Ok(sizes)
}

/// The document `id` and its design's sizes: the one parse.
fn load_design(id: &str) -> Result<(Sizes, Document), Error> {
    let document = read_from_disk(id)?;
    Ok((Sizes::of(&document.1), document))
}

fn keep_for_harry(id: &str, document: Document) {
    if let Ok(mut read) = READ.lock() {
        *read = Some((id.to_string(), document));
    }
}

/// A look that is read before it is asked for.
struct Ahead {
    look: usize,
    sizes: Sizes,
    document: Document,
}

/// The look after the one in force, read while the settings are open: a tap on the Look row
/// finds it parsed and the change starts with the swap. The lock is held while the look is
/// read, so who wants the look waits for the end of a read under way.
static AHEAD: Mutex<Option<Ahead>> = Mutex::new(None);
/// The settings are open: a look read ahead is wanted.
static AHEAD_WANTED: AtomicBool = AtomicBool::new(false);

/// The settings opened: the next look is read on a thread of its own. One document more is in
/// memory until the settings close.
pub fn read_next_look_ahead() {
    AHEAD_WANTED.store(true, Ordering::SeqCst);
    read_ahead();
}

/// Reads the look after the one in force ahead, while the settings are open and no change of
/// the look is under way (a change reads ahead itself when it is done).
fn read_ahead() {
    let Some(look) = looks().next() else { return };
    let spawned = std::thread::Builder::new().name("look-ahead".into()).spawn(move || {
        let Ok(mut ahead) = AHEAD.lock() else { return };
        let wanted = AHEAD_WANTED.load(Ordering::SeqCst) && !CHANGING.load(Ordering::SeqCst);
        if !wanted || ahead.as_ref().is_some_and(|ahead| ahead.look == look) {
            return;
        }
        *ahead = None;
        let (started, name) = (Instant::now(), &looks().names[look]);
        match load_design(&looks().document_id(look)) {
            // the settings closed meanwhile: nobody waits for it
            Ok(_) if !AHEAD_WANTED.load(Ordering::SeqCst) => {}
            Ok((sizes, document)) => {
                *ahead = Some(Ahead { look, sizes, document });
                log::info!("smarthome: the look {name} read ahead in {} ms", started.elapsed().as_millis());
            }
            Err(e) => log::warn!("smarthome: the look {name} cannot be read ahead: {e:?}"),
        }
    });
    if let Err(e) = spawned {
        log::warn!("smarthome: no thread to read a look ahead: {e}");
    }
}

/// The settings closed: the look read ahead goes. On a thread of its own, because freeing a
/// document takes as long as many frames and the caller is the reducer.
pub fn forget_look_ahead() {
    AHEAD_WANTED.store(false, Ordering::SeqCst);
    let spawned = std::thread::Builder::new().name("look-ahead".into()).spawn(|| {
        if let Ok(mut ahead) = AHEAD.lock() {
            // not when the settings opened again and the look is wanted after all
            if !AHEAD_WANTED.load(Ordering::SeqCst) {
                *ahead = None;
            }
        }
    });
    if let Err(e) = spawned {
        log::warn!("smarthome: no thread to free the look read ahead, it stays: {e}");
    }
}

/// The look `look` when it was read ahead; a read under way (one that holds `AHEAD`) is
/// waited for.
fn take_ahead(look: usize) -> Option<Ahead> {
    AHEAD.lock().ok()?.take_if(|ahead| ahead.look == look)
}

/// Drops a document harry did not ask for (a look that did not get to the screen).
fn forget_design() {
    if let Ok(mut read) = READ.lock() {
        *read = None;
    }
}

/// Hands harry the document `read_design` read; a document the app did not read (or one
/// harry asks for a second time) comes from the disk.
pub struct DesignLoader;

impl FigmaDocumentLoader for DesignLoader {
    fn load_document(&self, id: &str) -> Result<Document, Error> {
        let read = READ.lock().ok().and_then(|mut read| read.take_if(|(read, _)| read == id));
        match read {
            Some((_, document)) => Ok(document),
            None => read_from_disk(id),
        }
    }

    fn save_document(&self, id: &str, header: &DesignComposeDefinitionHeader, doc: &DesignComposeDefinition) -> Result<(), Error> {
        LocalDiskFigmaLoader::new(looks().dir_of(id).to_string()).save_document(id, header, doc)
    }
}

/// A look on its way to the screen: its document is read, the reducer takes the design's
/// sizes with the action `HomeAction::Look` and asks harry for the document.
pub struct NewLook {
    pub look: usize,
    pub sizes: Sizes,
    /// Where harry answers the document swap; `change_look` waits for it.
    pub responder: ChannelResponder<GenericResponse>,
}

/// How a look gets to the reducer (main.rs sets it: the reducer's actions).
static SEND_LOOK: OnceLock<Box<dyn Fn(usize) + Send + Sync>> = OnceLock::new();
/// The look that is read and waits for the reducer.
static NEW_LOOK: Mutex<Option<NewLook>> = Mutex::new(None);
/// A change of the look is under way: a tap on the row does nothing until its end.
static CHANGING: AtomicBool = AtomicBool::new(false);
/// How many changes of the look were started.
static CHANGES: AtomicUsize = AtomicUsize::new(0);

pub fn set_look_sender(send: impl Fn(usize) + Send + Sync + 'static) {
    let _ = SEND_LOOK.set(Box::new(send));
}

/// The look the reducer was told about (`HomeAction::Look`), once.
pub fn take_new_look(look: usize) -> Option<NewLook> {
    let mut new = NEW_LOOK.lock().ok()?;
    new.take_if(|new| new.look == look)
}

/// Changes the look of the running panel to `look`, off the reducer's thread: the document is
/// read first (the panel keeps drawing the look in force), then the reducer takes its sizes and
/// harry swaps to the document that was read. Nothing starts again: the zones' clients and the
/// links to the bundles stay, and so does the state that does not hang on the design's sizes.
/// A look whose document cannot be read is passed over for the one after it, so one broken
/// file does not end the round.
pub fn change_look(look: usize) {
    let Some(send) = SEND_LOOK.get() else { return };
    if CHANGING.swap(true, Ordering::SeqCst) {
        return;
    }
    let change = CHANGES.fetch_add(1, Ordering::SeqCst) + 1;
    let spawned = std::thread::Builder::new().name("look".into()).spawn(move || {
        let looks = looks();
        let started = Instant::now();
        let count = looks.names.len();
        // the look that was read ahead, or from `look` on, around, up to the look in force
        let ahead = take_ahead(look).map(|ahead| {
            keep_for_harry(&looks.document_id(look), ahead.document);
            (look, ahead.sizes)
        });
        let was_ahead = ahead.is_some();
        let read = ahead.or_else(|| {
            (0..count).map(|step| (look + step) % count).take_while(|look| *look != looks.current()).find_map(|look| {
                match read_design(&looks.document_id(look)) {
                    Ok(sizes) => Some((look, sizes)),
                    Err(e) => {
                        log::warn!("smarthome: the look {} cannot be read: {e:?}", looks.names[look]);
                        None
                    }
                }
            })
        });
        if let Some((look, sizes)) = read {
            let name = &looks.names[look];
            let read = if was_ahead { "ahead".to_string() } else { format!("in {} ms", started.elapsed().as_millis()) };
            let (responder, answer) = ChannelResponder::create_with_receiver();
            if let Ok(mut new) = NEW_LOOK.lock() {
                *new = Some(NewLook { look, sizes, responder });
            }
            send(look);
            match answer.recv_timeout(LOOK_TIMEOUT) {
                Ok(GenericResponse::Ok) => {
                    log::info!("smarthome: the look {name} in {} ms (its document read {read})", started.elapsed().as_millis());
                }
                other => {
                    // nobody is to take what is left of it later
                    drop(take_new_look(look));
                    forget_design();
                    log::warn!("smarthome: the look {name}: no document from harry ({other:?})")
                }
            }
        }
        CHANGING.store(false, Ordering::SeqCst);
        // The settings are still open: the look after the one now in force is read ahead, once
        // harry has drawn the new look (a read beside that first frame made it later), and only
        // when no other change came meanwhile: that one reads ahead after itself.
        std::thread::sleep(AHEAD_PAUSE);
        if CHANGES.load(Ordering::SeqCst) == change {
            read_ahead();
        }
    });
    if let Err(e) = spawned {
        log::warn!("smarthome: no thread for the look: {e}");
        CHANGING.store(false, Ordering::SeqCst);
    }
}

/// How long a changed look has for its first frame before the next one is read ahead: on the
/// RB3 that frame comes 0.6 to 0.75 s after the swap, and 0.15 s later with a read beside it.
const AHEAD_PAUSE: Duration = Duration::from_secs(1);

/// How long harry may take to load a look's document.
const LOOK_TIMEOUT: Duration = Duration::from_secs(10);
