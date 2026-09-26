//! Shell adapter for the single global schedule. The write gate also guards
//! configuration mutations and manual Night Mode commands, not volume traffic.
use crate::{config::AppConfiguration, runtime, state::AppState, tray};
use async_trait::async_trait;
use jiff::{Timestamp, tz::TimeZone};
use serde::Serialize;
use sonos_volume_bridge_integration::night_mode::{
    NightModeController, NightModePort, NightModeReading,
};
use sonos_volume_bridge_sonos::{FeatureAvailability, SonosClient, SonosDevice};
use std::{sync::atomic::Ordering, time::Duration};
use tauri::{AppHandle, Emitter, Manager, Runtime};

pub use sonos_volume_bridge_integration::night_mode::LOCK_MESSAGE;
#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScheduleStatus {
    pub active: bool,
    pub supported: bool,
    pub message: String,
    pub next_transition: Option<String>,
    pub time_zone: String,
    pub notifications_blocked: bool,
}
struct Port {
    client: SonosClient,
    device: SonosDevice,
}
#[async_trait]
impl NightModePort for Port {
    async fn read(&self) -> NightModeReading {
        let value = self.client.get_eq(&self.device, "NightMode").await;
        match FeatureAvailability::from_read(&value) {
            FeatureAvailability::Supported => NightModeReading::Supported(value.unwrap_or(false)),
            FeatureAvailability::Unsupported => NightModeReading::Unsupported,
            FeatureAvailability::Unavailable => NightModeReading::Unavailable,
        }
    }
    async fn write(&self, value: bool) -> Result<(), String> {
        self.client
            .set_eq(&self.device, "NightMode", value)
            .await
            .map_err(|_| "Could not update Night Mode. Retrying automatically.".into())
    }
}
async fn port(configuration: &AppConfiguration) -> Option<Port> {
    let client = SonosClient::builder()
        .timeout(Duration::from_secs(3))
        .build()
        .ok()?;
    let device = runtime::resolve_device(&client, configuration).await.ok()?;
    Some(Port { client, device })
}
pub async fn supported(configuration: &AppConfiguration) -> bool {
    if let Some(port) = port(configuration).await {
        matches!(port.read().await, NightModeReading::Supported(_))
    } else {
        false
    }
}
pub fn locked(configuration: &AppConfiguration) -> bool {
    configuration.selected_sonos_id.is_some()
        && TimeZone::try_system()
            .ok()
            .and_then(|zone| {
                configuration
                    .night_mode_schedule
                    .evaluate(Timestamp::now(), &zone)
                    .ok()
            })
            .is_some_and(|window| window.active)
}
#[allow(clippy::too_many_lines)] // One worker owns the selected speaker schedule lifecycle.
pub fn start<R: Runtime>(app: AppHandle<R>) {
    tauri::async_runtime::spawn(async move {
        let mut controller = NightModeController::default();
        let mut previous_zone = String::new();
        let mut previous_time = Timestamp::now();
        let mut expected = None;
        let mut read_at = Timestamp::now();
        let mut retry_seconds = 5_i64;
        let mut recovering = false;
        loop {
            tokio::time::sleep(Duration::from_secs(1)).await;
            let state = app.state::<AppState>();
            if state.schedule_stopped.load(Ordering::Relaxed) {
                return;
            }
            let gate = state.speaker_gate.lock().await;
            let Ok(configuration) = state.configuration.lock().map(|c| c.clone()) else {
                continue;
            };
            let now = Timestamp::now();
            let Ok(zone) = TimeZone::try_system() else {
                publish(
                    &app,
                    ScheduleStatus {
                        message: "Computer time zone unavailable. Scheduling paused.".into(),
                        ..Default::default()
                    },
                );
                continue;
            };
            let zone_name = zone
                .iana_name()
                .map_or_else(|| zone.to_offset(now).to_string(), str::to_owned);
            let changed = controller.configure(
                configuration.selected_sonos_id.as_deref(),
                &configuration.night_mode_schedule,
            );
            let clock_jump =
                (now.as_second() - previous_time.as_second()).abs() > 10 || now < previous_time;
            let refresh = state.schedule_refresh.swap(false, Ordering::Relaxed);
            let wake = state.schedule_reconcile.swap(false, Ordering::Relaxed);
            let reconcile = changed || clock_jump || wake || previous_zone != zone_name;
            previous_time = now;
            previous_zone.clone_from(&zone_name);
            if changed {
                expected = None;
            }
            let notifications_blocked = configuration
                .notify_night_mode_schedule_transitions
                .enabled()
                && state
                    .schedule_status
                    .lock()
                    .is_ok_and(|s| s.notifications_blocked);
            if !configuration.night_mode_schedule.enabled
                || configuration.selected_sonos_id.is_none()
            {
                publish(
                    &app,
                    ScheduleStatus {
                        message: if configuration.selected_sonos_id.is_none() {
                            "Select a compatible speaker."
                        } else {
                            "Schedule disabled."
                        }
                        .into(),
                        time_zone: zone_name,
                        notifications_blocked,
                        ..Default::default()
                    },
                );
                continue;
            }
            let Ok(window) = configuration.night_mode_schedule.evaluate(now, &zone) else {
                continue;
            };
            if !reconcile && !refresh && expected == Some(window.active) && now < read_at {
                continue;
            }
            expected = Some(window.active);
            let next = window
                .next
                .map(|time| time.to_zoned(zone.clone()).strftime("%a %H:%M").to_string());
            let mut status = ScheduleStatus {
                active: window.active,
                next_transition: next.clone(),
                time_zone: zone_name,
                ..Default::default()
            };
            let Some(port) = port(&configuration).await else {
                recovering = true;
                status.message =
                    "Speaker unavailable. Scheduling will resume automatically.".into();
                publish(&app, status);
                read_at = now.checked_add(Duration::from_secs(5)).unwrap_or(now);
                continue;
            };
            let result = controller
                .step(&port, window.active, reconcile || recovering)
                .await;
            recovering = false;
            status.supported = matches!(result.reading, NightModeReading::Supported(_));
            status.message = result
                .error
                .clone()
                .unwrap_or_else(|| match result.reading {
                    NightModeReading::Unsupported => {
                        "Not supported by this speaker. Schedule saved but paused.".into()
                    }
                    NightModeReading::Unavailable => {
                        "Temporarily unavailable. Retrying automatically.".into()
                    }
                    NightModeReading::Supported(_) if window.active => LOCK_MESSAGE.into(),
                    NightModeReading::Supported(_) => {
                        "Outside scheduled hours. Manual control is available.".into()
                    }
                });
            if result.error.is_some() || !status.supported {
                retry_seconds = (retry_seconds * 2).min(60);
            } else {
                retry_seconds = 5;
            }
            read_at = Timestamp::now()
                .checked_add(jiff::Span::new().seconds(retry_seconds))
                .unwrap_or(now);
            drop(gate);
            if configuration
                .notify_night_mode_schedule_transitions
                .enabled()
            {
                status.notifications_blocked =
                    !crate::schedule_notifications::permitted(&app, false).await;
                let _gate = state.speaker_gate.lock().await;
                let current = state.configuration.lock().map(|c| c.clone()).ok();
                if !current.as_ref().is_some_and(|c| {
                    c.selected_sonos_id == configuration.selected_sonos_id
                        && c.night_mode_schedule == configuration.night_mode_schedule
                        && c.notify_night_mode_schedule_transitions
                            == configuration.notify_night_mode_schedule_transitions
                }) {
                    continue;
                }
                if !status.notifications_blocked
                    && let Some(active) = result.notification
                    && configuration
                        .notify_night_mode_schedule_transitions
                        .allows(active)
                {
                    let body = crate::schedule_notifications::schedule_body(
                        &port.device.friendly_name,
                        active,
                        crate::schedule_notifications::ScheduleNotice::Boundary {
                            next: next.as_deref(),
                        },
                    );
                    crate::schedule_notifications::send(
                        &app,
                        if active {
                            "Night Mode schedule started"
                        } else {
                            "Night Mode schedule ended"
                        },
                        &body,
                    );
                }
            }
            publish(&app, status);
            let _ = app.emit("speaker-settings-changed", ());
            tray::refresh_speaker_controls(&app);
        }
    });
}
fn publish<R: Runtime>(app: &AppHandle<R>, status: ScheduleStatus) {
    if let Ok(mut current) = app.state::<AppState>().schedule_status.lock() {
        *current = status.clone();
    }
    let _ = app.emit("night-schedule-changed", status);
}

pub async fn set_manual(configuration: &AppConfiguration, enabled: bool) -> Result<(), String> {
    let port = port(configuration)
        .await
        .ok_or("Selected speaker is unavailable.")?;
    sonos_volume_bridge_integration::night_mode::apply_manual(&port, enabled, locked(configuration))
        .await
}

/// Called with the same write gate as persistence and manual speaker operations.
pub async fn apply_saved(configuration: &AppConfiguration) -> Result<(bool, String), String> {
    let zone = TimeZone::try_system().map_err(|_| "Computer time zone unavailable.")?;
    let mut schedule = configuration.night_mode_schedule.clone();
    schedule.enabled = true; // Explicit Save tests the grid even when recurrence is disabled.
    let window = schedule
        .evaluate(Timestamp::now(), &zone)
        .map_err(|_| "Could not evaluate the schedule.")?;
    let port = port(configuration)
        .await
        .ok_or("Selected speaker is unavailable.")?;
    let result =
        sonos_volume_bridge_integration::night_mode::apply_saved(&port, window.active).await;
    if let Some(error) = result.error {
        return Err(error);
    }
    if !matches!(result.reading, NightModeReading::Supported(_)) {
        return Err("Night Mode is unavailable for this speaker.".into());
    }
    Ok((window.active, port.device.friendly_name))
}
pub async fn notify_saved<R: Runtime>(
    app: &AppHandle<R>,
    configuration: &AppConfiguration,
    active: bool,
    speaker: &str,
) {
    if !configuration
        .notify_night_mode_schedule_transitions
        .allows(active)
    {
        return;
    }
    let permitted = crate::schedule_notifications::permitted(app, false).await;
    let state = app.state::<AppState>();
    let _gate = state.speaker_gate.lock().await;
    if !state.configuration.lock().is_ok_and(|current| {
        current.selected_sonos_id == configuration.selected_sonos_id
            && current.night_mode_schedule == configuration.night_mode_schedule
            && current.notify_night_mode_schedule_transitions
                == configuration.notify_night_mode_schedule_transitions
    }) {
        return;
    }
    if let Ok(mut status) = state.schedule_status.lock() {
        status.notifications_blocked = !permitted;
    }
    if permitted {
        crate::schedule_notifications::send(
            app,
            if active {
                "Night Mode schedule started"
            } else {
                "Night Mode schedule ended"
            },
            &crate::schedule_notifications::schedule_body(
                speaker,
                active,
                crate::schedule_notifications::ScheduleNotice::Saved,
            ),
        );
    }
}
