//! Safe, local-network-only primitives for Sonos UPnP control.

mod client;
mod discovery;
mod error;
mod event;
mod gena;
mod model;
mod xml;

pub use client::{SonosClient, SonosClientBuilder};
pub use discovery::{discover, parse_ssdp_response};
pub use error::SonosError;
pub use event::{parse_last_change, EventDeduplicator, GenaEvent, GenaState};
pub use gena::{CallbackListener, GenaClient, Subscription};
pub use model::{DiscoveredDevice, RenderingControlService, SonosDevice, SonosId};
pub use xml::parse_device_description;
