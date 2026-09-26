use crate::{
    autostart,
    config::{AppConfiguration, ConfigStore},
    runtime::{self, AvailableAudioOutput, DiscoveredSonos, SpeakerSetting, SpeakerSettings},
    state::{AppState, UiSnapshot},
};
use serde::Serialize;
use tauri::{AppHandle, Emitter, State};

// WebView Intl defaults do not include macOS region and clock overrides.
#[tauri::command]
pub fn get_system_hour12() -> Option<bool> {
    crate::clock_format::hour12()
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
pub async fn save_configuration(
    mut configuration: AppConfiguration,
    state: State<'_, AppState>,
    app: AppHandle,
) -> Result<UiSnapshot, String> {
    let gate = state.speaker_gate.lock().await;
    if let Ok(current) = state.configuration.lock() {
        configuration.night_mode_schedule = current.night_mode_schedule.clone();
        configuration.notify_night_mode_schedule_transitions =
            current.notify_night_mode_schedule_transitions;
    }
    let previous_start_at_login = state
        .configuration
        .lock()
        .map_err(|_| "application configuration is unavailable".to_owned())?
        .start_at_login;
    let configuration = persist_settings(
        &state.store,
        previous_start_at_login,
        configuration,
        |enabled| autostart::update(&app, enabled),
    )?;
    state.replace_configuration(configuration);
    state.start_runtime(app);
    drop(gate);
    get_snapshot(state)
}

fn persist_settings(
    store: &ConfigStore,
    previous_start_at_login: bool,
    configuration: AppConfiguration,
    update_autostart: impl FnOnce(bool) -> Result<(), String>,
) -> Result<AppConfiguration, String> {
    configuration
        .validate()
        .map_err(|error| error.to_string())?;
    if configuration.start_at_login != previous_start_at_login {
        update_autostart(configuration.start_at_login)?;
    }
    store
        .save(&configuration)
        .map_err(|error| error.to_string())?;
    Ok(configuration)
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)] // Tauri owns command argument extraction.
pub async fn reset_configuration(
    state: State<'_, AppState>,
    app: AppHandle,
) -> Result<UiSnapshot, String> {
    let gate = state.speaker_gate.lock().await;
    let configuration = state.store.reset().map_err(|error| error.to_string())?;
    state.replace_configuration(configuration);
    state.start_runtime(app);
    drop(gate);
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
    pub audio_input_format: Option<String>,
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)] // Tauri injects managed state by value.
pub async fn diagnostics(state: State<'_, AppState>) -> Result<Diagnostics, String> {
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
    let mut diagnostics = build_diagnostics(state.store.path().exists(), &snapshot);
    let configuration = state
        .configuration
        .lock()
        .map_err(|_| "application state is unavailable".to_owned())?
        .clone();
    diagnostics.audio_input_format = runtime::audio_input_format(configuration).await;
    Ok(diagnostics)
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
        audio_input_format: None,
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
pub async fn export_diagnostics(state: State<'_, AppState>) -> Result<String, String> {
    serde_json::to_string_pretty(&diagnostics(state).await?).map_err(|error| error.to_string())
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

#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
pub async fn get_speaker_settings(state: State<'_, AppState>) -> Result<SpeakerSettings, String> {
    let configuration = state
        .configuration
        .lock()
        .map_or_else(|_| AppConfiguration::default(), |value| value.clone());
    Ok(runtime::speaker_settings(configuration).await)
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
pub async fn use_tv_audio(state: State<'_, AppState>) -> Result<(), String> {
    let configuration = state
        .configuration
        .lock()
        .map_err(|_| "application state is unavailable".to_owned())?
        .clone();
    runtime::use_tv_audio(configuration).await
}
#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
pub async fn set_speaker_level(
    setting: SpeakerSetting,
    value: i8,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let configuration = state
        .configuration
        .lock()
        .map_err(|_| "application state is unavailable".to_owned())?
        .clone();
    runtime::set_speaker_level(configuration, setting, value).await
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
pub async fn set_speaker_setting(
    setting: SpeakerSetting,
    enabled: bool,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let configuration = state
        .configuration
        .lock()
        .map_err(|_| "application state is unavailable".to_owned())?
        .clone();
    let gate = state.speaker_gate.lock().await;
    let current = state
        .configuration
        .lock()
        .map_err(|_| "Configuration unavailable")?
        .clone();
    if current.selected_sonos_id != configuration.selected_sonos_id {
        return Err("Selected speaker changed. Try again.".into());
    }
    if matches!(setting, SpeakerSetting::NightSound)
        && !enabled
        && crate::night_schedule::locked(&current)
    {
        return Err(crate::night_schedule::LOCK_MESSAGE.into());
    }
    let result = runtime::set_speaker_setting(current, setting, enabled).await;
    drop(gate);
    result
}

#[cfg(test)]
mod persistence_tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    struct TestStore(ConfigStore);
    impl TestStore {
        fn new() -> Self {
            static NEXT: AtomicU64 = AtomicU64::new(0);
            let directory = std::env::temp_dir().join(format!(
                "sonos-save-test-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            Self(ConfigStore::new(directory.join("config.json")))
        }
    }
    impl Drop for TestStore {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(self.0.path().parent().unwrap());
        }
    }

    #[test]
    fn first_settings_save_does_not_require_login_item_registration() {
        let store = TestStore::new();
        assert!(!store.0.path().exists());
        let current = store.0.load_or_default().unwrap();
        let selected = AppConfiguration {
            selected_sonos_id: Some("test-speaker".to_owned()),
            synchronize_mute: false,
            ..current.clone()
        };
        let applied = persist_settings(&store.0, current.start_at_login, selected, |_| {
            Err("login service unavailable on fresh install".to_owned())
        })
        .unwrap();
        assert_eq!(applied.selected_sonos_id.as_deref(), Some("test-speaker"));
        assert!(!applied.start_at_login);
        let reloaded = store.0.load_or_default().unwrap();
        assert_eq!(reloaded.selected_sonos_id, applied.selected_sonos_id);
        assert!(!reloaded.synchronize_mute);
        assert!(!reloaded.start_at_login);
    }

    #[test]
    fn unchanged_enabled_login_setting_does_not_block_other_saves() {
        let store = TestStore::new();
        let configuration = AppConfiguration {
            start_at_login: true,
            ..AppConfiguration::default()
        };
        assert!(
            persist_settings(&store.0, true, configuration, |_| Err(
                "login service unavailable".to_owned()
            ))
            .is_ok()
        );
    }

    #[test]
    fn failed_explicit_login_change_preserves_saved_configuration() {
        let store = TestStore::new();
        let previous = AppConfiguration::default();
        store.0.save(&previous).unwrap();
        let next = AppConfiguration {
            start_at_login: true,
            ..previous
        };
        let result = persist_settings(&store.0, false, next, |enabled| {
            assert!(enabled);
            Err("registration denied".to_owned())
        });
        assert_eq!(result.unwrap_err(), "registration denied");
        assert!(!store.0.load_or_default().unwrap().start_at_login);
    }

    #[test]
    fn explicit_login_changes_are_applied_and_persisted() {
        let store = TestStore::new();
        for (previous, enabled) in [(false, true), (true, false)] {
            let configuration = AppConfiguration {
                start_at_login: enabled,
                ..AppConfiguration::default()
            };
            let called = std::cell::Cell::new(false);
            persist_settings(&store.0, previous, configuration, |requested| {
                assert_eq!(requested, enabled);
                called.set(true);
                Ok(())
            })
            .unwrap();
            assert!(called.get());
            assert_eq!(store.0.load_or_default().unwrap().start_at_login, enabled);
        }
    }
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)] // Tauri injects managed state.
pub fn get_schedule_status(
    state: State<'_, AppState>,
) -> Result<crate::night_schedule::ScheduleStatus, String> {
    state
        .schedule_status
        .lock()
        .map(|s| s.clone())
        .map_err(|_| "Schedule status unavailable".into())
}
fn persist_schedule(state: &AppState, configuration: &AppConfiguration) -> Result<(), String> {
    state.store.save(configuration).map_err(|e| e.to_string())?;
    *state
        .configuration
        .lock()
        .map_err(|_| "Configuration unavailable")? = configuration.clone();
    state
        .snapshot
        .lock()
        .map_err(|_| "Snapshot unavailable")?
        .configuration = configuration.clone();
    Ok(())
}
#[tauri::command]
pub async fn save_night_schedule(
    blocks: Vec<Vec<bool>>,
    state: State<'_, AppState>,
    app: AppHandle,
) -> Result<UiSnapshot, String> {
    let gate = state.speaker_gate.lock().await;
    let mut configuration = state
        .configuration
        .lock()
        .map_err(|_| "Configuration unavailable")?
        .clone();
    if !crate::night_schedule::supported(&configuration).await {
        return Err("Select a speaker that supports Night Mode.".into());
    }
    configuration.night_mode_schedule.blocks = blocks;
    persist_schedule(&state, &configuration)?;
    let applied = crate::night_schedule::apply_saved(&configuration).await;
    state
        .schedule_reconcile
        .store(true, std::sync::atomic::Ordering::Relaxed);
    drop(gate);
    let (active, speaker) =
        applied.map_err(|error| format!("Schedule saved, but {error} Try Save schedule again."))?;
    crate::night_schedule::notify_saved(&app, &configuration, active, &speaker).await;
    let _ = app.emit("speaker-settings-changed", ());
    crate::tray::refresh_speaker_controls(&app);
    get_snapshot(state)
}
#[tauri::command]
pub async fn enable_night_schedule(
    enabled: bool,
    state: State<'_, AppState>,
) -> Result<UiSnapshot, String> {
    let gate = state.speaker_gate.lock().await;
    let mut configuration = state
        .configuration
        .lock()
        .map_err(|_| "Configuration unavailable")?
        .clone();
    if enabled && !crate::night_schedule::supported(&configuration).await {
        return Err("Select a speaker that supports Night Mode.".into());
    }
    configuration.night_mode_schedule.enabled = enabled;
    persist_schedule(&state, &configuration)?;
    drop(gate);
    get_snapshot(state)
}
#[tauri::command]
pub async fn set_schedule_notifications(
    mode: crate::config::ScheduleNotifications,
    state: State<'_, AppState>,
    app: AppHandle,
) -> Result<UiSnapshot, String> {
    let gate = state.speaker_gate.lock().await;
    let mut configuration = state
        .configuration
        .lock()
        .map_err(|_| "Configuration unavailable")?
        .clone();
    configuration.notify_night_mode_schedule_transitions = mode;
    persist_schedule(&state, &configuration)?;
    drop(gate);
    if mode.enabled() {
        let granted = crate::schedule_notifications::permitted(&app, true).await;
        if let Ok(mut status) = state.schedule_status.lock() {
            status.notifications_blocked = !granted;
        }
    } else if let Ok(mut status) = state.schedule_status.lock() {
        status.notifications_blocked = false;
    }
    get_snapshot(state)
}
