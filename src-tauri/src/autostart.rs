#[cfg(any(not(target_os = "macos"), test))]
pub const fn registration_name(demo: bool) -> &'static str {
    if demo {
        "sonos-volume-bridge-ui-demo"
    } else {
        "sonos-volume-bridge"
    }
}

#[cfg(target_os = "macos")]
use objc2_service_management::{SMAppService, SMAppServiceStatus};
use tauri::AppHandle;
#[cfg(not(target_os = "macos"))]
use tauri_plugin_autostart::ManagerExt;

#[cfg(target_os = "macos")]
#[allow(unsafe_code)] // `objc2` marks Objective-C message dispatch as unsafe.
fn update_macos(start_at_login: bool) -> Result<(), String> {
    let service = unsafe { SMAppService::mainAppService() };
    update_macos_service(
        start_at_login,
        || unsafe { service.status() },
        |enabled| {
            let result = if enabled {
                unsafe { service.registerAndReturnError() }
            } else {
                unsafe { service.unregisterAndReturnError() }
            };
            result.map_err(|error| {
                tracing::warn!(
                    enabled,
                    domain = %error.domain(),
                    code = error.code(),
                    "macOS login-item update failed"
                );
                macos_login_error(enabled, &error.localizedDescription().to_string())
            })
        },
    )
}

#[cfg(target_os = "macos")]
fn update_macos_service(
    start_at_login: bool,
    mut status: impl FnMut() -> SMAppServiceStatus,
    update_registration: impl FnOnce(bool) -> Result<(), String>,
) -> Result<(), String> {
    let absent = |state| {
        matches!(
            state,
            SMAppServiceStatus::NotRegistered | SMAppServiceStatus::NotFound
        )
    };
    let current = status();
    // A fresh/replaced bundle can be NotFound, and unregistering it can return
    // EPERM even though there is no service to remove. RequiresApproval still
    // represents a registered service and must go through unregistration.
    if (start_at_login && current == SMAppServiceStatus::Enabled)
        || (!start_at_login && absent(current))
    {
        return Ok(());
    }
    match update_registration(start_at_login) {
        // System Settings or macOS cleanup may have removed the item since the
        // first status read. Never ignore a failure for an entry still present.
        Err(_) if !start_at_login && absent(status()) => Ok(()),
        result => result,
    }
}

#[cfg(target_os = "macos")]
fn macos_login_error(enabled: bool, reason: &str) -> String {
    if enabled {
        format!("macOS could not enable start at login: {reason}")
    } else {
        format!(
            "macOS could not disable start at login: {reason}. Open System Settings > General > \
             Login Items & Extensions, remove Speaker Volume Bridge (or Sonos Volume Bridge) \
             from Open at Login, then try again."
        )
    }
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

#[cfg(all(test, target_os = "macos"))]
mod macos_tests {
    use super::{SMAppServiceStatus as Status, macos_login_error, update_macos_service};

    #[test]
    fn disabling_an_absent_login_item_does_not_attempt_unregistration() {
        for status in [Status::NotFound, Status::NotRegistered] {
            update_macos_service(
                false,
                || status,
                |_| panic!("an absent login item must not be unregistered"),
            )
            .unwrap();
        }
    }

    #[test]
    fn disabling_a_registered_item_including_revoked_approval_removes_it() {
        for status in [Status::Enabled, Status::RequiresApproval] {
            let mut requested = None;
            update_macos_service(
                false,
                || status,
                |enabled| {
                    requested = Some(enabled);
                    Ok(())
                },
            )
            .unwrap();
            assert_eq!(requested, Some(false));
        }
    }

    #[test]
    fn concurrent_removal_allows_disabling_despite_an_unregister_error() {
        for absent in [Status::NotFound, Status::NotRegistered] {
            let mut statuses = [Status::Enabled, absent].into_iter();
            update_macos_service(
                false,
                || statuses.next().unwrap(),
                |_| Err("Operation not permitted".into()),
            )
            .unwrap();
            assert!(statuses.next().is_none());
        }
    }

    #[test]
    fn permission_errors_are_preserved_while_registration_remains_or_is_unknown() {
        for status in [Status::Enabled, Status::RequiresApproval, Status(99)] {
            let result =
                update_macos_service(false, || status, |_| Err("Operation not permitted".into()));
            assert_eq!(result, Err("Operation not permitted".into()));
        }
    }

    #[test]
    fn enabling_an_absent_or_unapproved_item_still_attempts_registration() {
        for status in [
            Status::NotFound,
            Status::NotRegistered,
            Status::RequiresApproval,
        ] {
            let mut requested = None;
            let result = update_macos_service(
                true,
                || status,
                |enabled| {
                    requested = Some(enabled);
                    Err("registration failed".into())
                },
            );
            assert_eq!(requested, Some(true));
            assert_eq!(result, Err("registration failed".into()));
        }
    }

    #[test]
    fn enabling_an_enabled_item_does_not_register_again() {
        update_macos_service(
            true,
            || Status::Enabled,
            |_| panic!("an enabled login item must not be registered again"),
        )
        .unwrap();
    }

    #[test]
    fn removal_error_explains_how_to_recover_after_an_upgrade() {
        let error = macos_login_error(false, "Operation not permitted");
        assert!(error.contains("Operation not permitted"));
        assert!(error.contains("System Settings > General > Login Items & Extensions"));
        assert!(error.contains("Speaker Volume Bridge (or Sonos Volume Bridge)"));
        assert!(error.contains("Open at Login, then try again"));
    }
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
