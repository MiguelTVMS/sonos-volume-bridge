use crate::state::{AppState, UiStatus};
use std::sync::Mutex;
use tauri::{
    AppHandle, Manager, Runtime, Theme,
    image::Image,
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::{MouseButton, TrayIconBuilder, TrayIconEvent},
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ConnectionState {
    Connected,
    Disconnected,
}

struct TrayMenuItems<R: Runtime> {
    speaker: MenuItem<R>,
    status: MenuItem<R>,
    connection: Mutex<ConnectionState>,
}

pub fn install<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<()> {
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
            "quit" => {
                if let Some(state) = app.try_state::<AppState>() {
                    state.stop_runtime();
                }
                app.exit(0);
            }
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::DoubleClick { button, .. } = event
                && opens_settings_on_double_click(button)
            {
                show_settings(tray.app_handle());
            }
        })
        .build(app)?;
    let _ = app.manage(TrayMenuItems {
        speaker,
        status,
        connection: Mutex::new(connection),
    });
    refresh(app);
    Ok(())
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
