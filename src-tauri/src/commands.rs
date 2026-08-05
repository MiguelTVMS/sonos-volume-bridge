use crate::{config::AppConfiguration, state::{AppState, UiSnapshot}};
use serde::Serialize;
use tauri::{AppHandle, State};
use tauri_plugin_autostart::ManagerExt;

#[tauri::command]
pub fn get_snapshot(state: State<'_, AppState>) -> Result<UiSnapshot, String> { state.snapshot.lock().map(|snapshot| snapshot.clone()).map_err(|_| "application state is unavailable".to_owned()) }

#[tauri::command]
pub fn save_configuration(configuration: AppConfiguration, state: State<'_, AppState>, app: AppHandle) -> Result<UiSnapshot, String> {
    state.store.save(&configuration).map_err(|error| error.to_string())?;
    if configuration.start_at_login { app.autolaunch().enable().map_err(|error| error.to_string())?; } else { app.autolaunch().disable().map_err(|error| error.to_string())?; }
    state.replace_configuration(configuration);
    get_snapshot(state)
}

#[tauri::command]
pub fn reset_configuration(state: State<'_, AppState>) -> Result<UiSnapshot, String> {
    let configuration = state.store.reset().map_err(|error| error.to_string())?;
    state.replace_configuration(configuration);
    get_snapshot(state)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Diagnostics { pub configuration_present: bool, pub sanitized: bool, pub message: String }

#[tauri::command]
pub fn diagnostics(state: State<'_, AppState>) -> Diagnostics { Diagnostics { configuration_present: state.store.path().exists(), sanitized: true, message: "No protocol payloads, device serial numbers, or host paths are included.".to_owned() } }

#[tauri::command]
pub fn export_diagnostics(state: State<'_, AppState>) -> Result<String, String> { serde_json::to_string_pretty(&diagnostics(state)).map_err(|error| error.to_string()) }

#[tauri::command]
pub fn test_volume(state: State<'_, AppState>) -> Result<(), String> {
    let configuration = state.configuration.lock().map_err(|_| "application state is unavailable".to_owned())?;
    if configuration.selected_sonos_id.is_none() { return Err("Select a Sonos device before testing volume.".to_owned()); }
    Ok(())
}
