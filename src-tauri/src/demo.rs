//! A window-only shell for UI testing. Never construct AppState or start adapters.
use tauri::{
    Manager,
    menu::{Menu, MenuItem},
    tray::TrayIconBuilder,
};

pub fn run(mut context: tauri::Context<tauri::Wry>) {
    // Demo and normal instances can coexist without forwarding to the wrong shell.
    context.config_mut().identifier.push_str(".ui-demo");
    commands(tauri::Builder::default())
        .plugin(tauri_plugin_single_instance::init(|app, _, _| show(app)))
        .setup(|app| {
            let settings =
                MenuItem::with_id(app, "settings", "UI demo settings", true, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", "Quit UI demo", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&settings, &quit])?;
            TrayIconBuilder::new()
                .icon(app.default_window_icon().expect("bundled app icon").clone())
                .tooltip("Sonos Volume Bridge — UI demo")
                .menu(&menu)
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "settings" => show(app),
                    "quit" => app.exit(0),
                    _ => {}
                })
                .build(app)?;
            if let Some(window) = app.get_webview_window("main") {
                window.set_title("Sonos Volume Bridge — UI demo")?;
                window.show()?;
                window.set_focus()?;
            }
            Ok(())
        })
        // No production command handlers, config store, scheduler, audio, or network workers.
        .run(context)
        .expect("UI demo window failed");
}

fn commands<R: tauri::Runtime>(builder: tauri::Builder<R>) -> tauri::Builder<R> {
    builder.invoke_handler(tauri::generate_handler![
        crate::ui_demo_enabled,
        crate::ui_demo_platform
    ])
}

fn show(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.set_focus();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tauri::test::{mock_builder, mock_context, noop_assets};

    #[test]
    fn demo_ipc_reports_build_mode_and_cannot_invoke_real_device_commands() {
        let app = commands(mock_builder())
            .build(mock_context(noop_assets()))
            .unwrap();
        let window = tauri::WebviewWindowBuilder::new(&app, "main", tauri::WebviewUrl::default())
            .build()
            .unwrap();
        let request = |command: &str| tauri::webview::InvokeRequest {
            cmd: command.into(),
            callback: tauri::ipc::CallbackFn(0),
            error: tauri::ipc::CallbackFn(1),
            url: if cfg!(windows) {
                "http://tauri.localhost"
            } else {
                "tauri://localhost"
            }
            .parse()
            .unwrap(),
            body: tauri::ipc::InvokeBody::default(),
            headers: tauri::http::HeaderMap::default(),
            invoke_key: tauri::test::INVOKE_KEY.to_string(),
        };
        tauri::test::assert_ipc_response(
            &window,
            request("ui_demo_enabled"),
            Ok(cfg!(feature = "ui-demo")),
        );
        tauri::test::assert_ipc_response(
            &window,
            request("ui_demo_platform"),
            Ok(crate::ui_demo_platform()),
        );
        for command in [
            "discover_sonos",
            "list_audio_outputs",
            "save_configuration",
            "set_speaker_setting",
            "test_volume",
            "enable_night_schedule",
        ] {
            assert!(
                tauri::test::get_ipc_response(&window, request(command)).is_err(),
                "demo shell exposed {command}"
            );
        }
    }
}
