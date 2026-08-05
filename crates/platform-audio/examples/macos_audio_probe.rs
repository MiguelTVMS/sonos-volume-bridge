//! Manual macOS-only Core Audio probe.
//!
//! Run with `cargo run -p sonos-volume-bridge-platform-audio --example macos_audio_probe`.

#[cfg(target_os = "macos")]
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    use sonos_volume_bridge_platform_audio::{macos::MacosAudioController, AudioDeviceSelection, SystemAudioController};
    let controller = MacosAudioController::start(AudioDeviceSelection::FollowDefault, 1)?;
    println!("Current output state: {:?}", controller.current_state().await?);
    let mut events = controller.subscribe();
    println!("Listening for Core Audio events; press Ctrl+C to stop.");
    loop { println!("{:#?}", events.recv().await?); }
}

#[cfg(not(target_os = "macos"))]
fn main() { eprintln!("macos_audio_probe must be run on macOS."); }
