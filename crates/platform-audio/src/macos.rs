//! macOS Core Audio adapter using a focused FFI surface.
//!
//! Core Audio invokes property callbacks on implementation-controlled threads.
//! Each callback only reads the changed state and sends a bounded event; it never
//! waits for synchronization or network work.

use crate::{AudioDeviceSelection, PlatformAudioError, SystemAudioController, SystemAudioEvent};
use async_trait::async_trait;
use sonos_volume_bridge_domain::{ExpectedLocalWrite, LocalAudioState, LocalOrigin, MuteState, NormalizedVolume};
use std::{
    ffi::c_void,
    sync::{atomic::{AtomicU64, Ordering}, mpsc::{self, Receiver, SyncSender}, Arc, Mutex},
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tokio::sync::{broadcast, oneshot};

type AudioObjectID = u32;
type OSStatus = i32;
type UInt32 = u32;
type Boolean = u8;

const NO_ERR: OSStatus = 0;
const SYSTEM_OBJECT: AudioObjectID = 1;
const SCOPE_GLOBAL: UInt32 = 0x676c_6f62; // 'glob'
const SCOPE_OUTPUT: UInt32 = 0x6f75_7470; // 'outp'
const ELEMENT_MASTER: UInt32 = 0;
const DEFAULT_OUTPUT_DEVICE: UInt32 = 0x646f_7574; // 'dOut'
const VOLUME_SCALAR: UInt32 = 0x766f_6c6d; // 'volm'
const MUTE: UInt32 = 0x6d75_7465; // 'mute'
const STREAM_CONFIGURATION: UInt32 = 0x736c_6179; // 'slay'
const EXPECTED_WRITE_LIFETIME_MS: u64 = 500;

#[repr(C)]
#[derive(Clone, Copy)]
struct AudioObjectPropertyAddress { selector: UInt32, scope: UInt32, element: UInt32 }

type Listener = unsafe extern "C" fn(AudioObjectID, UInt32, *const AudioObjectPropertyAddress, *mut c_void) -> OSStatus;

#[link(name = "CoreAudio", kind = "framework")]
unsafe extern "C" {
    fn AudioObjectHasProperty(object: AudioObjectID, address: *const AudioObjectPropertyAddress) -> Boolean;
    fn AudioObjectIsPropertySettable(object: AudioObjectID, address: *const AudioObjectPropertyAddress, settable: *mut Boolean) -> OSStatus;
    fn AudioObjectGetPropertyData(object: AudioObjectID, address: *const AudioObjectPropertyAddress, qualifier_size: UInt32, qualifier: *const c_void, data_size: *mut UInt32, data: *mut c_void) -> OSStatus;
    fn AudioObjectGetPropertyDataSize(object: AudioObjectID, address: *const AudioObjectPropertyAddress, qualifier_size: UInt32, qualifier: *const c_void, data_size: *mut UInt32) -> OSStatus;
    fn AudioObjectSetPropertyData(object: AudioObjectID, address: *const AudioObjectPropertyAddress, qualifier_size: UInt32, qualifier: *const c_void, data_size: UInt32, data: *const c_void) -> OSStatus;
    fn AudioObjectAddPropertyListener(object: AudioObjectID, address: *const AudioObjectPropertyAddress, listener: Listener, client_data: *mut c_void) -> OSStatus;
    fn AudioObjectRemovePropertyListener(object: AudioObjectID, address: *const AudioObjectPropertyAddress, listener: Listener, client_data: *mut c_void) -> OSStatus;
}

#[derive(Clone)]
pub struct MacosAudioController { commands: SyncSender<Command>, events: broadcast::Sender<SystemAudioEvent> }

impl MacosAudioController {
    pub fn start(selection: AudioDeviceSelection, tolerance: u8) -> Result<Self, PlatformAudioError> {
        let (commands, receiver) = mpsc::sync_channel(32);
        let (events, _) = broadcast::channel(64);
        let worker_events = events.clone();
        let worker_commands = commands.clone();
        thread::Builder::new().name("sonos-volume-bridge-core-audio".to_owned()).spawn(move || run_worker(selection, tolerance, receiver, worker_commands, worker_events)).map_err(|error| PlatformAudioError::Platform(error.to_string()))?;
        Ok(Self { commands, events })
    }

    async fn request<T>(&self, command: impl FnOnce(oneshot::Sender<Result<T, PlatformAudioError>>) -> Command) -> Result<T, PlatformAudioError> {
        let (response, receiver) = oneshot::channel();
        self.commands.try_send(command(response)).map_err(|_| PlatformAudioError::DeviceUnavailable)?;
        receiver.await.map_err(|_| PlatformAudioError::DeviceUnavailable)?
    }
}

#[async_trait]
impl SystemAudioController for MacosAudioController {
    async fn current_state(&self) -> Result<LocalAudioState, PlatformAudioError> { self.request(Command::Current).await }
    async fn set_volume(&self, volume: NormalizedVolume, _: LocalOrigin) -> Result<(), PlatformAudioError> { self.request(|response| Command::SetVolume { volume, response }).await }
    async fn set_muted(&self, muted: bool, _: LocalOrigin) -> Result<(), PlatformAudioError> { self.request(|response| Command::SetMuted { muted, response }).await }
    fn subscribe(&self) -> broadcast::Receiver<SystemAudioEvent> { self.events.subscribe() }
}

enum Command {
    Current(oneshot::Sender<Result<LocalAudioState, PlatformAudioError>>),
    SetVolume { volume: NormalizedVolume, response: oneshot::Sender<Result<(), PlatformAudioError>> },
    SetMuted { muted: bool, response: oneshot::Sender<Result<(), PlatformAudioError>> },
    Reattach,
}

struct CallbackContext { events: broadcast::Sender<SystemAudioEvent>, commands: SyncSender<Command>, expected: Mutex<Option<ExpectedLocalWrite>>, generation: AtomicU64, tolerance: u8 }

fn run_worker(selection: AudioDeviceSelection, tolerance: u8, receiver: Receiver<Command>, commands: SyncSender<Command>, events: broadcast::Sender<SystemAudioEvent>) {
    let context = Arc::new(CallbackContext { events: events.clone(), commands, expected: Mutex::new(None), generation: AtomicU64::new(0), tolerance });
    let mut endpoint = match unsafe { Endpoint::attach(&selection, Arc::clone(&context)) } { Ok(endpoint) => endpoint, Err(error) => { let _ = events.send(SystemAudioEvent::DeviceUnavailable { device_id: None }); return; } };
    while let Ok(command) = receiver.recv_timeout(Duration::from_millis(250)) {
        match command {
            Command::Current(response) => { let _ = response.send(endpoint.state().map_err(map_error)); },
            Command::SetVolume { volume, response } => { let _ = response.send(endpoint.set_volume(volume).map_err(map_error)); },
            Command::SetMuted { muted, response } => { let _ = response.send(endpoint.set_muted(muted).map_err(map_error)); },
            Command::Reattach if selection == AudioDeviceSelection::FollowDefault => match unsafe { Endpoint::attach(&selection, Arc::clone(&context)) } { Ok(next) => { endpoint.detach(); endpoint = next; let _ = events.send(SystemAudioEvent::DefaultOutputChanged); }, Err(_) => { let _ = events.send(SystemAudioEvent::DeviceUnavailable { device_id: None }); } },
            Command::Reattach => {},
        }
    }
    endpoint.detach();
}

struct Endpoint { id: AudioObjectID, channels: Vec<UInt32>, has_mute: bool, context: Arc<CallbackContext> }

impl Endpoint {
    unsafe fn attach(selection: &AudioDeviceSelection, context: Arc<CallbackContext>) -> Result<Self, OSStatus> {
        let id = match selection { AudioDeviceSelection::FollowDefault => unsafe { default_output_device()? }, AudioDeviceSelection::Fixed { device_id } => device_id.parse::<AudioObjectID>().map_err(|_| -1)? };
        let master = address(VOLUME_SCALAR, SCOPE_OUTPUT, ELEMENT_MASTER);
        let has_master = unsafe { AudioObjectHasProperty(id, &master) != 0 } && unsafe { settable(id, &master)? };
        let channels = if has_master { vec![ELEMENT_MASTER] } else { unsafe { output_channels(id)? } };
        if channels.is_empty() { return Err(-1); }
        let mute_address = address(MUTE, SCOPE_OUTPUT, ELEMENT_MASTER);
        let has_mute = unsafe { AudioObjectHasProperty(id, &mute_address) != 0 } && unsafe { settable(id, &mute_address)? };
        let endpoint = Self { id, channels, has_mute, context };
        unsafe { endpoint.listen(); }
        Ok(endpoint)
    }
    unsafe fn state(&self) -> Result<LocalAudioState, OSStatus> {
        let mut total = 0.0_f32;
        for channel in &self.channels { total += unsafe { scalar(self.id, address(VOLUME_SCALAR, SCOPE_OUTPUT, *channel))? }; }
        let volume = normalize(total / self.channels.len() as f32);
        let muted = if self.has_mute { unsafe { boolean(self.id, address(MUTE, SCOPE_OUTPUT, ELEMENT_MASTER))? } } else { false };
        Ok(LocalAudioState { volume, muted: MuteState(muted) })
    }
    unsafe fn set_volume(&self, volume: NormalizedVolume) -> Result<(), OSStatus> {
        let state = LocalAudioState { volume, muted: self.state()?.muted };
        self.expect(state);
        let value = f32::from(volume.get()) / 100.0;
        for channel in &self.channels { unsafe { set_scalar(self.id, address(VOLUME_SCALAR, SCOPE_OUTPUT, *channel), value)?; } }
        Ok(())
    }
    unsafe fn set_muted(&self, muted: bool) -> Result<(), OSStatus> {
        if !self.has_mute { return Err(-1); }
        let state = LocalAudioState { volume: self.state()?.volume, muted: MuteState(muted) };
        self.expect(state);
        unsafe { set_boolean(self.id, address(MUTE, SCOPE_OUTPUT, ELEMENT_MASTER), muted) }
    }
    fn expect(&self, state: LocalAudioState) {
        let generation = self.context.generation.fetch_add(1, Ordering::Relaxed) + 1;
        if let Ok(mut expected) = self.context.expected.lock() { *expected = Some(ExpectedLocalWrite { state, expires_at_ms: now_ms() + EXPECTED_WRITE_LIFETIME_MS, generation, tolerance: self.context.tolerance }); }
    }
    unsafe fn listen(&self) {
        let data = Arc::as_ptr(&self.context).cast_mut().cast::<c_void>();
        for channel in &self.channels { let _ = unsafe { AudioObjectAddPropertyListener(self.id, &address(VOLUME_SCALAR, SCOPE_OUTPUT, *channel), property_changed, data) }; }
        if self.has_mute { let _ = unsafe { AudioObjectAddPropertyListener(self.id, &address(MUTE, SCOPE_OUTPUT, ELEMENT_MASTER), property_changed, data) }; }
        let _ = unsafe { AudioObjectAddPropertyListener(SYSTEM_OBJECT, &address(DEFAULT_OUTPUT_DEVICE, SCOPE_GLOBAL, ELEMENT_MASTER), default_output_changed, data) };
    }
    fn detach(&self) {
        // SAFETY: registrations use this endpoint's stable context pointer; Core Audio no longer invokes it after removal returns.
        unsafe {
            let data = Arc::as_ptr(&self.context).cast_mut().cast::<c_void>();
            for channel in &self.channels { let _ = AudioObjectRemovePropertyListener(self.id, &address(VOLUME_SCALAR, SCOPE_OUTPUT, *channel), property_changed, data); }
            if self.has_mute { let _ = AudioObjectRemovePropertyListener(self.id, &address(MUTE, SCOPE_OUTPUT, ELEMENT_MASTER), property_changed, data); }
            let _ = AudioObjectRemovePropertyListener(SYSTEM_OBJECT, &address(DEFAULT_OUTPUT_DEVICE, SCOPE_GLOBAL, ELEMENT_MASTER), default_output_changed, data);
        }
    }
}

unsafe extern "C" fn property_changed(object: AudioObjectID, _: UInt32, _: *const AudioObjectPropertyAddress, data: *mut c_void) -> OSStatus {
    let Some(context) = (unsafe { data.cast::<CallbackContext>().as_ref() }) else { return -1; };
    let state = unsafe { state_for_callback(object) };
    if let Ok(state) = state {
        let origin = context.expected.lock().ok().and_then(|mut expected| expected.take()).map_or(LocalOrigin::User, |expected| match expected.classify(state, now_ms()) { sonos_volume_bridge_domain::SuppressionDecision::Suppress => LocalOrigin::Application, sonos_volume_bridge_domain::SuppressionDecision::Forward => LocalOrigin::User });
        let _ = context.events.send(SystemAudioEvent::StateChanged { state, origin });
    }
    NO_ERR
}

unsafe extern "C" fn default_output_changed(_: AudioObjectID, _: UInt32, _: *const AudioObjectPropertyAddress, data: *mut c_void) -> OSStatus {
    if let Some(context) = unsafe { data.cast::<CallbackContext>().as_ref() } { let _ = context.commands.try_send(Command::Reattach); }
    NO_ERR
}

unsafe fn state_for_callback(id: AudioObjectID) -> Result<LocalAudioState, OSStatus> {
    let master = address(VOLUME_SCALAR, SCOPE_OUTPUT, ELEMENT_MASTER);
    if unsafe { AudioObjectHasProperty(id, &master) != 0 } { return Ok(LocalAudioState { volume: normalize(unsafe { scalar(id, master)? }), muted: MuteState(unsafe { boolean_if_present(id, address(MUTE, SCOPE_OUTPUT, ELEMENT_MASTER))? }) }); }
    let channels = unsafe { output_channels(id)? };
    if channels.is_empty() { return Err(-1); }
    let total = channels.iter().try_fold(0.0_f32, |total, channel| unsafe { scalar(id, address(VOLUME_SCALAR, SCOPE_OUTPUT, *channel)).map(|value| total + value) })?;
    Ok(LocalAudioState { volume: normalize(total / channels.len() as f32), muted: MuteState(unsafe { boolean_if_present(id, address(MUTE, SCOPE_OUTPUT, ELEMENT_MASTER))? }) })
}

fn address(selector: UInt32, scope: UInt32, element: UInt32) -> AudioObjectPropertyAddress { AudioObjectPropertyAddress { selector, scope, element } }
unsafe fn default_output_device() -> Result<AudioObjectID, OSStatus> { get(SYSTEM_OBJECT, address(DEFAULT_OUTPUT_DEVICE, SCOPE_GLOBAL, ELEMENT_MASTER)) }
unsafe fn scalar(id: AudioObjectID, address: AudioObjectPropertyAddress) -> Result<f32, OSStatus> { get(id, address) }
unsafe fn boolean(id: AudioObjectID, address: AudioObjectPropertyAddress) -> Result<bool, OSStatus> { Ok(get::<u32>(id, address)? != 0) }
unsafe fn boolean_if_present(id: AudioObjectID, address: AudioObjectPropertyAddress) -> Result<bool, OSStatus> { if unsafe { AudioObjectHasProperty(id, &address) != 0 } { unsafe { boolean(id, address) } } else { Ok(false) } }
unsafe fn set_scalar(id: AudioObjectID, address: AudioObjectPropertyAddress, value: f32) -> Result<(), OSStatus> { set(id, address, &value) }
unsafe fn set_boolean(id: AudioObjectID, address: AudioObjectPropertyAddress, value: bool) -> Result<(), OSStatus> { set(id, address, &(u32::from(value))) }
unsafe fn get<T: Copy>(id: AudioObjectID, address: AudioObjectPropertyAddress) -> Result<T, OSStatus> { let mut value = std::mem::MaybeUninit::<T>::uninit(); let mut size = std::mem::size_of::<T>() as u32; let status = unsafe { AudioObjectGetPropertyData(id, &address, 0, std::ptr::null(), &mut size, value.as_mut_ptr().cast()) }; if status == NO_ERR && size == std::mem::size_of::<T>() as u32 { Ok(unsafe { value.assume_init() }) } else { Err(status) } }
unsafe fn set<T>(id: AudioObjectID, address: AudioObjectPropertyAddress, value: &T) -> Result<(), OSStatus> { let status = unsafe { AudioObjectSetPropertyData(id, &address, 0, std::ptr::null(), std::mem::size_of::<T>() as u32, std::ptr::from_ref(value).cast()) }; if status == NO_ERR { Ok(()) } else { Err(status) } }
unsafe fn settable(id: AudioObjectID, address: &AudioObjectPropertyAddress) -> Result<bool, OSStatus> { let mut settable = 0; let status = unsafe { AudioObjectIsPropertySettable(id, address, &mut settable) }; if status == NO_ERR { Ok(settable != 0) } else { Err(status) } }
unsafe fn output_channels(id: AudioObjectID) -> Result<Vec<UInt32>, OSStatus> {
    let stream_address = address(STREAM_CONFIGURATION, SCOPE_OUTPUT, ELEMENT_MASTER);
    let mut size = 0_u32;
    let status = unsafe { AudioObjectGetPropertyDataSize(id, &stream_address, 0, std::ptr::null(), &mut size) };
    if status != NO_ERR || size < 8 { return Err(status); }
    // AudioBufferList starts with `mNumberBuffers`; each buffer contributes its channel count.
    let mut bytes = vec![0_u8; size as usize];
    let status = unsafe { AudioObjectGetPropertyData(id, &stream_address, 0, std::ptr::null(), &mut size, bytes.as_mut_ptr().cast()) };
    if status != NO_ERR { return Err(status); }
    let buffers = unsafe { *(bytes.as_ptr().cast::<u32>()) } as usize;
    let mut offset = 8_usize;
    let mut channels = Vec::new();
    for _ in 0..buffers { if offset + 16 > bytes.len() { return Err(-1); } let count = unsafe { *(bytes.as_ptr().add(offset).cast::<u32>()) }; for channel in 1..=count { let volume_address = address(VOLUME_SCALAR, SCOPE_OUTPUT, channel); if unsafe { AudioObjectHasProperty(id, &volume_address) != 0 && settable(id, &volume_address)? } { channels.push(channel); } } offset += 16; }
    Ok(channels)
}
fn map_error(status: OSStatus) -> PlatformAudioError { if status == -1 { PlatformAudioError::UnsupportedDevice } else { PlatformAudioError::Platform(format!("Core Audio OSStatus {status}")) } }
fn now_ms() -> u64 { SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_millis().try_into().unwrap_or(u64::MAX) }
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn normalize(value: f32) -> NormalizedVolume { NormalizedVolume::new((value.clamp(0.0, 1.0) * 100.0).round() as u8).unwrap_or(NormalizedVolume::MIN) }

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn expected_writes_suppress_matching_callbacks() {
        let expected = ExpectedLocalWrite { state: LocalAudioState { volume: NormalizedVolume::new(42).unwrap(), muted: MuteState(false) }, expires_at_ms: 10, generation: 1, tolerance: 1 };
        let observed = LocalAudioState { volume: NormalizedVolume::new(43).unwrap(), muted: MuteState(false) };
        assert_eq!(expected.classify(observed, 9), sonos_volume_bridge_domain::SuppressionDecision::Suppress);
    }
}
