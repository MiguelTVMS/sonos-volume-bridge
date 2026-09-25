//! Safe, local-network-only primitives for Sonos UPnP control.

mod client;
mod discovery;
mod error;
mod event;
mod gena;
mod model;
mod xml;

pub use client::{SonosClient, SonosClientBuilder, SpeakerSettings};
pub use discovery::{discover, parse_ssdp_response};
pub use error::SonosError;
pub use event::{
    EventDeduplicator, GenaEvent, GenaState, RenderingControlNotification, parse_last_change,
    parse_rendering_control_notification,
};
pub use gena::{CallbackListener, GenaClient, Subscription};
pub use model::{
    AvTransportService, DiscoveredDevice, RenderingControlService, SonosDevice, SonosId,
};
pub use xml::parse_device_description;
