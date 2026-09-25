use sonos_volume_bridge_sonos::{
    FeatureAvailability as Availability, SonosClient, SonosDevice, parse_device_description,
};
use sonos_volume_bridge_test_support::{MockSoapServer, SoapReply};
use std::time::Duration;
use url::Url;

fn device(server: &MockSoapServer, model: &str, properties: bool) -> SonosDevice {
    let xml = format!(
        r"<root><device><UDN>uuid:test</UDN><modelName>{model}</modelName><serviceList>
        <service><serviceType>urn:schemas-upnp-org:service:RenderingControl:1</serviceType><controlURL>/render</controlURL><eventSubURL>/events</eventSubURL></service>
        {}</serviceList></device></root>",
        if properties {
            "<service><serviceType>urn:schemas-upnp-org:service:DeviceProperties:1</serviceType><controlURL>/advertised-properties</controlURL></service>"
        } else {
            ""
        }
    );
    parse_device_description(
        xml.as_bytes(),
        &Url::parse(&format!("http://{}/description", server.address)).unwrap(),
    )
    .unwrap()
}

#[tokio::test]
async fn probes_are_read_only_and_independent_and_off_is_supported() {
    let server = MockSoapServer::start().await.unwrap();
    server
        .reply("GetLoudness", SoapReply::value("CurrentLoudness", "0"))
        .await;
    server.reply("GetEQ:NightMode", SoapReply::fault(602)).await;
    server
        .reply("GetEQ:DialogLevel", SoapReply::value("CurrentValue", "1"))
        .await;
    server
        .reply("GetLEDState", SoapReply::value("CurrentLEDState", "Off"))
        .await;
    server
        .reply("GetBass", SoapReply::value("CurrentBass", "0"))
        .await;
    server.reply("GetTreble", SoapReply::fault(501)).await;
    let client = SonosClient::builder().build().unwrap();
    let settings = client
        .get_speaker_settings(&device(&server, "Sonos Ray", true))
        .await;
    assert_eq!(settings.loudness, Some(false));
    assert_eq!(settings.capabilities.loudness, Availability::Supported);
    assert_eq!(settings.capabilities.night_sound, Availability::Unsupported);
    assert_eq!(settings.speech_enhancement, Some(true));
    assert_eq!(settings.status_light, Some(false));
    assert_eq!(settings.capabilities.bass, Availability::Supported);
    assert_eq!(settings.bass, Some(0));
    assert_eq!(settings.capabilities.treble, Availability::Unavailable);
    let requests = server.requests().await;
    assert_eq!(requests.len(), 6);
    assert!(requests.iter().all(|r| !r.contains("#Set")));
    assert!(
        requests
            .iter()
            .any(|r| r.starts_with("POST /advertised-properties "))
    );
}

#[tokio::test]
async fn timeout_malformed_reply_and_ambiguous_fault_recover_on_next_refresh() {
    let server = MockSoapServer::start().await.unwrap();
    let mut delayed = SoapReply::value("CurrentLoudness", "1");
    delayed.delay = Duration::from_secs(1);
    server.reply("GetLoudness", delayed).await;
    server.reply("GetEQ:NightMode", SoapReply::fault(402)).await;
    server
        .reply(
            "GetEQ:DialogLevel",
            SoapReply::value("CurrentValue", "unknown"),
        )
        .await;
    let client = SonosClient::builder()
        .timeout(Duration::from_millis(100))
        .build()
        .unwrap();
    let device = device(&server, "Sonos Ray", false);
    let first = client.get_speaker_settings(&device).await;
    assert_eq!(first.capabilities.loudness, Availability::Unavailable);
    assert_eq!(first.capabilities.night_sound, Availability::Unavailable);
    assert_eq!(
        first.capabilities.speech_enhancement,
        Availability::Unavailable
    );
    assert_eq!(first.loudness, None);
    for (action, field) in [
        ("GetLoudness", "CurrentLoudness"),
        ("GetEQ:NightMode", "CurrentValue"),
        ("GetEQ:DialogLevel", "CurrentValue"),
    ] {
        server.reply(action, SoapReply::value(field, "0")).await;
    }
    let recovered = client.get_speaker_settings(&device).await;
    assert_eq!(recovered.capabilities.loudness, Availability::Supported);
    assert_eq!(recovered.capabilities.night_sound, Availability::Supported);
    assert_eq!(
        recovered.capabilities.speech_enhancement,
        Availability::Supported
    );
    assert_eq!(recovered.speech_enhancement, Some(false));
}

#[tokio::test]
async fn new_speaker_has_its_own_features_and_model_specific_speech_probe() {
    let server = MockSoapServer::start().await.unwrap();
    server
        .reply("GetEQ:DialogLevel", SoapReply::value("CurrentValue", "3"))
        .await;
    server
        .reply(
            "GetEQ:SpeechEnhanceEnabled",
            SoapReply::value("CurrentValue", "0"),
        )
        .await;
    let client = SonosClient::builder().build().unwrap();
    let ultra = client
        .get_speaker_settings(&device(&server, "Sonos Arc Ultra", false))
        .await;
    assert_eq!(ultra.speech_enhancement, Some(false));
    assert_eq!(
        ultra.capabilities.speech_enhancement,
        Availability::Supported
    );
    assert_eq!(ultra.capabilities.status_light, Availability::Unsupported);
    assert!(
        server
            .requests()
            .await
            .iter()
            .all(|r| !r.contains("GetLEDState") && !r.contains("<EQType>DialogLevel"))
    );
    let other = MockSoapServer::start().await.unwrap();
    let settings = client
        .get_speaker_settings(&device(&other, "Unknown speaker", false))
        .await;
    assert_eq!(settings.speech_enhancement, None);
    assert_eq!(
        settings.capabilities.speech_enhancement,
        Availability::Unsupported
    );
}

#[tokio::test]
async fn partial_push_followed_by_probe_preserves_omitted_capabilities() {
    use sonos_volume_bridge_sonos::{CallbackListener, Subscription};
    use std::net::{IpAddr, Ipv4Addr, SocketAddr};
    let server = MockSoapServer::start().await.unwrap();
    server
        .reply("GetEQ:DialogLevel", SoapReply::value("CurrentValue", "0"))
        .await;
    server
        .reply("GetLoudness", SoapReply::value("CurrentLoudness", "1"))
        .await;
    let device = device(&server, "Sonos Ray", false);
    let client = SonosClient::builder().build().unwrap();
    let before = client.get_speaker_settings(&device).await;
    assert_eq!(before.speech_enhancement, Some(false));
    let peer = IpAddr::V4(Ipv4Addr::LOCALHOST);
    let mut listener = CallbackListener::bind(SocketAddr::new(peer, 0), peer)
        .await
        .unwrap();
    listener.set_subscription(&Subscription {
        id: "uuid:mock".to_owned(),
        timeout: Duration::from_secs(300),
    });
    server
        .reply("GetEQ:DialogLevel", SoapReply::value("CurrentValue", "1"))
        .await;
    let response = reqwest::Client::new()
        .request(
            reqwest::Method::from_bytes(b"NOTIFY").unwrap(),
            listener.callback_url().clone(),
        )
        .header("SID", "uuid:mock")
        .header("SEQ", "1")
        .body("<LastChange>&lt;Event&gt;&lt;DialogLevel val=\"1\"/&gt;&lt;/Event&gt;</LastChange>")
        .send()
        .await
        .unwrap();
    assert!(response.status().is_success());
    let notification = tokio::time::timeout(Duration::from_secs(1), listener.recv())
        .await
        .unwrap()
        .unwrap();
    assert!(notification.settings_changed);
    assert!(notification.volume_state.is_none());
    let after = client.get_speaker_settings(&device).await;
    assert_eq!(after.speech_enhancement, Some(true));
    assert_eq!(after.capabilities, before.capabilities);
    assert_eq!(after.loudness, Some(true));
}

#[tokio::test]
async fn invalid_or_denied_responses_are_neither_unsupported_nor_off() {
    let server = MockSoapServer::start().await.unwrap();
    let device = device(&server, "Sonos Ray", true);
    let client = SonosClient::builder().build().unwrap();
    for reply in [
        SoapReply::fault(606),
        SoapReply::fault(402),
        SoapReply {
            status: 503,
            body: "Unavailable".to_owned(),
            delay: Duration::ZERO,
        },
        SoapReply::value("WrongField", "0"),
        SoapReply::value("CurrentValue", "3"),
    ] {
        server.reply("GetEQ:DialogLevel", reply).await;
        let settings = client.get_speaker_settings(&device).await;
        assert_eq!(
            settings.capabilities.speech_enhancement,
            Availability::Unavailable
        );
        assert_eq!(settings.speech_enhancement, None);
    }
    server
        .reply("GetBass", SoapReply::value("CurrentBass", "100"))
        .await;
    let settings = client.get_speaker_settings(&device).await;
    assert_eq!(settings.capabilities.bass, Availability::Unavailable);
    assert_eq!(settings.bass, None);
}

#[tokio::test]
async fn partial_volume_event_reads_missing_mute_without_a_feedback_write() {
    use sonos_volume_bridge_sonos::parse_rendering_control_notification;
    let server = MockSoapServer::start().await.unwrap();
    server
        .reply("GetVolume", SoapReply::value("CurrentVolume", "37"))
        .await;
    server
        .reply("GetMute", SoapReply::value("CurrentMute", "1"))
        .await;
    let client = SonosClient::builder().build().unwrap();
    let device = device(&server, "Sonos Ray", false);
    let notification = parse_rendering_control_notification(b"<LastChange>&lt;Event&gt;&lt;Volume channel=\"Master\" val=\"37\"/&gt;&lt;/Event&gt;</LastChange>", Some(3)).unwrap();
    let observed = client
        .notification_state(&device, &notification)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(observed.state.volume.get(), 37);
    assert!(observed.state.muted.0);
    let requests = server.requests().await;
    assert_eq!(requests.len(), 2);
    assert!(requests.iter().all(|r| !r.contains("#Set")));
}
