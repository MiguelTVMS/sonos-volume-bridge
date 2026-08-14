//! Reusable deterministic Sonos protocol test support.

use sonos_volume_bridge_domain::{MuteState, SonosVolume};
use std::sync::Arc;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
    sync::Mutex,
    task::JoinHandle,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MockSonosState {
    pub volume: SonosVolume,
    pub muted: MuteState,
}

impl Default for MockSonosState {
    fn default() -> Self {
        Self {
            volume: SonosVolume::new(20).unwrap_or(SonosVolume::MIN),
            muted: MuteState(false),
        }
    }
}

impl MockSonosState {
    pub fn soap_response(self, action: &str) -> String {
        match action {
            "GetVolume" => format!(
                "<s:Envelope><s:Body><u:GetVolumeResponse><CurrentVolume>{}</CurrentVolume></u:GetVolumeResponse></s:Body></s:Envelope>",
                self.volume.get()
            ),
            "GetMute" => format!(
                "<s:Envelope><s:Body><u:GetMuteResponse><CurrentMute>{}</CurrentMute></u:GetMuteResponse></s:Body></s:Envelope>",
                u8::from(self.muted.0)
            ),
            "GetZoneInfo" => "<s:Envelope><s:Body><u:GetZoneInfoResponse><MACAddress>001122334455</MACAddress></u:GetZoneInfoResponse></s:Body></s:Envelope>".to_owned(),
            "GetMediaInfo" => "<s:Envelope><s:Body><u:GetMediaInfoResponse><CurrentURI>x-sonos-htastream:RINCON_00112233445501400:spdif</CurrentURI></u:GetMediaInfoResponse></s:Body></s:Envelope>".to_owned(),
            "SetVolume" | "SetMute" | "SetAVTransportURI" => {
                "<s:Envelope><s:Body/></s:Envelope>".to_owned()
            }
            _ => "<s:Fault>Invalid Action</s:Fault>".to_owned(),
        }
    }
}

/// A minimal local RenderingControl HTTP server for integration tests.
pub struct MockSonosServer {
    pub address: std::net::SocketAddr,
    state: Arc<Mutex<MockSonosState>>,
    task: JoinHandle<()>,
}

impl MockSonosServer {
    pub async fn start(state: MockSonosState) -> std::io::Result<Self> {
        let listener = TcpListener::bind("127.0.0.1:0").await?;
        let address = listener.local_addr()?;
        let state = Arc::new(Mutex::new(state));
        let task_state = Arc::clone(&state);
        let task = tokio::spawn(async move {
            while let Ok((mut stream, _)) = listener.accept().await {
                let state = Arc::clone(&task_state);
                tokio::spawn(async move {
                    let mut request = vec![0_u8; 8192];
                    let read = stream.read(&mut request).await.unwrap_or_default();
                    let request = String::from_utf8_lossy(&request[..read]);
                    let mut state = state.lock().await;
                    if let Some(volume) = xml_value(&request, "DesiredVolume") {
                        if let Ok(volume) = volume.parse() {
                            state.volume = SonosVolume::new(volume).unwrap_or(state.volume);
                        }
                    }
                    if let Some(muted) = xml_value(&request, "DesiredMute") {
                        state.muted = MuteState(muted == "1");
                    }
                    let action = if request.contains("GetVolume") {
                        "GetVolume"
                    } else if request.contains("SetVolume") {
                        "SetVolume"
                    } else if request.contains("GetMute") {
                        "GetMute"
                    } else if request.contains("SetMute") {
                        "SetMute"
                    } else if request.contains("GetZoneInfo") {
                        "GetZoneInfo"
                    } else if request.contains("SetAVTransportURI") {
                        "SetAVTransportURI"
                    } else if request.contains("GetMediaInfo") {
                        "GetMediaInfo"
                    } else {
                        "Unknown"
                    };
                    let body = state.soap_response(action);
                    let response = format!(
                        "HTTP/1.1 200 OK\r\nContent-Type: text/xml\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                        body.len()
                    );
                    let _ = stream.write_all(response.as_bytes()).await;
                });
            }
        });
        Ok(Self {
            address,
            state,
            task,
        })
    }
    pub async fn state(&self) -> MockSonosState {
        *self.state.lock().await
    }
}

impl Drop for MockSonosServer {
    fn drop(&mut self) {
        self.task.abort();
    }
}

fn xml_value<'a>(request: &'a str, tag: &str) -> Option<&'a str> {
    let start = format!("<{tag}>");
    let end = format!("</{tag}>");
    let value = request.split_once(&start)?.1;
    value.split_once(&end).map(|(value, _)| value)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn mock_emits_rendering_control_values() {
        assert!(
            MockSonosState::default()
                .soap_response("GetVolume")
                .contains("20")
        );
    }
}
