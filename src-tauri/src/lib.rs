mod autostart;
mod clock_format;
mod commands;
mod config;
#[cfg(any(test, feature = "ui-demo"))]
mod demo;
mod logging;
mod night_schedule;
mod runtime;
mod schedule_notifications;
mod schedule_wake;
mod state;
mod tray;

use crate::{config::ConfigStore, state::AppState};
use tauri::Manager;

#[cfg(all(feature = "ui-demo", not(debug_assertions)))]
compile_error!("ui-demo requires a debug build; pass --debug --features ui-demo");

#[cfg(any(
    all(feature = "ui-windows", feature = "ui-macos"),
    all(feature = "ui-windows", feature = "ui-ubuntu"),
    all(feature = "ui-macos", feature = "ui-ubuntu")
))]
compile_error!("choose only one UI override: ui-windows, ui-macos, or ui-ubuntu");

#[tauri::command]
fn ui_demo_enabled() -> bool {
    cfg!(feature = "ui-demo")
}

#[tauri::command]
fn ui_demo_platform() -> Option<&'static str> {
    if cfg!(feature = "ui-windows") {
        Some("windows")
    } else if cfg!(feature = "ui-macos") {
        Some("macos")
    } else if cfg!(feature = "ui-ubuntu") {
        Some("linux")
    } else {
        None
    }
}

pub fn run() {
    let mut context = tauri::generate_context!();
    configure_identity(&mut context, ui_demo_enabled());
    run_normal(context);
}

fn configure_identity<R: tauri::Runtime>(context: &mut tauri::Context<R>, demo: bool) {
    if demo {
        context.config_mut().identifier.push_str(".ui-demo");
        // Windows/Linux autostart keys use the package name, not the identifier.
        context.package_info_mut().name.push_str("-ui-demo");
    }
}

#[allow(clippy::too_many_lines)] // One composition root for normal and demo runtime wiring.
fn run_normal(context: tauri::Context<tauri::Wry>) {
    let builder = tauri::Builder::default()
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_single_instance::init(
            |app, _arguments, _working_directory| {
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.show();
                    let _ = window.set_focus();
                }
            },
        ));
    #[cfg(not(target_os = "macos"))]
    let builder = builder.plugin(tauri_plugin_autostart::init(
        tauri_plugin_autostart::MacosLauncher::LaunchAgent,
        None::<Vec<&str>>,
    ));
    builder
        .setup(|app| {
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);
            let config_path = app.path().app_config_dir()?.join("config.json");
            let store = ConfigStore::new(config_path);
            #[allow(unused_mut)]
            let mut configuration = store
                .load_or_default()
                .map_err(|error| std::io::Error::other(error.to_string()))?;
            #[cfg(feature = "ui-demo")]
            {
                let speaker = tauri::async_runtime::block_on(demo::speaker())?;
                if !store.path().exists() {
                    configuration.selected_sonos_id = Some(demo::SPEAKER_ID.into());
                    configuration.last_known_sonos_address = Some(speaker.location.to_string());
                }
            }
            #[cfg(target_os = "macos")]
            let migrated_fixed_output = if configuration.follow_default_audio_device {
                false
            } else {
                configuration
                    .fixed_audio_device_id
                    .as_deref()
                    .and_then(sonos_volume_bridge_platform_audio::macos::migrate_legacy_device_id)
                    .map(|uid| {
                        configuration.fixed_audio_device_id = Some(uid);
                        store
                            .save(&configuration)
                            .map_err(|error| std::io::Error::other(error.to_string()))
                    })
                    .transpose()?
                    .is_some()
            };
            let guard = logging::initialize(&app.path().app_log_dir()?, configuration.log_level)?;
            tracing::info!("SonosVolumeBridge application shell starting");
            #[cfg(target_os = "macos")]
            if migrated_fixed_output {
                tracing::info!(
                    "migrated selected local audio output to a persistent Core Audio UID"
                );
            }
            let state = AppState::new(store, configuration, guard);
            app.manage(state);
            tray::install(app.handle())?;
            let state = app.state::<AppState>();
            state.start_runtime(app.handle().clone());
            schedule_wake::install(app.handle());
            schedule_notifications::install();
            night_schedule::start(app.handle().clone());
            if ui_demo_enabled()
                && let Some(window) = app.get_webview_window("main")
            {
                window.set_title("Sonos Volume Bridge — UI demo")?;
                window.show()?;
                window.set_focus()?;
            }
            Ok(())
        })
        .on_window_event(|window, event| {
            match event {
                tauri::WindowEvent::CloseRequested { api, .. } => {
                    // This is a menu-bar utility: closing Settings must leave its
                    // synchronization runtime and tray controls available.
                    let _ = window.hide();
                    api.prevent_close();
                }
                tauri::WindowEvent::ThemeChanged(theme) => {
                    tray::update_icon_for_theme(window.app_handle(), *theme);
                }
                _ => {}
            }
        })
        .invoke_handler(tauri::generate_handler![
            ui_demo_enabled,
            ui_demo_platform,
            commands::get_snapshot,
            commands::get_system_hour12,
            commands::save_night_schedule,
            commands::enable_night_schedule,
            commands::set_schedule_notifications,
            commands::get_schedule_status,
            commands::save_configuration,
            commands::reset_configuration,
            commands::diagnostics,
            commands::export_diagnostics,
            commands::discover_sonos,
            commands::list_audio_outputs,
            commands::test_volume,
            commands::get_speaker_settings,
            commands::set_speaker_setting,
            commands::set_speaker_level,
            commands::use_tv_audio
        ])
        .run(context)
        .expect("Tauri runtime failed");
}

#[cfg(test)]
mod identity_tests {
    #[test]
    fn demo_startup_keeps_autostart_identity_separate_from_normal_app() {
        let build = |demo| {
            let mut context = tauri::test::mock_context(tauri::test::noop_assets());
            context.package_info_mut().name = "sonos-volume-bridge".into();
            super::configure_identity(&mut context, demo);
            tauri::test::mock_builder().build(context).unwrap()
        };
        let normal = build(false);
        let demo = build(true);
        // The autostart plugin uses package_info().name as its registration key.
        assert_eq!(normal.package_info().name, "sonos-volume-bridge");
        assert_eq!(demo.package_info().name, "sonos-volume-bridge-ui-demo");
        assert_ne!(normal.config().identifier, demo.config().identifier);
    }
}
