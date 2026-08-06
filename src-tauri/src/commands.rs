use crate::{
    config::AppConfiguration,
    runtime::{self, AvailableAudioOutput, DiscoveredSonos},
    state::{AppState, UiSnapshot},
};
use serde::Serialize;
use tauri::{AppHandle, State};
use tauri_plugin_autostart::ManagerExt;
fn is_missing_autostart_entry(error: &str) -> bool {
    error.contains("(os error 2)")
}

fn update_autostart(app: &AppHandle, start_at_login: bool) -> Result<(), String> {
    if start_at_login {
        app.autolaunch().enable().map_err(|error| error.to_string())
    } else {
        match app.autolaunch().disable() {
            Ok(()) => Ok(()),
            Err(error) if is_missing_autostart_entry(&error.to_string()) => Ok(()),
            Err(error) => Err(error.to_string()),
        }
    }
}

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
    update_autostart(&app, configuration.start_at_login)?;
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
    let configuration = snapshot.configuration;
    Diagnostics {
        configuration_present: state.store.path().exists(),
        sanitized: true,
        message: "This report contains the saved local speaker endpoint and stable speaker identity. It excludes protocol payloads and host paths.".to_owned(),
        status: format!("{:?}", snapshot.status),
        speaker_name: snapshot.sonos_name,
        selected_sonos_id: configuration.selected_sonos_id,
        last_known_sonos_address: configuration.last_known_sonos_address,
        sonos_volume: snapshot.sonos_volume,
        local_volume: snapshot.local_volume,
        muted: snapshot.muted,
        follows_system_output: configuration.follow_default_audio_device,
        fixed_audio_device_id: configuration.fixed_audio_device_id,
        synchronize_mute: configuration.synchronize_mute,
        two_way_synchronization: configuration.two_way_synchronization,
        fallback_polling: configuration.fallback_polling,
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

#[cfg(test)]
mod tests {
    use super::is_missing_autostart_entry;

    #[test]
    fn identifies_a_missing_autostart_entry() {
        assert!(is_missing_autostart_entry(
            "The system cannot find the file specified. (os error 2)"
        ));
    }

    #[test]
    fn does_not_ignore_other_autostart_errors() {
        assert!(!is_missing_autostart_entry(
            "Access is denied. (os error 5)"
        ));
    }
}
