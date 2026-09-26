//! Native notifications. Tauri's desktop permission methods are unconditional,
//! so macOS uses UserNotifications for both authorization and delivery.
use tauri::{AppHandle, Runtime};
#[cfg(not(target_os = "macos"))]
use tauri_plugin_notification::NotificationExt;

#[derive(Clone, Copy)]
pub enum ScheduleNotice<'a> {
    Boundary { next: Option<&'a str> },
    Saved,
}

pub fn schedule_body(speaker: &str, active: bool, notice: ScheduleNotice<'_>) -> String {
    let speaker = crate::runtime::display_speaker_name(speaker);
    match notice {
        ScheduleNotice::Saved => format!(
            "{speaker}: Night Mode is {}. Applied from Save schedule.",
            if active { "on" } else { "off" }
        ),
        ScheduleNotice::Boundary { next } if active => format!(
            "{speaker}: Night Mode is on until {}.",
            next.unwrap_or("the schedule ends")
        ),
        ScheduleNotice::Boundary { .. } => {
            format!("{speaker}: Night Mode is off. Manual control is available.")
        }
    }
}

#[cfg(target_os = "macos")]
pub async fn permitted<R: Runtime>(_: &AppHandle<R>, request: bool) -> bool {
    use objc2_user_notifications::{
        UNAuthorizationOptions, UNAuthorizationStatus, UNNotificationSettings,
        UNUserNotificationCenter,
    };
    let (sender, receiver) = tokio::sync::oneshot::channel();
    let sender = std::sync::Mutex::new(Some(sender));
    if request {
        let block = block2::RcBlock::new(
            move |granted: objc2::runtime::Bool, _: *mut objc2_foundation::NSError| {
                if let Some(sender) = sender.lock().ok().and_then(|mut s| s.take()) {
                    let _ = sender.send(granted.as_bool());
                }
            },
        );
        UNUserNotificationCenter::currentNotificationCenter()
            .requestAuthorizationWithOptions_completionHandler(
                UNAuthorizationOptions::Alert | UNAuthorizationOptions::Sound,
                &block,
            );
    } else {
        let block =
            block2::RcBlock::new(move |settings: std::ptr::NonNull<UNNotificationSettings>| {
                // SAFETY: UserNotifications provides a valid settings object for this callback.
                #[allow(unsafe_code)]
                let granted = unsafe { settings.as_ref() }.authorizationStatus()
                    == UNAuthorizationStatus::Authorized;
                if let Some(sender) = sender.lock().ok().and_then(|mut s| s.take()) {
                    let _ = sender.send(granted);
                }
            });
        UNUserNotificationCenter::currentNotificationCenter()
            .getNotificationSettingsWithCompletionHandler(&block);
    }
    tokio::time::timeout(
        std::time::Duration::from_secs(if request { 60 } else { 2 }),
        receiver,
    )
    .await
    .ok()
    .and_then(Result::ok)
    .unwrap_or(false)
}
#[cfg(not(any(target_os = "macos", windows)))]
#[allow(clippy::unused_async)] // Shared async interface; macOS awaits its native permission callback.
pub async fn permitted<R: Runtime>(app: &AppHandle<R>, request: bool) -> bool {
    let result = if request {
        app.notification().request_permission()
    } else {
        app.notification().permission_state()
    };
    result.is_ok_and(|permission| permission == tauri::plugin::PermissionState::Granted)
}
#[cfg(target_os = "macos")]
#[allow(clippy::unused_async)] // Shared delivery interface; Linux awaits D-Bus.
pub async fn send<R: Runtime>(_: &AppHandle<R>, title: &str, body: &str) {
    use objc2_foundation::NSString;
    use objc2_user_notifications::{
        UNMutableNotificationContent, UNNotificationRequest, UNUserNotificationCenter,
    };
    let content = UNMutableNotificationContent::new();
    content.setTitle(&NSString::from_str(title));
    content.setBody(&NSString::from_str(body));
    let request = UNNotificationRequest::requestWithIdentifier_content_trigger(
        &NSString::from_str(&format!("night-schedule-{}", jiff::Timestamp::now())),
        &content,
        None,
    );
    UNUserNotificationCenter::currentNotificationCenter()
        .addNotificationRequest_withCompletionHandler(&request, None);
}
#[cfg(not(any(target_os = "macos", target_os = "linux")))]
#[allow(clippy::unused_async)] // Shared delivery interface; Linux awaits D-Bus.
pub async fn send<R: Runtime>(app: &AppHandle<R>, title: &str, body: &str) {
    let _ = app.notification().builder().title(title).body(body).show();
}

#[cfg(target_os = "linux")]
pub async fn send<R: Runtime>(app: &AppHandle<R>, title: &str, body: &str) {
    // Tauri's plugin calls blocking notify-rust from a Tokio task. With our
    // Tokio-enabled zbus that starts a nested runtime and panics before delivery.
    let mut notification = notify_rust::Notification::new();
    notification
        .appname(
            app.config()
                .product_name
                .as_deref()
                .unwrap_or("Sonos Volume Bridge"),
        )
        .summary(title)
        .body(body);
    match tokio::time::timeout(std::time::Duration::from_secs(2), notification.show_async()).await {
        Ok(Ok(_)) => {}
        Ok(Err(_)) => tracing::warn!("Night schedule notification delivery failed"),
        Err(_) => tracing::warn!("Night schedule notification delivery timed out"),
    }
}

#[cfg(windows)]
#[allow(clippy::unused_async)] // Shared async interface; macOS awaits its native permission callback.
pub async fn permitted<R: Runtime>(app: &AppHandle<R>, _: bool) -> bool {
    use windows::{
        UI::Notifications::{NotificationSetting, ToastNotificationManager},
        core::HSTRING,
    };
    ToastNotificationManager::CreateToastNotifierWithId(&HSTRING::from(&app.config().identifier))
        .and_then(|notifier| notifier.Setting())
        .is_ok_and(|setting| setting == NotificationSetting::Enabled)
}

#[cfg(target_os = "macos")]
#[allow(unsafe_code)] // Stateless Objective-C delegate presents native foreground notifications.
mod foreground {
    use objc2::{ClassType, define_class, msg_send, rc::Retained, runtime::ProtocolObject};
    use objc2_foundation::{NSObject, NSObjectProtocol};
    use objc2_user_notifications::{
        UNNotification, UNNotificationPresentationOptions, UNUserNotificationCenter,
        UNUserNotificationCenterDelegate,
    };
    define_class!(
        #[unsafe(super = NSObject)]
        #[name = "SVBNightScheduleNotificationDelegate"]
        struct Delegate;
        // SAFETY: NSObject has no additional protocol invariants.
        unsafe impl NSObjectProtocol for Delegate {}
        // SAFETY: matches the native delegate signature and always completes once.
        unsafe impl UNUserNotificationCenterDelegate for Delegate {
            #[unsafe(method(userNotificationCenter:willPresentNotification:withCompletionHandler:))]
            fn present(
                &self,
                _: &UNUserNotificationCenter,
                _: &UNNotification,
                completion: &block2::DynBlock<dyn Fn(UNNotificationPresentationOptions)>,
            ) {
                completion.call((UNNotificationPresentationOptions::Banner
                    | UNNotificationPresentationOptions::List,));
            }
        }
    );
    pub fn install() {
        // SAFETY: NSObject's new initializes the stateless delegate.
        let delegate: Retained<Delegate> = unsafe { msg_send![Delegate::class(), new] };
        UNUserNotificationCenter::currentNotificationCenter()
            .setDelegate(Some(ProtocolObject::from_ref(&*delegate)));
        // The center keeps a weak reference; retain this sole delegate for process lifetime.
        std::mem::forget(delegate);
    }
}
pub fn install() {
    #[cfg(target_os = "macos")]
    foreground::install();
}

#[cfg(test)]
mod tests {
    use super::{ScheduleNotice, schedule_body};

    #[cfg(target_os = "linux")]
    struct TestNotifications(tokio::sync::mpsc::UnboundedSender<(String, String)>);

    #[cfg(target_os = "linux")]
    #[zbus::interface(name = "org.freedesktop.Notifications")]
    impl TestNotifications {
        #[allow(clippy::too_many_arguments)] // Freedesktop notification protocol signature.
        fn notify(
            &self,
            app_name: &str,
            replaces_id: u32,
            app_icon: &str,
            summary: &str,
            body: &str,
            actions: Vec<String>,
            hints: std::collections::HashMap<String, zbus::zvariant::OwnedValue>,
            expire_timeout: i32,
        ) -> u32 {
            let _ = (
                app_name,
                replaces_id,
                app_icon,
                actions,
                hints,
                expire_timeout,
            );
            self.0.send((summary.to_owned(), body.to_owned())).unwrap();
            1
        }
    }

    // Subprocess isolation keeps the test bus out of the user's desktop and
    // avoids changing process environment while other Rust tests are running.
    #[cfg(target_os = "linux")]
    #[tokio::test]
    async fn linux_delivery_probe() {
        if std::env::var_os("SVB_NOTIFICATION_TEST_CHILD").is_none() {
            return;
        }
        let app = tauri::test::mock_builder()
            .plugin(tauri_plugin_notification::init())
            .build(tauri::test::mock_context(tauri::test::noop_assets()))
            .unwrap();
        assert!(super::permitted(app.handle(), false).await);
        for (active, notice) in [
            (true, ScheduleNotice::Saved),
            (false, ScheduleNotice::Saved),
            (
                true,
                ScheduleNotice::Boundary {
                    next: Some("07:00"),
                },
            ),
            (false, ScheduleNotice::Boundary { next: None }),
        ] {
            super::send(
                app.handle(),
                "Night schedule test",
                &schedule_body("Test speaker", active, notice),
            )
            .await;
        }
    }

    #[cfg(target_os = "linux")]
    #[tokio::test]
    async fn linux_notifications_reach_the_desktop_service_from_the_async_runtime() {
        use std::{
            io::{BufRead, BufReader},
            process::{Command, Stdio},
        };
        struct Bus(std::process::Child);
        impl Drop for Bus {
            fn drop(&mut self) {
                let _ = self.0.kill();
                let _ = self.0.wait();
            }
        }
        let mut bus = Bus(Command::new("dbus-daemon")
            .args(["--session", "--nofork", "--print-address=1"])
            .stdout(Stdio::piped())
            .spawn()
            .expect("Linux notification regression requires dbus-daemon"));
        let mut address = String::new();
        BufReader::new(bus.0.stdout.take().unwrap())
            .read_line(&mut address)
            .unwrap();
        let address = address.trim().to_owned();
        let (sender, mut receiver) = tokio::sync::mpsc::unbounded_channel();
        let _service = zbus::connection::Builder::address(address.as_str())
            .unwrap()
            .name("org.freedesktop.Notifications")
            .unwrap()
            .serve_at("/org/freedesktop/Notifications", TestNotifications(sender))
            .unwrap()
            .build()
            .await
            .unwrap();
        let child = tokio::task::spawn_blocking(move || {
            Command::new(std::env::current_exe().unwrap())
                .args([
                    "--exact",
                    "schedule_notifications::tests::linux_delivery_probe",
                    "--nocapture",
                ])
                .env("SVB_NOTIFICATION_TEST_CHILD", "1")
                .env("DBUS_SESSION_BUS_ADDRESS", address)
                .output()
                .unwrap()
        })
        .await
        .unwrap();
        assert!(
            child.status.success(),
            "{}",
            String::from_utf8_lossy(&child.stderr)
        );
        let mut bodies = Vec::new();
        while let Ok((title, body)) = receiver.try_recv() {
            assert_eq!(title, "Night schedule test");
            bodies.push(body);
        }
        bodies.sort();
        let mut expected = vec![
            "Test speaker: Night Mode is on. Applied from Save schedule.",
            "Test speaker: Night Mode is off. Applied from Save schedule.",
            "Test speaker: Night Mode is on until 07:00.",
            "Test speaker: Night Mode is off. Manual control is available.",
        ];
        expected.sort_unstable();
        assert_eq!(
            bodies, expected,
            "Save and boundary notifications must reach D-Bus without a nested-runtime panic"
        );
    }

    #[test]
    fn all_schedule_notifications_use_the_same_human_speaker_name_as_settings() {
        for name in [
            "Office Desk Speaker",
            "Office Desk Speaker - Sonos Ray Media Renderer - RINCON_TEST",
        ] {
            for (active, notice, expected) in [
                (
                    true,
                    ScheduleNotice::Boundary {
                        next: Some("07:00"),
                    },
                    "Office Desk Speaker: Night Mode is on until 07:00.",
                ),
                (
                    false,
                    ScheduleNotice::Boundary { next: None },
                    "Office Desk Speaker: Night Mode is off. Manual control is available.",
                ),
                (
                    true,
                    ScheduleNotice::Saved,
                    "Office Desk Speaker: Night Mode is on. Applied from Save schedule.",
                ),
                (
                    false,
                    ScheduleNotice::Saved,
                    "Office Desk Speaker: Night Mode is off. Applied from Save schedule.",
                ),
            ] {
                assert_eq!(schedule_body(name, active, notice), expected);
            }
        }
    }

    #[test]
    fn room_name_hyphens_are_preserved_and_missing_end_time_has_a_fallback() {
        assert_eq!(
            schedule_body(
                "Office - Desk Speaker - Sonos Ray Media Renderer - RINCON_TEST",
                true,
                ScheduleNotice::Boundary { next: None }
            ),
            "Office - Desk Speaker: Night Mode is on until the schedule ends."
        );
    }
}
