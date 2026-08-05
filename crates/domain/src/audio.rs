use serde::{Deserialize, Serialize};
use thiserror::Error;

/// A volume represented as an integer percentage in the inclusive range 0..=100.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct NormalizedVolume(u8);

/// Sonos RenderingControl volume, whose permitted range is 0..=100.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct SonosVolume(u8);

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(transparent)]
pub struct MuteState(pub bool);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LocalOrigin {
    User,
    Application,
    Unknown,
}

#[derive(Debug, Error, Eq, PartialEq)]
pub enum VolumeError {
    #[error("volume {value} is outside the inclusive range 0..=100")]
    OutOfRange { value: u8 },
}

impl NormalizedVolume {
    pub const MIN: Self = Self(0);
    pub const MAX: Self = Self(100);

    pub const fn new(value: u8) -> Result<Self, VolumeError> {
        if value <= 100 { Ok(Self(value)) } else { Err(VolumeError::OutOfRange { value }) }
    }
    pub const fn get(self) -> u8 { self.0 }
}

impl SonosVolume {
    pub const MIN: Self = Self(0);
    pub const MAX: Self = Self(100);

    pub const fn new(value: u8) -> Result<Self, VolumeError> {
        if value <= 100 { Ok(Self(value)) } else { Err(VolumeError::OutOfRange { value }) }
    }
    pub const fn get(self) -> u8 { self.0 }
    pub const fn capped_at(self, maximum: Self) -> Self { Self(if self.0 > maximum.0 { maximum.0 } else { self.0 }) }
}

