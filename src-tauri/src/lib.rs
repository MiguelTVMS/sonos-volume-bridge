mod commands;
mod config;
mod state;
mod tray;

use crate::{config::ConfigStore, state::AppState};
use tauri::Manager;

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_autostart::init(tauri_plugin_autostart::MacosLauncher::LaunchAgent, None::<Vec<&str>>))
        .setup(|app| {
            let config_path = app.path().app_config_dir()?.join("config.json");
            let store = ConfigStore::new(config_path);
            let configuration = store.load_or_default().map_err(|error| std::io::Error::other(error.to_string()))?;
            app.manage(AppState::new(store, configuration));
            tray::install(app.handle())?;
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![commands::get_snapshot, commands::save_configuration, commands::reset_configuration, commands::diagnostics, commands::export_diagnostics, commands::test_volume])
        .run(tauri::generate_context!())
        .expect("Tauri runtime failed");
}
