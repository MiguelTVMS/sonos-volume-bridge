use crate::{discovery::validate_local_url, RenderingControlService, SonosDevice, SonosError, SonosId};
use quick_xml::{events::Event, Reader};
use url::Url;

const MAX_XML_BYTES: usize = 512 * 1024;

pub fn parse_device_description(xml: &[u8], location: &Url) -> Result<SonosDevice, SonosError> {
    if xml.len() > MAX_XML_BYTES { return Err(SonosError::ResponseTooLarge { limit: MAX_XML_BYTES }); }
    let mut reader = Reader::from_reader(xml);
    reader.config_mut().trim_text(true);
    let mut buffer = Vec::new();
    let mut stack = Vec::<String>::new();
    let mut name = String::new();
    let mut udn = None;
    let mut model_name = None;
    let mut model_number = None;
    let mut service_type = None;
    let mut control_url = None;
    let mut event_url = None;
    loop {
        match reader.read_event_into(&mut buffer) {
            Ok(Event::Start(event)) => stack.push(tag_name(event.name().as_ref())?),
            Ok(Event::End(_)) => { stack.pop(); },
            Ok(Event::Text(text)) => {
                let value = text.unescape().map_err(|error| SonosError::Xml(error.to_string()))?.into_owned();
                match stack.last().map(String::as_str) {
                    Some("friendlyName") => name = value,
                    Some("UDN") => udn = Some(value),
                    Some("modelName") => model_name = Some(value),
                    Some("modelNumber") => model_number = Some(value),
                    Some("serviceType") => service_type = Some(value),
                    Some("controlURL") if service_type.as_deref().is_some_and(|kind| kind.contains("RenderingControl")) => control_url = Some(value),
                    Some("eventSubURL") if service_type.as_deref().is_some_and(|kind| kind.contains("RenderingControl")) => event_url = Some(value),
                    _ => {},
                }
            },
            Ok(Event::Eof) => break,
            Err(error) => return Err(SonosError::Xml(error.to_string())),
            _ => {},
        }
        buffer.clear();
    }
    let id = SonosId::new(udn.ok_or(SonosError::MissingUdn)?)?;
    let control_url = location.join(&control_url.ok_or(SonosError::MissingRenderingControl)?).map_err(|error| SonosError::Xml(error.to_string()))?;
    let event_url = location.join(&event_url.ok_or(SonosError::MissingRenderingControl)?).map_err(|error| SonosError::Xml(error.to_string()))?;
    validate_local_url(&control_url)?;
    validate_local_url(&event_url)?;
    Ok(SonosDevice { id, friendly_name: name, model_name, model_number, rendering_control: RenderingControlService { control_url, event_url } })
}

fn tag_name(bytes: &[u8]) -> Result<String, SonosError> { std::str::from_utf8(bytes).map(str::to_owned).map_err(|error| SonosError::Xml(error.to_string())) }
