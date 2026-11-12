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
/// (`current`, its token), with the frame size once the camera streams. No side effect.
pub fn picture(current: &ImageContentData, size: Option<(u32, u32)>) -> ImageContentData {
    let mut image = current.clone();
    if let Some((width, height)) = size {
        (image.width, image.height) = (width, height);
    }
    image
}

/// Looks for a camera and streams it into the tile; on its own thread for the panel's life.
pub fn spawn(actions: Sender<Actions<HomeAction>>, looper: Sender<LooperMessage>) {
    let token = everyware_ui::images::get(IMAGE_KEY).unwrap_or_else(placeholder).id;
    thread::spawn(move || {
        let mut waiting_said = false;
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
                if let Some(id) = first_camera(manager.get_camera_list().into_iter().map(|c| c.id).collect()) {
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
                    log::warn!("smarthome: camera: {id} does not open: {e:?}");
                    thread::sleep(CAMERA_POLL);
                    continue;
                }
            };
            let (stopped_tx, stopped_rx) = channel();
            let to_action = |w, h| Actions::CustomAction(HomeAction::Camera(w, h));
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
                    log::warn!("smarthome: camera: {id} does not stream: {e}");
                    thread::sleep(CAMERA_POLL);
                    continue;
                }
            }
            // until the stream ends (unplugged): then the device and the manager go, and the
            // panel looks again
            let _ = stopped_rx.recv();
            let _ = manager.close(&id);
            drop(device);
            thread::sleep(CAMERA_POLL);
        }
    });
}
