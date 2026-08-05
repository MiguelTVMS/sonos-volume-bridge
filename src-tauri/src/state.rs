use crate::config::{AppConfiguration, ConfigStore};
use serde::Serialize;
use std::sync::Mutex;
use tracing_appender::non_blocking::WorkerGuard;

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum UiStatus { Discovering, Connecting, Synchronized, WaitingForSonosConfirmation, SubscriptionDegraded, PollingFallback, SonosUnavailable, LocalAudioUnavailable, UnsupportedLocalDevice, ConfigurationRequired, Error }

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UiSnapshot { pub configuration: AppConfiguration, pub status: UiStatus, pub sonos_name: Option<String>, pub sonos_volume: Option<u8>, pub local_volume: Option<u8>, pub muted: Option<bool> }

pub struct AppState { pub store: ConfigStore, pub configuration: Mutex<AppConfiguration>, pub snapshot: Mutex<UiSnapshot>, _log_guard: WorkerGuard }

impl AppState {
    pub fn new(store: ConfigStore, configuration: AppConfiguration, log_guard: WorkerGuard) -> Self {
        let status = if configuration.selected_sonos_id.is_some() { UiStatus::Connecting } else { UiStatus::ConfigurationRequired };
        Self { store, configuration: Mutex::new(configuration.clone()), snapshot: Mutex::new(UiSnapshot { configuration, status, sonos_name: None, sonos_volume: None, local_volume: None, muted: None }), _log_guard: log_guard }
    }
    pub fn replace_configuration(&self, configuration: AppConfiguration) { if let Ok(mut current) = self.configuration.lock() { *current = configuration.clone(); } if let Ok(mut snapshot) = self.snapshot.lock() { snapshot.configuration = configuration; snapshot.status = UiStatus::Connecting; } }
}
