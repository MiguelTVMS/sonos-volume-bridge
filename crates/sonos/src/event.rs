use crate::SonosError;
use quick_xml::{Reader, escape::unescape, events::Event};
use sonos_volume_bridge_domain::{MuteState, SonosVolume};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GenaState {
    pub volume: SonosVolume,
    pub muted: MuteState,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GenaEvent {
    pub sequence: Option<u32>,
    pub state: GenaState,
}

/// Drops replayed GENA notifications while preserving a changed state.
#[derive(Default, Debug)]
pub struct EventDeduplicator {
    last: Option<GenaEvent>,
}

impl EventDeduplicator {
    pub fn accept(&mut self, event: GenaEvent) -> bool {
        if self.last.as_ref() == Some(&event) {
            return false;
        }
        self.last = Some(event);
        true
    }
}

pub fn parse_last_change(body: &[u8], sequence: Option<u32>) -> Result<GenaEvent, SonosError> {
    let outer =
        first_text(body, "LastChange")?.ok_or(SonosError::MissingSoapValue("LastChange"))?;
    let mut volume = None;
    let mut muted = None;
    let mut reader = Reader::from_str(&outer);
    reader.config_mut().trim_text(true);
    let mut buffer = Vec::new();
    loop {
        match reader.read_event_into(&mut buffer) {
            Ok(Event::Empty(event) | Event::Start(event)) => {
                let tag = event.name().as_ref().to_owned();
                if tag == "Volume"
                    && attribute(&event, "channel")?
                        .as_deref()
                        .is_none_or(|channel| channel == "Master")
                {
                    volume = attribute(&event, "val")?;
                }
                if tag == "Mute"
                    && attribute(&event, "channel")?
                        .as_deref()
                        .is_none_or(|channel| channel == "Master")
                {
                    muted = attribute(&event, "val")?;
                }
            }
            Ok(Event::Eof) => break,
            Err(error) => return Err(SonosError::Xml(error.to_string())),
            _ => {}
        }
        buffer.clear();
    }
    let volume = volume
        .ok_or(SonosError::MissingSoapValue("Volume"))?
        .parse::<u8>()
        .map_err(|error| SonosError::InvalidVolume(error.to_string()))?;
    let muted = match muted.ok_or(SonosError::MissingSoapValue("Mute"))?.as_str() {
        "0" => false,
        "1" => true,
        value => return Err(SonosError::InvalidMute(value.to_owned())),
    };
    Ok(GenaEvent {
        sequence,
        state: GenaState {
            volume: SonosVolume::new(volume)
                .map_err(|error| SonosError::InvalidVolume(error.to_string()))?,
            muted: MuteState(muted),
        },
    })
}

pub(crate) fn first_text(xml: &[u8], wanted: &str) -> Result<Option<String>, SonosError> {
    let mut reader = Reader::from_reader(xml);
    reader.config_mut().trim_text(true);
    let mut buffer = Vec::new();
    let mut current = false;
    let mut value = String::new();
    loop {
        match reader.read_event_into(&mut buffer) {
            Ok(Event::Start(event)) => {
                current = event.name().as_ref() == wanted;
            }
            Ok(Event::End(event)) if current && event.name().as_ref() == wanted => {
                return unescape(&value)
                    .map(|text| Some(text.into_owned()))
                    .map_err(|error| SonosError::Xml(error.to_string()));
            }
            Ok(Event::End(_)) => current = false,
            Ok(Event::Text(text)) if current => {
                value.push_str(&text);
            }
            Ok(Event::GeneralRef(reference)) if current => {
                value.push('&');
                value.push_str(&reference);
                value.push(';');
            }
            Ok(Event::Eof) => return Ok(None),
            Err(error) => return Err(SonosError::Xml(error.to_string())),
            _ => {}
        }
        buffer.clear();
    }
}
fn attribute(
    event: &quick_xml::events::BytesStart<'_>,
    wanted: &str,
) -> Result<Option<String>, SonosError> {
    for attribute in event.attributes() {
        let attribute = attribute.map_err(|error| SonosError::Xml(error.to_string()))?;
        if attribute.key.as_ref() == wanted {
            return unescape(&attribute.value)
                .map(|value| Some(value.into_owned()))
                .map_err(|error| SonosError::Xml(error.to_string()));
        }
    }
    Ok(None)
}

/// RenderingControl notifications can contain only EQ/tone changes, with no
/// volume or mute. Keep those invalidations independent of synchronization.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RenderingControlNotification {
    pub sequence: Option<u32>,
    pub volume_state: Option<GenaEvent>,
    pub settings_changed: bool,
}

pub fn parse_rendering_control_notification(
    body: &[u8],
    sequence: Option<u32>,
) -> Result<RenderingControlNotification, SonosError> {
    let outer =
        first_text(body, "LastChange")?.ok_or(SonosError::MissingSoapValue("LastChange"))?;
    let mut reader = Reader::from_str(&outer);
    let mut changed = false;
    loop {
        match reader.read_event() {
            Ok(Event::Empty(tag) | Event::Start(tag)) => {
                if matches!(
                    tag.local_name().as_ref(),
                    "EQ" | "DialogLevel"
                        | "SpeechEnhanceEnabled"
                        | "NightMode"
                        | "Loudness"
                        | "Bass"
                        | "Treble"
                ) {
                    changed = true;
                }
            }
            Ok(Event::Eof) => break,
            Err(error) => return Err(SonosError::Xml(error.to_string())),
            _ => {}
        }
    }
    let volume_state = parse_last_change(body, sequence).ok();
    if volume_state.is_none() && !changed {
        return Err(SonosError::MissingSoapValue("RenderingControl state"));
    }
    Ok(RenderingControlNotification {
        sequence,
        volume_state,
        settings_changed: changed,
    })
}

#[cfg(test)]
mod notification_tests {
    use super::*;

    #[test]
    fn eq_only_events_invalidate_settings_without_inventing_volume() {
        for tag in [
            "EQ",
            "DialogLevel",
            "SpeechEnhanceEnabled",
            "NightMode",
            "Loudness",
            "Bass",
            "Treble",
        ] {
            let body = format!(
                "<LastChange>&lt;Event&gt;&lt;InstanceID val=\"0\"&gt;&lt;{tag} val=\"1\"/&gt;&lt;/InstanceID&gt;&lt;/Event&gt;</LastChange>"
            );
            let notification =
                parse_rendering_control_notification(body.as_bytes(), Some(4)).unwrap();
            assert!(notification.settings_changed);
            assert!(notification.volume_state.is_none());
            assert_eq!(notification.sequence, Some(4));
        }
    }

    #[test]
    fn volume_events_keep_existing_synchronization_state() {
        let body = b"<LastChange>&lt;Event&gt;&lt;Volume channel=\"Master\" val=\"24\"/&gt;&lt;Mute channel=\"Master\" val=\"0\"/&gt;&lt;/Event&gt;</LastChange>";
        let notification = parse_rendering_control_notification(body, Some(5)).unwrap();
        assert!(!notification.settings_changed);
        assert_eq!(notification.volume_state.unwrap().state.volume.get(), 24);
    }

    #[test]
    fn invalid_or_unrelated_events_do_not_trigger_refreshes() {
        assert!(parse_rendering_control_notification(b"<broken", None).is_err());
        assert!(
            parse_rendering_control_notification(b"<LastChange>&lt;Event/&gt;</LastChange>", None)
                .is_err()
        );
    }
}
