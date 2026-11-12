//! The camera tile from a USB camera, the way harry shows a camera: harry's platform opens the
//! first V4L2 capture device (har-platform-android camera: /dev/video*, capture and streaming,
//! so the Venus codec and the UVC metadata node are skipped), turns its frames into RGBA, and
//! this listener puts each frame into the tile's picture (`IMAGE_KEY` in everyware_ui::images,
//! the renderer's raw image storage). New pixels for the same picture redraw the last display
//! list in the looper, so a frame costs no pre-render; only a new frame size is an action.
//! Without a camera the tile keeps its design (and the Wayland zone, when there is one); a
//! camera that comes later is found within CAMERA_POLL.

use crate::actions::HomeAction;
use customizations::ImageContentData;
use har_platform_api::camera::{
    CameraColorFormat, CameraStreamConfiguration, CameraStreamEvent, FrameRate, ICameraDevice,
    ICameraManager, ICameraStreamListener,
};
use har_platform_api::error::Error;
use har_platform_api::looper::{create_resource_manager_client, LooperMessage, ResourceManagerClient};
use har_types::resources::RawImageData;
use harry::reducer::Actions;
use harry_app_platform_specific::har_platform::camera::CameraManager;
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};
use std::thread;

/// The tile's picture in everyware_ui::images.
pub const IMAGE_KEY: &str = "camera";
/// The largest frame accepted: the camera is a USB device, its sizes are input to this process.
const MAX_FRAME: (usize, usize) = (1920, 1080);
/// What the platform is asked for (har-platform-android sets 640x360 itself).
const STREAM: (usize, usize) = (640, 360);
/// How often the panel looks for a camera while there is none.
const CAMERA_POLL: std::time::Duration = std::time::Duration::from_secs(2);

/// The tile's picture before the first frame: one transparent pixel, so the design shows.
pub fn placeholder() -> ImageContentData {
    let _ = everyware_ui::images::register_rgba(IMAGE_KEY, vec![0; 4], 1, 1);
    everyware_ui::images::get(IMAGE_KEY).expect("the camera's picture was just registered")
}

/// The tile's picture for a frame size from the camera (`State::camera`).
pub fn picture(size: Option<(u32, u32)>) -> ImageContentData {
    let mut image = everyware_ui::images::get(IMAGE_KEY).unwrap_or_else(placeholder);
    if let Some((width, height)) = size {
        (image.width, image.height) = (width, height);
    }
    image
}

struct Listener {
    device: Arc<Mutex<dyn ICameraDevice + Send>>,
    client: ResourceManagerClient,
    actions: Sender<Actions<HomeAction>>,
    size: Option<(usize, usize)>,
}

impl ICameraStreamListener for Listener {
    fn on_event(&mut self, event: CameraStreamEvent) -> Result<(), Error> {
        if let CameraStreamEvent::NewFrameBuffer(desc) = event {
            let size = (desc.width, desc.height);
            let fits = size.0 > 0 && size.1 > 0 && size.0 <= MAX_FRAME.0 && size.1 <= MAX_FRAME.1;
            let pixels = if fits { desc.data.read().to_owned() } else { Vec::new() };
            if fits && pixels.len() == size.0 * size.1 * 4 {
                let image = RawImageData::new(pixels, size.0 as u32, size.1 as u32);
                if let Err(e) = self.client.update(desc.resource_token, image) {
                    log::warn!("smarthome: camera: frame not stored: {e:?}");
                }
                if self.size != Some(size) {
                    self.size = Some(size);
                    log::info!("smarthome: camera: {}x{} frames in the tile", size.0, size.1);
                    let _ = self.actions.send(Actions::CustomAction(HomeAction::Camera(size.0 as u32, size.1 as u32)));
                }
            } else if self.size.is_some() || !fits {
                log::warn!("smarthome: camera: a {}x{} frame of {} bytes is dropped", size.0, size.1, pixels.len());
            }
            if let Ok(mut device) = self.device.lock() {
                device.done_with_frame(desc);
            }
        }
        Ok(())
    }
}

/// Starts the camera on its own thread; the manager and the device live as long as the panel.
pub fn spawn(actions: Sender<Actions<HomeAction>>, looper: Sender<LooperMessage>) {
    let token = placeholder().id;
    thread::spawn(move || {
        let client = create_resource_manager_client(looper);
        let mut manager = match CameraManager::new(client.clone()) {
            Ok(m) => m,
            Err(e) => return log::warn!("smarthome: camera: no camera manager: {e:?}"),
        };
        // a USB camera can come after the panel (at boot, or plugged in later): look again every
        // CAMERA_POLL until one is there; the tile keeps its design meanwhile
        let mut said = false;
        let camera = loop {
            if let Some(camera) = manager.get_camera_list().into_iter().next() {
                break camera;
            }
            if !said {
                log::info!("smarthome: camera: no capture device yet, the tile keeps its design");
                said = true;
            }
            thread::sleep(CAMERA_POLL);
        };
        let device = match manager.open(&camera.id, token) {
            Ok(d) => d,
            Err(e) => return log::warn!("smarthome: camera: {} does not open: {e:?}", camera.id),
        };
        let listener = Arc::new(Mutex::new(Listener { device: device.clone(), client, actions, size: None }));
        let config = CameraStreamConfiguration {
            width: STREAM.0,
            height: STREAM.1,
            frame_rate: FrameRate::new(30, 1),
            format: CameraColorFormat::RGBA32,
        };
        match device.lock().map_err(|e| format!("{e:?}")).and_then(|mut d| d.start(&config, listener).map_err(|e| format!("{e:?}"))) {
            Ok(got) => log::info!("smarthome: camera: {} streams {}x{}", camera.id, got.width, got.height),
            Err(e) => return log::warn!("smarthome: camera: {} does not stream: {e}", camera.id),
        }
        // the stream runs on the platform's thread; this one keeps the manager and the device
        let _keep = (manager, device);
        loop {
            thread::park();
        }
    });
}
