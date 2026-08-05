use crate::{discovery::{response_bytes, retrieve_device, validate_local_url}, event::first_text, DiscoveredDevice, SonosDevice, SonosError};
use sonos_volume_bridge_domain::{MuteState, SonosVolume};
use std::time::Duration;
use url::Url;

const DEFAULT_TIMEOUT: Duration = Duration::from_secs(3);
const DEFAULT_MAX_RESPONSE_BYTES: usize = 512 * 1024;
const RENDERING_CONTROL: &str = "urn:schemas-upnp-org:service:RenderingControl:1";

#[derive(Clone, Debug)]
pub struct SonosClientBuilder { timeout: Duration, max_response_bytes: usize }

impl Default for SonosClientBuilder {
    fn default() -> Self { Self { timeout: DEFAULT_TIMEOUT, max_response_bytes: DEFAULT_MAX_RESPONSE_BYTES } }
}

impl SonosClientBuilder {
    pub fn timeout(mut self, timeout: Duration) -> Self { self.timeout = timeout; self }
    pub fn max_response_bytes(mut self, maximum: usize) -> Self { self.max_response_bytes = maximum; self }
    pub fn build(self) -> Result<SonosClient, SonosError> {
        let http = reqwest::Client::builder().timeout(self.timeout).no_proxy().build()?;
        Ok(SonosClient { http, max_response_bytes: self.max_response_bytes })
    }
}

#[derive(Clone, Debug)]
pub struct SonosClient { http: reqwest::Client, max_response_bytes: usize }

impl SonosClient {
    pub fn builder() -> SonosClientBuilder { SonosClientBuilder::default() }
    pub async fn retrieve_device(&self, location: Url) -> Result<DiscoveredDevice, SonosError> { retrieve_device(&self.http, location, self.max_response_bytes).await }
    pub async fn get_volume(&self, device: &SonosDevice) -> Result<SonosVolume, SonosError> {
        let response = self.soap(device, "GetVolume", "<InstanceID>0</InstanceID><Channel>Master</Channel>").await?;
        let value = first_text(&response, "CurrentVolume")?.ok_or(SonosError::MissingSoapValue("CurrentVolume"))?;
        let value = value.parse::<u8>().map_err(|error| SonosError::InvalidVolume(error.to_string()))?;
        SonosVolume::new(value).map_err(|error| SonosError::InvalidVolume(error.to_string()))
    }
    pub async fn set_volume(&self, device: &SonosDevice, volume: SonosVolume) -> Result<(), SonosError> {
        self.soap(device, "SetVolume", &format!("<InstanceID>0</InstanceID><Channel>Master</Channel><DesiredVolume>{}</DesiredVolume>", volume.get())).await?;
        Ok(())
    }
    pub async fn get_mute(&self, device: &SonosDevice) -> Result<MuteState, SonosError> {
        let response = self.soap(device, "GetMute", "<InstanceID>0</InstanceID><Channel>Master</Channel>").await?;
        match first_text(&response, "CurrentMute")?.as_deref() { Some("0") => Ok(MuteState(false)), Some("1") => Ok(MuteState(true)), Some(value) => Err(SonosError::InvalidMute(value.to_owned())), None => Err(SonosError::MissingSoapValue("CurrentMute")) }
    }
    pub async fn set_mute(&self, device: &SonosDevice, muted: MuteState) -> Result<(), SonosError> {
        let desired = if muted.0 { 1 } else { 0 };
        self.soap(device, "SetMute", &format!("<InstanceID>0</InstanceID><Channel>Master</Channel><DesiredMute>{desired}</DesiredMute>")).await?;
        Ok(())
    }
    async fn soap(&self, device: &SonosDevice, action: &str, arguments: &str) -> Result<Vec<u8>, SonosError> {
        let endpoint = &device.rendering_control.control_url;
        validate_local_url(endpoint)?;
        let body = format!("<?xml version=\"1.0\" encoding=\"utf-8\"?><s:Envelope xmlns:s=\"http://schemas.xmlsoap.org/soap/envelope/\"><s:Body><u:{action} xmlns:u=\"{RENDERING_CONTROL}\">{arguments}</u:{action}></s:Body></s:Envelope>");
        let response = self.http.post(endpoint.clone()).header("Content-Type", "text/xml; charset=\"utf-8\"").header("SOAPACTION", format!("\"{RENDERING_CONTROL}#{action}\"")).body(body).send().await?.error_for_status()?;
        response_bytes(response, self.max_response_bytes).await
    }
}
