//! Native notifications. Tauri's desktop permission methods are unconditional,
//! so macOS uses UserNotifications for both authorization and delivery.
use tauri::{AppHandle, Runtime};
#[cfg(not(target_os = "macos"))]
use tauri_plugin_notification::NotificationExt;

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
pub async fn permitted<R: Runtime>(app: &AppHandle<R>, request: bool) -> bool {
    let result = if request {
        app.notification().request_permission()
    } else {
        app.notification().permission_state()
    };
    result.is_ok_and(|permission| permission == tauri::plugin::PermissionState::Granted)
}
#[cfg(target_os = "macos")]
pub fn send<R: Runtime>(_: &AppHandle<R>, title: &str, body: &str) {
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
#[cfg(not(target_os = "macos"))]
pub fn send<R: Runtime>(app: &AppHandle<R>, title: &str, body: &str) {
    let _ = app.notification().builder().title(title).body(body).show();
}

#[cfg(windows)]
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
