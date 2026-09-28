//! Shell-only legacy process inspection and conflict lifecycle.
use serde::Serialize;
use std::{
    path::Path,
    sync::atomic::{AtomicBool, AtomicU64, Ordering},
    time::Duration,
};
use tauri::{AppHandle, Emitter, Manager};
use tokio::sync::{Mutex, Notify, RwLock, RwLockReadGuard};

pub const MESSAGE: &str =
    "Sonos Volume Bridge is still running. Quit the old app to resume synchronization.";
static EPOCH: AtomicU64 = AtomicU64::new(0);
static CHECK: Notify = Notify::const_new();
static CHECK_GATE: Mutex<()> = Mutex::const_new(());
static WRITES: RwLock<bool> = RwLock::const_new(cfg!(not(test)));
static PAUSED: AtomicBool = AtomicBool::new(cfg!(not(test)));
static STATUS: std::sync::Mutex<Conflict> = std::sync::Mutex::new(Conflict {
    status: Detection::Unknown,
    paused: false,
});

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Detection {
    Clear,
    LegacyRunning,
    Unknown,
}
#[derive(Clone, Copy, Debug, Serialize)]
pub struct Conflict {
    pub status: Detection,
    pub paused: bool,
}
impl Conflict {
    fn observe(&mut self, next: Detection) -> (bool, bool) {
        let was_paused = self.paused;
        self.status = next;
        match next {
            Detection::LegacyRunning => self.paused = true,
            Detection::Clear => self.paused = false,
            Detection::Unknown => {}
        }
        (!was_paused && self.paused, was_paused && !self.paused)
    }
}
pub fn paused() -> bool {
    PAUSED.load(Ordering::SeqCst)
}
pub fn request_check() {
    CHECK.notify_one();
}
/// Hold through an entire write so a conflict transition drains in-flight work.
pub struct WriteIntent(u64);
pub fn write_intent() -> WriteIntent {
    WriteIntent(EPOCH.load(Ordering::SeqCst))
}
impl WriteIntent {
    pub fn validate(&self) -> Result<(), String> {
        if self.0 != EPOCH.load(Ordering::SeqCst) || paused() {
            Err(MESSAGE.into())
        } else {
            Ok(())
        }
    }
    pub async fn permit(&self) -> Result<RwLockReadGuard<'static, bool>, String> {
        let permit = permit_from(&WRITES).await?;
        self.validate()?;
        Ok(permit)
    }
}
pub async fn write_permit() -> Result<RwLockReadGuard<'static, bool>, String> {
    write_intent().permit().await
}
async fn permit_from(gate: &RwLock<bool>) -> Result<RwLockReadGuard<'_, bool>, String> {
    let permit = gate.read().await;
    if *permit {
        Err(MESSAGE.into())
    } else {
        Ok(permit)
    }
}
#[tauri::command]
pub fn get_legacy_status() -> Conflict {
    STATUS.lock().map_or(
        Conflict {
            status: Detection::Unknown,
            paused: paused(),
        },
        |s| *s,
    )
}
#[tauri::command]
pub async fn recheck_legacy_app(app: AppHandle) -> Conflict {
    check(&app).await;
    get_legacy_status()
}
const UPGRADE_URL: &str = "https://svb.miguel.ms/upgrade.html";
#[tauri::command]
pub async fn open_legacy_upgrade() -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(open_upgrade)
        .await
        .map_err(|_| "Could not open upgrade instructions.".to_owned())?
}
#[cfg(target_os = "macos")]
fn open_upgrade() -> Result<(), String> {
    use objc2_foundation::{NSString, NSURL};
    let url = NSURL::URLWithString(&NSString::from_str(UPGRADE_URL))
        .ok_or("Upgrade address is unavailable.")?;
    if objc2_app_kit::NSWorkspace::sharedWorkspace().openURL(&url) {
        Ok(())
    } else {
        Err("Could not open upgrade instructions.".into())
    }
}
#[cfg(not(target_os = "macos"))]
fn open_upgrade() -> Result<(), String> {
    #[cfg(windows)]
    let mut command = {
        let mut command = std::process::Command::new("rundll32.exe");
        command.arg("url.dll,FileProtocolHandler");
        command
    };
    #[cfg(target_os = "linux")]
    let mut command = std::process::Command::new("xdg-open");
    // Fixed address only: this command exposes no arbitrary URL or shell arguments.
    let result = command
        .arg(UPGRADE_URL)
        .status()
        .map_err(|e| e.to_string())?;
    if result.success() {
        Ok(())
    } else {
        Err("Could not open upgrade instructions.".into())
    }
}
fn legacy_path(path: &Path) -> bool {
    let resolved = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    let name = resolved.file_name().and_then(|n| n.to_str()).unwrap_or("");
    let name = name.strip_suffix(" (deleted)").unwrap_or(name);
    matches!(name, "sonos-volume-bridge" | "sonos-volume-bridge.exe")
        && !resolved
            .components()
            .any(|part| part.as_os_str().to_string_lossy().contains("ui-demo"))
}
fn inspect() -> Detection {
    use sysinfo::{ProcessRefreshKind, ProcessesToUpdate, System, UpdateKind};
    if crate::ui_demo_enabled() {
        return Detection::Clear;
    }
    let Ok(pid) = sysinfo::get_current_pid() else {
        return Detection::Unknown;
    };
    let mut system = System::new();
    system.refresh_processes_specifics(
        ProcessesToUpdate::All,
        true,
        ProcessRefreshKind::nothing()
            .with_exe(UpdateKind::Always)
            .with_user(UpdateKind::Always)
            .without_tasks(),
    );
    let Some(owner) = system.process(pid).and_then(sysinfo::Process::user_id) else {
        return Detection::Unknown;
    };
    // A sandbox may expose only this process. That is not evidence of absence.
    let mut uncertain = system.processes().len() <= 1;
    for (other_pid, process) in system.processes() {
        if *other_pid == pid {
            continue;
        }
        if let Some(other_owner) = process.user_id() {
            if other_owner != owner {
                continue;
            }
        } else {
            if process.name() == "sonos-volume-bridge"
                || process.name() == "sonos-volume-bridge.exe"
            {
                uncertain = true;
            }
            continue;
        }
        match process.exe() {
            Some(path) if legacy_path(path) => return Detection::LegacyRunning,
            None => {
                uncertain = true;
            }
            _ => {}
        }
    }
    if uncertain {
        Detection::Unknown
    } else {
        Detection::Clear
    }
}
async fn check(app: &AppHandle) {
    let _serial = CHECK_GATE.lock().await;
    let detection = tauri::async_runtime::spawn_blocking(inspect)
        .await
        .unwrap_or(Detection::Unknown);
    // Drain all write adapters before publishing the paused state.
    let mut gate = WRITES.write().await;
    let (entered, recovered, status) = {
        let Ok(mut state) = STATUS.lock() else {
            return;
        };
        let (entered, recovered) = state.observe(detection);
        (entered, recovered, *state)
    };
    *gate = status.paused;
    PAUSED.store(status.paused, Ordering::SeqCst);
    let state = app.state::<crate::state::AppState>();
    if entered {
        EPOCH.fetch_add(1, Ordering::SeqCst);
        state.pause_for_legacy();
    }
    drop(gate);
    if recovered {
        state.schedule_reconcile.store(true, Ordering::Relaxed);
        state.start_runtime(app.clone());
    }
    let _ = app.emit("legacy-app-changed", status);
    announce_conflict(entered, &DesktopNotice(app)).await;
}
#[async_trait::async_trait]
trait ConflictNotice: Sync {
    fn show_settings(&self);
    async fn permitted(&self) -> bool;
    async fn send(&self);
}
struct DesktopNotice<'a>(&'a AppHandle);
#[async_trait::async_trait]
impl ConflictNotice for DesktopNotice<'_> {
    fn show_settings(&self) {
        if let Some(window) = self.0.get_webview_window("main") {
            let _ = window.show();
            let _ = window.set_focus();
        }
    }
    async fn permitted(&self) -> bool {
        crate::schedule_notifications::permitted(self.0, false).await
    }
    async fn send(&self) {
        crate::schedule_notifications::send(self.0, "Speaker Volume Bridge paused", MESSAGE).await;
    }
}
async fn announce_conflict(entered: bool, notice: &impl ConflictNotice) {
    if entered {
        notice.show_settings();
        if notice.permitted().await {
            notice.send().await;
        }
    }
}
pub fn start(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        check(&app).await;
        app.state::<crate::state::AppState>()
            .start_runtime(app.clone());
        loop {
            tokio::select! { () = tokio::time::sleep(Duration::from_secs(5)) => {}, () = CHECK.notified() => {} }
            check(&app).await;
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn conflict_episode_notifies_once_and_unknown_cannot_resume() {
        let mut state = Conflict {
            status: Detection::Unknown,
            paused: false,
        };
        assert_eq!(state.observe(Detection::Unknown), (false, false));
        assert_eq!(state.observe(Detection::LegacyRunning), (true, false));
        assert_eq!(state.observe(Detection::LegacyRunning), (false, false));
        assert_eq!(state.observe(Detection::Unknown), (false, false));
        assert!(state.paused);
        assert_eq!(state.observe(Detection::Clear), (false, true));
        assert_eq!(state.observe(Detection::Clear), (false, false));
        assert_eq!(state.observe(Detection::LegacyRunning), (true, false));
    }
    #[tokio::test]
    async fn native_notice_is_attempted_once_per_episode_and_denial_keeps_settings() {
        use std::sync::atomic::AtomicUsize;
        struct Notice {
            granted: bool,
            shown: AtomicUsize,
            permissions: AtomicUsize,
            sent: AtomicUsize,
        }
        #[async_trait::async_trait]
        impl ConflictNotice for Notice {
            fn show_settings(&self) {
                self.shown.fetch_add(1, Ordering::SeqCst);
            }
            async fn permitted(&self) -> bool {
                self.permissions.fetch_add(1, Ordering::SeqCst);
                self.granted
            }
            async fn send(&self) {
                self.sent.fetch_add(1, Ordering::SeqCst);
            }
        }
        for granted in [false, true] {
            let notice = Notice {
                granted,
                shown: AtomicUsize::new(0),
                permissions: AtomicUsize::new(0),
                sent: AtomicUsize::new(0),
            };
            let mut state = Conflict {
                status: Detection::Unknown,
                paused: false,
            };
            for detection in [
                Detection::Unknown,
                Detection::LegacyRunning,
                Detection::LegacyRunning,
                Detection::Unknown,
                Detection::Clear,
                Detection::LegacyRunning,
            ] {
                let (entered, _) = state.observe(detection);
                announce_conflict(entered, &notice).await;
            }
            assert_eq!(notice.shown.load(Ordering::SeqCst), 2);
            assert_eq!(notice.permissions.load(Ordering::SeqCst), 2);
            assert_eq!(
                notice.sent.load(Ordering::SeqCst),
                if granted { 2 } else { 0 }
            );
            assert!(state.paused);
        }
    }
    #[test]
    fn exact_executables_only() {
        assert!(legacy_path(Path::new("/applications/sonos-volume-bridge")));
        assert!(!legacy_path(Path::new(
            "/applications/speaker-volume-bridge"
        )));
        assert!(!legacy_path(Path::new(
            "/applications/sonos-volume-bridge-test"
        )));
        assert!(!legacy_path(Path::new("/ui-demo/sonos-volume-bridge")));
    }
}

#[cfg(test)]
mod adapter_tests {
    use super::*;
    #[tokio::test]
    async fn conflict_drains_active_writes_and_rejects_queued_writes() {
        let gate = RwLock::new(false);
        let active = permit_from(&gate).await.unwrap();
        let pause = gate.write();
        tokio::pin!(pause);
        assert!(
            tokio::time::timeout(Duration::from_millis(10), &mut pause)
                .await
                .is_err()
        );
        drop(active);
        let mut paused = pause.await;
        *paused = true;
        drop(paused);
        assert!(permit_from(&gate).await.is_err());
        *gate.write().await = false;
        assert!(permit_from(&gate).await.is_ok());
    }
    #[cfg(all(unix, not(feature = "ui-demo")))]
    #[test]
    fn platform_adapter_detects_a_running_legacy_executable() {
        struct Fixture {
            child: std::process::Child,
            directory: std::path::PathBuf,
        }
        impl Drop for Fixture {
            fn drop(&mut self) {
                let _ = self.child.kill();
                let _ = self.child.wait();
                let _ = std::fs::remove_dir_all(&self.directory);
            }
        }
        let directory =
            std::env::temp_dir().join(format!("svb-process-fixture-{}", std::process::id()));
        std::fs::create_dir_all(&directory).unwrap();
        let executable = directory.join("sonos-volume-bridge");
        std::fs::copy("/bin/sleep", &executable).unwrap();
        let _fixture = Fixture {
            child: std::process::Command::new(&executable)
                .arg("30")
                .spawn()
                .unwrap(),
            directory,
        };
        let deadline = std::time::Instant::now() + Duration::from_secs(2);
        while std::time::Instant::now() < deadline {
            if inspect() == Detection::LegacyRunning {
                return;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        panic!("platform inspection did not identify the legacy executable");
    }
    #[cfg(unix)]
    #[test]
    fn compatibility_symlink_is_not_a_legacy_executable() {
        let dir = std::env::temp_dir().join(format!("svb-legacy-link-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let target = dir.join("speaker-volume-bridge");
        std::fs::write(&target, b"fixture").unwrap();
        let link = dir.join("sonos-volume-bridge");
        std::os::unix::fs::symlink(&target, &link).unwrap();
        assert!(!legacy_path(&link));
        std::fs::remove_dir_all(dir).unwrap();
    }
}
