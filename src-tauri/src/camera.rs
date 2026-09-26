//! Application-lifetime coordination, independent of volume runtime generations.
use crate::{config::AppConfiguration, runtime};
use async_trait::async_trait;
use serde::Serialize;
use sonos_volume_bridge_domain::camera::{CameraActivity, CameraAvailability, CameraObservation};
use sonos_volume_bridge_integration::camera::{
    AutomationStatus, CameraCoordinator, CameraPort, SpeechError, SpeechPort,
};
use sonos_volume_bridge_sonos::{SonosClient, SonosError};
use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    time::{Duration, Instant, SystemTime},
};
use tokio::sync::Mutex as AsyncMutex;

struct Speaker(AppConfiguration);
fn speech_error(error: &SonosError) -> SpeechError {
    match error {
        SonosError::UnsupportedService | SonosError::SoapFault(401 | 602) => {
            SpeechError::Unsupported
        }
        _ => SpeechError::Unavailable,
    }
}
#[async_trait]
impl SpeechPort for Speaker {
    async fn read(&self) -> Result<bool, SpeechError> {
        tokio::time::timeout(Duration::from_secs(3), async {
            let client = SonosClient::builder()
                .timeout(Duration::from_secs(2))
                .build()
                .map_err(|_| SpeechError::Unavailable)?;
            let device = runtime::resolve_device(&client, &self.0)
                .await
                .map_err(|_| SpeechError::Unavailable)?;
            client
                .get_speech_enhancement(&device)
                .await
                .map_err(|e| speech_error(&e))
        })
        .await
        .unwrap_or(Err(SpeechError::Unavailable))
    }
    async fn write(&self, enabled: bool) -> Result<(), SpeechError> {
        tokio::time::timeout(Duration::from_secs(3), async {
            let client = SonosClient::builder()
                .timeout(Duration::from_secs(2))
                .build()
                .map_err(|_| SpeechError::Unavailable)?;
            let device = runtime::resolve_device(&client, &self.0)
                .await
                .map_err(|_| SpeechError::Unavailable)?;
            client
                .set_speech_enhancement(&device, enabled)
                .await
                .map_err(|e| speech_error(&e))
        })
        .await
        .unwrap_or(Err(SpeechError::Unavailable))
    }
}
// A newer manual/configuration request invalidates work that has not started a write.
// A write already in flight completes under the session lock, allowing old-speaker cleanup.
struct GuardedSpeaker<'a> {
    speaker: &'a dyn SpeechPort,
    revision: &'a AtomicU64,
    expected: u64,
}
#[async_trait]
impl SpeechPort for GuardedSpeaker<'_> {
    async fn read(&self) -> Result<bool, SpeechError> {
        let value = self.speaker.read().await?;
        if self.revision.load(Ordering::SeqCst) != self.expected {
            return Err(SpeechError::Unavailable);
        }
        Ok(value)
    }
    async fn write(&self, enabled: bool) -> Result<(), SpeechError> {
        if self.revision.load(Ordering::SeqCst) != self.expected {
            return Err(SpeechError::Unavailable);
        }
        self.speaker.write(enabled).await
    }
}
type SpeakerFactory = dyn Fn(AppConfiguration) -> Box<dyn SpeechPort> + Send + Sync;

#[derive(Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CameraStatus {
    pub available: bool,
    pub message: &'static str,
    pub warning: Option<&'static str>,
}
struct Session {
    configuration: AppConfiguration,
    coordinator: CameraCoordinator,
    speech: Result<bool, SpeechError>,
}
pub struct CameraAutomation {
    session: AsyncMutex<Session>,
    revision: AtomicU64,
    refresh: AtomicBool,
    stopped: AtomicBool,
    shutdown_complete: AtomicBool,
    resumed: AtomicBool,
    status: Mutex<CameraStatus>,
    factory: Arc<SpeakerFactory>,
}
impl CameraAutomation {
    pub fn new(configuration: AppConfiguration) -> Arc<Self> {
        Self::with_factory(configuration, Arc::new(|c| Box::new(Speaker(c))))
    }
    fn with_factory(configuration: AppConfiguration, factory: Arc<SpeakerFactory>) -> Arc<Self> {
        Arc::new(Self {
            session: AsyncMutex::new(Session {
                configuration,
                coordinator: CameraCoordinator::default(),
                speech: Err(SpeechError::Unavailable),
            }),
            revision: AtomicU64::new(0),
            refresh: AtomicBool::new(true),
            stopped: AtomicBool::new(false),
            shutdown_complete: AtomicBool::new(false),
            resumed: AtomicBool::new(false),
            status: Mutex::new(CameraStatus {
                available: false,
                message: AutomationStatus::UnsupportedPlatform.message(),
                warning: None,
            }),
            factory,
        })
    }
    pub fn status(&self) -> CameraStatus {
        self.status.lock().map_or(
            CameraStatus {
                available: false,
                message: AutomationStatus::Unavailable.message(),
                warning: None,
            },
            |s| s.clone(),
        )
    }
    fn publish(&self, status: AutomationStatus, available: bool) {
        if let Ok(mut output) = self.status.lock() {
            output.available = available;
            output.message = status.message();
            if status == AutomationStatus::RestorationFailed {
                output.warning = Some(status.message());
            }
        }
    }
    pub fn request_refresh(&self) {
        self.refresh.store(true, Ordering::Release);
    }

    pub async fn configure(&self, configuration: AppConfiguration) {
        let cleanup_started = tokio::time::Instant::now();
        self.revision.fetch_add(1, Ordering::SeqCst);
        let mut session = self.session.lock().await;
        let changed = session.configuration.selected_sonos_id != configuration.selected_sonos_id
            || session.configuration.camera_speech_enhancement_enabled
                != configuration.camera_speech_enhancement_enabled;
        if changed {
            let speaker = (self.factory)(session.configuration.clone());
            let remaining = Duration::from_secs(5).saturating_sub(cleanup_started.elapsed());
            if remaining.is_zero()
                || tokio::time::timeout(remaining, session.coordinator.cleanup(&*speaker))
                    .await
                    .is_err()
            {
                session.coordinator.status = AutomationStatus::RestorationFailed;
            }
            self.publish(session.coordinator.status, false);
            session.coordinator = CameraCoordinator::default();
            session.speech = Err(SpeechError::Unavailable);
        }
        session.configuration = configuration;
        self.request_refresh();
    }
    pub async fn manual(
        &self,
        configuration: AppConfiguration,
        enabled: bool,
    ) -> Result<(), String> {
        self.revision.fetch_add(1, Ordering::SeqCst);
        let mut session = self.session.lock().await;
        if configuration.selected_sonos_id != session.configuration.selected_sonos_id {
            return Err("The selected speaker changed. Refresh and try again.".into());
        }
        let speaker = (self.factory)(configuration);
        let result = session.coordinator.manual(enabled, &*speaker).await;
        self.publish(
            session.coordinator.status,
            sonos_volume_bridge_platform_camera::compiled(),
        );
        self.request_refresh();
        result.map_err(|_| "Could not confirm Speech Enhancement. Refresh and try again.".into())
    }
    pub async fn shutdown(&self) {
        self.stopped.store(true, Ordering::Release);
        self.revision.fetch_add(1, Ordering::SeqCst);
        // The overall deadline includes waiting for any outstanding speaker operation.
        let result = tokio::time::timeout(Duration::from_secs(5), async {
            let mut session = self.session.lock().await;
            let speaker = (self.factory)(session.configuration.clone());
            session.coordinator.cleanup(&*speaker).await;
            self.publish(session.coordinator.status, false);
            session.configuration.camera_speech_enhancement_enabled = false;
        })
        .await;
        if result.is_err() {
            self.publish(AutomationStatus::RestorationFailed, false);
        }
        self.shutdown_complete.store(true, Ordering::Release);
    }
    pub fn begin_shutdown(&self) -> bool {
        !self.stopped.swap(true, Ordering::AcqRel)
    }
    pub fn shutdown_complete(&self) -> bool {
        self.shutdown_complete.load(Ordering::Acquire)
    }
    pub fn stopping(&self) -> bool {
        self.stopped.load(Ordering::Acquire)
    }
    pub fn resume(&self) {
        self.resumed.store(true, Ordering::Release);
    }

    // Shared production entry point: fake ports exercise the same lifecycle as native observation.
    async fn advance(&self, camera: CameraObservation, now: u64, refresh: bool) {
        let revision = self.revision.load(Ordering::SeqCst);
        let mut session = self.session.lock().await;
        if self.stopping() {
            return;
        }
        if camera.availability != CameraAvailability::Supported {
            session.coordinator.interrupt();
            self.publish(
                if camera.availability == CameraAvailability::Unsupported {
                    AutomationStatus::UnsupportedPlatform
                } else {
                    AutomationStatus::Unavailable
                },
                false,
            );
            return;
        }
        let speaker = (self.factory)(session.configuration.clone());
        if refresh {
            session.speech = speaker.read().await;
            let speech = session.speech;
            session.coordinator.observed_speech(speech);
        }
        if self.revision.load(Ordering::SeqCst) != revision {
            return;
        }
        if let Err(error) = session.speech {
            session.coordinator.observed_speech(Err(error));
            self.publish(session.coordinator.status, false);
            return;
        }
        if !session.configuration.camera_speech_enhancement_enabled {
            self.publish(AutomationStatus::Off, true);
            return;
        }
        let guarded = GuardedSpeaker {
            speaker: &*speaker,
            revision: &self.revision,
            expected: revision,
        };
        session.coordinator.tick(camera, now, &guarded).await;
        self.publish(
            session.coordinator.status,
            !matches!(
                session.coordinator.status,
                AutomationStatus::UnsupportedSpeaker | AutomationStatus::Unavailable
            ),
        );
    }

    pub fn start(self: &Arc<Self>, app: tauri::AppHandle) {
        let this = Arc::clone(self);
        tauri::async_runtime::spawn(async move {
            let mut detector: Option<Box<dyn CameraPort>> = None;
            let epoch = Instant::now();
            let mut previous = SystemTime::now();
            let mut last_read: Option<Instant> = None;
            let mut previous_status = this.status();
            while !this.stopping() {
                tokio::time::sleep(Duration::from_millis(100)).await;
                let status = this.status();
                if status != previous_status {
                    previous_status = status;
                    crate::tray::refresh_camera_status(&app);
                    crate::tray::refresh_speaker_controls(&app);
                }
                let gap = previous
                    .elapsed()
                    .map_or(true, |d| d > Duration::from_secs(10))
                    || this.resumed.swap(false, Ordering::AcqRel);
                previous = SystemTime::now();
                let enabled = {
                    let mut session = this.session.lock().await;
                    if gap {
                        session.coordinator.interrupt();
                    }
                    session.configuration.camera_speech_enhancement_enabled
                };
                if gap || !enabled {
                    detector = None;
                }
                if gap {
                    last_read = None;
                }
                let camera = if !sonos_volume_bridge_platform_camera::compiled() {
                    CameraObservation::default()
                } else if enabled {
                    detector
                        .get_or_insert_with(sonos_volume_bridge_platform_camera::start)
                        .observation()
                } else {
                    // Capability display may read speaker state, but does not monitor cameras until opt-in.
                    CameraObservation {
                        activity: CameraActivity::Unknown,
                        availability: CameraAvailability::Supported,
                    }
                };
                let refresh = this.refresh.swap(false, Ordering::AcqRel)
                    || last_read.is_none_or(|t| t.elapsed() >= Duration::from_secs(5));
                this.advance(
                    camera,
                    u64::try_from(epoch.elapsed().as_millis()).unwrap_or(u64::MAX),
                    refresh,
                )
                .await;
                if refresh {
                    last_read = Some(Instant::now());
                }
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;
    #[derive(Default)]
    struct FakeState {
        values: BTreeMap<String, bool>,
        writes: Vec<(String, bool)>,
        fail: bool,
    }
    struct FakeSpeaker {
        key: String,
        state: Arc<Mutex<FakeState>>,
    }
    #[async_trait]
    impl SpeechPort for FakeSpeaker {
        async fn read(&self) -> Result<bool, SpeechError> {
            let state = self.state.lock().unwrap();
            if state.fail {
                Err(SpeechError::Unavailable)
            } else {
                Ok(*state.values.get(&self.key).unwrap_or(&false))
            }
        }
        async fn write(&self, enabled: bool) -> Result<(), SpeechError> {
            let mut state = self.state.lock().unwrap();
            if state.fail {
                return Err(SpeechError::Unavailable);
            }
            state.values.insert(self.key.clone(), enabled);
            state.writes.push((self.key.clone(), enabled));
            Ok(())
        }
    }
    fn configuration(key: &str) -> AppConfiguration {
        AppConfiguration {
            selected_sonos_id: Some(key.into()),
            camera_speech_enhancement_enabled: true,
            ..AppConfiguration::default()
        }
    }
    fn setup() -> (Arc<CameraAutomation>, Arc<Mutex<FakeState>>) {
        let state = Arc::new(Mutex::new(FakeState::default()));
        let output = Arc::clone(&state);
        let factory: Arc<SpeakerFactory> = Arc::new(move |c| {
            Box::new(FakeSpeaker {
                key: c.selected_sonos_id.unwrap_or_default(),
                state: Arc::clone(&output),
            })
        });
        (
            CameraAutomation::with_factory(configuration("first"), factory),
            state,
        )
    }
    fn camera(activity: CameraActivity) -> CameraObservation {
        CameraObservation {
            activity,
            availability: CameraAvailability::Supported,
        }
    }
    async fn activate(service: &CameraAutomation) {
        service
            .advance(camera(CameraActivity::Active), 0, true)
            .await;
        service
            .advance(camera(CameraActivity::Active), 1000, false)
            .await;
    }
    #[tokio::test]
    async fn unrelated_settings_preserve_ownership_and_switch_cleans_old_speaker_first() {
        let (service, state) = setup();
        activate(&service).await;
        let mut changed = configuration("first");
        changed.synchronize_mute = false;
        service.configure(changed).await;
        assert_eq!(state.lock().unwrap().writes, [("first".into(), true)]);
        service.configure(configuration("second")).await;
        assert_eq!(
            state.lock().unwrap().writes,
            [("first".into(), true), ("first".into(), false)]
        );
        activate(&service).await;
        assert_eq!(
            state.lock().unwrap().writes.last(),
            Some(&("second".into(), true))
        );
    }
    #[tokio::test]
    async fn settings_and_tray_manual_entrypoint_relinquishes_ownership() {
        let (service, state) = setup();
        activate(&service).await;
        service.manual(configuration("first"), false).await.unwrap();
        service
            .advance(camera(CameraActivity::Active), 9000, true)
            .await;
        service
            .advance(camera(CameraActivity::Inactive), 10000, false)
            .await;
        service
            .advance(camera(CameraActivity::Inactive), 13000, false)
            .await;
        assert_eq!(
            state.lock().unwrap().writes,
            [("first".into(), true), ("first".into(), false)]
        );
        service
            .advance(camera(CameraActivity::Active), 14000, false)
            .await;
        service
            .advance(camera(CameraActivity::Active), 15000, false)
            .await;
        assert_eq!(state.lock().unwrap().writes.len(), 3);
    }
    #[tokio::test]
    async fn explicit_manual_on_keeps_speech_on_after_camera_stops() {
        let (service, state) = setup();
        activate(&service).await;
        service.manual(configuration("first"), true).await.unwrap();
        service
            .advance(camera(CameraActivity::Inactive), 2000, true)
            .await;
        service
            .advance(camera(CameraActivity::Inactive), 5000, false)
            .await;
        service.shutdown().await;
        assert_eq!(
            state.lock().unwrap().writes,
            [("first".into(), true), ("first".into(), true)]
        );
    }
    #[tokio::test]
    async fn disable_reset_and_quit_restore_only_owned_state() {
        for operation in 0..3 {
            let (service, state) = setup();
            activate(&service).await;
            match operation {
                0 => {
                    let mut c = configuration("first");
                    c.camera_speech_enhancement_enabled = false;
                    service.configure(c).await;
                }
                1 => service.configure(AppConfiguration::default()).await,
                _ => service.shutdown().await,
            }
            assert_eq!(
                state.lock().unwrap().writes,
                [("first".into(), true), ("first".into(), false)]
            );
        }
    }
    #[tokio::test]
    async fn unavailable_read_and_camera_gap_never_restore_stale_ownership() {
        for camera_gap in [true, false] {
            let (service, state) = setup();
            activate(&service).await;
            if camera_gap {
                service
                    .advance(CameraObservation::default(), 2000, false)
                    .await;
            } else {
                state.lock().unwrap().fail = true;
                service
                    .advance(camera(CameraActivity::Active), 2000, true)
                    .await;
                state.lock().unwrap().fail = false;
            }
            service.shutdown().await;
            assert_eq!(state.lock().unwrap().writes, [("first".into(), true)]);
        }
    }
    #[tokio::test]
    async fn failed_cleanup_is_reported_and_not_retried_on_new_speaker() {
        let (service, state) = setup();
        activate(&service).await;
        state.lock().unwrap().fail = true;
        service.configure(configuration("second")).await;
        assert!(service.status().warning.is_some());
        state.lock().unwrap().fail = false;
        activate(&service).await;
        assert_eq!(
            state.lock().unwrap().writes,
            [("first".into(), true), ("second".into(), true)]
        );
    }
    #[tokio::test]
    async fn external_off_and_preexisting_on_are_respected() {
        let (service, state) = setup();
        activate(&service).await;
        state.lock().unwrap().values.insert("first".into(), false);
        service
            .advance(camera(CameraActivity::Active), 2000, true)
            .await;
        service.shutdown().await;
        assert_eq!(state.lock().unwrap().writes, [("first".into(), true)]);
        let (service, state) = setup();
        state.lock().unwrap().values.insert("first".into(), true);
        activate(&service).await;
        service.shutdown().await;
        assert!(state.lock().unwrap().writes.is_empty());
    }
    #[tokio::test]
    async fn revision_guard_rejects_stale_read_before_any_write() {
        let state = Arc::new(Mutex::new(FakeState::default()));
        let speaker = FakeSpeaker {
            key: "first".into(),
            state: Arc::clone(&state),
        };
        let revision = AtomicU64::new(1);
        let guarded = GuardedSpeaker {
            speaker: &speaker,
            revision: &revision,
            expected: 0,
        };
        let mut coordinator = CameraCoordinator::default();
        coordinator
            .tick(camera(CameraActivity::Active), 0, &guarded)
            .await;
        coordinator
            .tick(camera(CameraActivity::Active), 1000, &guarded)
            .await;
        assert!(state.lock().unwrap().writes.is_empty());
    }
    struct GatedSpeaker {
        inner: FakeSpeaker,
        started: Arc<tokio::sync::Notify>,
        release: Arc<tokio::sync::Notify>,
    }
    #[async_trait]
    impl SpeechPort for GatedSpeaker {
        async fn read(&self) -> Result<bool, SpeechError> {
            self.inner.read().await
        }
        async fn write(&self, enabled: bool) -> Result<(), SpeechError> {
            if enabled {
                self.started.notify_one();
                self.release.notified().await;
            }
            self.inner.write(enabled).await
        }
    }
    #[tokio::test]
    async fn switch_waits_for_inflight_enable_then_restores_old_target() {
        let state = Arc::new(Mutex::new(FakeState::default()));
        let started = Arc::new(tokio::sync::Notify::new());
        let release = Arc::new(tokio::sync::Notify::new());
        let output = Arc::clone(&state);
        let notify_started = Arc::clone(&started);
        let notify_release = Arc::clone(&release);
        let service = CameraAutomation::with_factory(
            configuration("first"),
            Arc::new(move |c| {
                Box::new(GatedSpeaker {
                    inner: FakeSpeaker {
                        key: c.selected_sonos_id.unwrap_or_default(),
                        state: Arc::clone(&output),
                    },
                    started: Arc::clone(&notify_started),
                    release: Arc::clone(&notify_release),
                })
            }),
        );
        service
            .advance(camera(CameraActivity::Active), 0, true)
            .await;
        let pending = Arc::clone(&service);
        let activation = tokio::spawn(async move {
            pending
                .advance(camera(CameraActivity::Active), 1000, false)
                .await;
        });
        started.notified().await;
        let pending = Arc::clone(&service);
        let selection = tokio::spawn(async move {
            pending.configure(configuration("second")).await;
        });
        tokio::time::timeout(Duration::from_secs(1), async {
            while service.revision.load(Ordering::SeqCst) == 0 {
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        release.notify_one();
        activation.await.unwrap();
        selection.await.unwrap();
        assert_eq!(
            state.lock().unwrap().writes,
            [("first".into(), true), ("first".into(), false)]
        );
    }

    struct HangingSpeaker;
    #[async_trait]
    impl SpeechPort for HangingSpeaker {
        async fn read(&self) -> Result<bool, SpeechError> {
            std::future::pending().await
        }
        async fn write(&self, _: bool) -> Result<(), SpeechError> {
            std::future::pending().await
        }
    }
    #[tokio::test(start_paused = true)]
    async fn shutdown_deadline_includes_waiting_for_an_inflight_read() {
        let service = CameraAutomation::with_factory(
            configuration("first"),
            Arc::new(|_| Box::new(HangingSpeaker)),
        );
        let pending = Arc::clone(&service);
        let read = tokio::spawn(async move {
            pending
                .advance(camera(CameraActivity::Active), 0, true)
                .await;
        });
        tokio::task::yield_now().await;
        let start = tokio::time::Instant::now();
        service.shutdown().await;
        assert_eq!(start.elapsed(), Duration::from_secs(5));
        assert!(service.status().warning.is_some());
        read.abort();
    }
}
