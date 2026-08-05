//! Native system-output audio abstraction.
//!
//! Implementations must emit callbacks without blocking their platform callback
//! thread. The Windows implementation owns Core Audio COM interfaces on a
//! dedicated worker thread and forwards events through a bounded broadcast bus.

#![allow(unsafe_code)]

use async_trait::async_trait;
use sonos_volume_bridge_domain::{LocalAudioState, LocalOrigin, NormalizedVolume};
use thiserror::Error;
use tokio::sync::broadcast;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AudioDeviceSelection {
    FollowDefault,
    Fixed { device_id: String },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SystemAudioEvent {
    StateChanged {
        state: LocalAudioState,
        origin: LocalOrigin,
    },
    DefaultOutputChanged,
    DeviceUnavailable {
        device_id: Option<String>,
    },
}

#[derive(Debug, Error)]
pub enum PlatformAudioError {
    #[error("the selected output device is unavailable")]
    DeviceUnavailable,
    #[error("the selected output device does not expose writable software volume")]
    UnsupportedDevice,
    #[error("platform audio operation failed: {0}")]
    Platform(String),
}

#[async_trait]
pub trait SystemAudioController: Send + Sync {
    async fn current_state(&self) -> Result<LocalAudioState, PlatformAudioError>;
    async fn set_volume(
        &self,
        volume: NormalizedVolume,
        origin: LocalOrigin,
    ) -> Result<(), PlatformAudioError>;
    async fn set_muted(&self, muted: bool, origin: LocalOrigin) -> Result<(), PlatformAudioError>;
    fn subscribe(&self) -> broadcast::Receiver<SystemAudioEvent>;
}

#[cfg(windows)]
pub mod windows;

#[cfg(not(windows))]
pub mod windows {
    //! The Windows adapter is only compiled on Windows targets.
}

#[cfg(target_os = "macos")]
pub mod macos;

#[cfg(not(target_os = "macos"))]
pub mod macos {
    //! The macOS adapter is only compiled on macOS targets.
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn selection_models_follow_default_and_fixed_devices() {
        assert_eq!(
            AudioDeviceSelection::FollowDefault,
            AudioDeviceSelection::FollowDefault
        );
        assert_eq!(
            AudioDeviceSelection::Fixed {
                device_id: "id".to_owned()
            },
            AudioDeviceSelection::Fixed {
                device_id: "id".to_owned()
            }
        );
    }
}
