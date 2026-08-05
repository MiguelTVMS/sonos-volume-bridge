//! Manual Windows-only Core Audio probe.
//!
//! Run with `cargo run -p sonos-volume-bridge-platform-audio --example windows_audio_probe`.

#[cfg(windows)]
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    use sonos_volume_bridge_platform_audio::{windows::WindowsAudioController, AudioDeviceSelection, SystemAudioController};

    let controller = WindowsAudioController::start(AudioDeviceSelection::FollowDefault)?;
    println!("Current output state: {:?}", controller.current_state().await?);
    let mut events = controller.subscribe();
    println!("Listening for Core Audio events; press Ctrl+C to stop.");
    loop { println!("{:#?}", events.recv().await?); }
}

#[cfg(not(windows))]
fn main() { eprintln!("windows_audio_probe must be run on Windows."); }
