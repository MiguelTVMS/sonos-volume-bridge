use crate::{
    runtime::{self, SpeakerSetting, SpeakerSettings},
    state::{AppState, UiStatus},
};
use std::sync::{
    Mutex,
    atomic::{AtomicU64, Ordering},
};
use tauri::{
    AppHandle, Manager, Runtime, Theme,
    image::Image,
    menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ConnectionState {
    Connected,
    Disconnected,
}

struct TrayMenuItems<R: Runtime> {
    menu: Menu<R>,
    speaker: MenuItem<R>,
    status: MenuItem<R>,
    camera_status: MenuItem<R>,
    speaker_separator: PredefinedMenuItem<R>,
    speaker_controls: Mutex<Vec<CheckMenuItem<R>>>,
    connection: Mutex<ConnectionState>,
    controls_request: AtomicU64,
}

pub fn install<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<()> {
    initialize_tray(
        || register_tray(app),
        || refresh(app),
        || refresh_speaker_controls(app),
    )
}

// Keep startup refreshes independent of native mouse-event delivery.
fn initialize_tray<E>(
    register: impl FnOnce() -> Result<(), E>,
    refresh_status: impl FnOnce(),
    refresh_controls: impl FnOnce(),
) -> Result<(), E> {
    register()?;
    refresh_status();
    refresh_controls();
    Ok(())
}

fn register_tray<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<()> {
    let connection = app_connection(app);
    let icon = icon_for(theme(app), connection);
    let title = MenuItem::with_id(app, "title", "Sonos Volume Bridge", false, None::<&str>)?;
    let status = MenuItem::with_id(
        app,
        "status",
        "State: Configuration required",
        false,
        None::<&str>,
    )?;
    let speaker = MenuItem::with_id(
        app,
        "speaker",
        "Speaker: No speaker selected",
        false,
        None::<&str>,
    )?;
    let settings = MenuItem::with_id(app, "settings", "Open settings", true, None::<&str>)?;
    let diagnostics = MenuItem::with_id(app, "diagnostics", "Diagnostics", true, None::<&str>)?;
    let camera_status = MenuItem::with_id(
        app,
        "camera-status",
        "Camera automation is off",
        false,
        None::<&str>,
    )?;
    let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
    let speaker_separator = PredefinedMenuItem::separator(app)?;
    let separator = PredefinedMenuItem::separator(app)?;
    let menu = Menu::with_items(
        app,
        &[
            &title,
            &speaker,
            &status,
            &camera_status,
            &separator,
            &settings,
            &diagnostics,
            &quit,
        ],
    )?;
    TrayIconBuilder::with_id("main-tray")
        .icon(icon)
        .icon_as_template(connection == ConnectionState::Connected)
        .menu(&menu)
        .tooltip("Sonos Volume Bridge")
        .on_menu_event(|app, event| match event.id().as_ref() {
            "settings" | "diagnostics" => show_settings(app),
            "speaker-night-sound" => {
                toggle_speaker_setting(app, SpeakerSetting::NightSound, event.id().as_ref());
            }
            "speaker-loudness" => {
                toggle_speaker_setting(app, SpeakerSetting::Loudness, event.id().as_ref());
            }
            "speaker-status-light" => {
                toggle_speaker_setting(app, SpeakerSetting::StatusLight, event.id().as_ref());
            }
            "speaker-speech-enhancement" => {
                toggle_speaker_setting(app, SpeakerSetting::SpeechEnhancement, event.id().as_ref());
            }
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| match event {
            TrayIconEvent::Enter { .. }
            | TrayIconEvent::Click {
                button: MouseButton::Left | MouseButton::Right,
                button_state: MouseButtonState::Down,
                ..
            } => refresh_speaker_controls(tray.app_handle()),
            TrayIconEvent::DoubleClick { button, .. } if opens_settings_on_double_click(button) => {
                show_settings(tray.app_handle());
            }
            _ => {}
        })
        .build(app)?;
    let _ = app.manage(TrayMenuItems {
        menu,
        speaker,
        status,
        camera_status,
        speaker_separator,
        speaker_controls: Mutex::new(Vec::new()),
        connection: Mutex::new(connection),
        controls_request: AtomicU64::new(0),
    });
    Ok(())
}

pub(crate) fn refresh_speaker_controls<R: Runtime>(app: &AppHandle<R>) {
    let configuration = app
        .try_state::<AppState>()
        .and_then(|state| state.configuration.lock().ok().map(|value| value.clone()));
    let Some(items) = app.try_state::<TrayMenuItems<R>>() else {
        return;
    };
    let request = items.controls_request.fetch_add(1, Ordering::SeqCst) + 1;
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let selected = configuration
            .as_ref()
            .and_then(|value| value.selected_sonos_id.clone());
        let settings = match configuration {
            Some(configuration) => runtime::speaker_settings(configuration).await,
            None => SpeakerSettings::default(),
        };
        let handle = app.clone();
        let _ = app.run_on_main_thread(move || {
            let Some(items) = handle.try_state::<TrayMenuItems<R>>() else {
                return;
            };
            let current = handle.try_state::<AppState>().and_then(|state| {
                state
                    .configuration
                    .lock()
                    .ok()
                    .and_then(|value| value.selected_sonos_id.clone())
            });
            if accepts_controls_result(
                request,
                items.controls_request.load(Ordering::SeqCst),
                selected.as_deref(),
                current.as_deref(),
            ) {
                update_speaker_controls(&handle, &settings);
            }
        });
    });
}

fn accepts_controls_result(
    request: u64,
    latest: u64,
    selected: Option<&str>,
    current: Option<&str>,
) -> bool {
    request == latest && selected == current
}

fn update_speaker_controls<R: Runtime>(app: &AppHandle<R>, settings: &SpeakerSettings) {
    let Some(items) = app.try_state::<TrayMenuItems<R>>() else {
        return;
    };
    let Ok(mut controls) = items.speaker_controls.lock() else {
        return;
    };
    for control in controls.drain(..) {
        let _ = items.menu.remove(&control);
    }
    let _ = items.menu.remove(&items.speaker_separator);

    let available = available_speaker_controls(settings);
    if available.is_empty() {
        return;
    }

    let _ = items.menu.insert(&items.speaker_separator, 3);
    for (index, (id, label, enabled)) in available.into_iter().enumerate() {
        let Ok(control) = CheckMenuItem::with_id(app, id, label, true, enabled, None::<&str>)
        else {
            continue;
        };
        let _ = items.menu.insert(&control, 4 + index);
        controls.push(control);
    }
}

fn available_speaker_controls(
    settings: &SpeakerSettings,
) -> Vec<(&'static str, &'static str, bool)> {
    [
        ("speaker-night-sound", "Night sound", settings.night_sound),
        ("speaker-loudness", "Loudness", settings.loudness),
        (
            "speaker-status-light",
            "Status light",
            settings.status_light,
        ),
        (
            "speaker-speech-enhancement",
            "Speech enhancement",
            settings.speech_enhancement,
        ),
    ]
    .into_iter()
    .filter_map(|(id, label, enabled)| enabled.map(|enabled| (id, label, enabled)))
    .collect()
}
fn toggle_speaker_setting<R: Runtime>(app: &AppHandle<R>, setting: SpeakerSetting, id: &str) {
    let Some(items) = app.try_state::<TrayMenuItems<R>>() else {
        return;
    };
    let enabled = items.speaker_controls.lock().ok().and_then(|controls| {
        controls
            .iter()
            .find(|control| control.id().as_ref() == id)
            .and_then(|control| control.is_checked().ok())
    });
    let configuration = app
        .try_state::<AppState>()
        .and_then(|state| state.configuration.lock().ok().map(|value| value.clone()));
    if let (Some(enabled), Some(configuration)) = (enabled, configuration) {
        let app = app.clone();
        tauri::async_runtime::spawn(async move {
            if matches!(setting, SpeakerSetting::SpeechEnhancement) {
                if let Some(state) = app.try_state::<AppState>() {
                    let _ = state.camera.manual(configuration, enabled).await;
                }
            } else {
                let _ = runtime::set_speaker_setting(configuration, setting, enabled).await;
            }
            refresh_speaker_controls(&app);
        });
    }
}
pub fn update_icon_for_theme<R: Runtime>(app: &AppHandle<R>, theme: Theme) {
    let connection = app_connection(app);
    if let Some(tray) = app.tray_by_id("main-tray") {
        let _ = tray.set_icon_with_as_template(
            Some(icon_for(theme, connection)),
            connection == ConnectionState::Connected,
        );
    }
}

fn theme<R: Runtime>(app: &AppHandle<R>) -> Theme {
    app.get_webview_window("main")
        .and_then(|window| window.theme().ok())
        .unwrap_or(Theme::Light)
}

fn icon_for(theme: Theme, connection: ConnectionState) -> Image<'static> {
    let bytes: &[u8] = match (theme, connection) {
        (Theme::Light, ConnectionState::Connected) => {
            include_bytes!("../icons/tray-icon-light.png")
        }
        (Theme::Light, ConnectionState::Disconnected) => {
            include_bytes!("../icons/tray-icon-light-disconnected.png")
        }
        (_, ConnectionState::Connected) => include_bytes!("../icons/tray-icon-dark.png"),
        (_, ConnectionState::Disconnected) => {
            include_bytes!("../icons/tray-icon-dark-disconnected.png")
        }
    };
    Image::from_bytes(bytes).expect("embedded tray icon must be a valid PNG")
}

pub fn refresh<R: Runtime>(app: &AppHandle<R>) {
    let Some(state) = app.try_state::<AppState>() else {
        return;
    };
    let Ok(snapshot) = state.snapshot.lock() else {
        return;
    };
    let connection = connection_state(&snapshot.status);
    let status = format!("State: {}", connection_label(connection));
    let speaker = speaker_label(snapshot.sonos_name.as_deref(), snapshot.sonos_volume);
    let tooltip = format!(
        "SonosVolumeBridge\n{status}\n{speaker}\nLocal: {}",
        snapshot
            .local_volume
            .map_or_else(|| "—".to_owned(), |volume| format!("{volume}%"))
    );
    drop(snapshot);

    let mut update_icon = false;
    if let Some(items) = app.try_state::<TrayMenuItems<R>>() {
        let _ = items.status.set_text(status);
        let _ = items.speaker.set_text(speaker);
        if let Ok(mut current) = items.connection.lock()
            && *current != connection
        {
            *current = connection;
            update_icon = true;
        }
    }
    if update_icon {
        refresh_speaker_controls(app);
    }
    if let Some(tray) = app.tray_by_id("main-tray") {
        let _ = tray.set_tooltip(Some(tooltip));
        if update_icon {
            let _ = tray.set_icon_with_as_template(
                Some(icon_for(theme(app), connection)),
                connection == ConnectionState::Connected,
            );
        }
    }
}

fn connection_state(status: &UiStatus) -> ConnectionState {
    match status {
        UiStatus::Synchronized
        | UiStatus::WaitingForSonosConfirmation
        | UiStatus::SubscriptionDegraded
        | UiStatus::PollingFallback => ConnectionState::Connected,
        UiStatus::Discovering
        | UiStatus::Connecting
        | UiStatus::SonosUnavailable
        | UiStatus::LocalAudioUnavailable
        | UiStatus::UnsupportedLocalDevice
        | UiStatus::ConfigurationRequired
        | UiStatus::Error => ConnectionState::Disconnected,
    }
}

const fn connection_label(connection: ConnectionState) -> &'static str {
    match connection {
        ConnectionState::Connected => "Connected",
        ConnectionState::Disconnected => "Disconnected",
    }
}

fn app_connection<R: Runtime>(app: &AppHandle<R>) -> ConnectionState {
    app.try_state::<AppState>()
        .and_then(|state| {
            state
                .snapshot
                .lock()
                .ok()
                .map(|snapshot| connection_state(&snapshot.status))
        })
        .unwrap_or(ConnectionState::Disconnected)
}

fn speaker_label(name: Option<&str>, volume: Option<u8>) -> String {
    match (name, volume) {
        (Some(name), Some(volume)) => format!("Speaker: {name} ({volume}%)"),
        (Some(name), None) => format!("Speaker: {name}"),
        (None, _) => "Speaker: No speaker selected".to_owned(),
    }
}

fn show_settings<R: Runtime>(app: &AppHandle<R>) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.set_focus();
    }
}

fn opens_settings_on_double_click(button: MouseButton) -> bool {
    button == MouseButton::Left
}

pub(crate) fn refresh_camera_status<R: Runtime>(app: &AppHandle<R>) {
    let handle = app.clone();
    let _ = app.run_on_main_thread(move || {
        if let (Some(items), Some(state)) = (
            handle.try_state::<TrayMenuItems<R>>(),
            handle.try_state::<AppState>(),
        ) {
            let status = state.camera.status();
            let _ = items
                .camera_status
                .set_text(status.warning.unwrap_or(status.message));
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn startup_requests_speaker_controls_without_any_mouse_event() {
        use std::cell::{Cell, RefCell};

        let registered = Cell::new(false);
        let status_refreshed = Cell::new(false);
        let controls = RefCell::new(Vec::new());
        initialize_tray(
            || {
                registered.set(true);
                Ok::<_, ()>(())
            },
            || status_refreshed.set(true),
            || {
                assert!(registered.get(), "menu must exist before controls load");
                *controls.borrow_mut() = available_speaker_controls(&SpeakerSettings {
                    night_sound: Some(false),
                    loudness: Some(true),
                    status_light: Some(true),
                    speech_enhancement: Some(false),
                    ..SpeakerSettings::default()
                });
            },
        )
        .unwrap();
        // No click callback is dispatched, including the first left-click that
        // macOS can consume while displaying the native menu.
        assert!(status_refreshed.get());
        assert_eq!(
            *controls.borrow(),
            vec![
                ("speaker-night-sound", "Night sound", false),
                ("speaker-loudness", "Loudness", true),
                ("speaker-status-light", "Status light", true),
                ("speaker-speech-enhancement", "Speech enhancement", false),
            ]
        );
    }

    #[test]
    fn failed_tray_registration_does_not_start_refreshes() {
        assert_eq!(
            initialize_tray(
                || Err("unavailable"),
                || panic!("status refreshed without menu"),
                || panic!("controls requested without menu")
            ),
            Err("unavailable")
        );
    }

    #[test]
    fn controls_results_must_match_latest_request_and_selected_speaker() {
        assert!(accepts_controls_result(1, 1, Some("a"), Some("a")));
        assert!(!accepts_controls_result(1, 2, Some("a"), Some("a")));
        assert!(!accepts_controls_result(1, 1, Some("a"), Some("b")));
        assert!(!accepts_controls_result(1, 1, Some("a"), None));
    }

    #[test]
    fn supported_controls_preserve_checked_state_and_omit_unsupported_settings() {
        let settings = SpeakerSettings {
            loudness: Some(true),
            status_light: Some(false),
            ..SpeakerSettings::default()
        };
        assert_eq!(
            available_speaker_controls(&settings),
            vec![
                ("speaker-loudness", "Loudness", true),
                ("speaker-status-light", "Status light", false),
            ]
        );
        assert!(available_speaker_controls(&SpeakerSettings::default()).is_empty());
    }

    #[test]
    fn only_left_double_click_opens_settings() {
        assert!(opens_settings_on_double_click(MouseButton::Left));
        assert!(!opens_settings_on_double_click(MouseButton::Right));
        assert!(!opens_settings_on_double_click(MouseButton::Middle));
    }

    #[test]
    fn connected_states_remain_stable_during_normal_runtime_activity() {
        for status in [
            UiStatus::Synchronized,
            UiStatus::WaitingForSonosConfirmation,
            UiStatus::SubscriptionDegraded,
            UiStatus::PollingFallback,
        ] {
            assert_eq!(connection_state(&status), ConnectionState::Connected);
        }
        assert_eq!(connection_label(ConnectionState::Connected), "Connected");
    }

    #[test]
    fn unavailable_states_are_disconnected() {
        for status in [
            UiStatus::Discovering,
            UiStatus::Connecting,
            UiStatus::SonosUnavailable,
            UiStatus::LocalAudioUnavailable,
            UiStatus::UnsupportedLocalDevice,
            UiStatus::ConfigurationRequired,
            UiStatus::Error,
        ] {
            assert_eq!(connection_state(&status), ConnectionState::Disconnected);
        }
        assert_eq!(
            connection_label(ConnectionState::Disconnected),
            "Disconnected"
        );
    }

    #[test]
    fn speaker_label_includes_cached_volume() {
        assert_eq!(
            speaker_label(Some("Office"), Some(37)),
            "Speaker: Office (37%)"
        );
        assert_eq!(speaker_label(Some("Office"), None), "Speaker: Office");
        assert_eq!(
            speaker_label(None, Some(37)),
            "Speaker: No speaker selected"
        );
    }
}
