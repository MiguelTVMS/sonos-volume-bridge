use sonos_volume_bridge_domain::{MuteState, SonosVolume};
use sonos_volume_bridge_sonos::{
    EventDeduplicator, SonosError, parse_device_description, parse_last_change, parse_ssdp_response,
};
use sonos_volume_bridge_sonos::{RenderingControlService, SonosClient, SonosDevice, SonosId, GenaEvent};
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
        state: event.state.clone(),
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
    };
    let client = SonosClient::builder().build().unwrap();
    assert_eq!(client.get_volume(&device).await.unwrap().get(), 20);
    client
        .set_volume(&device, SonosVolume::new(33).unwrap())
        .await
        .unwrap();
    client.set_mute(&device, MuteState(true)).await.unwrap();
    assert_eq!(
        server.state().await,
        MockSonosState {
            volume: SonosVolume::new(33).unwrap(),
            muted: MuteState(true)
        }
    );
}
