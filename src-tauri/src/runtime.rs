//! Supervised application runtime.
//!
//! This module is the composition root for protocol and platform adapters. Tauri
//! commands only request a restart; they do not perform network or audio work.

use crate::{
    config::AppConfiguration,
    state::{UiSnapshot, UiStatus},
    tray,
};
use async_trait::async_trait;
use serde::Serialize;
use sonos_volume_bridge_domain::{LocalAudioState, LocalOrigin, MuteState, SonosVolume};
use sonos_volume_bridge_integration::{
    Coordinator, IntegrationError, LocalAudioPort, SonosPort, SynchronizationPolicy,
};
use sonos_volume_bridge_platform_audio::{
    AudioDeviceSelection, PlatformAudioError, SystemAudioController, SystemAudioEvent,
};
use sonos_volume_bridge_sonos::{
    CallbackListener, GenaClient, SonosClient, SonosDevice, SonosId, discover,
};
use sonos_volume_bridge_synchronization::Synchronizer;
use std::{
    net::{IpAddr, SocketAddr},
    sync::{Arc, Mutex},
    time::Duration,
};
use tauri::AppHandle;
use tokio::{
    net::UdpSocket,
    sync::watch,
    time::{Instant, sleep, sleep_until},
};
use tracing::warn;
use url::Url;

const SONOS_TIMEOUT: Duration = Duration::from_secs(3);
const DISCOVERY_TIMEOUT: Duration = Duration::from_secs(2);
const REQUESTED_SUBSCRIPTION: Duration = Duration::from_secs(300);
const INITIAL_RECONNECT_DELAY: Duration = Duration::from_secs(1);
const MAX_RECONNECT_DELAY: Duration = Duration::from_secs(30);

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiscoveredSonos {
    pub id: String,
    pub friendly_name: String,
    pub location: String,
}

/// Performs bounded local-network discovery for the settings application service.
pub async fn discover_available() -> Result<Vec<DiscoveredSonos>, String> {
    let client = SonosClient::builder()
        .timeout(SONOS_TIMEOUT)
        .build()
        .map_err(|_| "Unable to create the local Sonos client.".to_owned())?;
    let locations = discover(DISCOVERY_TIMEOUT)
        .await
        .map_err(|_| "Sonos discovery is unavailable on this network.".to_owned())?;
    let mut devices = Vec::new();
    for location in locations {
        if let Ok(found) = client.retrieve_device(location.clone()).await {
            devices.push(DiscoveredSonos {
                id: found.device.id.as_str().to_owned(),
                friendly_name: found.device.friendly_name,
                location: location.to_string(),
            });
        }
    }
    devices.sort_by(|left, right| left.friendly_name.cmp(&right.friendly_name));
    devices.dedup_by(|left, right| left.id == right.id);
    Ok(devices)
}

/// Confirms that the selected speaker accepts a RenderingControl volume write
/// without changing its currently confirmed volume.
pub async fn test_selected_device(configuration: AppConfiguration) -> Result<(), String> {
    let client = SonosClient::builder()
        .timeout(SONOS_TIMEOUT)
        .build()
        .map_err(|_| "Unable to create the local Sonos client.".to_owned())?;
    let device = resolve_device(&client, &configuration)
        .await
        .map_err(|_| "The selected Sonos device is unavailable.".to_owned())?;
    let volume = client
        .get_volume(&device)
        .await
        .map_err(|_| "The selected Sonos device did not return its volume.".to_owned())?;
    client
        .set_volume(&device, volume)
        .await
        .map_err(|_| "The selected Sonos device rejected the volume test.".to_owned())
}

/// Owns exactly one cancellable runtime generation. A newer configuration
/// invalidates updates from all older generations before starting its replacement.
#[derive(Default)]
pub struct RuntimeManager {
    active: Mutex<Option<watch::Sender<bool>>>,
    generation: std::sync::atomic::AtomicU64,
}

impl RuntimeManager {
    pub fn stop(&self) {
        if let Ok(mut active) = self.active.lock()
            && let Some(shutdown) = active.take()
        {
            let _ = shutdown.send(true);
        }
    }
    pub fn restart(
        &self,
        configuration: AppConfiguration,
        snapshot: Arc<Mutex<UiSnapshot>>,
        app: AppHandle,
    ) {
        if let Ok(mut active) = self.active.lock() {
            if let Some(shutdown) = active.take() {
                let _ = shutdown.send(true);
            }
            let (shutdown, receiver) = watch::channel(false);
            *active = Some(shutdown);
            let generation = self
                .generation
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed)
                + 1;
            if let Ok(mut current) = snapshot.lock() {
                current.runtime_generation = generation;
            }
            tauri::async_runtime::spawn(run_supervisor(
                configuration,
                snapshot,
                app,
                generation,
                receiver,
            ));
        }
    }
}

async fn run_supervisor(
    configuration: AppConfiguration,
    snapshot: Arc<Mutex<UiSnapshot>>,
    app: AppHandle,
    generation: u64,
    mut shutdown: watch::Receiver<bool>,
) {
    if configuration.selected_sonos_id.is_none() {
        update_snapshot(
            &snapshot,
            &app,
            generation,
            UiStatus::ConfigurationRequired,
            None,
        );
        return;
    }

    let mut delay = INITIAL_RECONNECT_DELAY;
    loop {
        if *shutdown.borrow() {
            return;
        }
        update_snapshot(&snapshot, &app, generation, UiStatus::Connecting, None);
        let result = run_session(&configuration, &snapshot, &app, generation, &mut shutdown).await;
        if *shutdown.borrow() {
            return;
        }
        let status = match result {
            Ok(()) => UiStatus::Connecting,
            Err(RuntimeError::Local(PlatformAudioError::UnsupportedDevice)) => {
                UiStatus::UnsupportedLocalDevice
            }
            Err(RuntimeError::Local(_)) => UiStatus::LocalAudioUnavailable,
            Err(RuntimeError::SonosUnavailable) => UiStatus::SonosUnavailable,
            Err(RuntimeError::Cancelled) => return,
            Err(RuntimeError::Integration) => UiStatus::Error,
        };
        warn!(
            kind = status_name(&status),
            "runtime session stopped; reconnecting with bounded backoff"
        );
        update_snapshot(&snapshot, &app, generation, status, None);
        tokio::select! {
            () = sleep(delay) => {},
            changed = shutdown.changed() => {
                if changed.is_err() || *shutdown.borrow() { return; }
            }
        }
        delay = (delay * 2).min(MAX_RECONNECT_DELAY);
    }
}

#[allow(clippy::too_many_lines)] // This select loop owns one session's complete lifecycle.
async fn run_session(
    configuration: &AppConfiguration,
    snapshot: &Arc<Mutex<UiSnapshot>>,
    app: &AppHandle,
    generation: u64,
    shutdown: &mut watch::Receiver<bool>,
) -> Result<(), RuntimeError> {
    let client = SonosClient::builder()
        .timeout(SONOS_TIMEOUT)
        .build()
        .map_err(|_| RuntimeError::SonosUnavailable)?;
    let device = resolve_device(&client, configuration).await?;
    let audio = create_audio(configuration)?;
    let machine = Synchronizer::new(
        configuration.mapping.clone(),
        configuration.maximum_sonos_volume,
        configuration.synchronize_mute,
    )
    .map_err(|_| RuntimeError::Integration)?;
    let sonos = DeviceSonosPort {
        client: client.clone(),
        device: device.clone(),
    };
    let local = DeviceAudioPort(Arc::clone(&audio));
    let mut coordinator = Coordinator::new(machine, sonos, local, SynchronizationPolicy::default());
    let mut audio_events = audio.subscribe();

    update_snapshot(
        snapshot,
        app,
        generation,
        UiStatus::Connecting,
        Some(SnapshotValues {
            name: &device.friendly_name,
            sonos_volume: None,
            local_volume: None,
            muted: None,
        }),
    );
    coordinator
        .reconcile_startup()
        .await
        .map_err(RuntimeError::from)?;
    let confirmed_volume = client
        .get_volume(&device)
        .await
        .map_err(|_| RuntimeError::SonosUnavailable)?;
    let confirmed_mute = client
        .get_mute(&device)
        .await
        .map_err(|_| RuntimeError::SonosUnavailable)?;
    let local_state = audio.current_state().await.map_err(RuntimeError::Local)?;
    update_snapshot(
        snapshot,
        app,
        generation,
        UiStatus::SubscriptionDegraded,
        Some(SnapshotValues {
            name: &device.friendly_name,
            sonos_volume: None,
            local_volume: Some(local_state.volume.get()),
            muted: Some(local_state.muted.0),
        }),
    );

    let peer = peer_address(&device)?;
    let bind = selected_local_bind(peer).await?;
    let mut listener = CallbackListener::bind(bind, peer.ip())
        .await
        .map_err(|_| RuntimeError::SonosUnavailable)?;
    let gena =
        GenaClient::new(SONOS_TIMEOUT, 64 * 1024).map_err(|_| RuntimeError::SonosUnavailable)?;
    let mut subscription = gena
        .subscribe(&device, listener.callback_url(), REQUESTED_SUBSCRIPTION)
        .await
        .ok();
    if let Some(value) = subscription.as_ref() {
        listener.set_subscription(value);
    }
    update_snapshot(
        snapshot,
        app,
        generation,
        if subscription.is_some() {
            UiStatus::Synchronized
        } else {
            UiStatus::SubscriptionDegraded
        },
        Some(SnapshotValues {
            name: &device.friendly_name,
            sonos_volume: Some(confirmed_volume.get()),
            local_volume: Some(local_state.volume.get()),
            muted: Some(confirmed_mute.0),
        }),
    );
    let mut renew_at = subscription
        .as_ref()
        .map_or_else(|| Instant::now() + Duration::from_secs(5), renewal_at);
    let mut last_local = Some(local_state);

    loop {
        let poll_delay = coordinator.next_poll_interval();
        tokio::select! {
            changed = shutdown.changed() => {
                if changed.is_err() || *shutdown.borrow() {
                    if let Some(value) = subscription.as_ref() { let _ = gena.unsubscribe(&device, value).await; }
                    return Err(RuntimeError::Cancelled);
                }
            }
            event = listener.recv() => {
                if let Some(event) = event {
                    coordinator.on_sonos_event(event.state.volume, event.state.muted).await.map_err(RuntimeError::from)?;
                    let local_state = audio.current_state().await.map_err(RuntimeError::Local)?;
                    update_snapshot(snapshot, app, generation, UiStatus::Synchronized, Some(SnapshotValues { name: &device.friendly_name, sonos_volume: Some(event.state.volume.get()), local_volume: Some(local_state.volume.get()), muted: Some(event.state.muted.0) }));
                }
            }
            received = audio_events.recv() => match received {
                Ok(SystemAudioEvent::StateChanged { state, origin }) => {
                    let mute_changed = last_local.is_none_or(|previous| previous.muted != state.muted);
                    last_local = Some(state);
                    update_snapshot(snapshot, app, generation, UiStatus::WaitingForSonosConfirmation, Some(SnapshotValues { name: &device.friendly_name, sonos_volume: None, local_volume: Some(state.volume.get()), muted: Some(state.muted.0) }));
                    coordinator.on_local_event(state, origin, mute_changed).await.map_err(RuntimeError::from)?;
                    if !mute_changed && origin != LocalOrigin::Application {
                        coordinator.run_local_once().await.map_err(RuntimeError::from)?;
                    }
                }
                Ok(SystemAudioEvent::DefaultOutputChanged | SystemAudioEvent::DeviceUnavailable { .. }) | Err(_) => return Err(RuntimeError::Local(PlatformAudioError::DeviceUnavailable)),
            },
            () = sleep_until(renew_at) => {
                if let Some(current) = subscription.as_ref() {
                    if let Ok(next) = gena.renew(&device, current, REQUESTED_SUBSCRIPTION).await {
                        listener.set_subscription(&next);
                        renew_at = renewal_at(&next);
                        subscription = Some(next);
                    } else {
                        subscription = None;
                        coordinator.on_subscription_lost();
                        update_snapshot(snapshot, app, generation, UiStatus::SubscriptionDegraded, None);
                        renew_at = Instant::now() + Duration::from_secs(5);
                    }
                } else {
                    subscription = gena.subscribe(&device, listener.callback_url(), REQUESTED_SUBSCRIPTION).await.ok();
                    if let Some(next) = subscription.as_ref() { listener.set_subscription(next); renew_at = renewal_at(next); } else { renew_at = Instant::now() + Duration::from_secs(5); }
                }
            }
            () = sleep(poll_delay), if subscription.is_none() && configuration.fallback_polling => {
                coordinator.poll_once().await.map_err(RuntimeError::from)?;
                update_snapshot(snapshot, app, generation, UiStatus::PollingFallback, None);
            }
        }
    }
}

fn renewal_at(subscription: &sonos_volume_bridge_sonos::Subscription) -> Instant {
    Instant::now()
        + subscription
            .timeout
            .mul_f32(0.8)
            .max(Duration::from_secs(1))
}

fn peer_address(device: &SonosDevice) -> Result<SocketAddr, RuntimeError> {
    let host = device
        .rendering_control
        .event_url
        .host_str()
        .ok_or(RuntimeError::SonosUnavailable)?;
    let ip = host
        .parse::<IpAddr>()
        .map_err(|_| RuntimeError::SonosUnavailable)?;
    Ok(SocketAddr::new(
        ip,
        device
            .rendering_control
            .event_url
            .port_or_known_default()
            .unwrap_or(1400),
    ))
}

async fn selected_local_bind(peer: SocketAddr) -> Result<SocketAddr, RuntimeError> {
    let socket = UdpSocket::bind(match peer {
        SocketAddr::V4(_) => "0.0.0.0:0",
        SocketAddr::V6(_) => "[::]:0",
    })
    .await
    .map_err(|_| RuntimeError::SonosUnavailable)?;
    socket
        .connect(peer)
        .await
        .map_err(|_| RuntimeError::SonosUnavailable)?;
    Ok(SocketAddr::new(
        socket
            .local_addr()
            .map_err(|_| RuntimeError::SonosUnavailable)?
            .ip(),
        0,
    ))
}

async fn resolve_device(
    client: &SonosClient,
    configuration: &AppConfiguration,
) -> Result<SonosDevice, RuntimeError> {
    let selected = SonosId::new(
        configuration
            .selected_sonos_id
            .clone()
            .ok_or(RuntimeError::SonosUnavailable)?,
    )
    .map_err(|_| RuntimeError::SonosUnavailable)?;
    if let Some(address) = configuration.last_known_sonos_address.as_deref()
        && let Ok(location) = Url::parse(address)
        && let Ok(found) = client.retrieve_device(location).await
        && found.device.id == selected
    {
        return Ok(found.device);
    }
    let locations = discover(DISCOVERY_TIMEOUT)
        .await
        .map_err(|_| RuntimeError::SonosUnavailable)?;
    for location in locations {
        if let Ok(found) = client.retrieve_device(location).await
            && found.device.id == selected
        {
            return Ok(found.device);
        }
    }
    Err(RuntimeError::SonosUnavailable)
}

fn create_audio(
    configuration: &AppConfiguration,
) -> Result<Arc<dyn SystemAudioController>, RuntimeError> {
    let selection = if configuration.follow_default_audio_device {
        AudioDeviceSelection::FollowDefault
    } else if let Some(device_id) = configuration.fixed_audio_device_id.clone() {
        AudioDeviceSelection::Fixed { device_id }
    } else {
        AudioDeviceSelection::FollowDefault
    };
    #[cfg(windows)]
    {
        Ok(Arc::new(
            sonos_volume_bridge_platform_audio::windows::WindowsAudioController::start(selection)
                .map_err(RuntimeError::Local)?,
        ))
    }
    #[cfg(target_os = "macos")]
    {
        Ok(Arc::new(
            sonos_volume_bridge_platform_audio::macos::MacosAudioController::start(selection, 1)
                .map_err(RuntimeError::Local)?,
        ))
    }
    #[cfg(not(any(windows, target_os = "macos")))]
    {
        let _ = selection;
        Err(RuntimeError::Local(PlatformAudioError::DeviceUnavailable))
    }
}

#[derive(Clone)]
struct DeviceSonosPort {
    client: SonosClient,
    device: SonosDevice,
}
#[async_trait]
impl SonosPort for DeviceSonosPort {
    async fn current_state(&self) -> Result<(SonosVolume, MuteState), IntegrationError> {
        Ok((
            self.client
                .get_volume(&self.device)
                .await
                .map_err(|error| IntegrationError::Sonos(error.to_string()))?,
            self.client
                .get_mute(&self.device)
                .await
                .map_err(|error| IntegrationError::Sonos(error.to_string()))?,
        ))
    }
    async fn set_volume(&self, volume: SonosVolume) -> Result<(), IntegrationError> {
        self.client
            .set_volume(&self.device, volume)
            .await
            .map_err(|error| IntegrationError::Sonos(error.to_string()))
    }
    async fn set_mute(&self, muted: MuteState) -> Result<(), IntegrationError> {
        self.client
            .set_mute(&self.device, muted)
            .await
            .map_err(|error| IntegrationError::Sonos(error.to_string()))
    }
}
struct DeviceAudioPort(Arc<dyn SystemAudioController>);
#[async_trait]
impl LocalAudioPort for DeviceAudioPort {
    async fn apply(&self, state: LocalAudioState) -> Result<(), IntegrationError> {
        self.0
            .set_volume(state.volume, LocalOrigin::Application)
            .await
            .map_err(|error| IntegrationError::Audio(error.to_string()))?;
        self.0
            .set_muted(state.muted.0, LocalOrigin::Application)
            .await
            .map_err(|error| IntegrationError::Audio(error.to_string()))
    }
}

#[derive(Debug)]
enum RuntimeError {
    SonosUnavailable,
    Local(PlatformAudioError),
    Integration,
    Cancelled,
}
impl From<IntegrationError> for RuntimeError {
    fn from(value: IntegrationError) -> Self {
        let _ = value;
        Self::Integration
    }
}

struct SnapshotValues<'a> {
    name: &'a str,
    sonos_volume: Option<u8>,
    local_volume: Option<u8>,
    muted: Option<bool>,
}

fn update_snapshot(
    snapshot: &Arc<Mutex<UiSnapshot>>,
    app: &AppHandle,
    generation: u64,
    status: UiStatus,
    values: Option<SnapshotValues<'_>>,
) {
    if let Ok(mut current) = snapshot.lock() {
        // A stopped generation may not overwrite a newer runtime's state.
        if current.runtime_generation != generation {
            return;
        }
        current.status = status;
        if let Some(values) = values {
            current.sonos_name = Some(values.name.to_owned());
            if values.sonos_volume.is_some() {
                current.sonos_volume = values.sonos_volume;
            }
            if values.local_volume.is_some() {
                current.local_volume = values.local_volume;
            }
            if values.muted.is_some() {
                current.muted = values.muted;
            }
        }
    }
    let _ = generation;
    tray::refresh(app);
}

fn status_name(status: &UiStatus) -> &'static str {
    match status {
        UiStatus::Discovering => "discovering",
        UiStatus::Connecting => "connecting",
        UiStatus::Synchronized => "synchronized",
        UiStatus::WaitingForSonosConfirmation => "waiting",
        UiStatus::SubscriptionDegraded => "subscription_degraded",
        UiStatus::PollingFallback => "polling_fallback",
        UiStatus::SonosUnavailable => "sonos_unavailable",
        UiStatus::LocalAudioUnavailable => "local_audio_unavailable",
        UiStatus::UnsupportedLocalDevice => "unsupported_local_device",
        UiStatus::ConfigurationRequired => "configuration_required",
        UiStatus::Error => "error",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn renewal_happens_before_subscription_expiry() {
        let subscription = sonos_volume_bridge_sonos::Subscription {
            id: "uuid:test".to_owned(),
            timeout: Duration::from_secs(10),
        };
        let remaining = renewal_at(&subscription).saturating_duration_since(Instant::now());
        assert!(remaining < subscription.timeout);
        assert!(remaining >= Duration::from_secs(7));
    }
    #[test]
    fn reconnect_delay_is_bounded() {
        assert_eq!(
            (MAX_RECONNECT_DELAY * 2).min(MAX_RECONNECT_DELAY),
            MAX_RECONNECT_DELAY
        );
    }
}
