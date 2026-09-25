use sonos_volume_bridge_domain::{MuteState, SonosVolume};
use sonos_volume_bridge_sonos::{
    AvTransportService, GenaEvent, RenderingControlService, SonosClient, SonosDevice, SonosId,
};
use sonos_volume_bridge_sonos::{
    EventDeduplicator, SonosError, parse_device_description, parse_last_change, parse_ssdp_response,
};
use sonos_volume_bridge_test_support::{MockSonosServer, MockSonosState};
use url::Url;

const LOCATION: &str = "http://192.168.1.40:1400/xml/device_description.xml";

#[test]
fn parses_ssdp_location_only_from_successful_response() {
    let response = format!(
        "HTTP/1.1 200 OK\r\nLOCATION: {LOCATION}\r\nST: urn:schemas-upnp-org:device:ZonePlayer:1\r\n\r\n"
    );
    assert_eq!(
        parse_ssdp_response(response.as_bytes()).unwrap().as_str(),
        LOCATION
    );
    assert!(matches!(
        parse_ssdp_response(b"HTTP/1.1 404 Not Found\r\n\r\n"),
        Err(SonosError::InvalidSsdpResponse)
    ));
}

#[test]
fn rejects_public_ssdp_location() {
    let response =
        b"HTTP/1.1 200 OK\r\nLOCATION: http://8.8.8.8/xml/device_description.xml\r\n\r\n";
    assert!(matches!(
        parse_ssdp_response(response),
        Err(SonosError::NonLocalHost(_))
    ));
}

#[test]
fn parses_device_description_and_stable_identity() {
    let device = parse_device_description(
        include_bytes!("fixtures/device-description.xml"),
        &Url::parse(LOCATION).unwrap(),
    )
    .unwrap();
    assert_eq!(device.id.as_str(), "uuid:RINCON_00000000000101400");
    assert_eq!(device.friendly_name, "Living Room");
    assert_eq!(
        device.rendering_control.control_url.as_str(),
        "http://192.168.1.40:1400/MediaRenderer/RenderingControl/Control"
    );
}

#[test]
fn rejects_malformed_device_description() {
    assert!(
        parse_device_description(
            include_bytes!("fixtures/malformed.xml"),
            &Url::parse(LOCATION).unwrap()
        )
        .is_err()
    );
}

#[test]
fn deduplicates_replayed_gena_notifications() {
    let event = parse_last_change(include_bytes!("fixtures/last-change.xml"), Some(7)).unwrap();
    let mut deduplicator = EventDeduplicator::default();
    assert!(deduplicator.accept(event.clone()));
    assert!(!deduplicator.accept(event));
}

#[test]
fn accepts_replayed_events_with_changed_sequence_number() {
    let event = parse_last_change(include_bytes!("fixtures/last-change.xml"), Some(7)).unwrap();
    let replay = GenaEvent {
        sequence: Some(8),
        state: event.state,
    };
    let mut deduplicator = EventDeduplicator::default();
    assert!(deduplicator.accept(event));
    assert!(deduplicator.accept(replay));
}

#[test]
fn grouped_metadata_keeps_selected_player_identity() {
    let device = parse_device_description(
        include_bytes!("fixtures/grouped-device-description.xml"),
        &Url::parse(LOCATION).unwrap(),
    )
    .unwrap();
    assert_eq!(device.id.as_str(), "uuid:RINCON_00000000000201400");
}

#[test]
fn discovers_av_transport_control_url() {
    let device = parse_device_description(
        include_bytes!("fixtures/grouped-device-description.xml"),
        &Url::parse(LOCATION).unwrap(),
    )
    .unwrap();
    assert_eq!(
        device.av_transport.unwrap().control_url.as_str(),
        "http://192.168.1.40:1400/MediaRenderer/AVTransport/Control"
    );
}
#[test]
fn standard_rendering_control_wins_over_group_rendering_control() {
    let xml = br"<root><device><friendlyName>Office</friendlyName><UDN>uuid:RINCON_test</UDN><serviceList><service><serviceType>urn:schemas-upnp-org:service:RenderingControl:1</serviceType><controlURL>/standard</controlURL><eventSubURL>/standard-event</eventSubURL></service><service><serviceType>urn:schemas-upnp-org:service:GroupRenderingControl:1</serviceType><controlURL>/group</controlURL><eventSubURL>/group-event</eventSubURL></service></serviceList></device></root>";
    let device = parse_device_description(xml, &Url::parse(LOCATION).unwrap()).unwrap();
    assert_eq!(
        device.rendering_control.control_url.as_str(),
        "http://192.168.1.40:1400/standard"
    );
}

#[test]
fn parses_escaped_last_change_and_master_values() {
    let event = parse_last_change(include_bytes!("fixtures/last-change.xml"), Some(7)).unwrap();
    assert_eq!(event.sequence, Some(7));
    assert_eq!(event.state.volume.get(), 24);
    assert!(!event.state.muted.0);
}

#[test]
fn rejects_missing_last_change_values() {
    assert!(matches!(parse_last_change(b"<propertyset><property><LastChange>&lt;Event/&gt;</LastChange></property></propertyset>", None), Err(SonosError::MissingSoapValue("Volume"))));
}

#[tokio::test]
async fn mock_server_supports_rendering_control_round_trip() {
    let server = MockSonosServer::start(MockSonosState::default())
        .await
        .unwrap();
    let base = Url::parse(&format!("http://{}/", server.address)).unwrap();
    let device = SonosDevice {
        id: SonosId::new("uuid:RINCON_test").unwrap(),
        friendly_name: "Test speaker".to_owned(),
        model_name: None,
        model_number: None,
        rendering_control: RenderingControlService {
            control_url: base.join("control").unwrap(),
            event_url: base.join("event").unwrap(),
        },
        device_properties: Some(base.join("DeviceProperties/Control").unwrap()),
        av_transport: Some(AvTransportService {
            control_url: base.join("avtransport").unwrap(),
        }),
    };
    let client = SonosClient::builder().build().unwrap();
    assert_eq!(client.get_volume(&device).await.unwrap().get(), 20);
    client
        .set_volume(&device, SonosVolume::new(33).unwrap())
        .await
        .unwrap();
    client.set_mute(&device, MuteState(true)).await.unwrap();
    client.select_home_theater_input(&device).await.unwrap();
    assert_eq!(
        server.state().await,
        MockSonosState {
            volume: SonosVolume::new(33).unwrap(),
            muted: MuteState(true)
        }
    );
}

#[test]
fn parses_numeric_references_in_last_change_attributes() {
    let body = br#"<LastChange>&lt;Event&gt;&lt;Volume channel="M&amp;#97;ster" val="&amp;#50;4"/&gt;&lt;Mute channel="Master" val="&amp;#49;"/&gt;&lt;/Event&gt;</LastChange>"#;
    let event = parse_last_change(body, Some(9)).unwrap();
    assert_eq!(event.state.volume.get(), 24);
    assert!(event.state.muted.0);
}

#[test]
fn rejects_invalid_utf8_in_xml_payloads() {
    let location = Url::parse(LOCATION).unwrap();
    assert!(matches!(
        parse_device_description(b"<root><friendlyName>\xff</friendlyName></root>", &location),
        Err(SonosError::Xml(_))
    ));
    assert!(matches!(
        parse_last_change(b"<LastChange>\xff</LastChange>", None),
        Err(SonosError::Xml(_))
    ));
}

#[tokio::test]
async fn ray_speech_state_uses_dialog_level_for_read_write_and_confirmation() {
    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::TcpListener,
    };
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let task = tokio::spawn(async move {
        for value in [Some("1"), None, Some("0")] {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut bytes = vec![0; 8192];
            let size = stream.read(&mut bytes).await.unwrap();
            let request = String::from_utf8_lossy(&bytes[..size]);
            assert!(request.contains("<EQType>DialogLevel</EQType>"));
            assert!(!request.contains("SpeechEnhanceEnabled"));
            if value.is_none() {
                assert!(request.contains("<DesiredValue>0</DesiredValue>"));
            }
            let body = value.map_or_else(
                || "<Response/>".to_owned(),
                |v| format!("<Response><CurrentValue>{v}</CurrentValue></Response>"),
            );
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            stream.write_all(response.as_bytes()).await.unwrap();
        }
    });
    let base = Url::parse(&format!("http://{address}/")).unwrap();
    let device = SonosDevice {
        id: SonosId::new("test-speaker").unwrap(),
        friendly_name: "Test speaker".to_owned(),
        model_name: Some("Sonos Ray".to_owned()),
        model_number: None,
        rendering_control: RenderingControlService {
            control_url: base.join("control").unwrap(),
            event_url: base.join("event").unwrap(),
        },
        av_transport: None,
        device_properties: None,
    };
    let client = SonosClient::builder().build().unwrap();
    assert!(client.get_speech_enhancement(&device).await.unwrap());
    client.set_speech_enhancement(&device, false).await.unwrap();
    task.await.unwrap();
}

#[tokio::test]
async fn callback_accepts_eq_only_push_without_volume_or_mute() {
    use sonos_volume_bridge_sonos::{CallbackListener, Subscription};
    use std::{
        net::{IpAddr, Ipv4Addr, SocketAddr},
        time::Duration,
    };
    let peer = IpAddr::V4(Ipv4Addr::LOCALHOST);
    let mut listener = CallbackListener::bind(SocketAddr::new(peer, 0), peer)
        .await
        .unwrap();
    listener.set_subscription(&Subscription {
        id: "uuid:test-subscription".to_owned(),
        timeout: Duration::from_secs(300),
    });
    let body = "<propertyset><property><LastChange>&lt;Event&gt;&lt;InstanceID val=\"0\"&gt;&lt;DialogLevel val=\"1\"/&gt;&lt;/InstanceID&gt;&lt;/Event&gt;</LastChange></property></propertyset>";
    let client = reqwest::Client::new();
    let response = client
        .request(
            reqwest::Method::from_bytes(b"NOTIFY").unwrap(),
            listener.callback_url().clone(),
        )
        .header("SID", "uuid:test-subscription")
        .header("SEQ", "9")
        .body(body)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), reqwest::StatusCode::OK);
    let event = tokio::time::timeout(Duration::from_secs(1), listener.recv())
        .await
        .unwrap()
        .unwrap();
    assert!(event.settings_changed);
    assert!(event.volume_state.is_none());
    // Subscription validation still applies to settings-only notifications.
    let rejected = client
        .request(
            reqwest::Method::from_bytes(b"NOTIFY").unwrap(),
            listener.callback_url().clone(),
        )
        .header("SID", "uuid:other-subscription")
        .header("SEQ", "10")
        .body(body)
        .send()
        .await
        .unwrap();
    assert_eq!(rejected.status(), reqwest::StatusCode::PRECONDITION_FAILED);
}

#[test]
fn partial_volume_notifications_request_authoritative_read_without_inventing_mute() {
    use sonos_volume_bridge_sonos::parse_rendering_control_notification;
    for tag in ["Volume", "Mute"] {
        let xml = format!(
            "<LastChange>&lt;Event&gt;&lt;{tag} channel=\"Master\" val=\"1\"/&gt;&lt;/Event&gt;</LastChange>"
        );
        let event = parse_rendering_control_notification(xml.as_bytes(), Some(2)).unwrap();
        assert!(event.volume_changed);
        assert!(event.volume_state.is_none());
        assert!(!event.settings_changed);
    }
}
