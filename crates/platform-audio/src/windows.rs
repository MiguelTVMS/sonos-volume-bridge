//! Windows Core Audio adapter.
//!
//! All COM interfaces stay on the worker thread that initialized COM. The only
//! cross-thread values are commands and copied state. `OnNotify` and default
//! endpoint callbacks use non-blocking `try_send`/`broadcast::send` operations.

#![allow(
    clippy::borrow_as_ptr,
    clippy::inline_always,
    clippy::needless_pass_by_value,
    clippy::ptr_as_ptr,
    clippy::ref_as_ptr
)]

use crate::{
    AudioDeviceSelection, AudioOutputDevice, PlatformAudioError, SystemAudioController,
    SystemAudioEvent,
};
use async_trait::async_trait;
use sonos_volume_bridge_domain::{LocalAudioState, LocalOrigin, MuteState, NormalizedVolume};
use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, RecvTimeoutError, SyncSender},
    },
    thread::{self, JoinHandle},
    time::Duration,
};
use tokio::sync::{broadcast, oneshot};
use windows::{
    Win32::{
        Devices::FunctionDiscovery::PKEY_Device_FriendlyName,
        Foundation::PROPERTYKEY,
        Media::Audio::{
            AUDIO_VOLUME_NOTIFICATION_DATA, DEVICE_STATE, DEVICE_STATE_ACTIVE, EDataFlow, ERole,
            Endpoints::{
                IAudioEndpointVolume, IAudioEndpointVolumeCallback,
                IAudioEndpointVolumeCallback_Impl,
            },
            IMMDeviceEnumerator, IMMNotificationClient, IMMNotificationClient_Impl,
            MMDeviceEnumerator, eMultimedia, eRender,
        },
        System::Com::{
            CLSCTX_ALL, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx, CoTaskMemFree,
            CoUninitialize, STGM_READ,
            StructuredStorage::{PropVariantClear, PropVariantToString},
        },
    },
    core::{GUID, HRESULT, HSTRING, PCWSTR, implement},
};

/// Stable GUID attached to every bridge-initiated Core Audio write.
pub const APPLICATION_EVENT_CONTEXT: GUID = GUID::from_u128(0x8d0c6f84_7e52_41cb_9d4f_f5e39708d3a0);

/// Lists active Windows playback endpoints using the same stable IDs accepted by
/// [`AudioDeviceSelection::Fixed`].
pub fn list_output_devices() -> Result<Vec<AudioOutputDevice>, PlatformAudioError> {
    let worker = thread::Builder::new()
        .name("sonos-volume-bridge-output-enumeration".to_owned())
        .spawn(list_output_devices_on_worker)
        .map_err(|error| PlatformAudioError::Platform(error.to_string()))?;
    worker.join().map_err(|_| {
        PlatformAudioError::Platform("output enumeration worker panicked".to_owned())
    })?
}

fn list_output_devices_on_worker() -> Result<Vec<AudioOutputDevice>, PlatformAudioError> {
    // SAFETY: this thread owns its COM initialization and uninitializes exactly once.
    if !unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) }.is_ok() {
        return Err(PlatformAudioError::Platform(
            "unable to initialize Windows Core Audio".to_owned(),
        ));
    }
    let result = enumerate_active_render_devices()
        .map_err(|error| PlatformAudioError::Platform(error.to_string()));
    // SAFETY: `CoInitializeEx` succeeded on this thread above.
    unsafe { CoUninitialize() };
    result
}

fn enumerate_active_render_devices() -> windows::core::Result<Vec<AudioOutputDevice>> {
    let enumerator: IMMDeviceEnumerator =
        unsafe { CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)? };
    let devices = unsafe { enumerator.EnumAudioEndpoints(eRender, DEVICE_STATE_ACTIVE)? };
    let count = unsafe { devices.GetCount()? };
    let mut outputs = Vec::with_capacity(count as usize);
    for index in 0..count {
        let device = unsafe { devices.Item(index)? };
        let id = device_id(&device)?;
        let name = device_name(&device).unwrap_or_else(|| format!("Audio output {id}"));
        outputs.push(AudioOutputDevice {
            writable_volume: endpoint_volume_is_available(&device),
            id,
            name,
        });
    }
    outputs.sort_by(|left, right| left.name.cmp(&right.name).then(left.id.cmp(&right.id)));
    Ok(outputs)
}

fn device_id(device: &windows::Win32::Media::Audio::IMMDevice) -> windows::core::Result<String> {
    let id = unsafe { device.GetId()? };
    let result = unsafe { id.to_string() };
    // SAFETY: `IMMDevice::GetId` returns a CoTaskMem allocation owned by this caller.
    unsafe { CoTaskMemFree(Some(id.0.cast())) };
    result.map_err(|_| {
        windows::core::Error::new(
            HRESULT(-2_147_024_809),
            "output device identifier is not valid UTF-16",
        )
    })
}

fn device_name(device: &windows::Win32::Media::Audio::IMMDevice) -> Option<String> {
    let store = unsafe { device.OpenPropertyStore(STGM_READ) }.ok()?;
    let mut value = unsafe { store.GetValue(&PKEY_Device_FriendlyName) }.ok()?;
    let mut buffer = [0_u16; 256];
    let result = unsafe { PropVariantToString(&value, &mut buffer) }
        .ok()
        .map(|()| string_from_utf16z(&buffer))
        .filter(|name| !name.is_empty());
    // SAFETY: `IPropertyStore::GetValue` transfers the PROPVARIANT contents to this caller.
    let _ = unsafe { PropVariantClear(&mut value) };
    result
}

fn endpoint_volume_is_available(device: &windows::Win32::Media::Audio::IMMDevice) -> bool {
    unsafe { device.Activate::<IAudioEndpointVolume>(CLSCTX_ALL, None) }.is_ok()
}

fn string_from_utf16z(value: &[u16]) -> String {
    let length = value
        .iter()
        .position(|unit| *unit == 0)
        .unwrap_or(value.len());
    String::from_utf16_lossy(&value[..length])
}
#[derive(Clone)]
pub struct WindowsAudioController {
    worker: Arc<WorkerHandle>,
    events: broadcast::Sender<SystemAudioEvent>,
}

struct WorkerHandle {
    commands: SyncSender<Command>,
    shutdown: Arc<AtomicBool>,
    thread: Mutex<Option<JoinHandle<()>>>,
}

impl Drop for WorkerHandle {
    fn drop(&mut self) {
        self.shutdown.store(true, Ordering::Release);
        let thread = self
            .thread
            .get_mut()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .take();
        if let Some(thread) = thread {
            let _ = thread.join();
        }
    }
}

impl WindowsAudioController {
    pub fn start(selection: AudioDeviceSelection) -> Result<Self, PlatformAudioError> {
        let (commands, receiver) = mpsc::sync_channel(32);
        let (events, _) = broadcast::channel(64);
        let worker_events = events.clone();
        let worker_commands = commands.clone();
        let shutdown = Arc::new(AtomicBool::new(false));
        let worker_shutdown = Arc::clone(&shutdown);
        let thread = thread::Builder::new()
            .name("sonos-volume-bridge-core-audio".to_owned())
            .spawn(move || {
                run_worker(
                    selection,
                    receiver,
                    worker_events,
                    worker_commands,
                    &worker_shutdown,
                );
            })
            .map_err(|error| PlatformAudioError::Platform(error.to_string()))?;
        Ok(Self {
            worker: Arc::new(WorkerHandle {
                commands,
                shutdown,
                thread: Mutex::new(Some(thread)),
            }),
            events,
        })
    }

    async fn request<T>(
        &self,
        command: impl FnOnce(oneshot::Sender<Result<T, PlatformAudioError>>) -> Command,
    ) -> Result<T, PlatformAudioError> {
        let (response, receiver) = oneshot::channel();
        self.worker
            .commands
            .send(command(response))
            .map_err(|_| PlatformAudioError::DeviceUnavailable)?;
        receiver
            .await
            .map_err(|_| PlatformAudioError::DeviceUnavailable)?
    }
}

#[async_trait]
impl SystemAudioController for WindowsAudioController {
    async fn current_state(&self) -> Result<LocalAudioState, PlatformAudioError> {
        self.request(Command::Current).await
    }
    async fn set_volume(
        &self,
        volume: NormalizedVolume,
        _origin: LocalOrigin,
    ) -> Result<(), PlatformAudioError> {
        self.request(|response| Command::SetVolume { volume, response })
            .await
    }
    async fn set_muted(&self, muted: bool, _origin: LocalOrigin) -> Result<(), PlatformAudioError> {
        self.request(|response| Command::SetMuted { muted, response })
            .await
    }
    fn subscribe(&self) -> broadcast::Receiver<SystemAudioEvent> {
        self.events.subscribe()
    }
}

enum Command {
    Current(oneshot::Sender<Result<LocalAudioState, PlatformAudioError>>),
    SetVolume {
        volume: NormalizedVolume,
        response: oneshot::Sender<Result<(), PlatformAudioError>>,
    },
    SetMuted {
        muted: bool,
        response: oneshot::Sender<Result<(), PlatformAudioError>>,
    },
    Reattach,
}

fn run_worker(
    selection: AudioDeviceSelection,
    receiver: Receiver<Command>,
    events: broadcast::Sender<SystemAudioEvent>,
    commands: SyncSender<Command>,
    shutdown: &AtomicBool,
) {
    // SAFETY: this thread calls `CoUninitialize` exactly once after successful initialization,
    // and Core Audio interfaces never leave this thread.
    let initialized = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) }.is_ok();
    if !initialized {
        let _ = events.send(SystemAudioEvent::DeviceUnavailable { device_id: None });
        return;
    }
    let result = worker_loop(selection, receiver, events.clone(), commands, shutdown);
    if result.is_err() {
        let _ = events.send(SystemAudioEvent::DeviceUnavailable { device_id: None });
    }
    unsafe {
        CoUninitialize();
    }
}

fn worker_loop(
    selection: AudioDeviceSelection,
    receiver: Receiver<Command>,
    events: broadcast::Sender<SystemAudioEvent>,
    commands: SyncSender<Command>,
    shutdown: &AtomicBool,
) -> Result<(), windows::core::Error> {
    let enumerator: IMMDeviceEnumerator =
        unsafe { CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)? };
    let _default_registration =
        DefaultDeviceRegistration::register(&enumerator, &selection, commands)?;
    let mut endpoint = attach_endpoint(&enumerator, &selection, &events)?;
    loop {
        if shutdown.load(Ordering::Acquire) {
            break;
        }
        let command = match receiver.recv_timeout(Duration::from_millis(250)) {
            Ok(command) => command,
            Err(error) => {
                if worker_continues_after(error) {
                    continue;
                }
                break;
            }
        };
        match command {
            Command::Current(response) => {
                let _ = response.send(
                    endpoint
                        .current_state()
                        .map_err(PlatformAudioError::Platform),
                );
            }
            Command::SetVolume { volume, response } => {
                let _ = response.send(
                    endpoint
                        .set_volume(volume)
                        .map_err(PlatformAudioError::Platform),
                );
            }
            Command::SetMuted { muted, response } => {
                let _ = response.send(
                    endpoint
                        .set_muted(muted)
                        .map_err(PlatformAudioError::Platform),
                );
            }
            Command::Reattach if selection == AudioDeviceSelection::FollowDefault => {
                match attach_endpoint(&enumerator, &selection, &events) {
                    Ok(next) => {
                        endpoint = next;
                        let _ = events.send(SystemAudioEvent::DefaultOutputChanged);
                    }
                    Err(_) => {
                        let _ =
                            events.send(SystemAudioEvent::DeviceUnavailable { device_id: None });
                    }
                }
            }
            Command::Reattach => {}
        }
    }
    drop(endpoint);
    Ok(())
}

fn worker_continues_after(error: RecvTimeoutError) -> bool {
    matches!(error, RecvTimeoutError::Timeout)
}
struct EndpointRegistration {
    volume: IAudioEndpointVolume,
    callback: IAudioEndpointVolumeCallback,
}

impl Drop for EndpointRegistration {
    fn drop(&mut self) {
        let _ = unsafe { self.volume.UnregisterControlChangeNotify(&self.callback) };
    }
}

impl EndpointRegistration {
    fn current_state(&self) -> Result<LocalAudioState, String> {
        let volume = unsafe { self.volume.GetMasterVolumeLevelScalar() }
            .map_err(|error| error.to_string())?;
        let muted = unsafe { self.volume.GetMute() }
            .map_err(|error| error.to_string())?
            .as_bool();
        Ok(LocalAudioState {
            volume: normalized(volume),
            muted: MuteState(muted),
        })
    }
    fn set_volume(&self, volume: NormalizedVolume) -> Result<(), String> {
        unsafe {
            self.volume.SetMasterVolumeLevelScalar(
                f32::from(volume.get()) / 100.0,
                &APPLICATION_EVENT_CONTEXT,
            )
        }
        .map_err(|error| error.to_string())
    }
    fn set_muted(&self, muted: bool) -> Result<(), String> {
        unsafe { self.volume.SetMute(muted, &APPLICATION_EVENT_CONTEXT) }
            .map_err(|error| error.to_string())
    }
}

fn needs_default_device_notifications(selection: &AudioDeviceSelection) -> bool {
    *selection == AudioDeviceSelection::FollowDefault
}

struct DefaultDeviceRegistration {
    enumerator: IMMDeviceEnumerator,
    callback: IMMNotificationClient,
}

impl DefaultDeviceRegistration {
    fn register(
        enumerator: &IMMDeviceEnumerator,
        selection: &AudioDeviceSelection,
        commands: SyncSender<Command>,
    ) -> windows::core::Result<Option<Self>> {
        if !needs_default_device_notifications(selection) {
            return Ok(None);
        }
        let callback: IMMNotificationClient = DefaultDeviceCallback { commands }.into();
        unsafe {
            enumerator.RegisterEndpointNotificationCallback(&callback)?;
        }
        Ok(Some(Self {
            enumerator: enumerator.clone(),
            callback,
        }))
    }
}

impl Drop for DefaultDeviceRegistration {
    fn drop(&mut self) {
        let _ = unsafe {
            self.enumerator
                .UnregisterEndpointNotificationCallback(&self.callback)
        };
    }
}

fn attach_endpoint(
    enumerator: &IMMDeviceEnumerator,
    selection: &AudioDeviceSelection,
    events: &broadcast::Sender<SystemAudioEvent>,
) -> Result<EndpointRegistration, windows::core::Error> {
    let device = match selection {
        AudioDeviceSelection::FollowDefault => unsafe {
            enumerator.GetDefaultAudioEndpoint(eRender, eMultimedia)?
        },
        AudioDeviceSelection::Fixed { device_id } => unsafe {
            enumerator.GetDevice(&HSTRING::from(device_id))?
        },
    };
    let volume = unsafe { device.Activate::<IAudioEndpointVolume>(CLSCTX_ALL, None)? };
    let callback: IAudioEndpointVolumeCallback = EndpointVolumeCallback {
        events: events.clone(),
    }
    .into();
    unsafe {
        volume.RegisterControlChangeNotify(&callback)?;
    }
    Ok(EndpointRegistration { volume, callback })
}

#[implement(IAudioEndpointVolumeCallback)]
struct EndpointVolumeCallback {
    events: broadcast::Sender<SystemAudioEvent>,
}

impl IAudioEndpointVolumeCallback_Impl for EndpointVolumeCallback_Impl {
    fn OnNotify(
        &self,
        notification: *mut AUDIO_VOLUME_NOTIFICATION_DATA,
    ) -> windows_core::Result<()> {
        // SAFETY: Core Audio provides a valid notification pointer for the duration of this call.
        let Some(notification) = (unsafe { notification.as_ref() }) else {
            return Ok(());
        };
        let origin = if notification.guidEventContext == APPLICATION_EVENT_CONTEXT {
            LocalOrigin::Application
        } else {
            LocalOrigin::User
        };
        let _ = self.events.send(SystemAudioEvent::StateChanged {
            state: LocalAudioState {
                volume: normalized(notification.fMasterVolume),
                muted: MuteState(notification.bMuted.as_bool()),
            },
            origin,
        });
        Ok(())
    }
}

#[implement(IMMNotificationClient)]
struct DefaultDeviceCallback {
    commands: SyncSender<Command>,
}

impl IMMNotificationClient_Impl for DefaultDeviceCallback_Impl {
    fn OnDeviceStateChanged(&self, _: &PCWSTR, _: DEVICE_STATE) -> windows_core::Result<()> {
        Ok(())
    }
    fn OnDeviceAdded(&self, _: &PCWSTR) -> windows_core::Result<()> {
        Ok(())
    }
    fn OnDeviceRemoved(&self, _: &PCWSTR) -> windows_core::Result<()> {
        Ok(())
    }
    fn OnDefaultDeviceChanged(
        &self,
        flow: EDataFlow,
        role: ERole,
        _: &PCWSTR,
    ) -> windows_core::Result<()> {
        if flow == eRender && role == eMultimedia {
            let _ = self.commands.try_send(Command::Reattach);
        }
        Ok(())
    }
    fn OnPropertyValueChanged(&self, _: &PCWSTR, _: &PROPERTYKEY) -> windows_core::Result<()> {
        Ok(())
    }
}

#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn normalized(value: f32) -> NormalizedVolume {
    NormalizedVolume::new((value.clamp(0.0, 1.0) * 100.0).round() as u8)
        .unwrap_or(NormalizedVolume::MIN)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn strings_stop_at_the_first_utf16_null() {
        assert_eq!(
            string_from_utf16z(&[79, 102, 102, 105, 99, 101, 0, 120]),
            "Office"
        );
    }
    #[test]
    fn application_event_context_is_stable_and_non_null() {
        assert_ne!(APPLICATION_EVENT_CONTEXT, GUID::default());
    }
    #[test]
    fn normalizes_core_audio_scalar_endpoints() {
        assert_eq!(normalized(0.0), NormalizedVolume::MIN);
        assert_eq!(normalized(1.0), NormalizedVolume::MAX);
    }

    #[test]
    fn fixed_endpoints_do_not_register_for_default_device_changes() {
        assert!(needs_default_device_notifications(
            &AudioDeviceSelection::FollowDefault
        ));
        assert!(!needs_default_device_notifications(
            &AudioDeviceSelection::Fixed {
                device_id: "g8".to_owned(),
            }
        ));
    }

    #[test]
    fn final_worker_owner_signals_shutdown_and_joins() {
        let (commands, _receiver) = mpsc::sync_channel(1);
        let shutdown = Arc::new(AtomicBool::new(false));
        let worker_shutdown = Arc::clone(&shutdown);
        let stopped = Arc::new(AtomicBool::new(false));
        let worker_stopped = Arc::clone(&stopped);
        let thread = thread::spawn(move || {
            while !worker_shutdown.load(Ordering::Acquire) {
                thread::yield_now();
            }
            worker_stopped.store(true, Ordering::Release);
        });
        let first = Arc::new(WorkerHandle {
            commands,
            shutdown,
            thread: Mutex::new(Some(thread)),
        });
        let final_owner = Arc::clone(&first);

        drop(first);
        assert!(!stopped.load(Ordering::Acquire));

        drop(final_owner);
        assert!(stopped.load(Ordering::Acquire));
    }
}
#[test]
fn worker_stays_attached_after_command_timeout() {
    assert!(worker_continues_after(RecvTimeoutError::Timeout));
    assert!(!worker_continues_after(RecvTimeoutError::Disconnected));
}
