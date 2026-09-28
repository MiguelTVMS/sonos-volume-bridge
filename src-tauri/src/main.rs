#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    speaker_volume_bridge::run();
}
