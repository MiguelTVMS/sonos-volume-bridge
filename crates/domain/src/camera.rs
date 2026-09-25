//! Camera automation policy. Times are monotonic milliseconds supplied by the caller.

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CameraActivity {
    Active,
    Inactive,
    #[default]
    Unknown,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CameraAvailability {
    Supported,
    #[default]
    Unsupported,
    Unavailable,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CameraObservation {
    pub activity: CameraActivity,
    pub availability: CameraAvailability,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CameraAction {
    Activate,
    Restore,
}

#[derive(Debug, Default)]
pub struct CameraPolicy {
    activity: CameraActivity,
    since: u64,
    attempted: bool,
    paused: bool,
    owned: bool,
}

impl CameraPolicy {
    pub fn observe(&mut self, activity: CameraActivity, now: u64) {
        if activity == CameraActivity::Unknown {
            self.interrupt();
        }
        if self.activity != activity {
            self.activity = activity;
            self.since = now;
        }
    }

    pub fn action(&mut self, now: u64) -> Option<CameraAction> {
        let elapsed = now.saturating_sub(self.since);
        match self.activity {
            CameraActivity::Active if elapsed >= 1_000 && !self.attempted && !self.paused => {
                self.attempted = true;
                Some(CameraAction::Activate)
            }
            CameraActivity::Inactive if elapsed >= 3_000 => {
                self.attempted = false;
                self.paused = false;
                self.owned.then_some(CameraAction::Restore)
            }
            _ => None,
        }
    }

    pub fn claim(&mut self) {
        self.owned = true;
    }

    pub fn release(&mut self) {
        self.owned = false;
    }

    pub fn manual_override(&mut self) {
        self.owned = false;
        self.paused = true;
    }

    pub fn interrupt(&mut self) {
        self.owned = false;
        self.attempted = false;
        self.activity = CameraActivity::Unknown;
    }

    pub fn owned(&self) -> bool {
        self.owned
    }

    pub fn paused(&self) -> bool {
        self.paused
    }
}
