//! Register-before-read orchestration shared by the native observer and tests.
use sonos_volume_bridge_domain::camera::{CameraActivity, CameraAvailability, CameraObservation};

pub trait CameraSource {
    fn devices(&mut self) -> Option<Vec<u32>>;
    fn watch(&mut self, devices: &[u32]) -> bool;
    fn running(&mut self, device: u32) -> Option<bool>;
    fn clear(&mut self);
}
pub struct Observer<S: CameraSource> {
    source: S,
    inventory: Option<Vec<u32>>,
}
impl<S: CameraSource> Observer<S> {
    pub fn new(source: S) -> Self {
        Self {
            source,
            inventory: None,
        }
    }
    pub fn sample(&mut self) -> CameraObservation {
        if let Some(active) = self.read() {
            CameraObservation {
                activity: if active {
                    CameraActivity::Active
                } else {
                    CameraActivity::Inactive
                },
                availability: CameraAvailability::Supported,
            }
        } else {
            self.source.clear();
            self.inventory = None;
            CameraObservation {
                activity: CameraActivity::Unknown,
                availability: CameraAvailability::Unavailable,
            }
        }
    }

    fn read(&mut self) -> Option<bool> {
        let devices = self.source.devices()?;
        if self.inventory.as_ref() != Some(&devices) {
            if !self.source.watch(&devices) {
                return None;
            }
            self.inventory = Some(devices.clone());
        }
        if self.source.devices()? != devices {
            return None;
        }
        let mut active = false;
        for device in &devices {
            active |= self.source.running(*device)?;
        }
        if self.source.devices()? != devices {
            return None;
        }
        Some(active)
    }
}
impl<S: CameraSource> Drop for Observer<S> {
    fn drop(&mut self) {
        self.source.clear();
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        collections::VecDeque,
        sync::{Arc, Mutex},
    };
    struct Fake {
        snapshots: VecDeque<Vec<u32>>,
        log: Arc<Mutex<Vec<&'static str>>>,
        watch_ok: bool,
    }
    impl CameraSource for Fake {
        fn devices(&mut self) -> Option<Vec<u32>> {
            self.log.lock().unwrap().push("enumerate");
            if self.snapshots.len() > 1 {
                self.snapshots.pop_front()
            } else {
                self.snapshots.front().cloned()
            }
        }
        fn watch(&mut self, _: &[u32]) -> bool {
            self.log.lock().unwrap().push("watch");
            self.watch_ok
        }
        fn running(&mut self, _: u32) -> Option<bool> {
            self.log.lock().unwrap().push("read");
            Some(true)
        }
        fn clear(&mut self) {
            self.log.lock().unwrap().push("clear");
        }
    }
    #[test]
    fn startup_registers_before_read_and_drop_cleans_up() {
        let log = Arc::new(Mutex::new(Vec::new()));
        let mut observer = Observer::new(Fake {
            snapshots: [vec![1]].into(),
            log: Arc::clone(&log),
            watch_ok: true,
        });
        assert_eq!(observer.sample().activity, CameraActivity::Active);
        assert_eq!(
            *log.lock().unwrap(),
            ["enumerate", "watch", "enumerate", "read", "enumerate"]
        );
        drop(observer);
        assert_eq!(log.lock().unwrap().last(), Some(&"clear"));
    }
    #[test]
    fn hotplug_during_registration_is_unknown_and_retries_registration() {
        let log = Arc::new(Mutex::new(Vec::new()));
        let mut observer = Observer::new(Fake {
            snapshots: [vec![1], vec![1, 2]].into(),
            log: Arc::clone(&log),
            watch_ok: true,
        });
        assert_eq!(observer.sample().activity, CameraActivity::Unknown);
        assert_eq!(observer.sample().activity, CameraActivity::Active);
        assert_eq!(
            log.lock()
                .unwrap()
                .iter()
                .filter(|s| **s == "watch")
                .count(),
            2
        );
    }
    #[test]
    fn failed_registration_never_publishes_inactivity() {
        let log = Arc::new(Mutex::new(Vec::new()));
        let mut observer = Observer::new(Fake {
            snapshots: [vec![]].into(),
            log,
            watch_ok: false,
        });
        assert_eq!(observer.sample().activity, CameraActivity::Unknown);
        observer.source.watch_ok = true;
        assert_eq!(observer.sample().activity, CameraActivity::Inactive);
    }
}
