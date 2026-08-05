//! Pure domain model for SonosVolumeBridge.

mod audio;
mod mapping;
mod state;

pub use audio::{LocalOrigin, MuteState, NormalizedVolume, SonosVolume, VolumeError};
pub use mapping::{MappingError, MappingPoint, VolumeMapping};
pub use state::{
    ConfirmedSonosState, ExpectedLocalWrite, LocalAudioState, PendingIntent, SuppressionDecision,
    SyncState,
};

