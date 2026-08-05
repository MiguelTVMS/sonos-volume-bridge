//! Windows Core Audio adapter.
//!
//! All COM interfaces stay on the worker thread that initialized COM. The only
//! cross-thread values are commands and copied state. `OnNotify` and default
//! endpoint callbacks use non-blocking `try_send`/`broadcast::send` operations.

use crate::{AudioDeviceSelection, PlatformAudioError, SystemAudioController, SystemAudioEvent};
use async_trait::async_trait;
use sonos_volume_bridge_domain::{LocalAudioState, LocalOrigin, MuteState, NormalizedVolume};
use std::{sync::mpsc::{self, Receiver, SyncSender}, thread, time::Duration};
use tokio::sync::{broadcast, oneshot};
use windows::{
    core::{implement, GUID, HSTRING, PCWSTR},
    Win32::{
        Foundation::PROPERTYKEY,
        Media::Audio::{
            Endpoints::{IAudioEndpointVolume, IAudioEndpointVolumeCallback, IAudioEndpointVolumeCallback_Impl},
            eMultimedia, eRender, EDataFlow, ERole, IMMDeviceEnumerator, IMMNotificationClient, IMMNotificationClient_Impl, MMDeviceEnumerator, AUDIO_VOLUME_NOTIFICATION_DATA,
        },
        System::Com::{CoCreateInstance, CoInitializeEx, CoUninitialize, CLSCTX_ALL, COINIT_MULTITHREADED},
    },
};

/// Stable GUID attached to every bridge-initiated Core Audio write.
pub const APPLICATION_EVENT_CONTEXT: GUID = GUID::from_u128(0x8d0c6f84_7e52_41cb_9d4f_f5e39708d3a0);

#[derive(Clone)]
pub struct WindowsAudioController { commands: SyncSender<Command>, events: broadcast::Sender<SystemAudioEvent> }

impl WindowsAudioController {
    pub fn start(selection: AudioDeviceSelection) -> Result<Self, PlatformAudioError> {
        let (commands, receiver) = mpsc::sync_channel(32);
        let (events, _) = broadcast::channel(64);
        let worker_events = events.clone();
        let worker_commands = commands.clone();
        thread::Builder::new().name("sonos-volume-bridge-core-audio".to_owned()).spawn(move || run_worker(selection, receiver, worker_events, worker_commands)).map_err(|error| PlatformAudioError::Platform(error.to_string()))?;
        Ok(Self { commands, events })
    }

    async fn request<T>(&self, command: impl FnOnce(oneshot::Sender<Result<T, PlatformAudioError>>) -> Command) -> Result<T, PlatformAudioError> {
        let (response, receiver) = oneshot::channel();
        self.commands.send(command(response)).map_err(|_| PlatformAudioError::DeviceUnavailable)?;
        receiver.await.map_err(|_| PlatformAudioError::DeviceUnavailable)?
    }
}

#[async_trait]
impl SystemAudioController for WindowsAudioController {
    async fn current_state(&self) -> Result<LocalAudioState, PlatformAudioError> { self.request(Command::Current).await }
    async fn set_volume(&self, volume: NormalizedVolume, _origin: LocalOrigin) -> Result<(), PlatformAudioError> { self.request(|response| Command::SetVolume { volume, response }).await }
    async fn set_muted(&self, muted: bool, _origin: LocalOrigin) -> Result<(), PlatformAudioError> { self.request(|response| Command::SetMuted { muted, response }).await }
    fn subscribe(&self) -> broadcast::Receiver<SystemAudioEvent> { self.events.subscribe() }
}

enum Command {
    Current(oneshot::Sender<Result<LocalAudioState, PlatformAudioError>>),
    SetVolume { volume: NormalizedVolume, response: oneshot::Sender<Result<(), PlatformAudioError>> },
    SetMuted { muted: bool, response: oneshot::Sender<Result<(), PlatformAudioError>> },
    Reattach,
}

fn run_worker(selection: AudioDeviceSelection, receiver: Receiver<Command>, events: broadcast::Sender<SystemAudioEvent>, commands: SyncSender<Command>) {
    // SAFETY: this thread calls `CoUninitialize` exactly once after successful initialization,
    // and Core Audio interfaces never leave this thread.
    let initialized = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) }.is_ok();
    if !initialized { let _ = events.send(SystemAudioEvent::DeviceUnavailable { device_id: None }); return; }
    let result = unsafe { worker_loop(selection, receiver, events.clone(), commands) };
    if result.is_err() { let _ = events.send(SystemAudioEvent::DeviceUnavailable { device_id: None }); }
    unsafe { CoUninitialize(); }
}

unsafe fn worker_loop(selection: AudioDeviceSelection, receiver: Receiver<Command>, events: broadcast::Sender<SystemAudioEvent>, commands: SyncSender<Command>) -> Result<(), windows::core::Error> {
    let enumerator: IMMDeviceEnumerator = unsafe { CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)? };
    let default_callback: IMMNotificationClient = DefaultDeviceCallback { commands }.into();
    unsafe { enumerator.RegisterEndpointNotificationCallback(&default_callback)?; }
    let mut endpoint = attach_endpoint(&enumerator, &selection, &events)?;
    while let Ok(command) = receiver.recv_timeout(Duration::from_millis(250)) {
        match command {
            Command::Current(response) => { let _ = response.send(endpoint.current_state().map_err(PlatformAudioError::Platform)); },
            Command::SetVolume { volume, response } => { let _ = response.send(endpoint.set_volume(volume).map_err(PlatformAudioError::Platform)); },
            Command::SetMuted { muted, response } => { let _ = response.send(endpoint.set_muted(muted).map_err(PlatformAudioError::Platform)); },
            Command::Reattach if selection == AudioDeviceSelection::FollowDefault => { endpoint.detach(); endpoint = attach_endpoint(&enumerator, &selection, &events)?; let _ = events.send(SystemAudioEvent::DefaultOutputChanged); },
            Command::Reattach => {},
        }
    }
    endpoint.detach();
    unsafe { enumerator.UnregisterEndpointNotificationCallback(&default_callback)?; }
    Ok(())
}

struct EndpointRegistration { volume: IAudioEndpointVolume, callback: IAudioEndpointVolumeCallback }

impl EndpointRegistration {
    unsafe fn current_state(&self) -> Result<LocalAudioState, String> {
        let volume = unsafe { self.volume.GetMasterVolumeLevelScalar() }.map_err(|error| error.to_string())?;
        let muted = unsafe { self.volume.GetMute() }.map_err(|error| error.to_string())?.as_bool();
        Ok(LocalAudioState { volume: normalized(volume), muted: MuteState(muted) })
    }
    unsafe fn set_volume(&self, volume: NormalizedVolume) -> Result<(), String> { unsafe { self.volume.SetMasterVolumeLevelScalar(f32::from(volume.get()) / 100.0, &APPLICATION_EVENT_CONTEXT) }.map_err(|error| error.to_string()) }
    unsafe fn set_muted(&self, muted: bool) -> Result<(), String> { unsafe { self.volume.SetMute(muted, &APPLICATION_EVENT_CONTEXT) }.map_err(|error| error.to_string()) }
    unsafe fn detach(&self) { let _ = unsafe { self.volume.UnregisterControlChangeNotify(&self.callback) }; }
}

unsafe fn attach_endpoint(enumerator: &IMMDeviceEnumerator, selection: &AudioDeviceSelection, events: &broadcast::Sender<SystemAudioEvent>) -> Result<EndpointRegistration, windows::core::Error> {
    let device = match selection {
        AudioDeviceSelection::FollowDefault => unsafe { enumerator.GetDefaultAudioEndpoint(eRender, eMultimedia)? },
        AudioDeviceSelection::Fixed { device_id } => unsafe { enumerator.GetDevice(&HSTRING::from(device_id))? },
    };
    let volume: IAudioEndpointVolume = unsafe { device.Activate(CLSCTX_ALL, None)? };
    let callback: IAudioEndpointVolumeCallback = EndpointVolumeCallback { events: events.clone() }.into();
    unsafe { volume.RegisterControlChangeNotify(&callback)?; }
    Ok(EndpointRegistration { volume, callback })
}

#[implement(IAudioEndpointVolumeCallback)]
struct EndpointVolumeCallback { events: broadcast::Sender<SystemAudioEvent> }

impl IAudioEndpointVolumeCallback_Impl for EndpointVolumeCallback_Impl {
    fn OnNotify(&self, notification: *mut AUDIO_VOLUME_NOTIFICATION_DATA) -> windows_core::Result<()> {
        // SAFETY: Core Audio provides a valid notification pointer for the duration of this call.
        let Some(notification) = (unsafe { notification.as_ref() }) else { return Ok(()); };
        let origin = if notification.guidEventContext == APPLICATION_EVENT_CONTEXT { LocalOrigin::Application } else { LocalOrigin::User };
        let _ = self.events.send(SystemAudioEvent::StateChanged { state: LocalAudioState { volume: normalized(notification.fMasterVolume), muted: MuteState(notification.bMuted.as_bool()) }, origin });
        Ok(())
    }
}

#[implement(IMMNotificationClient)]
struct DefaultDeviceCallback { commands: SyncSender<Command> }

impl IMMNotificationClient_Impl for DefaultDeviceCallback_Impl {
    fn OnDeviceStateChanged(&self, _: &PCWSTR, _: u32) -> windows_core::Result<()> { Ok(()) }
    fn OnDeviceAdded(&self, _: &PCWSTR) -> windows_core::Result<()> { Ok(()) }
    fn OnDeviceRemoved(&self, _: &PCWSTR) -> windows_core::Result<()> { Ok(()) }
    fn OnDefaultDeviceChanged(&self, flow: EDataFlow, role: ERole, _: &PCWSTR) -> windows_core::Result<()> { if flow == eRender && role == eMultimedia { let _ = self.commands.try_send(Command::Reattach); } Ok(()) }
    fn OnPropertyValueChanged(&self, _: &PCWSTR, _: &PROPERTYKEY) -> windows_core::Result<()> { Ok(()) }
}

#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn normalized(value: f32) -> NormalizedVolume { NormalizedVolume::new((value.clamp(0.0, 1.0) * 100.0).round() as u8).unwrap_or(NormalizedVolume::MIN) }

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn application_event_context_is_stable_and_non_null() { assert_ne!(APPLICATION_EVENT_CONTEXT, GUID::default()); }
    #[test]
    fn normalizes_core_audio_scalar_endpoints() { assert_eq!(normalized(0.0), NormalizedVolume::MIN); assert_eq!(normalized(1.0), NormalizedVolume::MAX); }
}
