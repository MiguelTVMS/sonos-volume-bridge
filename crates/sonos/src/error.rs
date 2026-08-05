use thiserror::Error;

#[derive(Debug, Error)]
pub enum SonosError {
    #[error("discovered URL must use HTTP")]
    NonHttpUrl,
    #[error("discovered URL has no host")]
    MissingHost,
    #[error("discovered URL host is not a local, private, or link-local address: {0}")]
    NonLocalHost(String),
    #[error("SSDP response has no valid LOCATION header")]
    MissingLocation,
    #[error("SSDP response is not an HTTP 200 response")]
    InvalidSsdpResponse,
    #[error("response body exceeded {limit} bytes")]
    ResponseTooLarge { limit: usize },
    #[error("XML payload was malformed: {0}")]
    Xml(String),
    #[error("device description does not provide a RenderingControl service")]
    MissingRenderingControl,
    #[error("device description has no UDN")]
    MissingUdn,
    #[error("invalid Sonos volume: {0}")]
    InvalidVolume(String),
    #[error("invalid Sonos mute value: {0}")]
    InvalidMute(String),
    #[error("HTTP request failed: {0}")]
    Http(#[from] reqwest::Error),
    #[error("network operation failed: {0}")]
    Network(#[from] std::io::Error),
    #[error("SOAP response did not contain {0}")]
    MissingSoapValue(&'static str),
}
