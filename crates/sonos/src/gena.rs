//! Local GENA subscription and callback listener primitives.

use crate::{
    GenaEvent, SonosDevice, SonosError,
    discovery::{response_bytes, validate_local_url},
    event::parse_last_change,
};
use rand::RngExt;
use std::{
    fmt::Write,
    net::{IpAddr, SocketAddr},
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
    sync::mpsc,
};
use url::Url;

const MAX_NOTIFY_BYTES: usize = 64 * 1024;
const DEFAULT_SUBSCRIPTION_TIMEOUT: Duration = Duration::from_secs(300);

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Subscription {
    pub id: String,
    pub timeout: Duration,
}

#[derive(Clone, Debug)]
pub struct GenaClient {
    http: reqwest::Client,
    max_response_bytes: usize,
}

impl GenaClient {
    pub fn new(timeout: Duration, max_response_bytes: usize) -> Result<Self, SonosError> {
        Ok(Self {
            http: reqwest::Client::builder()
                .timeout(timeout)
                .no_proxy()
                .build()?,
            max_response_bytes,
        })
    }
    pub async fn subscribe(
        &self,
        device: &SonosDevice,
        callback: &Url,
        requested_timeout: Duration,
    ) -> Result<Subscription, SonosError> {
        validate_local_url(&device.rendering_control.event_url)?;
        let method = reqwest::Method::from_bytes(b"SUBSCRIBE")
            .map_err(|error| SonosError::Protocol(error.to_string()))?;
        let response = self
            .http
            .request(method, device.rendering_control.event_url.clone())
            .header("CALLBACK", format!("<{callback}>"))
            .header("NT", "upnp:event")
            .header("TIMEOUT", timeout_header(requested_timeout))
            .send()
            .await?
            .error_for_status()?;
        subscription_from_response(response, self.max_response_bytes).await
    }
    pub async fn renew(
        &self,
        device: &SonosDevice,
        subscription: &Subscription,
        requested_timeout: Duration,
    ) -> Result<Subscription, SonosError> {
        let method = reqwest::Method::from_bytes(b"SUBSCRIBE")
            .map_err(|error| SonosError::Protocol(error.to_string()))?;
        let response = self
            .http
            .request(method, device.rendering_control.event_url.clone())
            .header("SID", &subscription.id)
            .header("TIMEOUT", timeout_header(requested_timeout))
            .send()
            .await?
            .error_for_status()?;
        subscription_from_response(response, self.max_response_bytes).await
    }
    pub async fn unsubscribe(
        &self,
        device: &SonosDevice,
        subscription: &Subscription,
    ) -> Result<(), SonosError> {
        let method = reqwest::Method::from_bytes(b"UNSUBSCRIBE")
            .map_err(|error| SonosError::Protocol(error.to_string()))?;
        self.http
            .request(method, device.rendering_control.event_url.clone())
            .header("SID", &subscription.id)
            .send()
            .await?
            .error_for_status()?;
        Ok(())
    }
}

async fn subscription_from_response(
    response: reqwest::Response,
    cap: usize,
) -> Result<Subscription, SonosError> {
    let headers = response.headers().clone();
    let _ = response_bytes(response, cap).await?;
    let id = headers
        .get("sid")
        .and_then(|value| value.to_str().ok())
        .filter(|value| value.starts_with("uuid:"))
        .ok_or(SonosError::MissingSoapValue("SID"))?
        .to_owned();
    let timeout = headers
        .get("timeout")
        .and_then(|value| value.to_str().ok())
        .and_then(parse_timeout)
        .unwrap_or(DEFAULT_SUBSCRIPTION_TIMEOUT);
    Ok(Subscription { id, timeout })
}
fn timeout_header(timeout: Duration) -> String {
    format!("Second-{}", timeout.as_secs())
}
fn parse_timeout(value: &str) -> Option<Duration> {
    let value = value.trim().trim_matches('"');
    let value = value.to_ascii_lowercase();
    let value = value.strip_prefix("second-")?;
    if value == "infinite" {
        return Some(DEFAULT_SUBSCRIPTION_TIMEOUT);
    }
    value.parse::<u64>().ok().map(Duration::from_secs)
}

/// Listener restricted to the supplied local bind address and Sonos peer address.
pub struct CallbackListener {
    callback_url: Url,
    expected_sid: Arc<Mutex<Option<String>>>,
    events: mpsc::Receiver<GenaEvent>,
}

impl CallbackListener {
    pub async fn bind(bind: SocketAddr, sonos_peer: IpAddr) -> Result<Self, SonosError> {
        let listener = TcpListener::bind(bind).await?;
        let local = listener.local_addr()?;
        let random: [u8; 16] = rand::rng().random();
        let mut path = String::with_capacity(random.len() * 2);
        for byte in random {
            write!(&mut path, "{byte:02x}").expect("writing to a String cannot fail");
        }
        let callback_url = Url::parse(&format!("http://{local}/sonos-volume-bridge/{path}"))
            .map_err(|error| SonosError::Xml(error.to_string()))?;
        let expected_sid = Arc::new(Mutex::new(None));
        let (sender, events) = mpsc::channel(32);
        let expected = Arc::clone(&expected_sid);
        let expected_path = callback_url.path().to_owned();
        tokio::spawn(async move {
            while let Ok((mut stream, peer)) = listener.accept().await {
                let sender = sender.clone();
                let expected = Arc::clone(&expected);
                let path = expected_path.clone();
                tokio::spawn(async move {
                    let mut bytes = vec![0_u8; MAX_NOTIFY_BYTES];
                    let read = stream.read(&mut bytes).await.unwrap_or_default();
                    bytes.truncate(read);
                    let subscription_id = expected.lock().ok().and_then(|sid| sid.clone());
                    let accepted = peer.ip() == sonos_peer
                        && parse_notify(&bytes, &path, subscription_id.as_deref())
                            .and_then(|(body, sequence)| parse_last_change(body, sequence).ok())
                            .is_some_and(|event| sender.try_send(event).is_ok());
                    let response = if accepted {
                        "HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n"
                    } else {
                        "HTTP/1.1 412 Precondition Failed\r\nContent-Length: 0\r\n\r\n"
                    };
                    let _ = stream.write_all(response.as_bytes()).await;
                });
            }
        });
        Ok(Self {
            callback_url,
            expected_sid,
            events,
        })
    }
    pub fn callback_url(&self) -> &Url {
        &self.callback_url
    }
    pub fn set_subscription(&self, subscription: &Subscription) {
        if let Ok(mut sid) = self.expected_sid.lock() {
            *sid = Some(subscription.id.clone());
        }
    }
    pub async fn recv(&mut self) -> Option<GenaEvent> {
        self.events.recv().await
    }
}

fn parse_notify<'a>(
    bytes: &'a [u8],
    expected_path: &str,
    expected_sid: Option<&str>,
) -> Option<(&'a [u8], Option<u32>)> {
    let split = bytes.windows(4).position(|window| window == b"\r\n\r\n")?;
    let (head, body) = bytes.split_at(split + 4);
    let head = std::str::from_utf8(head).ok()?;
    let mut lines = head.lines();
    let request = lines.next()?;
    if !request.starts_with("NOTIFY ") || !request.contains(expected_path) {
        return None;
    }
    let mut sid = None;
    let mut sequence = None;
    for line in lines {
        if let Some((key, value)) = line.split_once(':') {
            match key.trim().to_ascii_lowercase().as_str() {
                "sid" => sid = Some(value.trim().to_owned()),
                "seq" => sequence = value.trim().parse().ok(),
                _ => {}
            }
        }
    }
    if sid.as_deref() != expected_sid {
        return None;
    }
    Some((body, sequence))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parses_notify_only_for_matching_path_and_subscription() {
        let request =
            b"NOTIFY /sonos-volume-bridge/a HTTP/1.1\r\nSID: uuid:sub\r\nSEQ: 2\r\n\r\nbody";
        let (body, sequence) =
            parse_notify(request, "/sonos-volume-bridge/a", Some("uuid:sub")).unwrap();
        assert_eq!(body, b"body");
        assert_eq!(sequence, Some(2));
    }

    #[test]
    fn parses_timeout_with_expected_prefix_case_and_whitespace() {
        assert_eq!(
            parse_timeout(" second-300 "),
            Some(Duration::from_secs(300))
        );
        assert_eq!(parse_timeout("Second-300"), Some(Duration::from_secs(300)));
        assert_eq!(parse_timeout("SECOND-45"), Some(Duration::from_secs(45)));
        assert_eq!(
            parse_timeout("\"Second-120\""),
            Some(Duration::from_secs(120))
        );
    }

    #[test]
    fn parses_infinite_or_invalid_timeout_as_fallback() {
        assert_eq!(
            parse_timeout("Second-infinite"),
            Some(Duration::from_secs(300))
        );
        assert_eq!(parse_timeout(""), None);
    }
}
