//! Ubuntu audio adapter using PulseAudio's `pactl` interface.
//!
//! PipeWire on Ubuntu exposes the same interface through pipewire-pulse, so this
//! adapter works with either supported Ubuntu audio server. `pactl subscribe`
//! provides change notifications without depending on a desktop environment.

use crate::{
    AudioDeviceSelection, AudioOutputDevice, PlatformAudioError, SystemAudioController,
    SystemAudioEvent,
};
use async_trait::async_trait;
use serde_json::Value;
use sonos_volume_bridge_domain::{LocalAudioState, LocalOrigin, MuteState, NormalizedVolume};
use std::{
    io::{BufRead, BufReader},
    process::{Child, Command, Stdio},
    sync::{Arc, Mutex},
    thread,
    time::{Duration, Instant},
};
use tokio::sync::broadcast;

const PACTL: &str = "pactl";
const EXPECTED_WRITE_LIFETIME: Duration = Duration::from_millis(500);

/// Lists currently available PulseAudio/PipeWire output sinks.
pub fn list_output_devices() -> Result<Vec<AudioOutputDevice>, PlatformAudioError> {
    let mut outputs = sinks()?
        .iter()
        .filter(|sink| sink["state"].as_str() != Some("UNLINKED"))
        .filter_map(|sink| {
            let id = sink["name"].as_str()?.to_owned();
            let name = sink["description"].as_str().unwrap_or(&id).to_owned();
            Some(AudioOutputDevice {
                id,
                name,
                writable_volume: true,
            })
        })
        .collect::<Vec<_>>();
    outputs.sort_by(|left, right| left.name.cmp(&right.name).then(left.id.cmp(&right.id)));
    Ok(outputs)
}

#[derive(Clone)]
pub struct LinuxAudioController {
    selection: AudioDeviceSelection,
    events: broadcast::Sender<SystemAudioEvent>,
    expected: Arc<Mutex<Option<(LocalAudioState, Instant)>>>,
    subscriber: Arc<Mutex<Option<Child>>>,
}

impl Drop for LinuxAudioController {
    fn drop(&mut self) {
        if let Ok(mut child) = self.subscriber.lock()
            && let Some(child) = child.as_mut()
        {
            let _ = child.kill();
        }
    }
}

impl LinuxAudioController {
    pub fn start(selection: AudioDeviceSelection) -> Result<Self, PlatformAudioError> {
        let controller = Self::new(selection);
        controller.current_state_sync()?;
        controller.start_monitor()?;
        Ok(controller)
    }

    fn new(selection: AudioDeviceSelection) -> Self {
        let (events, _) = broadcast::channel(64);
        Self {
            selection,
            events,
            expected: Arc::new(Mutex::new(None)),
            subscriber: Arc::new(Mutex::new(None)),
        }
    }

    fn start_monitor(&self) -> Result<(), PlatformAudioError> {
        let mut child = Command::new(PACTL)
            .arg("subscribe")
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|error| map_command_error(&error))?;
        let stdout = child.stdout.take().ok_or_else(|| {
            PlatformAudioError::Platform("unable to read PulseAudio events".to_owned())
        })?;
        *self.subscriber.lock().map_err(|_| poisoned())? = Some(child);

        let controller = self.clone();
        thread::Builder::new()
            .name("sonos-volume-bridge-pulse-events".to_owned())
            .spawn(move || controller.monitor(BufReader::new(stdout)))
            .map_err(|error| PlatformAudioError::Platform(error.to_string()))?;
        Ok(())
    }

    fn monitor(&self, reader: BufReader<impl std::io::Read>) {
        for line in reader.lines().map_while(Result::ok) {
            if line.contains("on server")
                && matches!(self.selection, AudioDeviceSelection::FollowDefault)
            {
                let _ = self.events.send(SystemAudioEvent::DefaultOutputChanged);
            } else if line.contains("on sink") {
                self.emit_current_state();
            }
        }
        let _ = self
            .events
            .send(SystemAudioEvent::DeviceUnavailable { device_id: None });
        if let Ok(mut child) = self.subscriber.lock()
            && let Some(mut child) = child.take()
        {
            let _ = child.wait();
        }
    }

    fn emit_current_state(&self) {
        let Ok(state) = self.current_state_sync() else {
            let _ = self
                .events
                .send(SystemAudioEvent::DeviceUnavailable { device_id: None });
            return;
        };
        let origin = self.expected_origin(state);
        let _ = self
            .events
            .send(SystemAudioEvent::StateChanged { state, origin });
    }

    fn expected_origin(&self, state: LocalAudioState) -> LocalOrigin {
        let Ok(mut expected) = self.expected.lock() else {
            return LocalOrigin::Unknown;
        };
        let Some((expected_state, expires_at)) = *expected else {
            return LocalOrigin::Unknown;
        };
        if Instant::now() > expires_at {
            *expected = None;
            return LocalOrigin::Unknown;
        }
        if expected_state == state {
            *expected = None;
            LocalOrigin::Application
        } else {
            LocalOrigin::Unknown
        }
    }

    fn current_state_sync(&self) -> Result<LocalAudioState, PlatformAudioError> {
        state_for(&self.selection)
    }

    fn set_expected(&self, state: LocalAudioState) -> Result<(), PlatformAudioError> {
        *self.expected.lock().map_err(|_| poisoned())? =
            Some((state, Instant::now() + EXPECTED_WRITE_LIFETIME));
        Ok(())
    }
}

#[async_trait]
impl SystemAudioController for LinuxAudioController {
    async fn current_state(&self) -> Result<LocalAudioState, PlatformAudioError> {
        self.current_state_sync()
    }

    async fn set_volume(
        &self,
        volume: NormalizedVolume,
        _: LocalOrigin,
    ) -> Result<(), PlatformAudioError> {
        let mut state = self.current_state_sync()?;
        state.volume = volume;
        self.set_expected(state)?;
        let sink = sink_name(&self.selection)?;
        let level = format!("{}%", volume.get());
        run(&["set-sink-volume", &sink, &level])
    }

    async fn set_muted(&self, muted: bool, _: LocalOrigin) -> Result<(), PlatformAudioError> {
        let mut state = self.current_state_sync()?;
        state.muted = MuteState(muted);
        self.set_expected(state)?;
        let sink = sink_name(&self.selection)?;
        run(&["set-sink-mute", &sink, if muted { "1" } else { "0" }])
    }

    fn subscribe(&self) -> broadcast::Receiver<SystemAudioEvent> {
        self.events.subscribe()
    }
}

fn sinks() -> Result<Vec<Value>, PlatformAudioError> {
    serde_json::from_str(&output(&["-f", "json", "list", "sinks"])?).map_err(|error| {
        PlatformAudioError::Platform(format!("PulseAudio returned invalid sink data: {error}"))
    })
}

fn state_for(selection: &AudioDeviceSelection) -> Result<LocalAudioState, PlatformAudioError> {
    let name = sink_name(selection)?;
    let sink = sinks()?
        .into_iter()
        .find(|sink| sink["name"].as_str() == Some(&name))
        .ok_or(PlatformAudioError::DeviceUnavailable)?;
    let volume = sink["volume"]
        .as_object()
        .and_then(|channels| channels.values().next())
        .and_then(|channel| channel["value_percent"].as_str())
        .and_then(parse_percent)
        .ok_or(PlatformAudioError::UnsupportedDevice)?;
    Ok(LocalAudioState {
        volume,
        muted: MuteState(sink["mute"].as_bool().unwrap_or(false)),
    })
}

fn sink_name(selection: &AudioDeviceSelection) -> Result<String, PlatformAudioError> {
    match selection {
        AudioDeviceSelection::Fixed { device_id } => Ok(device_id.clone()),
        AudioDeviceSelection::FollowDefault => output(&["get-default-sink"]),
    }
}

fn parse_percent(value: &str) -> Option<NormalizedVolume> {
    let value = value.trim().trim_end_matches('%');
    let (whole, fractional) = value.split_once('.').map_or((value, ""), |parts| parts);
    let whole = whole.parse::<u16>().ok()?;
    if !fractional.chars().all(|digit| digit.is_ascii_digit()) {
        return None;
    }
    let round_up = fractional.chars().next().is_some_and(|digit| digit >= '5');
    let rounded = whole.saturating_add(u16::from(round_up)).min(100);
    NormalizedVolume::new(u8::try_from(rounded).ok()?).ok()
}

fn output(arguments: &[&str]) -> Result<String, PlatformAudioError> {
    let output = Command::new(PACTL)
        .args(arguments)
        .output()
        .map_err(|error| map_command_error(&error))?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_owned())
    } else {
        Err(PlatformAudioError::DeviceUnavailable)
    }
}

fn run(arguments: &[&str]) -> Result<(), PlatformAudioError> {
    output(arguments).map(|_| ())
}

fn map_command_error(error: &std::io::Error) -> PlatformAudioError {
    PlatformAudioError::Platform(format!("PulseAudio control is unavailable: {error}"))
}

fn poisoned() -> PlatformAudioError {
    PlatformAudioError::Platform("PulseAudio controller state is unavailable".to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_pulseaudio_percentage_values() {
        assert_eq!(parse_percent("42%"), NormalizedVolume::new(42).ok());
        assert_eq!(parse_percent("100.4%"), NormalizedVolume::new(100).ok());
        assert_eq!(parse_percent("not a percentage"), None);
    }

    #[test]
    fn expected_write_is_suppressed_once_before_expiry() {
        let controller = LinuxAudioController::new(AudioDeviceSelection::FollowDefault);
        let state = LocalAudioState {
            volume: NormalizedVolume::new(40).unwrap(),
            muted: MuteState(false),
        };
        controller.set_expected(state).unwrap();
        assert_eq!(controller.expected_origin(state), LocalOrigin::Application);
        assert_eq!(controller.expected_origin(state), LocalOrigin::Unknown);
    }
}
