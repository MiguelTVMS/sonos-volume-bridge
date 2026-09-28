#[cfg(any(not(target_os = "macos"), test))]
pub const fn registration_name(demo: bool) -> &'static str {
    if demo {
        "sonos-volume-bridge-ui-demo"
    } else {
        "sonos-volume-bridge"
    }
}

use tauri::AppHandle;
#[cfg(not(target_os = "macos"))]
use tauri_plugin_autostart::ManagerExt;

#[cfg(target_os = "macos")]
#[allow(unsafe_code)] // `objc2` marks Objective-C message dispatch as unsafe.
fn update_macos(start_at_login: bool) -> Result<(), String> {
    use objc2_service_management::{SMAppService, SMAppServiceStatus};

    let service = unsafe { SMAppService::mainAppService() };
    let status = unsafe { service.status() };

    if start_at_login {
        if status == SMAppServiceStatus::Enabled {
            return Ok(());
        }
        return unsafe { service.registerAndReturnError() }
            .map_err(|error| format!("macOS could not enable start at login: {error:?}"));
    }

    if status == SMAppServiceStatus::NotRegistered {
        return Ok(());
    }
    unsafe { service.unregisterAndReturnError() }
        .map_err(|error| format!("macOS could not disable start at login: {error:?}"))
}

#[cfg(not(target_os = "macos"))]
fn update_unpacked(app: &AppHandle, start_at_login: bool) -> Result<(), String> {
    if start_at_login {
        app.autolaunch()
            .enable()
            .map_err(|error| error.to_string())?;
        #[cfg(target_os = "linux")]
        refresh_linux_label(app)?;
        Ok(())
    } else {
        match app.autolaunch().disable() {
            Ok(()) => Ok(()),
            Err(error) if is_missing_entry(&error.to_string()) => Ok(()),
            Err(error) => Err(error.to_string()),
        }
    }
}

#[cfg(not(target_os = "macos"))]
fn is_missing_entry(error: &str) -> bool {
    error.contains("(os error 2)")
}

#[cfg(windows)]
fn update_packaged(start_at_login: bool) -> Result<(), String> {
    use windows::{
        ApplicationModel::{StartupTask, StartupTaskState},
        core::HSTRING,
    };

    let task = StartupTask::GetAsync(&HSTRING::from("SonosVolumeBridgeStartup"))
        .and_then(|operation| operation.join())
        .map_err(|error| format!("the packaged startup task is unavailable: {error}"))?;

    if !start_at_login {
        return task.Disable().map_err(|error| error.to_string());
    }

    match task.State().map_err(|error| error.to_string())? {
        StartupTaskState::Enabled | StartupTaskState::EnabledByPolicy => Ok(()),
        StartupTaskState::Disabled => {
            let state = task
                .RequestEnableAsync()
                .and_then(|operation| operation.join())
                .map_err(|error| error.to_string())?;
            match state {
                StartupTaskState::Enabled | StartupTaskState::EnabledByPolicy => Ok(()),
                _ => Err("Windows did not enable start at login".to_owned()),
            }
        }
        StartupTaskState::DisabledByUser => Err(
            "start at login was disabled in Windows Settings and must be re-enabled there"
                .to_owned(),
        ),
        StartupTaskState::DisabledByPolicy => {
            Err("start at login is disabled by Windows policy".to_owned())
        }
        _ => Err("Windows returned an unknown startup-task state".to_owned()),
    }
}

#[cfg(target_os = "macos")]
pub fn update(_app: &AppHandle, start_at_login: bool) -> Result<(), String> {
    update_macos(start_at_login)
}

#[cfg(not(target_os = "macos"))]
pub fn update(app: &AppHandle, start_at_login: bool) -> Result<(), String> {
    #[cfg(windows)]
    if windows::ApplicationModel::Package::Current().is_ok() {
        return update_packaged(start_at_login);
    }

    update_unpacked(app, start_at_login)
}

/// Retarget an existing registration without re-enabling an OS-disabled startup item.
#[cfg(not(target_os = "macos"))]
pub fn refresh_existing(app: &AppHandle) -> Result<(), String> {
    #[cfg(windows)]
    {
        // The Store manifest updates the executable while preserving task state.
        if windows::ApplicationModel::Package::Current().is_ok() {
            return Ok(());
        }
        if app.autolaunch().is_enabled().map_err(|e| e.to_string())? {
            app.autolaunch().enable().map_err(|e| e.to_string())?;
        }
        Ok(())
    }
    #[cfg(target_os = "linux")]
    refresh_linux_label(app)
}

#[cfg(all(test, not(target_os = "macos")))]
mod tests {
    use super::is_missing_entry;

    #[test]
    fn identifies_a_missing_unpacked_autostart_entry() {
        assert!(is_missing_entry(
            "The system cannot find the file specified. (os error 2)"
        ));
    }

    #[test]
    fn preserves_other_unpacked_autostart_errors() {
        assert!(!is_missing_entry("access denied"));
    }
}

#[cfg(target_os = "linux")]
fn refresh_linux_label(app: &AppHandle) -> Result<(), String> {
    use tauri::Manager;
    // auto-launch uses this same home-relative location, independently of XDG_CONFIG_HOME.
    let path = app
        .path()
        .home_dir()
        .map_err(|e| e.to_string())?
        .join(".config/autostart")
        .join(format!(
            "{}.desktop",
            registration_name(crate::ui_demo_enabled())
        ));
    let content = match std::fs::read_to_string(&path) {
        Ok(content) => content,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error.to_string()),
    };
    let display_name = if crate::ui_demo_enabled() {
        "Speaker Volume Bridge — UI demo"
    } else {
        "Speaker Volume Bridge"
    };
    std::fs::write(path, branded_desktop_entry(&content, display_name)).map_err(|e| e.to_string())
}
#[cfg(any(target_os = "linux", test))]
fn branded_desktop_entry(content: &str, name: &str) -> String {
    content
        .lines()
        .map(|line| {
            if line.starts_with("Name=") {
                format!("Name={name}")
            } else if line.starts_with("Comment=") {
                format!("Comment={name} startup")
            } else if line == "Exec=/usr/bin/sonos-volume-bridge"
                || line.starts_with("Exec=/usr/bin/sonos-volume-bridge ")
            {
                line.replacen(
                    "/usr/bin/sonos-volume-bridge",
                    "/usr/bin/speaker-volume-bridge",
                    1,
                )
            } else {
                line.to_owned()
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
        + "\n"
}
#[cfg(test)]
mod rebrand_tests {
    #[test]
    fn startup_migration_preserves_desktop_disabled_state_and_arguments() {
        let entry = "[Desktop Entry]\nName=sonos-volume-bridge\nExec=/usr/bin/sonos-volume-bridge --quiet\nHidden=true\nX-GNOME-Autostart-enabled=false\n";
        let renamed = super::branded_desktop_entry(entry, "Speaker Volume Bridge");
        assert!(renamed.contains("Exec=/usr/bin/speaker-volume-bridge --quiet\n"));
        assert!(renamed.contains("Hidden=true\n"));
        assert!(renamed.contains("X-GNOME-Autostart-enabled=false\n"));
    }
    #[test]
    fn display_name_changes_without_changing_startup_target() {
        let entry = "[Desktop Entry]\nName=sonos-volume-bridge\nExec=/usr/bin/speaker-volume-bridge\nTerminal=false\n";
        let renamed = super::branded_desktop_entry(entry, "Speaker Volume Bridge");
        assert!(renamed.contains("Name=Speaker Volume Bridge\n"));
        assert!(renamed.contains("Exec=/usr/bin/speaker-volume-bridge\n"));
        assert!(renamed.contains("Terminal=false\n"));
    }
}
