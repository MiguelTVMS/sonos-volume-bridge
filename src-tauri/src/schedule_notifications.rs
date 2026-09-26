//! Native notifications. Tauri's desktop permission methods are unconditional,
//! so macOS uses UserNotifications for both authorization and delivery.
use tauri::{AppHandle, Runtime};
#[cfg(not(any(target_os = "macos", target_os = "linux", windows)))]
use tauri_plugin_notification::NotificationExt;

#[cfg(any(windows, test))]
mod windows;

#[derive(Clone, Copy)]
pub enum ScheduleNotice<'a> {
    Boundary { next: Option<&'a str> },
    Applied,
}

pub fn schedule_body(speaker: &str, active: bool, notice: ScheduleNotice<'_>) -> String {
    let speaker = crate::runtime::display_speaker_name(speaker);
    match notice {
        ScheduleNotice::Applied => format!(
            "{speaker}: Night Mode is {}.",
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
#[cfg(not(any(target_os = "macos", target_os = "linux", windows)))]
#[allow(clippy::unused_async)] // Shared delivery interface; Linux awaits D-Bus.
pub async fn send<R: Runtime>(app: &AppHandle<R>, title: &str, body: &str) {
    let _ = app.notification().builder().title(title).body(body).show();
}

#[cfg(target_os = "linux")]
pub async fn send<R: Runtime>(app: &AppHandle<R>, title: &str, body: &str) {
    // GNOME watches the application's sender name and removes its notification
    // source when that name vanishes. Reuse one asynchronous connection for the
    // process lifetime instead of dropping a per-notification handle on return.
    static CONNECTION: tokio::sync::Mutex<Option<zbus::Connection>> =
        tokio::sync::Mutex::const_new(None);
    let mut connection = CONNECTION.lock().await;
    let delivery = async {
        if connection.is_none() {
            *connection = Some(zbus::Connection::session().await?);
        }
        let app_name = app
            .config()
            .product_name
            .as_deref()
            .unwrap_or("Sonos Volume Bridge");
        let response = connection
            .as_ref()
            .unwrap()
            .call_method(
                Some("org.freedesktop.Notifications"),
                "/org/freedesktop/Notifications",
                Some("org.freedesktop.Notifications"),
                "Notify",
                &(
                    app_name,
                    0_u32,
                    "",
                    title,
                    body,
                    Vec::<String>::new(),
                    std::collections::HashMap::<String, zbus::zvariant::Value<'_>>::new(),
                    -1_i32,
                ),
            )
            .await?;
        let _: u32 = response.body().deserialize()?;
        Ok::<(), zbus::Error>(())
    };
    match tokio::time::timeout(std::time::Duration::from_secs(2), delivery).await {
        Ok(Ok(())) => tracing::info!("Night schedule notification accepted by desktop"),
        Ok(Err(_)) => {
            *connection = None;
            tracing::warn!("Night schedule notification delivery failed");
        }
        Err(_) => {
            *connection = None;
            tracing::warn!("Night schedule notification delivery timed out");
        }
    }
}

#[cfg(windows)]
#[allow(clippy::unused_async)] // Shared async interface; macOS awaits its native permission callback.
pub async fn permitted<R: Runtime>(app: &AppHandle<R>, _: bool) -> bool {
    windows::notifier(
        &app.config().identifier,
        app.config()
            .product_name
            .as_deref()
            .unwrap_or("Sonos Volume Bridge"),
    )
    .is_ok_and(|notifier| notifier.permitted())
}

#[cfg(windows)]
#[allow(clippy::unused_async)]
pub async fn send<R: Runtime>(app: &AppHandle<R>, title: &str, body: &str) {
    if let Err(error) = windows::notifier(
        &app.config().identifier,
        app.config()
            .product_name
            .as_deref()
            .unwrap_or("Sonos Volume Bridge"),
    )
    .and_then(|notifier| notifier.send(title, body))
    {
        tracing::warn!(%error, "Could not deliver Windows schedule notification");
    }
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
    struct TestNotifications {
        messages: tokio::sync::mpsc::UnboundedSender<(String, String)>,
        senders: std::sync::Mutex<Vec<String>>,
    }

    #[cfg(target_os = "linux")]
    #[zbus::interface(name = "org.freedesktop.Notifications")]
    impl TestNotifications {
        #[allow(clippy::too_many_arguments, clippy::needless_pass_by_value)] // Protocol signature and zbus-injected header.
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
            #[zbus(header)] header: zbus::message::Header<'_>,
        ) -> u32 {
            let _ = (
                app_name,
                replaces_id,
                app_icon,
                actions,
                hints,
                expire_timeout,
            );
            self.senders
                .lock()
                .unwrap()
                .push(header.sender().unwrap().to_string());
            self.messages
                .send((summary.to_owned(), body.to_owned()))
                .unwrap();
            1
        }

        async fn sender_still_connected(
            &self,
            #[zbus(connection)] connection: &zbus::Connection,
        ) -> bool {
            let senders = self.senders.lock().unwrap().clone();
            let Some(first) = senders.first() else {
                return false;
            };
            if !senders.iter().all(|sender| sender == first) {
                return false;
            }
            zbus::fdo::DBusProxy::new(connection)
                .await
                .unwrap()
                .name_has_owner(first.as_str().try_into().unwrap())
                .await
                .unwrap()
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
            (true, ScheduleNotice::Applied),
            (false, ScheduleNotice::Applied),
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
        let connection = zbus::Connection::session().await.unwrap();
        let alive: bool = connection
            .call_method(
                Some("org.freedesktop.Notifications"),
                "/org/freedesktop/Notifications",
                Some("org.freedesktop.Notifications"),
                "SenderStillConnected",
                &(),
            )
            .await
            .unwrap()
            .body()
            .deserialize()
            .unwrap();
        assert!(
            alive,
            "GNOME needs the app notification sender to remain connected after delivery"
        );
    }

    #[cfg(target_os = "linux")]
    #[tokio::test]
    async fn saved_transition_probe() {
        use crate::{
            config::{AppConfiguration, ConfigStore, ScheduleNotifications},
            state::AppState,
        };
        use sonos_volume_bridge_integration::night_mode::{
            NightModePort, NightModeReading, apply_saved,
        };
        use tauri::Manager;
        struct Speaker(std::sync::Mutex<bool>);
        #[async_trait::async_trait]
        impl NightModePort for Speaker {
            async fn read(&self) -> NightModeReading {
                NightModeReading::Supported(*self.0.lock().unwrap())
            }
            async fn write(&self, active: bool) -> Result<(), String> {
                *self.0.lock().unwrap() = active;
                Ok(())
            }
        }
        if std::env::var_os("SVB_NOTIFICATION_TEST_CHILD").is_none() {
            return;
        }
        let (_, guard) = tracing_appender::non_blocking(std::io::sink());
        let app = tauri::test::mock_builder()
            .plugin(tauri_plugin_notification::init())
            .manage(AppState::new(
                ConfigStore::new(std::env::temp_dir().join("unused-notification-test-config.json")),
                AppConfiguration::default(),
                guard,
            ))
            .build(tauri::test::mock_context(tauri::test::noop_assets()))
            .unwrap();
        for mode in [
            ScheduleNotifications::Start,
            ScheduleNotifications::End,
            ScheduleNotifications::Both,
            ScheduleNotifications::Never,
        ] {
            for enabled in [false, true] {
                let mut configuration = AppConfiguration {
                    selected_sonos_id: Some("test-speaker".into()),
                    notify_night_mode_schedule_transitions: mode,
                    ..AppConfiguration::default()
                };
                configuration.night_mode_schedule.enabled = enabled;
                *app.state::<AppState>().configuration.lock().unwrap() = configuration.clone();
                let speaker = Speaker(std::sync::Mutex::new(false));
                let mut was_scheduled = false;
                // Exercise the shared Save orchestration: unchanged outside,
                // entry, repeated save, exit, repeated save. Only entry/exit notify.
                for active in [false, true, true, false, false] {
                    let is_scheduled = enabled && active;
                    let applied = apply_saved(&speaker, active, was_scheduled, is_scheduled).await;
                    was_scheduled = is_scheduled;
                    assert!(applied.error.is_none());
                    crate::night_schedule::notify_saved(
                        app.handle(),
                        &configuration,
                        applied.notification,
                        "Test speaker",
                    )
                    .await;
                }
            }
        }
    }

    #[cfg(target_os = "linux")]
    #[tokio::test]
    async fn saving_notifies_only_on_confirmed_changes_matching_the_preference() {
        let received =
            capture_linux_notifications("schedule_notifications::tests::saved_transition_probe")
                .await;
        assert_eq!(received.len(), 4);
        for active in [false, true] {
            let expected = schedule_body("Test speaker", active, ScheduleNotice::Applied);
            let title = if active {
                "Night Mode schedule started"
            } else {
                "Night Mode schedule ended"
            };
            assert_eq!(
                received
                    .iter()
                    .filter(|(summary, body)| summary == title && body == &expected)
                    .count(),
                2
            );
        }
    }

    #[cfg(target_os = "linux")]
    #[tokio::test]
    async fn enable_transition_probe() {
        use crate::{
            config::{AppConfiguration, ConfigStore, ScheduleNotifications},
            state::AppState,
        };
        use tauri::Manager;
        if std::env::var_os("SVB_NOTIFICATION_TEST_CHILD").is_none() {
            return;
        }
        let speaker = crate::demo::speaker().await.unwrap();
        let directory =
            std::env::temp_dir().join(format!("schedule-enable-test-{}", std::process::id()));
        let (_, guard) = tracing_appender::non_blocking(std::io::sink());
        let app = tauri::test::mock_builder()
            .plugin(tauri_plugin_notification::init())
            .manage(AppState::new(
                ConfigStore::new(directory.join("config.json")),
                AppConfiguration::default(),
                guard,
            ))
            .build(tauri::test::mock_context(tauri::test::noop_assets()))
            .unwrap();
        // Call the same command as Settings and the tray before the worker's
        // first tick. Cover current-time membership and every preference.
        for mode in [
            ScheduleNotifications::Start,
            ScheduleNotifications::End,
            ScheduleNotifications::Both,
            ScheduleNotifications::Never,
        ] {
            for inside in [false, true] {
                let mut configuration = AppConfiguration {
                    selected_sonos_id: Some(crate::demo::SPEAKER_ID.into()),
                    last_known_sonos_address: Some(speaker.location.to_string()),
                    notify_night_mode_schedule_transitions: mode,
                    ..AppConfiguration::default()
                };
                configuration.night_mode_schedule.blocks = vec![vec![inside; 48]; 7];
                app.state::<AppState>()
                    .replace_configuration(configuration.clone());
                crate::commands::set_speaker_setting(
                    crate::runtime::SpeakerSetting::NightSound,
                    false,
                    app.state(),
                )
                .await
                .unwrap();
                for enabled in [true, true, false] {
                    crate::commands::enable_night_schedule(
                        enabled,
                        app.state(),
                        app.handle().clone(),
                    )
                    .await
                    .unwrap();
                    assert_eq!(
                        crate::runtime::speaker_settings(configuration.clone())
                            .await
                            .night_sound,
                        Some(inside)
                    );
                }
            }
        }
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[cfg(target_os = "linux")]
    #[tokio::test]
    async fn enabling_inside_schedule_notifies_once_matching_start_preference() {
        let received =
            capture_linux_notifications("schedule_notifications::tests::enable_transition_probe")
                .await;
        assert_eq!(
            received.len(),
            2,
            "only Start and Both notify on entry; repeated enable, disable and outside stay silent"
        );
        assert!(
            received
                .iter()
                .all(|(title, body)| title == "Night Mode schedule started"
                    && body.ends_with("Night Mode is on."))
        );
    }

    #[cfg(target_os = "linux")]
    async fn capture_linux_notifications(probe: &'static str) -> Vec<(String, String)> {
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
            .serve_at(
                "/org/freedesktop/Notifications",
                TestNotifications {
                    messages: sender,
                    senders: std::sync::Mutex::default(),
                },
            )
            .unwrap()
            .build()
            .await
            .unwrap();
        let child = tokio::task::spawn_blocking(move || {
            Command::new(std::env::current_exe().unwrap())
                .args(["--exact", probe, "--nocapture"])
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
        let mut notifications = Vec::new();
        while let Ok(notification) = receiver.try_recv() {
            notifications.push(notification);
        }
        notifications
    }

    #[cfg(target_os = "linux")]
    #[tokio::test]
    async fn linux_notifications_reach_the_desktop_service_from_the_async_runtime() {
        let received =
            capture_linux_notifications("schedule_notifications::tests::linux_delivery_probe")
                .await;
        let mut bodies = Vec::new();
        for (title, body) in received {
            assert_eq!(title, "Night schedule test");
            bodies.push(body);
        }
        bodies.sort();
        let mut expected = vec![
            "Test speaker: Night Mode is on.",
            "Test speaker: Night Mode is off.",
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
                    ScheduleNotice::Applied,
                    "Office Desk Speaker: Night Mode is on.",
                ),
                (
                    false,
                    ScheduleNotice::Applied,
                    "Office Desk Speaker: Night Mode is off.",
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
