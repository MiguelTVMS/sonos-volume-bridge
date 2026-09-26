//! Process-lifetime native wake registrations. Callbacks only mark reconciliation.
use crate::state::AppState;
use tauri::{AppHandle, Manager, Runtime};
fn wake<R: Runtime>(app: &AppHandle<R>) {
    if let Some(state) = app.try_state::<AppState>() {
        state
            .schedule_reconcile
            .store(true, std::sync::atomic::Ordering::Relaxed);
    }
}
#[cfg(target_os = "macos")]
#[allow(unsafe_code)] // Foundation retains a sendable block; observer lives for the app process.
pub fn install<R: Runtime>(app: &AppHandle<R>) {
    use objc2_app_kit::{NSWorkspace, NSWorkspaceDidWakeNotification};
    let app = app.clone();
    let block = block2::RcBlock::new(
        move |_: std::ptr::NonNull<objc2_foundation::NSNotification>| wake(&app),
    );
    // SAFETY: no object/queue filter; the captured AppHandle is Send + Sync.
    let observer = unsafe {
        NSWorkspace::sharedWorkspace()
            .notificationCenter()
            .addObserverForName_object_queue_usingBlock(
                Some(NSWorkspaceDidWakeNotification),
                None,
                None,
                &block,
            )
    };
    // Exactly one registration, installed at setup and retained until process exit.
    std::mem::forget(observer);
}
#[cfg(windows)]
#[allow(unsafe_code)] // Win32 requires a C callback and stable process-lifetime context.
pub fn install<R: Runtime>(app: &AppHandle<R>) {
    use windows::Win32::{
        Foundation::HANDLE,
        System::Power::{
            DEVICE_NOTIFY_SUBSCRIBE_PARAMETERS, PowerRegisterSuspendResumeNotification,
        },
        UI::WindowsAndMessaging::{DEVICE_NOTIFY_CALLBACK, PBT_APMRESUMEAUTOMATIC},
    };
    unsafe extern "system" fn resumed<R: Runtime>(
        context: *const core::ffi::c_void,
        kind: u32,
        _: *const core::ffi::c_void,
    ) -> u32 {
        if kind == PBT_APMRESUMEAUTOMATIC {
            // SAFETY: context points to the retained AppHandle created below.
            wake(unsafe { &*context.cast::<AppHandle<R>>() });
        }
        0
    }
    let context = Box::into_raw(Box::new(app.clone()));
    let mut parameters = DEVICE_NOTIFY_SUBSCRIBE_PARAMETERS {
        Callback: Some(resumed::<R>),
        Context: context.cast(),
    };
    let mut registration = std::ptr::null_mut();
    // SAFETY: valid callback parameters; registration owns the callback until process exit.
    let result = unsafe {
        PowerRegisterSuspendResumeNotification(
            DEVICE_NOTIFY_CALLBACK,
            HANDLE(std::ptr::from_mut(&mut parameters).cast()),
            &raw mut registration,
        )
    };
    if result.0 != 0 {
        // SAFETY: registration failed; no callback can access this allocation.
        drop(unsafe { Box::from_raw(context) });
    }
}
#[cfg(target_os = "linux")]
pub fn install<R: Runtime>(app: &AppHandle<R>) {
    use futures_util::StreamExt;
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        loop {
            if let Ok(connection) = zbus::Connection::system().await
                && let Ok(proxy) = zbus::Proxy::new(
                    &connection,
                    "org.freedesktop.login1",
                    "/org/freedesktop/login1",
                    "org.freedesktop.login1.Manager",
                )
                .await
                && let Ok(mut signals) = proxy.receive_signal("PrepareForSleep").await
            {
                while let Some(message) = signals.next().await {
                    if matches!(message.body().deserialize::<bool>(), Ok(false)) {
                        wake(&app);
                    }
                }
            }
            tokio::time::sleep(std::time::Duration::from_secs(30)).await;
        }
    });
}
#[cfg(not(any(target_os = "macos", windows, target_os = "linux")))]
pub fn install<R: Runtime>(_: &AppHandle<R>) {}
