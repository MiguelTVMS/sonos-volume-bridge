//! Conservative aggregation: every enumerated camera needs an observation.
use sonos_volume_bridge_domain::camera::{CameraActivity, CameraAvailability, CameraObservation};
use std::collections::BTreeMap;

#[derive(Default)]
pub struct Inventory<K: Ord> {
    devices: BTreeMap<K, Option<bool>>,
    enumerated: bool,
}
impl<K: Ord> Inventory<K> {
    pub fn reconcile(&mut self, keys: impl IntoIterator<Item = K>) {
        let mut previous = std::mem::take(&mut self.devices);
        self.devices = keys
            .into_iter()
            .map(|key| {
                let value = previous.remove(&key).flatten();
                (key, value)
            })
            .collect();
        self.enumerated = true;
    }
    pub fn update(&mut self, key: &K, active: bool) {
        if let Some(value) = self.devices.get_mut(key) {
            *value = Some(active);
        }
    }
    pub fn fail(&mut self) {
        self.enumerated = false;
        for value in self.devices.values_mut() {
            *value = None;
        }
    }
    pub fn observation(&self) -> CameraObservation {
        if !self.enumerated || self.devices.values().any(Option::is_none) {
            return CameraObservation {
                activity: CameraActivity::Unknown,
                availability: CameraAvailability::Unavailable,
            };
        }
        CameraObservation {
            activity: if self.devices.values().any(|v| *v == Some(true)) {
                CameraActivity::Active
            } else {
                CameraActivity::Inactive
            },
            availability: CameraAvailability::Supported,
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn partial_reports_and_overlapping_cameras_never_imply_inactive() {
        let mut inventory = Inventory::default();
        assert_eq!(inventory.observation().activity, CameraActivity::Unknown);
        inventory.reconcile([1, 2]);
        inventory.update(&1, false);
        assert_eq!(inventory.observation().activity, CameraActivity::Unknown);
        inventory.update(&2, true);
        assert_eq!(inventory.observation().activity, CameraActivity::Active);
        inventory.update(&1, true);
        inventory.update(&2, false);
        assert_eq!(inventory.observation().activity, CameraActivity::Active);
        inventory.update(&1, false);
        assert_eq!(inventory.observation().activity, CameraActivity::Inactive);
    }
    #[test]
    fn removal_hotplug_and_monitor_failure_reconcile() {
        let mut inventory = Inventory::default();
        inventory.reconcile([1]);
        inventory.update(&1, true);
        inventory.reconcile([]);
        assert_eq!(inventory.observation().activity, CameraActivity::Inactive);
        inventory.reconcile([2]);
        assert_eq!(inventory.observation().activity, CameraActivity::Unknown);
        inventory.update(&2, false);
        inventory.fail();
        inventory.reconcile([2]);
        assert_eq!(inventory.observation().activity, CameraActivity::Unknown);
        inventory.update(&2, true);
        assert_eq!(inventory.observation().activity, CameraActivity::Active);
    }
}
