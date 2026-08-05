use crate::{parse_device_description, DiscoveredDevice, SonosError};
use std::{collections::BTreeMap, time::Duration};
use tokio::{net::UdpSocket, time::{timeout, Instant}};
use url::Url;

const SSDP_ADDRESS: &str = "239.255.255.250:1900";
const SEARCH: &[u8] = b"M-SEARCH * HTTP/1.1\r\nHOST: 239.255.255.250:1900\r\nMAN: \"ssdp:discover\"\r\nMX: 1\r\nST: ssdp:all\r\n\r\n";

pub fn parse_ssdp_response(response: &[u8]) -> Result<Url, SonosError> {
    let response = std::str::from_utf8(response).map_err(|error| SonosError::Xml(error.to_string()))?;
    let mut lines = response.lines();
    if !lines.next().is_some_and(|line| line.starts_with("HTTP/1.1 200")) { return Err(SonosError::InvalidSsdpResponse); }
    let headers: BTreeMap<_, _> = lines.filter_map(|line| line.split_once(':')).map(|(key, value)| (key.trim().to_ascii_lowercase(), value.trim())).collect();
    let location = headers.get("location").ok_or(SonosError::MissingLocation)?;
    let url = Url::parse(location).map_err(|_| SonosError::MissingLocation)?;
    validate_local_url(&url)?;
    Ok(url)
}

pub async fn discover(timeout_after: Duration) -> Result<Vec<Url>, SonosError> {
    let socket = UdpSocket::bind("0.0.0.0:0").await?;
    socket.send_to(SEARCH, SSDP_ADDRESS).await?;
    let deadline = Instant::now() + timeout_after;
    let mut locations = Vec::new();
    let mut buffer = [0_u8; 4096];
    while let Ok(Ok((length, _))) = timeout(deadline.saturating_duration_since(Instant::now()), socket.recv_from(&mut buffer)).await {
        if let Ok(location) = parse_ssdp_response(&buffer[..length]) && !locations.contains(&location) { locations.push(location); }
        if Instant::now() >= deadline { break; }
    }
    Ok(locations)
}

pub async fn retrieve_device(client: &reqwest::Client, location: Url, max_bytes: usize) -> Result<DiscoveredDevice, SonosError> {
    validate_local_url(&location)?;
    let bytes = response_bytes(client.get(location.clone()).send().await?, max_bytes).await?;
    let device = parse_device_description(&bytes, &location)?;
    Ok(DiscoveredDevice { location, device })
}

pub(crate) async fn response_bytes(response: reqwest::Response, max_bytes: usize) -> Result<Vec<u8>, SonosError> {
    if response.content_length().is_some_and(|length| length > max_bytes as u64) { return Err(SonosError::ResponseTooLarge { limit: max_bytes }); }
    let mut response = response;
    let mut bytes = Vec::new();
    while let Some(chunk) = response.chunk().await? {
        if bytes.len().saturating_add(chunk.len()) > max_bytes { return Err(SonosError::ResponseTooLarge { limit: max_bytes }); }
        bytes.extend_from_slice(&chunk);
    }
    Ok(bytes)
}

pub(crate) fn validate_local_url(url: &Url) -> Result<(), SonosError> {
    if url.scheme() != "http" { return Err(SonosError::NonHttpUrl); }
    let host = url.host_str().ok_or(SonosError::MissingHost)?;
    let address = host.parse::<std::net::IpAddr>().map_err(|_| SonosError::NonLocalHost(host.to_owned()))?;
    let allowed = match address {
        std::net::IpAddr::V4(ip) => ip.is_private() || ip.is_link_local() || ip.is_loopback(),
        std::net::IpAddr::V6(ip) => ip.is_unique_local() || ip.is_unicast_link_local() || ip.is_loopback(),
    };
    if allowed { Ok(()) } else { Err(SonosError::NonLocalHost(host.to_owned())) }
}
