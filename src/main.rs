//! smarthome-app: hello world of the smarthome panel on harry.
//!
//! Loads the Figma design (data/smarthome.dcf), draws #smarthome-root on the first DRM
//! connector and writes the local time into #home/date, #home/time and #home/sec once a second. The flow is harry's:
//!
//!   ticker thread --HomeAction::Tick--> reducer: action.apply(state) -> presenter(state, model)
//!     -> customizations of the UI model -> looper -> pre-renderer -> GPU -> display

use crate::actions::HomeAction;
use crate::presenter::UiModelPresenter;
use crate::resource_loader::ResourceManager;
use crate::state::State;
use crate::ui_event_listener::UiListener;
use crate::ui_model::UiModel;
use config::Display;
use har_monitoring::default_performance_monitor::DefaultPerformanceMonitor;
use har_monitoring::har_performance_monitor::HarPerformanceMonitor;
use har_platform_api::commlib::api::DocumentSwitch;
use har_platform_api::commlib::responder::{ChannelResponder, ResponseMonitorThread};
use har_platform_api::graphics::FontInfo;
use har_platform_api::looper::{create_resource_manager_client, DisplayId, LayoutHelperManager, Looper, LooperOptions};
use har_user_input_evdev::EvdevService;
use harry::reducer::{Actions, DefaultReducer, InputDispatcherCallback};
use harry_app_utils::document_loader::{
    LocalDiskFigmaLoader, ShutdownOnGenericErrorResponseHandler,
};
use harry_app_utils::layout_helper::HarLayoutHelperManager;
use std::sync::mpsc::channel;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

mod actions;
mod agenda;
mod agenda_card;
#[cfg(browser)]
mod browser;
mod camera;
mod clock;
mod config;
mod devices;
mod game;
mod screen_binder;

mod presenter;
mod resource_loader;
mod state;
mod text_measure;
mod ui_event_listener;
mod ui_model;
mod weather;
mod zone_plane;
mod zones;

fn main() {
    har_log::init_logger("smarthome-app");
    // The logger passes everything and liblog drops it by log.tag, but only after Rust has
    // formatted it: harry's per-node traces would cost every frame.
    // SMARTHOME_LOG=debug|trace for a diagnostic run (harry's layout and text measurement).
    let level = match std::env::var("SMARTHOME_LOG").as_deref() {
        Ok("trace") => log::LevelFilter::Trace,
        Ok("debug") => log::LevelFilter::Debug,
        _ => log::LevelFilter::Info,
    };
    log::set_max_level(level);

    let mut args = config::get_commandline_args();
    // A board whose display has no planes for zones (Cuttlefish's virtio-gpu has no zpos) says
    // so with a property; one APEX serves every board, so its init script passes --zone-plane.
    if args.zone_plane && !zone_plane::on(args.zone_plane, &crate::state::property(zone_plane::PROPERTY)) {
        log::info!("smarthome: zone planes off ({} is false)", zone_plane::PROPERTY);
        args.zone_plane = false;
        // a zone is then drawn by the dashboard, still only when its client has a new frame
        args.zone_redraw = config::ZoneRedraw::Frames;
    }
    // Before the display config: it raises the dashboard's plane for zone planes.
    config::set_zone_plane(args.zone_plane);
    let displays = config::get_display_config();
    // Before UiModel::new: the camera tile's zone field takes its first value from it.
    config::set_wayland_zones(args.wayland_zones);
    // A zone on a plane must not be redrawn by a timer: that is the cost the plane saves.
    config::set_zone_redraw(if args.zone_plane { config::ZoneRedraw::Frames } else { args.zone_redraw });
    let performance_monitor =
        Arc::new(DefaultPerformanceMonitor::new().expect("Error creating performance monitor."));

    // No side effects yet: the reducer still needs a channel for them.
    let (side_effects_tx, side_effects_rx) = channel::<()>();
    thread::spawn(move || for _ in side_effects_rx {});

    let display_root_node_map =
        displays.iter().map(|display| (display.id, display.root_node_id.clone())).collect();
    let (actions_tx, actions_rx) = channel();
    // The design's sizes (a dialog's card hugs its content: the app needs the content's height
    // to put the card in the middle), from the document harry loads: the override's copy wins.
    let sizes = {
        let dcf = args.document_id.clone();
        let overridden = args.override_path.as_ref().map(|p| std::path::Path::new(p).join(&dcf)).filter(|p| p.exists());
        let path = overridden.unwrap_or_else(|| std::path::Path::new(&args.data_path).join(&dcf));
        let started = std::time::Instant::now();
        match everyware_ui::design::Sizes::load(&path) {
            Ok(sizes) => {
                log::info!("smarthome: {} variant sizes from {} in {} ms", sizes.len(), path.display(), started.elapsed().as_millis());
                sizes
            }
            Err(e) => {
                log::warn!("smarthome: no design sizes ({e}): the sheets keep their default geometry");
                everyware_ui::design::Sizes::none()
            }
        }
    };
    // Text is measured with the layout helper's own fonts, not the renderer's.
    let mut layout_helper = HarLayoutHelperManager::new().expect("Failed to create layout helper");
    for (family, weight, data) in resource_loader::FONTS {
        // SAFETY: OpenType fonts are supported.
        unsafe { layout_helper.load_font(vec![FontInfo::new(family, *weight)], data) }
            .expect("Failed to load a font into the layout helper");
    }
    // A helper of the app's own, with the same fonts: the agenda measures its texts with it as
    // harry lays them out (text_measure.rs).
    match layout_helper.new_layout_helper() {
        Ok(mut measuring) => match layout_helper.update_helper_if_needed(&mut measuring) {
            Ok(()) => text_measure::set_helper(Box::new(measuring)),
            Err(e) => log::warn!("smarthome: no fonts for the text measure: {e:?}"),
        },
        Err(e) => log::warn!("smarthome: no layout helper to measure text: {e:?}"),
    }
    let reducer: DefaultReducer<State, HomeAction, UiModel, (), _, _> = DefaultReducer::new(
        State::new(sizes),
        UiModel::new(display_root_node_map),
        side_effects_tx,
        displays.iter().map(|display| display.id).collect::<Vec<DisplayId>>(),
        actions_tx.clone(),
        actions_rx,
        None,
        Some(performance_monitor.clone()),
        Some(layout_helper),
    );
    let actions = reducer.action_tx();
    let listener_actions = reducer.action_tx();
    let (looper_tx, looper_rx) = channel();
    // a picture that changes (the agenda's QR code) reaches the renderer through the looper
    everyware_ui::images::set_client(create_resource_manager_client(looper_tx.clone()));
    let velocity = everyware_ui::Velocity::default();
    // The document-load monitor below takes looper_tx; the bridge needs its own sender.
    let bridge_tx = looper_tx.clone();

    // The design loads while the display and the GPU come up: the reducer only sends to the
    // looper's channel, which holds the display lists until the looper starts. Before, the
    // two ran one after the other (create_looper, then the document): about 0.5 s of the
    // boot to the first frame (tools/boot-time).
    let document_dir = args.document_dir();
    log::info!("smarthome: the design from {document_dir}");
    let document_loader = LocalDiskFigmaLoader::new(document_dir);
    let reducer_thread = reducer.execute::<UiModelPresenter, _>(looper_tx.clone(), document_loader);

    let (responder, doc_load_response_rx) = ChannelResponder::create_with_receiver();
    actions
        .send(Actions::SwapDocument(
            DocumentSwitch { document_id: args.document_id.clone() },
            responder,
        ))
        .expect("Cannot send document load action");
    // The first frame of the document, from the performance monitor, logged with the time since
    // the kernel started (CLOCK_BOOTTIME, the clock of the kernel log and of init's
    // ro.boottime.*): the boot-time measurement reads it (everyware/tools/boot-time). The time is
    // the frame's own (`FirstFrameTiming::recorded_at`), not the time this thread woke up.
    {
        let monitor = performance_monitor.clone();
        let document_id = args.document_id.clone();
        thread::spawn(move || match monitor.time_to_first_frame(&document_id, None, None) {
            Ok(frame) => {
                let at = clock::since_boot() - frame.recorded_at.elapsed().as_secs_f64();
                log::info!("smarthome: first frame of '{document_id}' at {at:.3} s since boot")
            }
            Err(e) => log::warn!("smarthome: no first frame of '{document_id}': {e:?}"),
        });
    }
    // The settings change the look by starting the panel again (config.rs Looks). That is an
    // orderly end: the reducer and the looper stop and the GPU context is released, as when
    // the document cannot be loaded. An exit in the middle of a frame leaves the GPU's
    // command stream cut off.
    {
        let stop = std::sync::Mutex::new((actions.clone(), looper_tx.clone()));
        config::set_quit(move || {
            if let Ok(stop) = stop.lock() {
                let _ = stop.0.send(Actions::Exit);
                let _ = stop.1.send(har_platform_api::looper::LooperMessage::Exit);
            }
        });
    }
    // Exit if the document cannot be loaded.
    let _document_load_monitor = ResponseMonitorThread::new(
        doc_load_response_rx,
        ShutdownOnGenericErrorResponseHandler::new(actions.clone(), looper_tx),
    )
    .spawn();

    // Ticker: one action a second; the pre-render cache skips what did not change.
    {
        let actions = actions.clone();
        thread::spawn(move || loop {
            let now = clock::now();
            if actions.send(Actions::CustomAction(HomeAction::Tick(now))).is_err() {
                break;
            }
            thread::sleep(Duration::from_secs(1));
        });
    }

    // The house's data: the dashboard bundle pushes it here over binder, where the caller is
    // known (screen_binder.rs).
    screen_binder::spawn(actions.clone());
    // How the page in the panel's browser is doing: the loader goes when it shows. Only with the
    // browser (experimental: built with the cfg browser, and an image that has it: config::browser).
    #[cfg(browser)]
    if config::browser() {
        browser::spawn_events(actions.clone());
    }
    // The camera: camera-app's control socket next to the Wayland socket (live while the full
    // view is open; on/off for the LIVE chips). Only with zones: without them nothing shows it.
    if args.wayland_zones {
        let actions = actions.clone();
        let runtime_dir = std::env::var_os("XDG_RUNTIME_DIR");
        let path = camera::socket_path(runtime_dir.as_deref().map(std::path::Path::new));
        camera::spawn(path, move |event| actions.send(Actions::CustomAction(HomeAction::Camera(event))).is_ok());
    }

    // Touch and keyboard from evdev; --touch-size maps panel units to pixels (RB3). Opened on a
    // thread of their own: opening the devices takes about 0.2 s (the USB HID open, the release
    // of a probed device) that the display's start does not have to wait for.
    let touch_size = args.touch_size.as_deref().map(|s| {
        config::parse_size(s).unwrap_or_else(|| panic!("--touch-size {s}: expected WxH"))
    });
    let evdev_thread = {
        let callback = InputDispatcherCallback::new(actions_tx);
        let velocity = velocity.clone();
        let inputs: Vec<_> = displays.iter().map(|display| (display.id, display.input_devices.clone())).collect();
        thread::spawn(move || {
            inputs
                .into_iter()
                .map(|(display_id, devices)| {
                    let callback = callback.clone();
                    let velocity = velocity.clone();
                    // The finger's speed is measured here, as the kernel hands the events over
                    // (past this point they reach the listener in per-frame batches): a list
                    // flings with it.
                    let on_input = move |input| {
                        everyware_ui::stamp(&velocity, &input);
                        callback.process_input(input)
                    };
                    let service = match touch_size {
                        Some(size) => EvdevService::new_scaled(display_id, devices, size, on_input),
                        None => EvdevService::new(display_id, devices, on_input),
                    };
                    if let Err(e) = &service {
                        log::warn!("smarthome: no input for display {display_id}: {e:?}");
                    }
                    service
                })
                .collect::<Vec<_>>()
        })
    };

    // Zones on planes join the flips of the display's KMS context, which learns of them when
    // the looper sets the display up: before create_looper.
    if args.wayland_zones && args.zone_plane {
        har_gl_context::wayland_bridge::listen_for_zone_planes();
    }
    // The display, the GPU and the renderer (the connector probe, EGL, the mode set, the shaders).
    let mut looper = create_looper(&displays);
    looper
        .set_ui_event_listener(UiListener::new(listener_actions, velocity))
        .expect("Cannot set the UI event listener");

    log::info!("smarthome: started, loading document '{}'", args.document_id);
    // --wayland-zones: serve Wayland clients and composite them into their zones: har_zone
    // clients of ZONE_CAMERA_TAG into the camera tile (UiModel::camera_zone), the browser (when the
    // image has it, config::browser) into the panel (ZONE_BROWSER_TAG, UiModel::browser_zone). Needs the looper's EGL display, so after
    // create_looper and before start, which blocks. A client that dies leaves the tile with
    // its last frame; the dashboard itself is not affected.
    let _bridge = args.wayland_zones.then(|| {
        use har_gl_context::wayland_bridge::{start, BridgeConfig, SocketSource};
        let first = &displays[0];
        let display_ptr =
            looper.get_egl_display(&first.id).expect("EGL display of the first display");
        log::info!("smarthome: wayland zones on, socket '{}'", config::wayland_socket_name());
        // SAFETY: display_ptr came straight from the looper, which keeps that EGL display
        // alive for as long as it runs; the bridge thread ends with the looper.
        unsafe {
            start(
                display_ptr,
                bridge_tx,
                BridgeConfig {
                    socket: SocketSource::Name(config::wayland_socket_name()),
                    display_ids: displays.iter().map(|d| d.id).collect(),
                    context_id: "smarthome master".to_string(),
                    // With the browser an ordinary Wayland app (Cog) shows in the panel
                    // (UiModel::browser_zone); the camera tile keeps har_zone clients.
                    xdg_app_zones: Vec::new(),
                    zone_max_fps: args.zone_fps,
                    zone_plane: args.zone_plane,
                    // Without the browser no ordinary window gets a zone; har_zone clients do.
                    xdg_default_tag: browser_tag(),
                },
            )
        }
    });

    looper.start(looper_rx).expect("Cannot start UI thread");

    for service in evdev_thread.join().expect("The input thread failed").into_iter().flatten() {
        service.stop();
    }
    reducer_thread.join().expect("Error reported on the reducer thread.");
}

fn create_looper(displays: &[Display]) -> impl Looper<ResourceManager> {
    let mut options = displays.iter().map(|display| LooperOptions {
        display_id: Some(display.id),
        display_params: display.display_params.clone(),
        width: display.width,
        height: display.height,
        dpi: display.dpi,
        root_node_id: display.root_node_id.clone(),
        take_screenshots: false,
        driverui_health_node_id: String::new(),
    });
    let first = options.next().expect("At least one display needs to be configured.");
    let mut looper = harry_app_platform_specific::create_looper(first, ResourceManager)
        .expect("Cannot initialize renderer");
    options.for_each(|o| {
        let id = o.display_id;
        looper.add_display(o).unwrap_or_else(|_| panic!("Cannot add display: {id:?}."));
    });
    looper
}

/// The zone every ordinary Wayland window binds to: the browser's panel, when the panel is built
/// with the browser (the cfg browser) and the image has it; else none.
#[cfg(browser)]
fn browser_tag() -> Option<String> {
    config::browser().then(|| ui_model::ZONE_BROWSER_TAG.to_string())
}

#[cfg(not(browser))]
fn browser_tag() -> Option<String> {
    None
}
