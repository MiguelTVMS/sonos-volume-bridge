use crate::state::AppState;
use tauri::{
    AppHandle, Manager, Runtime, Theme,
    image::Image,
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::{MouseButton, TrayIconBuilder, TrayIconEvent},
};

pub fn install<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<()> {
    let icon = icon_for(theme(app));
    let title = MenuItem::with_id(app, "title", "Sonos Volume Bridge", false, None::<&str>)?;
    let status = MenuItem::with_id(
        app,
        "status",
        "State: Configuration required",
        false,
        None::<&str>,
    )?;
    let settings = MenuItem::with_id(app, "settings", "Open settings", true, None::<&str>)?;
    let diagnostics = MenuItem::with_id(app, "diagnostics", "Diagnostics", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
    let separator = PredefinedMenuItem::separator(app)?;
    let menu = Menu::with_items(
        app,
        &[&title, &status, &separator, &settings, &diagnostics, &quit],
    )?;
    TrayIconBuilder::with_id("main-tray")
        .icon(icon)
        .icon_as_template(true)
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
    refresh(app);
    Ok(())
}

pub fn update_icon_for_theme<R: Runtime>(app: &AppHandle<R>, theme: Theme) {
    if let Some(tray) = app.tray_by_id("main-tray") {
        let _ = tray.set_icon_with_as_template(Some(icon_for(theme)), true);
    }
}

fn theme<R: Runtime>(app: &AppHandle<R>) -> Theme {
    app.get_webview_window("main")
        .and_then(|window| window.theme().ok())
        .unwrap_or(Theme::Light)
}

fn icon_for(theme: Theme) -> Image<'static> {
    let bytes: &[u8] = match theme {
        Theme::Light => include_bytes!("../icons/tray-icon-light.png"),
        _ => include_bytes!("../icons/tray-icon-dark.png"),
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
    if let Some(tray) = app.tray_by_id("main-tray") {
        let _ = tray.set_tooltip(Some(format!(
            "SonosVolumeBridge\nState: {:?}\nSonos: {}\nLocal: {}",
            snapshot.status,
            snapshot
                .sonos_volume
                .map_or_else(|| "—".to_owned(), |volume| format!("{volume}%")),
            snapshot
                .local_volume
                .map_or_else(|| "—".to_owned(), |volume| format!("{volume}%"))
        )));
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
}
