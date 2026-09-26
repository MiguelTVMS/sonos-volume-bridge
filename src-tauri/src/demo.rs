//! Loopback Sonos device for native demo builds. All application services stay real.
use std::{collections::HashMap, io, sync::Arc, time::Duration};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    sync::{Mutex, OnceCell},
    task::{JoinHandle, JoinSet},
};
use url::Url;

pub const SPEAKER_ID: &str = "uuid:RINCON_DEMO01400";
const SID: &str = "uuid:sonos-volume-bridge-demo";

pub struct DemoSpeaker {
    pub location: Url,
    task: JoinHandle<()>,
}
impl Drop for DemoSpeaker {
    fn drop(&mut self) {
        self.task.abort();
    }
}

pub async fn speaker() -> io::Result<&'static DemoSpeaker> {
    static SPEAKER: OnceCell<DemoSpeaker> = OnceCell::const_new();
    SPEAKER.get_or_try_init(DemoSpeaker::start).await
}

struct DeviceState {
    values: HashMap<String, String>,
    callback: Option<Url>,
    sequence: u32,
}
impl Default for DeviceState {
    fn default() -> Self {
        Self {
            values: [
                ("Volume", "28"),
                ("Mute", "0"),
                ("Loudness", "1"),
                ("LEDState", "On"),
                ("NightMode", "0"),
                ("DialogLevel", "0"),
                ("Treble", "0"),
                ("Bass", "0"),
                ("URI", ""),
            ]
            .into_iter()
            .map(|(k, v)| (k.into(), v.into()))
            .collect(),
            callback: None,
            sequence: 0,
        }
    }
}
impl DemoSpeaker {
    pub async fn start() -> io::Result<Self> {
        let listener = TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0)).await?;
        let location = Url::parse(&format!("http://{}/device.xml", listener.local_addr()?))
            .map_err(io::Error::other)?;
        let state = Arc::new(Mutex::new(DeviceState::default()));
        let http = reqwest::Client::builder()
            .no_proxy()
            .timeout(Duration::from_secs(2))
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(io::Error::other)?;
        let task = tokio::spawn(async move {
            let mut clients = JoinSet::new();
            loop {
                tokio::select! {
                    connection = listener.accept() => {
                        let Ok((stream, _)) = connection else { break; };
                        let state = Arc::clone(&state);
                        let http = http.clone();
                        clients.spawn(async move {
                            let _ = tokio::time::timeout(Duration::from_secs(5), serve(stream, state, http)).await;
                        });
                    }
                    _ = clients.join_next(), if !clients.is_empty() => {}
                }
            }
        });
        Ok(Self { location, task })
    }
}

fn header<'a>(request: &'a str, name: &str) -> Option<&'a str> {
    request.split("\r\n\r\n").next()?.lines().find_map(|line| {
        let (key, value) = line.split_once(':')?;
        key.eq_ignore_ascii_case(name).then(|| value.trim())
    })
}
fn xml_value<'a>(body: &'a str, name: &str) -> Option<&'a str> {
    body.split_once(&format!("<{name}>"))?
        .1
        .split_once(&format!("</{name}>"))
        .map(|(value, _)| value)
}
fn description() -> String {
    format!(
        r"<root><device><friendlyName>Living Room (simulated)</friendlyName>
<UDN>{SPEAKER_ID}</UDN><modelName>Sonos Ray</modelName><modelNumber>S36</modelNumber><serviceList>
<service><serviceType>urn:schemas-upnp-org:service:RenderingControl:1</serviceType><controlURL>/control</controlURL><eventSubURL>/events</eventSubURL></service>
<service><serviceType>urn:schemas-upnp-org:service:DeviceProperties:1</serviceType><controlURL>/properties</controlURL></service>
<service><serviceType>urn:schemas-upnp-org:service:AVTransport:1</serviceType><controlURL>/transport</controlURL></service>
</serviceList></device></root>"
    )
}

async fn read_request(stream: &mut TcpStream) -> io::Result<String> {
    let mut bytes = Vec::new();
    loop {
        let mut chunk = [0; 4096];
        let size = stream.read(&mut chunk).await?;
        if size == 0 || bytes.len() + size > 65536 {
            return Err(io::Error::other("Invalid demo request"));
        }
        bytes.extend_from_slice(&chunk[..size]);
        let text = String::from_utf8_lossy(&bytes);
        if let Some((head, body)) = text.split_once("\r\n\r\n") {
            let length = header(head, "Content-Length")
                .unwrap_or("0")
                .parse::<usize>()
                .map_err(io::Error::other)?;
            if body.len() >= length {
                return Ok(text.into_owned());
            }
        }
    }
}

async fn serve(
    mut stream: TcpStream,
    state: Arc<Mutex<DeviceState>>,
    http: reqwest::Client,
) -> io::Result<()> {
    let request = read_request(&mut stream).await?;
    let method = request.split_whitespace().next().unwrap_or("");
    let mut state = state.lock().await;
    let mut extra_headers = String::new();
    let mut notify = false;
    let body = match method {
        "GET" => description(),
        "SUBSCRIBE" => {
            if let Some(callback) = header(&request, "CALLBACK") {
                let url =
                    Url::parse(callback.trim_matches(['<', '>'])).map_err(io::Error::other)?;
                if url.scheme() != "http" || url.host_str() != Some("127.0.0.1") {
                    return Err(io::Error::other("Demo callbacks must be loopback HTTP"));
                }
                state.callback = Some(url);
                state.sequence = 0;
            }
            extra_headers = format!("SID: {SID}\r\nTIMEOUT: Second-300\r\n");
            String::new()
        }
        "UNSUBSCRIBE" => {
            state.callback = None;
            String::new()
        }
        "POST" => {
            let action = header(&request, "SOAPACTION")
                .and_then(|v| v.split_once('#'))
                .map_or("", |(_, a)| a.trim_end_matches('"'));
            let result = soap(&mut state, action, &request);
            notify = action.starts_with("Set") && result.is_some();
            let Some(body) = result else {
                stream.write_all(b"HTTP/1.1 500 Unsupported\r\nContent-Length: 0\r\nConnection: close\r\n\r\n").await?;
                return Ok(());
            };
            body
        }
        _ => return Err(io::Error::other("Unsupported demo method")),
    };
    let response = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: text/xml\r\n{extra_headers}Content-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    stream.write_all(response.as_bytes()).await?;
    stream.shutdown().await?;
    if notify && let Some(callback) = state.callback.clone() {
        state.sequence = state.sequence.wrapping_add(1);
        let sequence = state.sequence;
        let event = format!(
            r#"<e:propertyset xmlns:e="urn:schemas-upnp-org:event-1-0"><e:property><LastChange>&lt;Event&gt;&lt;InstanceID val="0"&gt;&lt;Volume channel="Master" val="{}"/&gt;&lt;Mute channel="Master" val="{}"/&gt;&lt;NightMode val="{}"/&gt;&lt;/InstanceID&gt;&lt;/Event&gt;</LastChange></e:property></e:propertyset>"#,
            state.values["Volume"], state.values["Mute"], state.values["NightMode"]
        );
        // Keep updates ordered while the real callback listener acknowledges delivery.
        let _ = http
            .request(
                reqwest::Method::from_bytes(b"NOTIFY").map_err(io::Error::other)?,
                callback,
            )
            .header("SID", SID)
            .header("SEQ", sequence)
            .header("NT", "upnp:event")
            .header("NTS", "upnp:propchange")
            .body(event)
            .send()
            .await;
    }
    Ok(())
}

fn soap(state: &mut DeviceState, action: &str, request: &str) -> Option<String> {
    if action == "GetZoneInfo" {
        return Some("<Response><HTAudioIn>2</HTAudioIn><MACAddress>02:00:00:00:00:01</MACAddress></Response>".into());
    }
    if action == "SetAVTransportURI" {
        let uri = xml_value(request, "CurrentURI")?;
        if !uri.starts_with("x-sonos-htastream:") || uri.contains(['<', '>', '&']) {
            return None;
        }
        state.values.insert("URI".into(), uri.into());
        return Some("<Response/>".into());
    }
    if action == "GetMediaInfo" {
        return Some(format!(
            "<Response><CurrentURI>{}</CurrentURI></Response>",
            state.values["URI"]
        ));
    }
    let (write, name) = if let Some(name) = action.strip_prefix("Get") {
        (false, name)
    } else {
        (true, action.strip_prefix("Set")?)
    };
    let key = if name == "EQ" {
        xml_value(request, "EQType")?
    } else {
        name
    };
    let value = state.values.get_mut(key)?;
    let field = if name == "EQ" { "Value" } else { name };
    if write {
        let desired = xml_value(request, &format!("Desired{field}"))?;
        let valid = match key {
            "Volume" => desired.parse::<u8>().is_ok_and(|v| v <= 100),
            "Treble" | "Bass" => desired.parse::<i8>().is_ok_and(|v| (-10..=10).contains(&v)),
            "LEDState" => matches!(desired, "On" | "Off"),
            _ => matches!(desired, "0" | "1"),
        };
        if !valid {
            return None;
        }
        *value = desired.into();
        Some("<Response/>".into())
    } else {
        Some(format!(
            "<Response><Current{field}>{value}</Current{field}></Response>"
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        commands,
        config::{AppConfiguration, ConfigStore},
        night_schedule, runtime,
        state::AppState,
    };
    use sonos_volume_bridge_domain::{MuteState, SonosVolume};
    use sonos_volume_bridge_sonos::{CallbackListener, GenaClient, SonosClient};
    use tauri::Manager;

    #[tokio::test]
    async fn simulated_speaker_uses_real_soap_and_gena_clients() {
        let speaker = DemoSpeaker::start().await.unwrap();
        let client = SonosClient::builder().build().unwrap();
        let device = client
            .retrieve_device(speaker.location.clone())
            .await
            .unwrap()
            .device;
        let mut listener =
            CallbackListener::bind("127.0.0.1:0".parse().unwrap(), "127.0.0.1".parse().unwrap())
                .await
                .unwrap();
        let gena = GenaClient::new(Duration::from_secs(2), 65536).unwrap();
        let subscription = gena
            .subscribe(&device, listener.callback_url(), Duration::from_secs(300))
            .await
            .unwrap();
        listener.set_subscription(&subscription);
        client
            .set_volume(&device, SonosVolume::new(35).unwrap())
            .await
            .unwrap();
        let event = tokio::time::timeout(Duration::from_secs(2), listener.recv())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(event.volume_state.unwrap().state.volume.get(), 35);
        client.set_mute(&device, MuteState(true)).await.unwrap();
        client.set_eq(&device, "NightMode", true).await.unwrap();
        client.set_speech_enhancement(&device, true).await.unwrap();
        client.set_loudness(&device, false).await.unwrap();
        client.set_status_light(&device, false).await.unwrap();
        client.set_tone(&device, "Bass", -4).await.unwrap();
        client.set_tone(&device, "Treble", 3).await.unwrap();
        let settings = client.get_speaker_settings(&device).await;
        assert_eq!(settings.night_sound, Some(true));
        assert_eq!(settings.speech_enhancement, Some(true));
        assert_eq!(settings.loudness, Some(false));
        assert_eq!(settings.status_light, Some(false));
        assert_eq!(settings.bass, Some(-4));
        assert_eq!(settings.treble, Some(3));
        assert_eq!(client.get_volume(&device).await.unwrap().get(), 35);
        assert!(client.get_mute(&device).await.unwrap().0);
        client.select_home_theater_input(&device).await.unwrap();
        assert_eq!(
            client
                .get_audio_input_format(&device)
                .await
                .unwrap()
                .as_deref(),
            Some("Stereo")
        );
        gena.renew(&device, &subscription, Duration::from_secs(300))
            .await
            .unwrap();
        gena.unsubscribe(&device, &subscription).await.unwrap();
    }

    #[tokio::test]
    async fn normal_schedule_worker_and_commands_lock_demo_speaker_after_enable() {
        let speaker = speaker().await.unwrap();
        let configuration = AppConfiguration {
            selected_sonos_id: Some(SPEAKER_ID.into()),
            last_known_sonos_address: Some(speaker.location.to_string()),
            ..AppConfiguration::default()
        };
        let directory =
            std::env::temp_dir().join(format!("sonos-demo-schedule-test-{}", std::process::id()));
        let store = ConfigStore::new(directory.join("config.json"));
        let (_, guard) = tracing_appender::non_blocking(std::io::sink());
        let app = tauri::test::mock_builder()
            .manage(AppState::new(store, configuration.clone(), guard))
            .build(tauri::test::mock_context(tauri::test::noop_assets()))
            .unwrap();
        let state = app.state::<AppState>();
        night_schedule::start(app.handle().clone());
        // Start with recurrence disabled, save a block covering the current time,
        // manually turn it off, then enable via the actual production command.
        let mut saved = configuration.clone();
        saved.night_mode_schedule.blocks = vec![vec![true; 48]; 7];
        state.store.save(&saved).unwrap();
        state.replace_configuration(saved.clone());
        night_schedule::apply_saved(&saved).await.unwrap();
        commands::set_speaker_setting(runtime::SpeakerSetting::NightSound, false, app.state())
            .await
            .unwrap();
        commands::enable_night_schedule(true, app.state())
            .await
            .unwrap();
        // Manual off must be rejected even before the worker's first startup tick.
        assert!(
            commands::set_speaker_setting(runtime::SpeakerSetting::NightSound, false, app.state())
                .await
                .unwrap_err()
                .contains("Disable the schedule")
        );
        tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                let status = commands::get_schedule_status(app.state()).unwrap();
                if status.active && status.supported {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
        })
        .await
        .unwrap();
        assert_eq!(
            runtime::speaker_settings(saved.clone()).await.night_sound,
            Some(true)
        );
        commands::enable_night_schedule(false, app.state())
            .await
            .unwrap();
        assert_eq!(
            runtime::speaker_settings(saved.clone()).await.night_sound,
            Some(true)
        );
        commands::set_speaker_setting(runtime::SpeakerSetting::NightSound, false, app.state())
            .await
            .unwrap();
        assert_eq!(
            runtime::speaker_settings(saved.clone()).await.night_sound,
            Some(false)
        );
        // An enabled empty schedule must allow manual on and off.
        saved.night_mode_schedule.blocks = vec![vec![false; 48]; 7];
        state.replace_configuration(saved);
        commands::enable_night_schedule(true, app.state())
            .await
            .unwrap();
        commands::set_speaker_setting(runtime::SpeakerSetting::NightSound, true, app.state())
            .await
            .unwrap();
        commands::set_speaker_setting(runtime::SpeakerSetting::NightSound, false, app.state())
            .await
            .unwrap();
        state.stop_runtime();
        drop(app);
        std::fs::remove_dir_all(directory).unwrap();
    }
}
