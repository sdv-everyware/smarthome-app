//! The camera tile from a USB camera, the way harry shows a camera: harry's platform opens a
//! V4L2 capture device (har-platform-android camera: /dev/video*, capture and streaming, so the
//! Venus codec and the UVC metadata node are skipped), bounds its format (check_capture_format),
//! turns its frames into RGBA and puts each one into the tile's picture (`IMAGE_KEY` in
//! everyware_ui::images, the renderer's raw image storage) through the looper, which redraws
//! the last display list: a frame costs no pre-render. The listener (camera_listener.rs) only
//! makes a new frame size an action. Without a camera the tile keeps its design (and the Wayland
//! zone, when there is one); the panel looks for a camera every CAMERA_POLL, also after the
//! stream stopped (the camera was unplugged) or a camera did not open or start.

use crate::actions::HomeAction;
use crate::camera_listener::{first_camera, Listener};
use customizations::ImageContentData;
use har_platform_api::camera::{CameraColorFormat, CameraStreamConfiguration, FrameRate, ICameraManager};
use har_platform_api::looper::{create_resource_manager_client, LooperMessage};
use harry::reducer::Actions;
use harry_app_platform_specific::har_platform::camera::CameraManager;
use std::sync::mpsc::{channel, Sender};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

/// The tile's picture in everyware_ui::images.
pub const IMAGE_KEY: &str = "camera";
/// What the platform is asked for; it reports the size the driver chose.
const STREAM: (usize, usize) = (640, 360);
/// How often the panel looks for a camera while it has none.
const CAMERA_POLL: Duration = Duration::from_secs(2);

/// Registers the tile's picture: one transparent pixel, so the design shows until the camera
/// streams. Called once, for the UI model's first value (ui_model.rs); the camera's frames
/// replace its pixels under the same token.
pub fn placeholder() -> ImageContentData {
    let _ = everyware_ui::images::register_rgba(IMAGE_KEY, vec![0; 4], 1, 1);
    everyware_ui::images::get(IMAGE_KEY).expect("the camera's picture was just registered")
}

/// The tile's picture for the camera's frame size (`State::camera`): the same picture
/// (`current`, its token) with the frame size, or the placeholder's one pixel when the camera
/// does not stream. No side effect.
pub fn picture(current: &ImageContentData, size: Option<(u32, u32)>) -> ImageContentData {
    let mut image = current.clone();
    (image.width, image.height) = size.unwrap_or((1, 1));
    image
}

/// The placeholder's transparent pixel, put back into the tile's picture through the looper
/// (which redraws) when a stream ended: the last frame must not stay in the tile.
fn clear(looper: &Sender<LooperMessage>, token: har_types::resources::ResourceToken) {
    let client = create_resource_manager_client(looper.clone());
    if let Err(e) = client.update(token, har_types::resources::RawImageData::new(vec![0; 4], 1, 1)) {
        log::warn!("smarthome: camera: the tile's picture was not cleared: {e:?}");
    }
}

/// Looks for a camera and streams it into the tile; on its own thread for the panel's life.
pub fn spawn(actions: Sender<Actions<HomeAction>>, looper: Sender<LooperMessage>) {
    let token = everyware_ui::images::get(IMAGE_KEY).unwrap_or_else(placeholder).id;
    thread::spawn(move || {
        let mut waiting_said = false;
        // the devices of the last camera that did not open or stream: it is tried again only
        // when the list of devices changes (plugged out and in, another camera), not every round
        let mut failed_with: Option<Vec<String>> = None;
        loop {
            // a new manager after every stream or failure: its list is the devices of now
            // (a manager keeps the list it found; an empty list is looked up again, quietly)
            let client = create_resource_manager_client(looper.clone());
            let mut manager = match CameraManager::new(client) {
                Ok(m) => m,
                Err(e) => {
                    log::warn!("smarthome: camera: no camera manager: {e:?}");
                    thread::sleep(CAMERA_POLL);
                    continue;
                }
            };
            let id = loop {
                let ids: Vec<String> = manager.get_camera_list().into_iter().map(|c| c.id).collect();
                if failed_with.as_ref() == Some(&ids) {
                    // the same devices as the failure: wait for a change, with a new manager
                    // (its list is the devices of now)
                    thread::sleep(CAMERA_POLL);
                    match CameraManager::new(create_resource_manager_client(looper.clone())) {
                        Ok(m) => manager = m,
                        Err(e) => log::warn!("smarthome: camera: no camera manager: {e:?}"),
                    }
                    continue;
                }
                failed_with = None;
                if let Some(id) = first_camera(ids) {
                    break id;
                }
                if !waiting_said {
                    log::info!("smarthome: camera: no capture device yet, the tile keeps its design");
                    waiting_said = true;
                }
                thread::sleep(CAMERA_POLL);
            };
            waiting_said = false;
            let device = match manager.open(&id, token) {
                Ok(d) => d,
                Err(e) => {
                    log::warn!("smarthome: camera: {id} does not open: {e:?}; tried again when the devices change");
                    failed_with = Some(manager.get_camera_list().into_iter().map(|c| c.id).collect());
                    thread::sleep(CAMERA_POLL);
                    continue;
                }
            };
            let (stopped_tx, stopped_rx) = channel();
            let to_action = |size| Actions::CustomAction(HomeAction::Camera(size));
            let listener = Arc::new(Mutex::new(Listener::new(device.clone(), actions.clone(), to_action, stopped_tx)));
            let config = CameraStreamConfiguration {
                width: STREAM.0,
                height: STREAM.1,
                frame_rate: FrameRate::new(30, 1),
                format: CameraColorFormat::RGBA32,
            };
            let started = device.lock().map_err(|e| format!("{e:?}")).and_then(|mut d| d.start(&config, listener).map_err(|e| format!("{e:?}")));
            match started {
                Ok(chosen) => log::info!("smarthome: camera: {id} streams {}x{}", chosen.width, chosen.height),
                Err(e) => {
                    log::warn!("smarthome: camera: {id} does not stream: {e}; tried again when the devices change");
                    failed_with = Some(manager.get_camera_list().into_iter().map(|c| c.id).collect());
                    let _ = manager.close(&id);
                    drop(device);
                    thread::sleep(CAMERA_POLL);
                    continue;
                }
            }
            // until the stream ends (unplugged): then the device and the manager go, and the
            // panel looks again
            let _ = stopped_rx.recv();
            clear(&looper, token);
            let _ = manager.close(&id);
            drop(device);
            thread::sleep(CAMERA_POLL);
        }
    });
}
