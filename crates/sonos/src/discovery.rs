use crate::{DiscoveredDevice, SonosError, parse_device_description};
use if_addrs::get_if_addrs;
use std::{
    collections::BTreeMap,
    net::{IpAddr, Ipv4Addr, SocketAddr},
    time::Duration,
};
use tokio::{
    net::UdpSocket,
    task::JoinSet,
    time::{Instant, timeout},
};
use url::Url;

const SSDP_ADDRESS: &str = "239.255.255.250:1900";
const SEARCH: &[u8] = b"M-SEARCH * HTTP/1.1\r\nHOST: 239.255.255.250:1900\r\nMAN: \"ssdp:discover\"\r\nMX: 1\r\nST: urn:schemas-upnp-org:device:ZonePlayer:1\r\n\r\n";

fn discovery_bind_addresses() -> Vec<SocketAddr> {
    let addresses = get_if_addrs()
        .map(|interfaces| {
            bind_addresses(interfaces.into_iter().map(|interface| interface.addr.ip()))
        })
        .unwrap_or_default();
    if addresses.is_empty() {
        vec![SocketAddr::from((Ipv4Addr::UNSPECIFIED, 0))]
    } else {
        addresses
    }
}

fn bind_addresses(ips: impl IntoIterator<Item = IpAddr>) -> Vec<SocketAddr> {
    let mut addresses: Vec<_> = ips
        .into_iter()
        .filter_map(|ip| match ip {
            IpAddr::V4(ip) if !ip.is_loopback() && !ip.is_unspecified() => {
                Some(SocketAddr::from((ip, 0)))
            }
            IpAddr::V6(ip) if !ip.is_loopback() && !ip.is_unspecified() => {
                Some(SocketAddr::from((ip, 0)))
            }
            _ => None,
        })
        .collect();
    addresses.sort_unstable();
    addresses.dedup();
    addresses
}
pub fn parse_ssdp_response(response: &[u8]) -> Result<Url, SonosError> {
    let response =
        std::str::from_utf8(response).map_err(|error| SonosError::Xml(error.to_string()))?;
    let mut lines = response.lines();
    if !lines
        .next()
        .is_some_and(|line| line.starts_with("HTTP/1.1 200"))
    {
        return Err(SonosError::InvalidSsdpResponse);
    }
    let headers: BTreeMap<_, _> = lines
        .filter_map(|line| line.split_once(':'))
        .map(|(key, value)| (key.trim().to_ascii_lowercase(), value.trim()))
        .collect();
    let location = headers.get("location").ok_or(SonosError::MissingLocation)?;
    let url = Url::parse(location).map_err(|_| SonosError::MissingLocation)?;
    validate_local_url(&url)?;
    Ok(url)
}

pub async fn discover(timeout_after: Duration) -> Result<Vec<Url>, SonosError> {
    let mut searches = JoinSet::new();
    for address in discovery_bind_addresses() {
        let Ok(socket) = UdpSocket::bind(address).await else {
            continue;
        };
        if socket.send_to(SEARCH, SSDP_ADDRESS).await.is_ok() {
            searches.spawn(receive_locations(socket, timeout_after));
        }
    }

    let mut locations = Vec::new();
    while let Some(Ok(found_locations)) = searches.join_next().await {
        for location in found_locations {
            if !locations.contains(&location) {
                locations.push(location);
            }
        }
    }
    Ok(locations)
}

async fn receive_locations(socket: UdpSocket, timeout_after: Duration) -> Vec<Url> {
    let deadline = Instant::now() + timeout_after;
    let mut locations = Vec::new();
    let mut buffer = [0_u8; 4096];
    while let Ok(Ok((length, _))) = timeout(
        deadline.saturating_duration_since(Instant::now()),
        socket.recv_from(&mut buffer),
    )
    .await
    {
        if let Ok(location) = parse_ssdp_response(&buffer[..length])
            && !locations.contains(&location)
        {
            locations.push(location);
        }
        if Instant::now() >= deadline {
            break;
        }
    }
    locations
}
pub async fn retrieve_device(
    client: &reqwest::Client,
    location: Url,
    max_bytes: usize,
) -> Result<DiscoveredDevice, SonosError> {
    validate_local_url(&location)?;
    let bytes = response_bytes(client.get(location.clone()).send().await?, max_bytes).await?;
    let device = parse_device_description(&bytes, &location)?;
    Ok(DiscoveredDevice { location, device })
}

pub(crate) async fn response_bytes(
    response: reqwest::Response,
    max_bytes: usize,
) -> Result<Vec<u8>, SonosError> {
    if response
        .content_length()
        .is_some_and(|length| length > max_bytes as u64)
    {
        return Err(SonosError::ResponseTooLarge { limit: max_bytes });
    }
    let mut response = response;
    let mut bytes = Vec::new();
    while let Some(chunk) = response.chunk().await? {
        if bytes.len().saturating_add(chunk.len()) > max_bytes {
            return Err(SonosError::ResponseTooLarge { limit: max_bytes });
        }
        bytes.extend_from_slice(&chunk);
    }
    Ok(bytes)
}

pub(crate) fn validate_local_url(url: &Url) -> Result<(), SonosError> {
    if url.scheme() != "http" {
        return Err(SonosError::NonHttpUrl);
    }
    let host = url.host_str().ok_or(SonosError::MissingHost)?;
    let address = host
        .trim_matches(|c| c == '[' || c == ']')
        .parse::<std::net::IpAddr>()
        .map_err(|_| SonosError::NonLocalHost(host.to_owned()))?;
    let allowed = match address {
        std::net::IpAddr::V4(ip) => ip.is_private() || ip.is_link_local() || ip.is_loopback(),
        std::net::IpAddr::V6(ip) => {
            ip.is_unique_local() || ip.is_unicast_link_local() || ip.is_loopback()
        }
    };
    if allowed {
        Ok(())
    } else {
        Err(SonosError::NonLocalHost(host.to_owned()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bind_addresses_use_each_non_loopback_ipv4_interface_once() {
        assert_eq!(
            bind_addresses([
                IpAddr::V4(Ipv4Addr::LOCALHOST),
                IpAddr::from([10, 3, 10, 42]),
                IpAddr::from([10, 3, 10, 42]),
                IpAddr::from([169, 254, 1, 10]),
                IpAddr::V6("fe80::1".parse().unwrap()),
                IpAddr::V6("fe80::2".parse().unwrap()),
                IpAddr::V6("fe80::1".parse().unwrap()),
            ]),
            vec![
                "10.3.10.42:0".parse().unwrap(),
                "169.254.1.10:0".parse().unwrap(),
                "[fe80::1]:0".parse().unwrap(),
                "[fe80::2]:0".parse().unwrap(),
            ]
        );
    }

    #[test]
    fn validate_local_urls_allow_private_ipv4_and_ipv6_hosts() {
        assert!(
            validate_local_url(&Url::parse("http://192.168.1.42:1400/description.xml").unwrap())
                .is_ok()
        );
        assert!(
            validate_local_url(&Url::parse("http://[fe80::1]:1400/description.xml").unwrap())
                .is_ok()
        );
        assert!(
            validate_local_url(&Url::parse("http://[fd00::1]:1400/description.xml").unwrap())
                .is_ok()
        );
    }

    #[test]
    fn validate_local_urls_reject_non_http_or_public_addresses() {
        assert!(matches!(
            validate_local_url(&Url::parse("https://192.168.1.42:1400/description.xml").unwrap()),
            Err(SonosError::NonHttpUrl)
        ));
        assert!(matches!(
            validate_local_url(&Url::parse("http://93.184.216.34:1400/description.xml").unwrap()),
            Err(SonosError::NonLocalHost(_))
        ));
        assert!(matches!(
            validate_local_url(
                &Url::parse("http://[2001:4860:4860::8888]:1400/description.xml").unwrap()
            ),
            Err(SonosError::NonLocalHost(_))
        ));
    }
}
