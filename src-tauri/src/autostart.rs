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
        app.autolaunch().enable().map_err(|error| error.to_string())
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
