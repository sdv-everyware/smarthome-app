//! Lists the V4L2 capture devices with their formats and frame sizes, as harry's platform sees
//! them (har-platform-android camera: /dev/video*, VIDEO_CAPTURE and STREAMING).

use v4l2r::bindings::v4l2_frmsizeenum;
use v4l2r::device::{Device, DeviceConfig};
use v4l2r::ioctl::{enum_frame_sizes, g_fmt, Capabilities, FormatIterator, FrmSizeTypes};
use v4l2r::{Format, QueueType};

fn main() {
    for entry in glob::glob("/dev/video*").expect("glob").flatten() {
        let device = match Device::open(&entry, DeviceConfig::new()) {
            Ok(d) => d,
            Err(e) => {
                println!("{}: cannot open: {e:?}", entry.display());
                continue;
            }
        };
        let caps = device.caps();
        let dc = caps.device_caps();
        let capture = dc.contains(Capabilities::VIDEO_CAPTURE) && dc.contains(Capabilities::STREAMING);
        println!("{}: {} ({}), capture: {capture}", entry.display(), caps.card, caps.driver);
        if !capture {
            continue;
        }
        if let Ok(f) = g_fmt::<Format>(&device, QueueType::VideoCapture) {
            println!("  current: {} {}x{}", f.pixelformat, f.width, f.height);
        }
        for fmt in FormatIterator::new(&device, QueueType::VideoCapture) {
            let mut sizes = Vec::new();
            for i in 0..64 {
                match enum_frame_sizes::<v4l2_frmsizeenum>(&device, i, fmt.pixelformat) {
                    Ok(s) => match s.size() {
                        Some(FrmSizeTypes::Discrete(d)) => sizes.push(format!("{}x{}", d.width, d.height)),
                        Some(FrmSizeTypes::StepWise(w)) => sizes.push(format!(
                            "{}..{}x{}..{}",
                            w.min_width, w.max_width, w.min_height, w.max_height
                        )),
                        None => {}
                    },
                    Err(_) => break,
                }
            }
            println!("  {}: {}", fmt.pixelformat, sizes.join(" "));
        }
    }
}
