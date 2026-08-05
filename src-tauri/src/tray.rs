use crate::state::AppState;
use tauri::{
    AppHandle, Manager, Runtime,
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::TrayIconBuilder,
};

pub fn install<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<()> {
    let title = MenuItem::with_id(app, "title", "SonosVolumeBridge", false, None::<&str>)?;
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
        .menu(&menu)
        .tooltip("SonosVolumeBridge")
        .on_menu_event(|app, event| match event.id().as_ref() {
            "settings" | "diagnostics" => show_settings(app),
            "quit" => app.exit(0),
            _ => {}
        })
        .build(app)?;
    refresh(app);
    Ok(())
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
