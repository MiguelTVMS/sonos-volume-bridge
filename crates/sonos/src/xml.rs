use crate::{
    RenderingControlService, SonosDevice, SonosError, SonosId, discovery::validate_local_url,
};
use quick_xml::{Reader, escape::unescape, events::Event};
use url::Url;

const MAX_XML_BYTES: usize = 512 * 1024;

#[allow(clippy::too_many_lines)] // Bounded single-pass parsing keeps untrusted XML handling auditable.
pub fn parse_device_description(xml: &[u8], location: &Url) -> Result<SonosDevice, SonosError> {
    if xml.len() > MAX_XML_BYTES {
        return Err(SonosError::ResponseTooLarge {
            limit: MAX_XML_BYTES,
        });
    }
    let mut reader = Reader::from_reader(xml);
    reader.config_mut().trim_text(true);
    let mut buffer = Vec::new();
    let mut stack = Vec::<String>::new();
    let mut name = String::new();
    let mut udn = None;
    let mut model_name = None;
    let mut model_number = None;
    let mut service = None::<ServiceCandidate>;
    let mut rendering_control = None;
    let mut group_rendering_control = None;
    loop {
        match reader.read_event_into(&mut buffer) {
            Ok(Event::Start(event)) => {
                let tag = tag_name(event.name().as_ref())?;
                if tag == "service" {
                    service = Some(ServiceCandidate::default());
                }
                stack.push(tag);
            }
            Ok(Event::End(event)) => {
                let tag = tag_name(event.name().as_ref())?;
                if tag == "service"
                    && let Some(candidate) = service.take()
                {
                    match candidate.kind.as_deref() {
                        Some("urn:schemas-upnp-org:service:RenderingControl:1") => {
                            rendering_control = Some(candidate);
                        }
                        Some("urn:schemas-upnp-org:service:GroupRenderingControl:1")
                            if group_rendering_control.is_none() =>
                        {
                            group_rendering_control = Some(candidate);
                        }
                        _ => {}
                    }
                }
                stack.pop();
            }
            Ok(Event::Text(text)) => {
                let decoded = text
                    .decode()
                    .map_err(|error| SonosError::Xml(error.to_string()))?;
                let value = unescape(&decoded)
                    .map_err(|error| SonosError::Xml(error.to_string()))?
                    .into_owned();
                match stack.last().map(String::as_str) {
                    Some("friendlyName") => name = value,
                    Some("UDN") => udn = Some(value),
                    Some("modelName") => model_name = Some(value),
                    Some("modelNumber") => model_number = Some(value),
                    Some("serviceType") => {
                        if let Some(service) = service.as_mut() {
                            service.kind = Some(value);
                        }
                    }
                    Some("controlURL") => {
                        if let Some(service) = service.as_mut() {
                            service.control_url = Some(value);
                        }
                    }
                    Some("eventSubURL") => {
                        if let Some(service) = service.as_mut() {
                            service.event_url = Some(value);
                        }
                    }
                    _ => {}
                }
            }
            Ok(Event::Eof) => break,
            Err(error) => return Err(SonosError::Xml(error.to_string())),
            _ => {}
        }
        buffer.clear();
    }
    let id = SonosId::new(udn.ok_or(SonosError::MissingUdn)?)?;
    let service = rendering_control
        .or(group_rendering_control)
        .ok_or(SonosError::MissingRenderingControl)?;
    let control_url = location
        .join(
            &service
                .control_url
                .ok_or(SonosError::MissingRenderingControl)?,
        )
        .map_err(|error| SonosError::Xml(error.to_string()))?;
    let event_url = location
        .join(
            &service
                .event_url
                .ok_or(SonosError::MissingRenderingControl)?,
        )
        .map_err(|error| SonosError::Xml(error.to_string()))?;
    validate_local_url(&control_url)?;
    validate_local_url(&event_url)?;
    Ok(SonosDevice {
        id,
        friendly_name: name,
        model_name,
        model_number,
        rendering_control: RenderingControlService {
            control_url,
            event_url,
        },
    })
}

#[derive(Default)]
struct ServiceCandidate {
    kind: Option<String>,
    control_url: Option<String>,
    event_url: Option<String>,
}

fn tag_name(bytes: &[u8]) -> Result<String, SonosError> {
    std::str::from_utf8(bytes)
        .map(str::to_owned)
        .map_err(|error| SonosError::Xml(error.to_string()))
}
