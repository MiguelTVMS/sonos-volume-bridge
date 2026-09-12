use crate::{
    DiscoveredDevice, SonosDevice, SonosError,
    discovery::{response_bytes, retrieve_device, validate_local_url},
    event::first_text,
};
use sonos_volume_bridge_domain::{MuteState, SonosVolume};
use std::time::Duration;
use url::Url;

const DEFAULT_TIMEOUT: Duration = Duration::from_secs(3);
const DEFAULT_MAX_RESPONSE_BYTES: usize = 512 * 1024;
const RENDERING_CONTROL: &str = "urn:schemas-upnp-org:service:RenderingControl:1";
const AV_TRANSPORT: &str = "urn:schemas-upnp-org:service:AVTransport:1";

#[derive(Clone, Debug)]
pub struct SonosClientBuilder {
    timeout: Duration,
    max_response_bytes: usize,
}

impl Default for SonosClientBuilder {
    fn default() -> Self {
        Self {
            timeout: DEFAULT_TIMEOUT,
            max_response_bytes: DEFAULT_MAX_RESPONSE_BYTES,
        }
    }
}

impl SonosClientBuilder {
    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }
    pub fn max_response_bytes(mut self, maximum: usize) -> Self {
        self.max_response_bytes = maximum;
        self
    }
    pub fn build(self) -> Result<SonosClient, SonosError> {
        let http = reqwest::Client::builder()
            .timeout(self.timeout)
            .no_proxy()
            .build()?;
        Ok(SonosClient {
            http,
            max_response_bytes: self.max_response_bytes,
        })
    }
}

#[derive(Clone, Debug)]
pub struct SonosClient {
    http: reqwest::Client,
    max_response_bytes: usize,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct SpeakerSettings {
    pub loudness: Option<bool>,
    pub night_sound: Option<bool>,
    pub speech_enhancement: Option<bool>,
}

impl SonosClient {
    pub fn builder() -> SonosClientBuilder {
        SonosClientBuilder::default()
    }
    pub async fn retrieve_device(&self, location: Url) -> Result<DiscoveredDevice, SonosError> {
        retrieve_device(&self.http, location, self.max_response_bytes).await
    }
    pub async fn get_volume(&self, device: &SonosDevice) -> Result<SonosVolume, SonosError> {
        let response = self
            .soap(
                device,
                "GetVolume",
                "<InstanceID>0</InstanceID><Channel>Master</Channel>",
            )
            .await?;
        let value = first_text(&response, "CurrentVolume")?
            .ok_or(SonosError::MissingSoapValue("CurrentVolume"))?;
        let value = value
            .parse::<u8>()
            .map_err(|error| SonosError::InvalidVolume(error.to_string()))?;
        SonosVolume::new(value).map_err(|error| SonosError::InvalidVolume(error.to_string()))
    }
    pub async fn set_volume(
        &self,
        device: &SonosDevice,
        volume: SonosVolume,
    ) -> Result<(), SonosError> {
        self.soap(device, "SetVolume", &format!("<InstanceID>0</InstanceID><Channel>Master</Channel><DesiredVolume>{}</DesiredVolume>", volume.get())).await?;
        Ok(())
    }
    pub async fn get_mute(&self, device: &SonosDevice) -> Result<MuteState, SonosError> {
        let response = self
            .soap(
                device,
                "GetMute",
                "<InstanceID>0</InstanceID><Channel>Master</Channel>",
            )
            .await?;
        match first_text(&response, "CurrentMute")?.as_deref() {
            Some("0") => Ok(MuteState(false)),
            Some("1") => Ok(MuteState(true)),
            Some(value) => Err(SonosError::InvalidMute(value.to_owned())),
            None => Err(SonosError::MissingSoapValue("CurrentMute")),
        }
    }
    pub async fn set_mute(&self, device: &SonosDevice, muted: MuteState) -> Result<(), SonosError> {
        let desired = i32::from(muted.0);
        self.soap(device, "SetMute", &format!("<InstanceID>0</InstanceID><Channel>Master</Channel><DesiredMute>{desired}</DesiredMute>")).await?;
        Ok(())
    }
    pub async fn get_status_light(&self, device: &SonosDevice) -> Result<bool, SonosError> {
        let response = self.device_properties(device, "GetLEDState", "").await?;
        match first_text(&response, "CurrentLEDState")?.as_deref() {
            Some("On") => Ok(true),
            Some("Off") => Ok(false),
            Some(value) => Err(SonosError::Protocol(format!("invalid LED state: {value}"))),
            None => Err(SonosError::MissingSoapValue("CurrentLEDState")),
        }
    }
    pub async fn set_status_light(
        &self,
        device: &SonosDevice,
        enabled: bool,
    ) -> Result<(), SonosError> {
        self.device_properties(
            device,
            "SetLEDState",
            if enabled {
                "<DesiredLEDState>On</DesiredLEDState>"
            } else {
                "<DesiredLEDState>Off</DesiredLEDState>"
            },
        )
        .await?;
        Ok(())
    }
    pub async fn get_loudness(&self, device: &SonosDevice) -> Result<bool, SonosError> {
        let response = self
            .soap(
                device,
                "GetLoudness",
                "<InstanceID>0</InstanceID><Channel>Master</Channel>",
            )
            .await?;
        parse_boolean(&response, "CurrentLoudness")
    }
    pub async fn set_loudness(
        &self,
        device: &SonosDevice,
        enabled: bool,
    ) -> Result<(), SonosError> {
        let desired = i32::from(enabled);
        self.soap(device, "SetLoudness", &format!("<InstanceID>0</InstanceID><Channel>Master</Channel><DesiredLoudness>{desired}</DesiredLoudness>")).await?;
        Ok(())
    }
    pub async fn get_eq(&self, device: &SonosDevice, setting: &str) -> Result<bool, SonosError> {
        let response = self
            .soap(
                device,
                "GetEQ",
                &format!("<InstanceID>0</InstanceID><EQType>{setting}</EQType>"),
            )
            .await?;
        parse_boolean(&response, "CurrentValue")
    }
    pub async fn set_eq(
        &self,
        device: &SonosDevice,
        setting: &str,
        enabled: bool,
    ) -> Result<(), SonosError> {
        let desired = i32::from(enabled);
        self.soap(device, "SetEQ", &format!("<InstanceID>0</InstanceID><EQType>{setting}</EQType><DesiredValue>{desired}</DesiredValue>")).await?;
        Ok(())
    }
    pub async fn get_volume_channel(
        &self,
        device: &SonosDevice,
        channel: &str,
    ) -> Result<u8, SonosError> {
        let r = self
            .soap(
                device,
                "GetVolume",
                &format!("<InstanceID>0</InstanceID><Channel>{channel}</Channel>"),
            )
            .await?;
        first_text(&r, "CurrentVolume")?
            .ok_or(SonosError::MissingSoapValue("CurrentVolume"))?
            .parse()
            .map_err(|e: std::num::ParseIntError| SonosError::InvalidVolume(e.to_string()))
    }
    pub async fn set_volume_channel(
        &self,
        device: &SonosDevice,
        channel: &str,
        value: u8,
    ) -> Result<(), SonosError> {
        self.soap(device,"SetVolume",&format!("<InstanceID>0</InstanceID><Channel>{channel}</Channel><DesiredVolume>{value}</DesiredVolume>")).await?;
        Ok(())
    }
    pub async fn get_tone(&self, device: &SonosDevice, name: &str) -> Result<i8, SonosError> {
        let r = self
            .soap(
                device,
                &format!("Get{name}"),
                "<InstanceID>0</InstanceID><Channel>Master</Channel>",
            )
            .await?;
        first_text(&r, &format!("Current{name}"))?
            .ok_or(SonosError::MissingSoapValue("tone"))?
            .parse()
            .map_err(|e: std::num::ParseIntError| SonosError::Protocol(e.to_string()))
    }
    pub async fn set_tone(
        &self,
        device: &SonosDevice,
        name: &str,
        value: i8,
    ) -> Result<(), SonosError> {
        self.soap(
            device,
            &format!("Set{name}"),
            &format!("<InstanceID>0</InstanceID><Desired{name}>{value}</Desired{name}>"),
        )
        .await?;
        Ok(())
    }
    pub async fn get_speaker_settings(&self, device: &SonosDevice) -> SpeakerSettings {
        SpeakerSettings {
            loudness: self.get_loudness(device).await.ok(),
            night_sound: self.get_eq(device, "NightMode").await.ok(),
            speech_enhancement: self
                .get_eq(device, "SpeechEnhanceEnabled")
                .await
                .ok()
                .or(self.get_eq(device, "DialogLevel").await.ok()),
        }
    }
    pub async fn get_audio_input_format(
        &self,
        device: &SonosDevice,
    ) -> Result<Option<String>, SonosError> {
        let response = self.device_properties(device, "GetZoneInfo", "").await?;
        let code = first_text(&response, "HTAudioIn")?
            .ok_or(SonosError::MissingSoapValue("HTAudioIn"))?
            .parse::<u32>()
            .map_err(|error| SonosError::Protocol(error.to_string()))?;
        Ok(Some(
            match code {
                0 => "No input connected",
                2 => "Stereo",
                7 | 33_554_488 => "Dolby 2.0",
                18 | 84_934_713 => "Dolby 5.1",
                21 => "No input",
                22 => "No audio",
                59 | 63 => "Dolby Atmos",
                33_554_434 => "PCM 2.0",
                33_554_454 => "PCM 2.0 no audio",
                33_554_490 => "Dolby Digital Plus 2.0",
                33_554_494 => "Dolby Multichannel PCM 2.0",
                84_934_658 => "Multichannel PCM 5.1",
                84_934_714 => "Dolby Digital Plus 5.1",
                _ => "Unknown input format",
            }
            .to_owned(),
        ))
    }
    pub async fn select_home_theater_input(&self, device: &SonosDevice) -> Result<(), SonosError> {
        let zone_info = self.device_properties(device, "GetZoneInfo", "").await?;
        let mac_address = first_text(&zone_info, "MACAddress")?
            .ok_or(SonosError::MissingSoapValue("MACAddress"))?;
        let mac_address = mac_address.replace(':', "");
        if mac_address.len() != 12 || !mac_address.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err(SonosError::Protocol(
                "invalid speaker MAC address".to_owned(),
            ));
        }
        let input_uri = format!(
            "x-sonos-htastream:RINCON_{}01400:spdif",
            mac_address.to_uppercase()
        );
        let endpoint = &device
            .av_transport
            .as_ref()
            .ok_or(SonosError::MissingAvTransport)?
            .control_url;
        self.soap_at(
            endpoint,
            AV_TRANSPORT,
            "SetAVTransportURI",
            &format!(
                "<InstanceID>0</InstanceID><CurrentURI>{input_uri}</CurrentURI><CurrentURIMetaData></CurrentURIMetaData>"
            ),
        )
        .await?;
        let media_info = self
            .soap_at(
                endpoint,
                AV_TRANSPORT,
                "GetMediaInfo",
                "<InstanceID>0</InstanceID>",
            )
            .await?;
        if first_text(&media_info, "CurrentURI")?.as_deref() != Some(&input_uri) {
            return Err(SonosError::Protocol(
                "speaker did not select the TV input".to_owned(),
            ));
        }
        Ok(())
    }
    async fn device_properties(
        &self,
        device: &SonosDevice,
        action: &str,
        arguments: &str,
    ) -> Result<Vec<u8>, SonosError> {
        let endpoint = device
            .rendering_control
            .control_url
            .join("/DeviceProperties/Control")
            .map_err(|error| SonosError::Protocol(error.to_string()))?;
        self.soap_at(
            &endpoint,
            "urn:schemas-upnp-org:service:DeviceProperties:1",
            action,
            arguments,
        )
        .await
    }
    async fn soap(
        &self,
        device: &SonosDevice,
        action: &str,
        arguments: &str,
    ) -> Result<Vec<u8>, SonosError> {
        self.soap_at(
            &device.rendering_control.control_url,
            RENDERING_CONTROL,
            action,
            arguments,
        )
        .await
    }
    async fn soap_at(
        &self,
        endpoint: &Url,
        service: &str,
        action: &str,
        arguments: &str,
    ) -> Result<Vec<u8>, SonosError> {
        validate_local_url(endpoint)?;
        let body = format!(
            "<?xml version=\"1.0\" encoding=\"utf-8\"?><s:Envelope xmlns:s=\"http://schemas.xmlsoap.org/soap/envelope/\"><s:Body><u:{action} xmlns:u=\"{service}\">{arguments}</u:{action}></s:Body></s:Envelope>"
        );
        let response = self
            .http
            .post(endpoint.clone())
            .header("Content-Type", "text/xml; charset=\"utf-8\"")
            .header("SOAPACTION", format!("\"{service}#{action}\""))
            .body(body)
            .send()
            .await?
            .error_for_status()?;
        response_bytes(response, self.max_response_bytes).await
    }
}

fn parse_boolean(response: &[u8], field: &'static str) -> Result<bool, SonosError> {
    match first_text(response, field)?.as_deref() {
        Some("0") => Ok(false),
        Some("1") => Ok(true),
        Some(value) => Err(SonosError::InvalidBoolean {
            field,
            value: value.to_owned(),
        }),
        None => Err(SonosError::MissingSoapValue(field)),
    }
}
