//! The camera tile's stream listener (camera.rs), without the platform: harry's platform
//! bounds the frame size and puts each frame into the tile's picture itself (har-platform-android
//! camera: check_capture_format, resource_manager_client.update), so this listener only turns
//! a new frame size into one action, gives every frame back to the device, and reports the end
//! of the stream (a camera unplugged) so that the panel looks for a camera again.

use har_platform_api::camera::{CameraStreamEvent, ICameraDevice, ICameraStreamListener};
use har_platform_api::error::Error;
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};

/// Listens to one camera stream for the panel.
pub struct Listener<A> {
    device: Arc<Mutex<dyn ICameraDevice + Send>>,
    actions: Sender<A>,
    to_action: fn(u32, u32) -> A,
    stopped: Sender<()>,
    size: Option<(usize, usize)>,
}

impl<A> Listener<A> {
    /// `to_action` makes the action of a new frame size; `stopped` gets one message when the
    /// stream ends.
    pub fn new(
        device: Arc<Mutex<dyn ICameraDevice + Send>>,
        actions: Sender<A>,
        to_action: fn(u32, u32) -> A,
        stopped: Sender<()>,
    ) -> Self {
        Listener { device, actions, to_action, stopped, size: None }
    }
}

impl<A: Send> ICameraStreamListener for Listener<A> {
    fn on_event(&mut self, event: CameraStreamEvent) -> Result<(), Error> {
        match event {
            CameraStreamEvent::NewFrameBuffer(desc) => {
                let size = (desc.width, desc.height);
                if self.size != Some(size) {
                    self.size = Some(size);
                    log::info!("smarthome: camera: {}x{} frames in the tile", size.0, size.1);
                    let _ = self.actions.send((self.to_action)(size.0 as u32, size.1 as u32));
                }
                if let Ok(mut device) = self.device.lock() {
                    device.done_with_frame(desc);
                }
            }
            CameraStreamEvent::StreamStopped(_) => {
                log::info!("smarthome: camera: the stream stopped");
                let _ = self.stopped.send(());
            }
            CameraStreamEvent::StreamStarted(_) | CameraStreamEvent::StreamHang(_) => {}
        }
        Ok(())
    }
}

/// The capture device to use: the lowest /dev/videoN (numerically, so video10 comes after
/// video2) of the ones the platform found.
pub fn first_camera(ids: Vec<String>) -> Option<String> {
    let number = |id: &String| id.trim_start_matches(|c: char| !c.is_ascii_digit()).parse::<u32>().unwrap_or(u32::MAX);
    ids.into_iter().min_by_key(number)
}

#[cfg(test)]
mod tests {
    use super::*;
    use har_platform_api::camera::{
        CameraCharacteristics, CameraColorFormat, CameraDescriptor, CameraParameter, CameraParameterRange,
        CameraStreamConfiguration, CameraStreamEventDesc, FrameBufferEventDesc, FrameBufferShared,
        FrameBufferTimestamp,
    };
    use har_platform_api::vehicle_data::VehicleDataType;
    use har_types::resources::ResourceToken;
    use std::sync::mpsc;
    use std::sync::RwLock;

    /// Counts the frames given back.
    struct Mock(Arc<Mutex<usize>>);

    impl ICameraDevice for Mock {
        fn start(
            &mut self,
            _: &CameraStreamConfiguration,
            _: Arc<Mutex<dyn ICameraStreamListener + Send>>,
        ) -> Result<CameraStreamConfiguration, Error> {
            unimplemented!()
        }
        fn stop(&mut self) {}
        fn get_descriptor(&mut self) -> Result<CameraDescriptor, Error> {
            unimplemented!()
        }
        fn done_with_frame(&mut self, _: FrameBufferEventDesc) {
            *self.0.lock().unwrap() += 1;
        }
        fn get_parameter_list(&self) -> Result<Vec<CameraParameter>, Error> {
            unimplemented!()
        }
        fn set_int_parameter(&mut self, _: &CameraParameter, _: i32) -> Result<i32, Error> {
            unimplemented!()
        }
        fn set_custom_parameter(&mut self, _: VehicleDataType) -> Result<VehicleDataType, Error> {
            unimplemented!()
        }
        fn get_int_parameter(&self, _: &CameraParameter) -> Result<i32, Error> {
            unimplemented!()
        }
        fn get_custom_parameter(&self, _: &str) -> Result<VehicleDataType, Error> {
            unimplemented!()
        }
        fn get_int_parameter_range(&self, _: &CameraParameter) -> Result<&CameraParameterRange, Error> {
            unimplemented!()
        }
        fn get_characteristics(&self) -> Result<&Vec<CameraCharacteristics>, Error> {
            unimplemented!()
        }
        fn get_resource_token(&self) -> ResourceToken {
            ResourceToken::new(0, 0)
        }
    }

    fn frame(width: usize, height: usize) -> CameraStreamEvent {
        CameraStreamEvent::NewFrameBuffer(FrameBufferEventDesc {
            buffer_id: 0,
            width,
            height,
            stride: width * 4,
            format: CameraColorFormat::RGBA32,
            timestamp: FrameBufferTimestamp { nsec: 0 },
            data: FrameBufferShared::new(&Arc::new(RwLock::new(vec![]))),
            resource_token: ResourceToken::new(0, 0),
            metadata: None,
        })
    }

    fn listener() -> (Listener<(u32, u32)>, mpsc::Receiver<(u32, u32)>, mpsc::Receiver<()>, Arc<Mutex<usize>>) {
        let given_back = Arc::new(Mutex::new(0));
        let (tx, rx) = mpsc::channel();
        let (stop_tx, stop_rx) = mpsc::channel();
        let device: Arc<Mutex<dyn ICameraDevice + Send>> = Arc::new(Mutex::new(Mock(given_back.clone())));
        (Listener::new(device, tx, |w, h| (w, h), stop_tx), rx, stop_rx, given_back)
    }

    #[test]
    fn one_action_per_new_size_and_every_frame_given_back() {
        let (mut l, rx, _stop, given_back) = listener();
        for _ in 0..3 {
            l.on_event(frame(640, 360)).unwrap();
        }
        l.on_event(frame(320, 240)).unwrap();
        l.on_event(frame(320, 240)).unwrap();
        assert_eq!(rx.try_iter().collect::<Vec<_>>(), vec![(640, 360), (320, 240)]);
        assert_eq!(*given_back.lock().unwrap(), 5);
    }

    #[test]
    fn the_end_of_the_stream_is_reported_once_and_no_frame_action() {
        let (mut l, rx, stop, _) = listener();
        l.on_event(CameraStreamEvent::StreamStopped(CameraStreamEventDesc { data: None })).unwrap();
        assert_eq!(stop.try_iter().count(), 1);
        assert_eq!(rx.try_iter().count(), 0);
    }

    #[test]
    fn the_lowest_device_number_wins() {
        let ids = ["/dev/video10", "/dev/video2", "/dev/video3"].map(String::from).to_vec();
        assert_eq!(first_camera(ids).as_deref(), Some("/dev/video2"));
        assert_eq!(first_camera(Vec::new()), None);
    }

    #[test]
    fn other_events_change_nothing() {
        let (mut l, rx, stop, given_back) = listener();
        l.on_event(CameraStreamEvent::StreamStarted(CameraStreamEventDesc { data: None })).unwrap();
        assert_eq!((rx.try_iter().count(), stop.try_iter().count(), *given_back.lock().unwrap()), (0, 0, 0));
    }
}
