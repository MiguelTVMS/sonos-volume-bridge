//! Manual macOS-only Core Audio probe.
//!
//! Run with `cargo run -p sonos-volume-bridge-platform-audio --example macos_audio_probe`.

#[cfg(target_os = "macos")]
#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    use sonos_volume_bridge_platform_audio::{
        AudioDeviceSelection, SystemAudioController,
        macos::{MacosAudioController, list_output_devices},
    };
    let arguments = std::env::args().collect::<Vec<_>>();
    if arguments.iter().any(|argument| argument == "--list") {
        for device in list_output_devices()? {
            println!(
                "{}: {} (writable volume: {})",
                device.id, device.name, device.writable_volume
            );
        }
        return Ok(());
    }
    let selection = arguments
        .windows(2)
        .find(|arguments| arguments[0] == "--device")
        .map_or(AudioDeviceSelection::FollowDefault, |arguments| {
            AudioDeviceSelection::Fixed {
                device_id: arguments[1].clone(),
            }
        });
    let controller = MacosAudioController::start(selection, 1)?;
    println!(
        "Current output state: {:?}",
        controller.current_state().await?
    );
    if arguments.iter().any(|argument| argument == "--idle-check") {
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        println!(
            "Output state after idle: {:?}",
            controller.current_state().await?
        );
        return Ok(());
    }
    let mut events = controller.subscribe();
    println!("Listening for Core Audio events; press Ctrl+C to stop.");
    loop {
        println!("{:#?}", events.recv().await?);
    }
}

#[cfg(not(target_os = "macos"))]
fn main() {
    eprintln!("macos_audio_probe must be run on macOS.");
}
