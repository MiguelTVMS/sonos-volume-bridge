//! Camera adapters never expose device identifiers or capture media.
use sonos_volume_bridge_domain::camera::CameraObservation;
use sonos_volume_bridge_integration::camera::CameraPort;

#[cfg(all(target_os = "macos", feature = "camera-automation-macos"))]
mod macos;
#[cfg(all(windows, feature = "camera-automation-windows"))]
mod windows;

#[cfg(any(test, all(windows, feature = "camera-automation-windows")))]
mod inventory;
#[cfg(any(test, all(target_os = "macos", feature = "camera-automation-macos")))]
mod observer;

pub fn start() -> Box<dyn CameraPort> {
    #[cfg(all(target_os = "macos", feature = "camera-automation-macos"))]
    return macos::start();
    #[cfg(all(windows, feature = "camera-automation-windows"))]
    return windows::start();
    #[cfg(not(any(
        all(target_os = "macos", feature = "camera-automation-macos"),
        all(windows, feature = "camera-automation-windows")
    )))]
    Box::new(Unsupported)
}

#[cfg(not(any(
    all(target_os = "macos", feature = "camera-automation-macos"),
    all(windows, feature = "camera-automation-windows")
)))]
struct Unsupported;
#[cfg(not(any(
    all(target_os = "macos", feature = "camera-automation-macos"),
    all(windows, feature = "camera-automation-windows")
)))]
impl CameraPort for Unsupported {
    fn observation(&self) -> CameraObservation {
        CameraObservation::default()
    }
}

#[cfg(any(
    all(target_os = "macos", feature = "camera-automation-macos"),
    all(windows, feature = "camera-automation-windows")
))]
mod worker {
    use super::{CameraObservation, CameraPort};
    use sonos_volume_bridge_domain::camera::{CameraActivity, CameraAvailability};
    use std::sync::{Arc, Mutex, mpsc};
    use std::time::{Duration, Instant};

    pub struct Monitor {
        pub state: Arc<Mutex<(CameraObservation, Instant)>>,
        pub stop: mpsc::Sender<()>,
        pub thread: Option<std::thread::JoinHandle<()>>,
    }
    impl CameraPort for Monitor {
        fn observation(&self) -> CameraObservation {
            self.state
                .lock()
                .ok()
                .filter(|s| s.1.elapsed() < Duration::from_secs(8))
                .map_or_else(unavailable, |s| s.0)
        }
    }
    impl Drop for Monitor {
        fn drop(&mut self) {
            let _ = self.stop.send(());
            if let Some(thread) = self.thread.take() {
                let _ = thread.join();
            }
        }
    }
    pub fn unavailable() -> CameraObservation {
        CameraObservation {
            activity: CameraActivity::Unknown,
            availability: CameraAvailability::Unavailable,
        }
    }
}

/// Build capability only; a running detector must still establish availability.
pub const fn compiled() -> bool {
    cfg!(any(
        all(target_os = "macos", feature = "camera-automation-macos"),
        all(windows, feature = "camera-automation-windows")
    ))
}
