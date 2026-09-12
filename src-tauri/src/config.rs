use serde::{Deserialize, Serialize};
use sonos_volume_bridge_domain::{MappingPoint, SonosVolume, VolumeMapping};
use std::{
    fs, io,
    path::{Path, PathBuf},
};
use thiserror::Error;

const SCHEMA_VERSION: u32 = 1;

#[allow(clippy::struct_excessive_bools)] // Persistent settings are independent toggles by design.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppConfiguration {
    pub schema_version: u32,
    pub selected_sonos_id: Option<String>,
    pub last_known_sonos_address: Option<String>,
    pub follow_default_audio_device: bool,
    pub fixed_audio_device_id: Option<String>,
    pub synchronize_mute: bool,
    #[serde(default)]
    pub mute_speaker_at_zero_volume: bool,
    #[serde(default = "default_two_way_synchronization")]
    pub two_way_synchronization: bool,
    pub start_at_login: bool,
    pub fallback_polling: bool,
    #[serde(default)]
    pub log_level: LogLevel,
    pub maximum_sonos_volume: SonosVolume,
    pub mapping: VolumeMapping,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LogLevel {
    Error,
    Warn,
    #[default]
    Info,
    Debug,
    Trace,
}

impl Default for AppConfiguration {
    fn default() -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            selected_sonos_id: None,
            last_known_sonos_address: None,
            follow_default_audio_device: true,
            fixed_audio_device_id: None,
            synchronize_mute: true,
            mute_speaker_at_zero_volume: false,
            two_way_synchronization: true,
            start_at_login: false,
            fallback_polling: true,
            log_level: LogLevel::default(),
            maximum_sonos_volume: SonosVolume::new(55).unwrap_or(SonosVolume::MAX),
            mapping: VolumeMapping::Piecewise {
                points: default_points(),
            },
        }
    }
}

impl AppConfiguration {
    pub fn validate(&self) -> Result<(), ConfigError> {
        if self.schema_version != SCHEMA_VERSION {
            return Err(ConfigError::UnsupportedSchema(self.schema_version));
        }
        if self.maximum_sonos_volume.get() > 100 {
            return Err(ConfigError::Invalid("maximumSonosVolume"));
        }
        self.mapping
            .validate()
            .map_err(|_| ConfigError::Invalid("mapping"))
    }
}

const fn default_two_way_synchronization() -> bool {
    true
}

fn default_points() -> Vec<MappingPoint> {
    [(0, 0), (20, 5), (40, 12), (60, 23), (80, 40), (100, 55)]
        .into_iter()
        .filter_map(|(local, sonos)| {
            Some(MappingPoint {
                local: sonos_volume_bridge_domain::NormalizedVolume::new(local).ok()?,
                sonos: SonosVolume::new(sonos).ok()?,
            })
        })
        .collect()
}

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("configuration I/O failed: {0}")]
    Io(#[from] io::Error),
    #[error("configuration JSON failed: {0}")]
    Json(#[from] serde_json::Error),
    #[error("unsupported configuration schema version {0}")]
    UnsupportedSchema(u32),
    #[error("invalid configuration field {0}")]
    Invalid(&'static str),
}

pub struct ConfigStore {
    path: PathBuf,
}
impl ConfigStore {
    pub const fn new(path: PathBuf) -> Self {
        Self { path }
    }
    pub fn load_or_default(&self) -> Result<AppConfiguration, ConfigError> {
        if !self.path.exists() {
            return Ok(AppConfiguration::default());
        }
        let bytes = fs::read(&self.path)?;
        match serde_json::from_slice::<AppConfiguration>(&bytes).and_then(|configuration| {
            configuration
                .validate()
                .map_err(|error| serde_json::Error::io(io::Error::other(error.to_string())))?;
            Ok(configuration)
        }) {
            Ok(configuration) => Ok(configuration),
            Err(error) => {
                self.back_up_corrupt()?;
                let _ = error;
                Ok(AppConfiguration::default())
            }
        }
    }
    pub fn save(&self, configuration: &AppConfiguration) -> Result<(), ConfigError> {
        configuration.validate()?;
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)?;
        }
        let temporary = self.path.with_extension("json.tmp");
        fs::write(&temporary, serde_json::to_vec_pretty(configuration)?)?;
        fs::rename(temporary, &self.path)?;
        Ok(())
    }
    pub fn reset(&self) -> Result<AppConfiguration, ConfigError> {
        let configuration = AppConfiguration::default();
        self.save(&configuration)?;
        Ok(configuration)
    }
    fn back_up_corrupt(&self) -> Result<(), ConfigError> {
        let backup = self.path.with_extension("json.corrupt");
        fs::rename(&self.path, backup)?;
        Ok(())
    }
    pub fn path(&self) -> &Path {
        &self.path
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn default_configuration_is_valid() {
        AppConfiguration::default().validate().unwrap();
    }
    #[test]
    fn configuration_without_direction_setting_defaults_to_two_way() {
        let mut value = serde_json::to_value(AppConfiguration::default()).unwrap();
        value
            .as_object_mut()
            .unwrap()
            .remove("twoWaySynchronization");
        let configuration: AppConfiguration = serde_json::from_value(value).unwrap();
        assert!(configuration.two_way_synchronization);
    }
}
