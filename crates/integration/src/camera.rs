//! Serialized orchestration shared by native camera automation and its tests.
use async_trait::async_trait;
use sonos_volume_bridge_domain::camera::{
    CameraAction, CameraActivity, CameraAvailability, CameraObservation, CameraPolicy,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SpeechError {
    Unsupported,
    Unavailable,
}

#[async_trait]
pub trait SpeechPort: Send + Sync {
    async fn read(&self) -> Result<bool, SpeechError>;
    /// Must verify authoritative readback before returning success.
    async fn write(&self, enabled: bool) -> Result<(), SpeechError>;
}

pub trait CameraPort: Send {
    fn observation(&self) -> CameraObservation;
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum AutomationStatus {
    #[default]
    Off,
    Waiting,
    Owned,
    Paused,
    UnsupportedPlatform,
    UnsupportedSpeaker,
    Unavailable,
    RestorationFailed,
}

impl AutomationStatus {
    pub fn message(self) -> &'static str {
        match self {
            Self::Off => "Camera automation is off",
            Self::Waiting => "Camera automation is waiting",
            Self::Owned => "Speech Enhancement enabled by camera automation",
            Self::Paused => "Camera automation paused after a manual change",
            Self::UnsupportedPlatform => "Camera automation is unavailable in this platform build",
            Self::UnsupportedSpeaker => "Speech Enhancement is not supported by this speaker",
            Self::Unavailable => "Camera automation is temporarily unavailable",
            Self::RestorationFailed => {
                "The previous speaker may still have Speech Enhancement enabled"
            }
        }
    }
}

#[derive(Default)]
pub struct CameraCoordinator {
    policy: CameraPolicy,
    ambiguous_write: bool,
    pub status: AutomationStatus,
}

impl CameraCoordinator {
    pub async fn tick(&mut self, camera: CameraObservation, now: u64, speaker: &dyn SpeechPort) {
        if camera.availability != CameraAvailability::Supported {
            self.policy.interrupt();
            self.status = if camera.availability == CameraAvailability::Unsupported {
                AutomationStatus::UnsupportedPlatform
            } else {
                AutomationStatus::Unavailable
            };
            return;
        }
        self.policy.observe(camera.activity, now);
        if camera.activity == CameraActivity::Unknown {
            self.status = AutomationStatus::Unavailable;
            return;
        }
        if let Some(action) = self.policy.action(now) {
            match action {
                CameraAction::Activate => match speaker.read().await {
                    Ok(true) => {}
                    Ok(false) => match speaker.write(true).await {
                        Ok(()) => self.policy.claim(),
                        Err(error) => {
                            self.fail(error);
                            // Do not repeat an ambiguous write in this active period.
                            self.policy.manual_override();
                            self.ambiguous_write = true;
                            return;
                        }
                    },
                    Err(error) => {
                        self.fail(error);
                        return;
                    }
                },
                CameraAction::Restore => {
                    self.cleanup(speaker).await;
                    if self.status == AutomationStatus::RestorationFailed {
                        return;
                    }
                }
            }
        }
        if !self.policy.paused() {
            self.ambiguous_write = false;
        }
        self.status = if self.ambiguous_write {
            AutomationStatus::Unavailable
        } else if self.policy.owned() {
            AutomationStatus::Owned
        } else if self.policy.paused() {
            AutomationStatus::Paused
        } else {
            AutomationStatus::Waiting
        };
    }

    pub fn observed_speech(&mut self, value: Result<bool, SpeechError>) {
        match value {
            Ok(false) if self.policy.owned() => {
                self.policy.manual_override();
                self.status = AutomationStatus::Paused;
            }
            Err(error) => self.fail(error),
            _ => {}
        }
    }

    pub async fn manual(
        &mut self,
        enabled: bool,
        speaker: &dyn SpeechPort,
    ) -> Result<(), SpeechError> {
        self.policy.manual_override();
        self.ambiguous_write = false;
        self.status = AutomationStatus::Paused;
        speaker.write(enabled).await
    }

    pub async fn cleanup(&mut self, speaker: &dyn SpeechPort) {
        let owned = self.policy.owned();
        self.policy.release();
        self.status = AutomationStatus::Off;
        if owned {
            let result = match speaker.read().await {
                Ok(true) => speaker.write(false).await,
                Ok(false) => Ok(()),
                Err(error) => Err(error),
            };
            if result.is_err() {
                self.status = AutomationStatus::RestorationFailed;
            }
        }
    }

    pub fn interrupt(&mut self) {
        self.policy.interrupt();
        self.status = AutomationStatus::Unavailable;
    }

    fn fail(&mut self, error: SpeechError) {
        self.policy.interrupt();
        self.status = match error {
            SpeechError::Unsupported => AutomationStatus::UnsupportedSpeaker,
            SpeechError::Unavailable => AutomationStatus::Unavailable,
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    #[derive(Default)]
    struct Speaker(Mutex<(bool, Vec<bool>, bool)>);
    #[async_trait]
    impl SpeechPort for Speaker {
        async fn read(&self) -> Result<bool, SpeechError> {
            let s = self.0.lock().unwrap();
            if s.2 {
                Err(SpeechError::Unavailable)
            } else {
                Ok(s.0)
            }
        }
        async fn write(&self, enabled: bool) -> Result<(), SpeechError> {
            let mut s = self.0.lock().unwrap();
            s.0 = enabled;
            s.1.push(enabled);
            if s.2 {
                Err(SpeechError::Unavailable)
            } else {
                Ok(())
            }
        }
    }
    fn observation(activity: CameraActivity) -> CameraObservation {
        CameraObservation {
            activity,
            availability: CameraAvailability::Supported,
        }
    }
    async fn tick(c: &mut CameraCoordinator, s: &Speaker, activity: CameraActivity, now: u64) {
        c.tick(observation(activity), now, s).await;
    }
    #[tokio::test]
    async fn startup_active_debounce_gap_and_last_camera_restore() {
        let s = Speaker::default();
        let mut c = CameraCoordinator::default();
        tick(&mut c, &s, CameraActivity::Active, 0).await;
        tick(&mut c, &s, CameraActivity::Active, 999).await;
        assert!(s.0.lock().unwrap().1.is_empty());
        tick(&mut c, &s, CameraActivity::Active, 1000).await;
        tick(&mut c, &s, CameraActivity::Active, 2000).await;
        tick(&mut c, &s, CameraActivity::Inactive, 3000).await;
        tick(&mut c, &s, CameraActivity::Active, 4000).await;
        tick(&mut c, &s, CameraActivity::Inactive, 5000).await;
        tick(&mut c, &s, CameraActivity::Inactive, 7999).await;
        assert_eq!(s.0.lock().unwrap().1, [true]);
        tick(&mut c, &s, CameraActivity::Inactive, 8000).await;
        tick(&mut c, &s, CameraActivity::Inactive, 9000).await;
        assert_eq!(s.0.lock().unwrap().1, [true, false]);
    }
    #[tokio::test]
    async fn preexisting_on_and_crash_restart_never_restore() {
        let s = Speaker::default();
        s.0.lock().unwrap().0 = true;
        let mut c = CameraCoordinator::default();
        tick(&mut c, &s, CameraActivity::Active, 0).await;
        tick(&mut c, &s, CameraActivity::Active, 1000).await;
        c.cleanup(&s).await;
        assert!(s.0.lock().unwrap().1.is_empty());
    }
    #[tokio::test]
    async fn manual_override_does_not_reactivate_until_next_period() {
        let s = Speaker::default();
        let mut c = CameraCoordinator::default();
        tick(&mut c, &s, CameraActivity::Active, 0).await;
        tick(&mut c, &s, CameraActivity::Active, 1000).await;
        c.manual(false, &s).await.unwrap();
        tick(&mut c, &s, CameraActivity::Active, 2000).await;
        c.cleanup(&s).await;
        assert_eq!(s.0.lock().unwrap().1, [true, false]);
    }
    #[tokio::test]
    async fn external_override_and_observation_loss_abandon_ownership() {
        for external in [true, false] {
            let s = Speaker::default();
            let mut c = CameraCoordinator::default();
            tick(&mut c, &s, CameraActivity::Active, 0).await;
            tick(&mut c, &s, CameraActivity::Active, 1000).await;
            if external {
                c.observed_speech(Ok(false));
            } else {
                c.interrupt();
            }
            c.cleanup(&s).await;
            assert_eq!(s.0.lock().unwrap().1, [true]);
        }
    }
    #[tokio::test]
    async fn unsupported_camera_and_short_bursts_do_not_write() {
        let s = Speaker::default();
        let mut c = CameraCoordinator::default();
        c.tick(CameraObservation::default(), 0, &s).await;
        assert_eq!(c.status, AutomationStatus::UnsupportedPlatform);
        tick(&mut c, &s, CameraActivity::Active, 0).await;
        tick(&mut c, &s, CameraActivity::Inactive, 500).await;
        tick(&mut c, &s, CameraActivity::Inactive, 4000).await;
        assert!(s.0.lock().unwrap().1.is_empty());
    }
    struct UnconfirmedWrite(Mutex<Vec<bool>>);
    #[async_trait]
    impl SpeechPort for UnconfirmedWrite {
        async fn read(&self) -> Result<bool, SpeechError> {
            Ok(false)
        }
        async fn write(&self, enabled: bool) -> Result<(), SpeechError> {
            self.0.lock().unwrap().push(enabled);
            Err(SpeechError::Unavailable)
        }
    }
    #[tokio::test]
    async fn ambiguous_enable_is_not_retried_or_mislabelled_as_manual() {
        let speaker = UnconfirmedWrite(Mutex::new(Vec::new()));
        let mut coordinator = CameraCoordinator::default();
        coordinator
            .tick(observation(CameraActivity::Active), 0, &speaker)
            .await;
        coordinator
            .tick(observation(CameraActivity::Active), 1000, &speaker)
            .await;
        coordinator
            .tick(observation(CameraActivity::Active), 9000, &speaker)
            .await;
        assert_eq!(coordinator.status, AutomationStatus::Unavailable);
        assert_eq!(*speaker.0.lock().unwrap(), [true]);
        coordinator.cleanup(&speaker).await;
        assert_eq!(*speaker.0.lock().unwrap(), [true]);
    }
}
