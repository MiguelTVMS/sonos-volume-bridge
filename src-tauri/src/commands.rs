use crate::{
    autostart,
    config::AppConfiguration,
    runtime::{self, AvailableAudioOutput, DiscoveredSonos},
    state::{AppState, UiSnapshot},
};
use serde::Serialize;
use tauri::{AppHandle, State};

#[tauri::command]
#[allow(clippy::needless_pass_by_value)] // Tauri injects managed state by value.
pub fn get_snapshot(state: State<'_, AppState>) -> Result<UiSnapshot, String> {
    state
        .snapshot
        .lock()
        .map(|snapshot| snapshot.clone())
        .map_err(|_| "application state is unavailable".to_owned())
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)] // Tauri owns command argument extraction.
pub fn save_configuration(
    configuration: AppConfiguration,
    state: State<'_, AppState>,
    app: AppHandle,
) -> Result<UiSnapshot, String> {
    state
        .store
        .save(&configuration)
        .map_err(|error| error.to_string())?;
    autostart::update(&app, configuration.start_at_login)?;
    state.replace_configuration(configuration);
    state.start_runtime(app);
    get_snapshot(state)
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)] // Tauri owns command argument extraction.
pub fn reset_configuration(
    state: State<'_, AppState>,
    app: AppHandle,
) -> Result<UiSnapshot, String> {
    let configuration = state.store.reset().map_err(|error| error.to_string())?;
    state.replace_configuration(configuration);
    state.start_runtime(app);
    get_snapshot(state)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
#[allow(clippy::struct_excessive_bools)] // The diagnostics report mirrors independent persisted toggles.
pub struct Diagnostics {
    pub configuration_present: bool,
    pub sanitized: bool,
    pub message: String,
    pub status: String,
    pub speaker_name: Option<String>,
    pub selected_sonos_id: Option<String>,
    pub last_known_sonos_address: Option<String>,
    pub sonos_volume: Option<u8>,
    pub local_volume: Option<u8>,
    pub muted: Option<bool>,
    pub follows_system_output: bool,
    pub fixed_audio_device_id: Option<String>,
    pub synchronize_mute: bool,
    pub two_way_synchronization: bool,
    pub fallback_polling: bool,
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)] // Tauri injects managed state by value.
pub fn diagnostics(state: State<'_, AppState>) -> Diagnostics {
    let snapshot = state.snapshot.lock().map_or_else(
        |_| UiSnapshot {
            runtime_generation: 0,
            configuration: AppConfiguration::default(),
            status: crate::state::UiStatus::Error,
            sonos_name: None,
            sonos_volume: None,
            local_volume: None,
            muted: None,
        },
        |snapshot| snapshot.clone(),
    );
    build_diagnostics(state.store.path().exists(), &snapshot)
}

fn build_diagnostics(configuration_present: bool, snapshot: &UiSnapshot) -> Diagnostics {
    let configuration = &snapshot.configuration;
    Diagnostics {
        configuration_present,
        sanitized: true,
        message:
            "This report intentionally redacts local speaker identity and endpoint information. It excludes protocol payloads and host paths."
                .to_owned(),
        status: format!("{:?}", snapshot.status),
        speaker_name: snapshot.sonos_name.clone(),
        selected_sonos_id: None,
        last_known_sonos_address: None,
        sonos_volume: snapshot.sonos_volume,
        local_volume: snapshot.local_volume,
        muted: snapshot.muted,
        follows_system_output: configuration.follow_default_audio_device,
        fixed_audio_device_id: configuration.fixed_audio_device_id.clone(),
        synchronize_mute: configuration.synchronize_mute,
        two_way_synchronization: configuration.two_way_synchronization,
        fallback_polling: configuration.fallback_polling,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn diagnostics_redacts_identity_and_endpoint() {
        let configuration = AppConfiguration {
            selected_sonos_id: Some("uuid:1234".to_owned()),
            last_known_sonos_address: Some(
                "http://192.168.1.42:1400/xml/device_description.xml".to_owned(),
            ),
            ..AppConfiguration::default()
        };

        let snapshot = UiSnapshot {
            runtime_generation: 1,
            configuration,
            status: crate::state::UiStatus::Synchronized,
            sonos_name: Some("Office".to_owned()),
            sonos_volume: Some(40),
            local_volume: Some(20),
            muted: Some(false),
        };

        let diagnostics = build_diagnostics(true, &snapshot);

        assert!(diagnostics.sanitized);
        assert!(diagnostics.message.contains("redacts"));
        assert!(diagnostics.configuration_present);
        assert_eq!(diagnostics.status, "Synchronized");
        assert_eq!(diagnostics.speaker_name, Some("Office".to_owned()));
        assert_eq!(diagnostics.selected_sonos_id, None);
        assert_eq!(diagnostics.last_known_sonos_address, None);
        assert_eq!(diagnostics.sonos_volume, Some(40));
        assert_eq!(diagnostics.local_volume, Some(20));
        assert_eq!(diagnostics.muted, Some(false));
    }
}

#[tauri::command]
pub fn export_diagnostics(state: State<'_, AppState>) -> Result<String, String> {
    serde_json::to_string_pretty(&diagnostics(state)).map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn discover_sonos() -> Result<Vec<DiscoveredSonos>, String> {
    runtime::discover_available().await
}

#[tauri::command]
pub fn list_audio_outputs() -> Result<Vec<AvailableAudioOutput>, String> {
    runtime::available_audio_outputs()
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)] // Tauri injects managed state by value.
pub async fn test_volume(state: State<'_, AppState>) -> Result<(), String> {
    let configuration = state
        .configuration
        .lock()
        .map_err(|_| "application state is unavailable".to_owned())?
        .clone();
    if configuration.selected_sonos_id.is_none() {
        return Err("Select a Sonos device before testing volume.".to_owned());
    }
    runtime::test_selected_device(configuration).await
}
