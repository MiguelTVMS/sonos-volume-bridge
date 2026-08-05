//! Read-only local Sonos discovery probe.
//!
//! Run with `cargo run -p sonos-volume-bridge-sonos --example discover`.

use sonos_volume_bridge_sonos::{SonosClient, discover};
use std::time::Duration;

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let verify_write = std::env::args().any(|argument| argument == "--verify-write");
    let client = SonosClient::builder()
        .timeout(Duration::from_secs(3))
        .build()?;
    let locations = discover(Duration::from_secs(3)).await?;
    if locations.is_empty() {
        println!("No local Sonos devices were discovered.");
        return Ok(());
    }
    for location in locations {
        match client.retrieve_device(location.clone()).await {
            Ok(device) => {
                let volume = client.get_volume(&device.device).await?;
                let muted = client.get_mute(&device.device).await?;
                if verify_write {
                    client.set_volume(&device.device, volume).await?;
                }
                println!(
                    "{} ({}) at {}; volume: {}, muted: {}; write verified: {}",
                    device.device.friendly_name,
                    device.device.id.as_str(),
                    location,
                    volume.get(),
                    muted.0,
                    verify_write
                );
            }
            Err(error) => println!("Unable to read {location}: {error}"),
        }
    }
    Ok(())
}
