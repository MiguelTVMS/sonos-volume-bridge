use crate::SonosError;
use serde::{Deserialize, Serialize};
use url::Url;

#[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(transparent)]
pub struct SonosId(String);

impl SonosId {
    pub fn new(value: impl Into<String>) -> Result<Self, SonosError> {
        let value = value.into();
        if value.trim().is_empty() { return Err(SonosError::MissingUdn); }
        Ok(Self(value))
    }
    pub fn as_str(&self) -> &str { &self.0 }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RenderingControlService { pub control_url: Url, pub event_url: Url }

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SonosDevice {
    pub id: SonosId,
    pub friendly_name: String,
    pub model_name: Option<String>,
    pub model_number: Option<String>,
    pub rendering_control: RenderingControlService,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DiscoveredDevice { pub location: Url, pub device: SonosDevice }
