//! Optional Night Mode orchestration, independent from volume synchronization.
use async_trait::async_trait;
use sonos_volume_bridge_domain::NightModeSchedule;

pub const LOCK_MESSAGE: &str =
    "Night Mode is on because of your schedule. Disable the schedule to turn Night Mode off.";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NightModeReading {
    Supported(bool),
    Unsupported,
    Unavailable,
}
#[async_trait]
pub trait NightModePort: Send + Sync {
    async fn read(&self) -> NightModeReading;
    async fn write(&self, value: bool) -> Result<(), String>;
}
#[derive(Default)]
pub struct NightModeController {
    binding: Option<(Option<String>, NightModeSchedule)>,
    expected: Option<bool>,
    pending: bool,
    pending_notification: bool,
    unavailable: bool,
}
#[derive(Debug, PartialEq, Eq)]
pub struct NightModeResult {
    pub reading: NightModeReading,
    pub notification: Option<bool>,
    pub error: Option<String>,
}
impl NightModeController {
    /// Rebind only for selection or schedule changes, never unrelated preferences.
    pub fn configure(&mut self, speaker: Option<&str>, schedule: &NightModeSchedule) -> bool {
        let next = (speaker.map(str::to_owned), schedule.clone());
        if self.binding.as_ref() == Some(&next) {
            return false;
        }
        *self = Self {
            binding: Some(next),
            ..Self::default()
        };
        true
    }

    /// Called by the shell under its selection/write gate. A discontinuity means
    /// startup/wake/recovery/edit rather than a normal scheduled boundary.
    pub async fn step(
        &mut self,
        port: &impl NightModePort,
        expected: bool,
        reconcile: bool,
    ) -> NightModeResult {
        if self
            .binding
            .as_ref()
            .is_some_and(|(speaker, schedule)| speaker.is_none() || !schedule.enabled)
        {
            return NightModeResult {
                reading: NightModeReading::Unavailable,
                notification: None,
                error: None,
            };
        }
        let changed = self.expected.is_some_and(|previous| previous != expected);
        if reconcile || self.expected.is_none() || self.unavailable {
            self.pending = true;
            self.pending_notification = false;
        } else if changed {
            self.pending = true;
            self.pending_notification = true;
        }
        self.expected = Some(expected);
        let reading = port.read().await;
        let NightModeReading::Supported(actual) = reading else {
            self.unavailable = true;
            self.pending_notification = false;
            return NightModeResult {
                reading,
                notification: None,
                error: None,
            };
        };
        self.unavailable = false;
        if (self.pending || expected) && actual != expected {
            if let Err(error) = port.write(expected).await {
                return NightModeResult {
                    reading,
                    notification: None,
                    error: Some(error),
                };
            }
            let confirmed = port.read().await;
            if confirmed != NightModeReading::Supported(expected) {
                return NightModeResult {
                    reading: confirmed,
                    notification: None,
                    error: Some("Could not confirm Night Mode. Retrying automatically.".into()),
                };
            }
        }
        let notification = self.pending_notification.then_some(expected);
        let applied = self.pending || expected;
        self.pending = false;
        self.pending_notification = false;
        NightModeResult {
            reading: NightModeReading::Supported(if applied { expected } else { actual }),
            notification,
            error: None,
        }
    }
}

/// Used by both Settings and tray commands, under the shell's shared write gate.
pub async fn apply_manual(
    port: &impl NightModePort,
    enabled: bool,
    scheduled_active: bool,
) -> Result<(), String> {
    if !enabled && scheduled_active {
        return Err(LOCK_MESSAGE.into());
    }
    match port.read().await {
        NightModeReading::Supported(actual) if actual == enabled => return Ok(()),
        NightModeReading::Supported(_) => {}
        _ => return Err("Night Mode is unavailable for this speaker.".into()),
    }
    port.write(enabled).await?;
    if port.read().await != NightModeReading::Supported(enabled) {
        return Err("Could not confirm Night Mode. Refresh and try again.".into());
    }
    Ok(())
}

/// An explicit Save is a repeatable one-shot application, independent of recurring
/// enablement. Unlike background reconciliation, each confirmed Save may notify.
pub async fn apply_saved(port: &impl NightModePort, expected: bool) -> NightModeResult {
    let mut result = NightModeController::default()
        .step(port, expected, true)
        .await;
    if result.error.is_none() && result.reading == NightModeReading::Supported(expected) {
        result.notification = Some(expected);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;
    struct Port {
        value: Mutex<NightModeReading>,
        writes: Mutex<Vec<bool>>,
        fail: Mutex<bool>,
    }
    impl Port {
        fn new(value: bool) -> Self {
            Self {
                value: Mutex::new(NightModeReading::Supported(value)),
                writes: Mutex::default(),
                fail: Mutex::new(false),
            }
        }
    }
    #[async_trait]
    impl NightModePort for Port {
        async fn read(&self) -> NightModeReading {
            *self.value.lock().unwrap()
        }
        async fn write(&self, value: bool) -> Result<(), String> {
            if *self.fail.lock().unwrap() {
                return Err("offline".into());
            }
            self.writes.lock().unwrap().push(value);
            *self.value.lock().unwrap() = NightModeReading::Supported(value);
            Ok(())
        }
    }
    #[tokio::test]
    async fn startup_external_change_exit_and_manual_activation_sequence() {
        let p = Port::new(false);
        let mut c = NightModeController::default();
        assert_eq!(c.step(&p, true, true).await.notification, None);
        c.step(&p, true, false).await;
        assert_eq!(*p.writes.lock().unwrap(), vec![true]);
        *p.value.lock().unwrap() = NightModeReading::Supported(false);
        c.step(&p, true, false).await;
        assert_eq!(c.step(&p, false, false).await.notification, Some(false));
        *p.value.lock().unwrap() = NightModeReading::Supported(true);
        c.step(&p, false, false).await;
        assert_eq!(*p.value.lock().unwrap(), NightModeReading::Supported(true));
        assert_eq!(*p.writes.lock().unwrap(), vec![true, true, false]);
        assert_eq!(c.step(&p, true, false).await.notification, Some(true));
        assert_eq!(c.step(&p, true, false).await.notification, None);
    }
    #[tokio::test]
    async fn recovery_resume_and_failures_do_not_notify_reconciliation() {
        let p = Port::new(false);
        let mut c = NightModeController::default();
        c.step(&p, false, true).await;
        *p.fail.lock().unwrap() = true;
        assert!(c.step(&p, true, false).await.error.is_some());
        *p.fail.lock().unwrap() = false;
        assert_eq!(c.step(&p, true, false).await.notification, Some(true));
        *p.value.lock().unwrap() = NightModeReading::Unavailable;
        c.step(&p, false, false).await;
        *p.value.lock().unwrap() = NightModeReading::Supported(true);
        assert_eq!(c.step(&p, false, false).await.notification, None);
        assert_eq!(c.step(&p, true, true).await.notification, None);
        *p.value.lock().unwrap() = NightModeReading::Unsupported;
        let before = p.writes.lock().unwrap().len();
        c.step(&p, true, false).await;
        assert_eq!(before, p.writes.lock().unwrap().len());
    }
    #[tokio::test]
    async fn selection_and_disable_cancel_previous_enforcement_and_notifications() {
        let old = Port::new(false);
        let next = Port::new(false);
        let mut schedule = NightModeSchedule {
            enabled: true,
            ..NightModeSchedule::default()
        };
        let mut controller = NightModeController::default();
        assert!(controller.configure(Some("first"), &schedule));
        controller.step(&old, true, true).await;
        assert!(apply_manual(&old, false, true).await.is_err());
        assert!(!controller.configure(Some("first"), &schedule));
        assert!(controller.configure(Some("second"), &schedule));
        assert_eq!(controller.step(&next, true, false).await.notification, None);
        assert_eq!(*old.writes.lock().unwrap(), vec![true]);
        schedule.enabled = false;
        controller.configure(Some("second"), &schedule);
        controller.step(&next, false, true).await;
        assert_eq!(
            *next.value.lock().unwrap(),
            NightModeReading::Supported(true)
        );
        apply_manual(&next, false, false).await.unwrap();
        assert_eq!(*next.writes.lock().unwrap(), vec![true, false]);
        schedule.enabled = true;
        controller.configure(None, &schedule);
        controller.step(&next, true, true).await;
        assert_eq!(*next.writes.lock().unwrap(), vec![true, false]);
    }
    #[tokio::test]
    async fn saving_applies_on_and_off_and_repeats_notifications_without_duplicate_writes() {
        let port = Port::new(false);
        assert_eq!(apply_saved(&port, true).await.notification, Some(true));
        assert_eq!(apply_saved(&port, true).await.notification, Some(true));
        assert_eq!(*port.writes.lock().unwrap(), vec![true]);
        assert_eq!(apply_saved(&port, false).await.notification, Some(false));
        assert_eq!(*port.writes.lock().unwrap(), vec![true, false]);
        *port.fail.lock().unwrap() = true;
        let failed = apply_saved(&port, true).await;
        assert!(failed.error.is_some());
        assert_eq!(failed.notification, None);
        *port.value.lock().unwrap() = NightModeReading::Unsupported;
        assert_eq!(apply_saved(&port, true).await.notification, None);
    }
}
