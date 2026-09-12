use crate::{
    config::{AppConfiguration, ConfigStore},
    runtime::RuntimeManager,
};
use serde::Serialize;
use std::sync::{Arc, Mutex};
use tauri::AppHandle;
use tracing_appender::non_blocking::WorkerGuard;

#[allow(dead_code)] // The shell exposes the full status vocabulary before runtime wiring.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum UiStatus {
    Discovering,
    Connecting,
    Synchronized,
    WaitingForSonosConfirmation,
    SubscriptionDegraded,
    PollingFallback,
    SonosUnavailable,
    LocalAudioUnavailable,
    UnsupportedLocalDevice,
    ConfigurationRequired,
    Error,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UiSnapshot {
    #[serde(skip)]
    pub runtime_generation: u64,
    pub configuration: AppConfiguration,
    pub status: UiStatus,
    pub sonos_name: Option<String>,
    pub sonos_volume: Option<u8>,
    pub local_volume: Option<u8>,
    pub muted: Option<bool>,
}

pub struct AppState {
    pub store: ConfigStore,
    pub configuration: Mutex<AppConfiguration>,
    pub snapshot: Arc<Mutex<UiSnapshot>>,
    runtime: RuntimeManager,
    _log_guard: WorkerGuard,
}

impl AppState {
    pub fn new(
        store: ConfigStore,
        configuration: AppConfiguration,
        log_guard: WorkerGuard,
    ) -> Self {
        let status = if configuration.selected_sonos_id.is_some() {
            UiStatus::Connecting
        } else {
            UiStatus::ConfigurationRequired
        };
        Self {
            store,
            configuration: Mutex::new(configuration.clone()),
            snapshot: Arc::new(Mutex::new(UiSnapshot {
                runtime_generation: 0,
                configuration,
                status,
                sonos_name: None,
                sonos_volume: None,
                local_volume: None,
                muted: None,
            })),
            runtime: RuntimeManager::default(),
            _log_guard: log_guard,
        }
    }
    pub fn replace_configuration(&self, configuration: AppConfiguration) {
        if let Ok(mut current) = self.configuration.lock() {
            *current = configuration.clone();
        }
        if let Ok(mut snapshot) = self.snapshot.lock() {
            snapshot.configuration = configuration;
            snapshot.status = UiStatus::Connecting;
        }
    }
    pub fn start_runtime(&self, app: AppHandle) {
        let Ok(configuration) = self.configuration.lock().map(|value| value.clone()) else {
            return;
        };
        self.runtime
            .restart(configuration, Arc::clone(&self.snapshot), app);
    }
    pub fn stop_runtime(&self) {
        self.runtime.stop();
    }
}
