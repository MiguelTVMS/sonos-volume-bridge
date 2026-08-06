use tauri::AppHandle;
use tauri_plugin_autostart::ManagerExt;

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

pub fn update(app: &AppHandle, start_at_login: bool) -> Result<(), String> {
    #[cfg(windows)]
    if windows::ApplicationModel::Package::Current().is_ok() {
        return update_packaged(start_at_login);
    }

    update_unpacked(app, start_at_login)
}

#[cfg(test)]
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
