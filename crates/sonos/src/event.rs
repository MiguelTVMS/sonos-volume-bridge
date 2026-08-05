use crate::SonosError;
use quick_xml::{events::Event, Reader};
use sonos_volume_bridge_domain::{MuteState, SonosVolume};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GenaState { pub volume: SonosVolume, pub muted: MuteState }
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GenaEvent { pub sequence: Option<u32>, pub state: GenaState }

/// Drops replayed GENA notifications while preserving a changed state.
#[derive(Default, Debug)]
pub struct EventDeduplicator { last: Option<GenaEvent> }

impl EventDeduplicator {
    pub fn accept(&mut self, event: GenaEvent) -> bool {
        if self.last.as_ref() == Some(&event) { return false; }
        self.last = Some(event);
        true
    }
}

pub fn parse_last_change(body: &[u8], sequence: Option<u32>) -> Result<GenaEvent, SonosError> {
    let outer = first_text(body, "LastChange")?.ok_or(SonosError::MissingSoapValue("LastChange"))?;
    let mut volume = None;
    let mut muted = None;
    let mut reader = Reader::from_str(&outer);
    reader.config_mut().trim_text(true);
    let mut buffer = Vec::new();
    loop {
        match reader.read_event_into(&mut buffer) {
            Ok(Event::Empty(event)) | Ok(Event::Start(event)) => {
                let tag = tag(event.name().as_ref())?;
                if tag == "Volume" && attribute(&event, "channel")?.as_deref().is_none_or(|channel| channel == "Master") { volume = attribute(&event, "val")?; }
                if tag == "Mute" && attribute(&event, "channel")?.as_deref().is_none_or(|channel| channel == "Master") { muted = attribute(&event, "val")?; }
            },
            Ok(Event::Eof) => break,
            Err(error) => return Err(SonosError::Xml(error.to_string())),
            _ => {},
        }
        buffer.clear();
    }
    let volume = volume.ok_or(SonosError::MissingSoapValue("Volume"))?.parse::<u8>().map_err(|error| SonosError::InvalidVolume(error.to_string()))?;
    let muted = match muted.ok_or(SonosError::MissingSoapValue("Mute"))?.as_str() { "0" => false, "1" => true, value => return Err(SonosError::InvalidMute(value.to_owned())) };
    Ok(GenaEvent { sequence, state: GenaState { volume: SonosVolume::new(volume).map_err(|error| SonosError::InvalidVolume(error.to_string()))?, muted: MuteState(muted) } })
}

pub(crate) fn first_text(xml: &[u8], wanted: &str) -> Result<Option<String>, SonosError> {
    let mut reader = Reader::from_reader(xml);
    reader.config_mut().trim_text(true);
    let mut buffer = Vec::new();
    let mut current = false;
    loop {
        match reader.read_event_into(&mut buffer) {
            Ok(Event::Start(event)) => current = tag(event.name().as_ref())? == wanted,
            Ok(Event::End(_)) => current = false,
            Ok(Event::Text(text)) if current => return text.unescape().map(|text| Some(text.into_owned())).map_err(|error| SonosError::Xml(error.to_string())),
            Ok(Event::Eof) => return Ok(None),
            Err(error) => return Err(SonosError::Xml(error.to_string())),
            _ => {},
        }
        buffer.clear();
    }
}
fn tag(bytes: &[u8]) -> Result<&str, SonosError> { std::str::from_utf8(bytes).map_err(|error| SonosError::Xml(error.to_string())) }
fn attribute(event: &quick_xml::events::BytesStart<'_>, wanted: &str) -> Result<Option<String>, SonosError> {
    for attribute in event.attributes() {
        let attribute = attribute.map_err(|error| SonosError::Xml(error.to_string()))?;
        if tag(attribute.key.as_ref())? == wanted { return attribute.unescape_value().map(|value| Some(value.into_owned())).map_err(|error| SonosError::Xml(error.to_string())); }
    }
    Ok(None)
}
