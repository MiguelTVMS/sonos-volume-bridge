use crate::{
    runtime::{self, SpeakerSetting, SpeakerSettings},
    state::{AppState, UiStatus},
};
use std::sync::{
    Mutex,
    atomic::{AtomicU64, Ordering},
};
use tauri::{
    AppHandle, Emitter, Manager, Runtime, Theme,
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
    let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
    let speaker_separator = PredefinedMenuItem::separator(app)?;
    let separator = PredefinedMenuItem::separator(app)?;
    let menu = Menu::with_items(
        app,
        &[
            &title,
            &speaker,
            &status,
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
            "night-schedule-enabled" => toggle_schedule(app),
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
            "quit" => {
                if let Some(state) = app.try_state::<AppState>() {
                    state.stop_runtime();
                }
                app.exit(0);
            }
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
    let schedule_enabled = app.try_state::<AppState>().is_some_and(|state| {
        state
            .configuration
            .lock()
            .is_ok_and(|configuration| configuration.night_mode_schedule.enabled)
    });
    let available = available_tray_controls(settings, schedule_enabled);
    let visible: Vec<String> = items
        .menu
        .items()
        .unwrap_or_default()
        .iter()
        .map(|item| item.id().as_ref().to_owned())
        .collect();
    let desired: Vec<&str> = available.iter().map(|(id, _, _)| *id).collect();
    let (remove, insert) = control_changes(&visible, &desired);
    for id in remove {
        if let Some(control) = controls.iter().find(|control| control.id().as_ref() == id) {
            let _ = items.menu.remove(control);
        }
    }
    let separator_visible = visible
        .iter()
        .any(|id| id == items.speaker_separator.id().as_ref());
    if available.is_empty() && separator_visible {
        let _ = items.menu.remove(&items.speaker_separator);
    } else if !available.is_empty() && !separator_visible {
        let _ = items.menu.insert(&items.speaker_separator, 3);
    }
    for (id, label, enabled) in available {
        let locked = id == "speaker-night-sound"
            && app.try_state::<AppState>().is_some_and(|state| {
                state
                    .configuration
                    .lock()
                    .is_ok_and(|configuration| crate::night_schedule::locked(&configuration))
            });
        let label = if locked {
            "Night sound (disable schedule to turn off)"
        } else {
            label
        };
        // Keep native action targets alive for the lifetime of the tray, including
        // when capability changes hide them while macOS is tracking a menu click.
        let control =
            if let Some(control) = controls.iter().find(|control| control.id().as_ref() == id) {
                control.clone()
            } else {
                let Ok(control) =
                    CheckMenuItem::with_id(app, id, label, !locked, enabled, None::<&str>)
                else {
                    continue;
                };
                controls.push(control.clone());
                control
            };
        let _ = control.set_text(label);
        let _ = control.set_enabled(!locked);
        let _ = control.set_checked(enabled);
        if let Some((index, _)) = insert.iter().find(|(_, added)| *added == id) {
            let _ = items.menu.insert(&control, 4 + index);
        }
    }
}

fn control_changes<'a>(
    visible: &'a [String],
    desired: &'a [&str],
) -> (Vec<&'a str>, Vec<(usize, &'a str)>) {
    let remove = visible
        .iter()
        .map(String::as_str)
        .filter(|id| {
            (id.starts_with("speaker-") || *id == "night-schedule-enabled") && !desired.contains(id)
        })
        .collect();
    let insert = desired
        .iter()
        .copied()
        .enumerate()
        .filter(|(_, id)| !visible.iter().any(|current| current == id))
        .collect();
    (remove, insert)
}

fn available_tray_controls(
    settings: &SpeakerSettings,
    schedule_enabled: bool,
) -> Vec<(&'static str, &'static str, bool)> {
    let mut controls = available_speaker_controls(settings);
    if settings.night_sound.is_some() || schedule_enabled {
        controls.insert(
            0,
            ("night-schedule-enabled", "Night schedule", schedule_enabled),
        );
    }
    controls
}

fn toggle_schedule<R: Runtime>(app: &AppHandle<R>) {
    let enabled = app.try_state::<TrayMenuItems<R>>().and_then(|items| {
        items.speaker_controls.lock().ok().and_then(|controls| {
            controls
                .iter()
                .find(|control| control.id().as_ref() == "night-schedule-enabled")
                .and_then(|control| control.is_checked().ok())
        })
    });
    let Some(enabled) = enabled else {
        return;
    };
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let state = app.state::<AppState>();
        if let Err(error) = crate::commands::enable_night_schedule(enabled, state.clone()).await
            && let Ok(mut status) = state.schedule_status.lock()
        {
            status.message = error;
            let _ = app.emit("night-schedule-changed", status.clone());
        }
        let _ = app.emit("speaker-settings-changed", ());
        refresh_speaker_controls(&app);
    });
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
            let state = app.state::<AppState>();
            let _gate = state.speaker_gate.lock().await;
            let Ok(current) = state.configuration.lock().map(|c| c.clone()) else {
                return;
            };
            if current.selected_sonos_id != configuration.selected_sonos_id {
                return;
            }
            if matches!(setting, SpeakerSetting::NightSound)
                && !enabled
                && crate::night_schedule::locked(&current)
            {
                show_settings(&app);
                let _ = app.emit("open-night-schedule", ());
                return;
            }
            let _ = runtime::set_speaker_setting(current, setting, enabled).await;
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
    // Ubuntu's dark top bar is independent of the application window theme.
    // AppIndicator uses the supplied pixels rather than a macOS template tint.
    let theme = if cfg!(target_os = "linux") {
        Theme::Dark
    } else {
        theme
    };
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

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(target_os = "linux")]
    #[test]
    fn ubuntu_panel_icon_stays_white_across_startup_theme_and_connection_changes() {
        // Ubuntu's panel stays dark even when the Settings window is light.
        // Exercise the image selection shared by registration, theme events
        // and connection refreshes, without requiring a native tray in CI.
        for (theme, connection) in [
            (Theme::Light, ConnectionState::Disconnected),
            (Theme::Light, ConnectionState::Connected),
            (Theme::Dark, ConnectionState::Connected),
            (Theme::Dark, ConnectionState::Disconnected),
            (Theme::Light, ConnectionState::Disconnected),
        ] {
            let icon = icon_for(theme, connection);
            let expected = Image::from_bytes(match connection {
                ConnectionState::Connected => include_bytes!("../icons/tray-icon-dark.png"),
                ConnectionState::Disconnected => {
                    include_bytes!("../icons/tray-icon-dark-disconnected.png")
                }
            })
            .unwrap();
            assert!(
                icon.rgba() == expected.rgba(),
                "Ubuntu panel icon must keep the white glyph and connection badge"
            );
        }
    }

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
    fn refresh_during_menu_tracking_keeps_existing_action_targets() {
        let desired = ["night-schedule-enabled", "speaker-night-sound"];
        let empty = Vec::new();
        assert_eq!(
            control_changes(&empty, &desired).1,
            vec![(0, desired[0]), (1, desired[1])]
        );
        let visible = desired.iter().map(ToString::to_string).collect::<Vec<_>>();
        // Startup has populated the menu. Hover refresh and a schedule toggle
        // must only update values, not remove/recreate the clicked native item.
        assert_eq!(control_changes(&visible, &desired), (vec![], vec![]));
        assert_eq!(control_changes(&visible, &desired), (vec![], vec![]));
        assert_eq!(
            control_changes(&visible, &desired[..1]),
            (vec![desired[1]], vec![])
        );
        let paused = vec![desired[0].to_string()];
        assert_eq!(control_changes(&paused, &desired).1, vec![(1, desired[1])]);
    }

    #[test]
    fn schedule_toggle_precedes_night_sound_and_can_always_be_disabled() {
        let settings = SpeakerSettings {
            night_sound: Some(false),
            ..SpeakerSettings::default()
        };
        let controls = available_tray_controls(&settings, false);
        assert_eq!(
            controls[0],
            ("night-schedule-enabled", "Night schedule", false)
        );
        assert_eq!(controls[1].0, "speaker-night-sound");
        assert_eq!(
            available_tray_controls(&settings, true)[0],
            ("night-schedule-enabled", "Night schedule", true)
        );
        assert!(available_tray_controls(&SpeakerSettings::default(), false).is_empty());
        assert_eq!(
            available_tray_controls(&SpeakerSettings::default(), true),
            vec![("night-schedule-enabled", "Night schedule", true)]
        );
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
