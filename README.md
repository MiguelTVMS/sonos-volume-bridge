# Sonos Volume Bridge

Control the volume of a Sonos speaker with the volume controls you already use
on your computer.

Sonos Volume Bridge is a small background app for Windows and macOS. It keeps a
Sonos speaker and your chosen computer audio output in step—without requiring a
cloud account, Home Assistant, or a separate remote control.

## Why use it?

If you use a Sonos speaker while working at your computer, changing the volume
usually means reaching for the Sonos app or the speaker itself. Sonos Volume
Bridge connects that speaker to your computer's normal volume controls, so
keyboard volume keys, system controls, and supported audio-device controls can
adjust it for you.

The app lives quietly in the menu bar on macOS or the system tray on Windows.
Open it when you want to change a setting; otherwise, it stays out of the way.

## What it can do

- **Keep volume in sync.** Change the volume on your computer and the selected
  Sonos speaker follows.
- **Synchronize in both directions.** Optionally, changes made on the Sonos
  speaker can update your computer volume too.
- **Follow your current audio output.** Let the app follow the system output as
  you switch devices, or keep it attached to one specific output.
- **Synchronize mute.** Choose whether muting one side should mute the other.
- **Set a safe maximum.** Limit how loud the Sonos speaker is allowed to become,
  even when the computer volume is turned all the way up.
- **Choose how volume changes feel.** Use a direct match, a gentler curve with
  more control at low volumes, or scale the full computer range to your chosen
  maximum.
- **Start automatically.** Run the bridge when you sign in so it is ready when
  you need it.
- **Recover from interruptions.** The app reconnects when a speaker, audio
  output, or network connection becomes available again.
- **Show clear status information.** See the selected speaker, current volumes,
  connection state, and helpful diagnostics from the settings window.

## What you need

- A Windows or macOS computer
- A Sonos speaker on the same local network as the computer
- A computer audio output whose volume can be changed by software

Sonos Volume Bridge controls volume and optional mute state only. It does not
play, stream, capture, redirect, or modify your audio, and it is not intended to
replace the Sonos app as a full speaker controller.

## Getting started

1. Download the package for your computer from the
   [latest release](https://github.com/MiguelTVMS/sonos-volume-bridge/releases/latest).
2. Install and open Sonos Volume Bridge.
3. Choose the Sonos speaker you want to control.
4. Choose whether to follow the computer's current audio output or a specific
   output.
5. Adjust mute synchronization, two-way synchronization, volume feel, and the
   maximum speaker volume to suit you.

Your settings are saved automatically. After setup, you can close the settings
window and leave the app running from the menu bar or system tray.

> [!IMPORTANT]
> macOS downloads are signed with Developer ID and notarized by Apple. The
> Windows installer is not yet digitally signed, so Windows SmartScreen may
> show a warning even when it was downloaded from this project's official
> GitHub release page.

## Privacy

Sonos Volume Bridge works directly between your computer and Sonos speaker on
your local network. It does not require an online account or send your volume
activity to a cloud service. Internet access is not needed for everyday volume
synchronization.

## Project status

Sonos Volume Bridge is an early community project. Windows and macOS packages
are available, but hardware combinations vary, so feedback about your computer,
audio output, and Sonos setup is welcome. Please report problems through
[GitHub Issues](https://github.com/MiguelTVMS/sonos-volume-bridge/issues).

## For contributors

Want to help improve the app? See the [development guide](docs/development.md),
[architecture overview](docs/architecture.md), and
[release guide](docs/release.md). The project is available under the
[MIT License](LICENSE).

## Sonos trademark and independence notice

Sonos Volume Bridge is an independent, community-developed project. It is not
affiliated with, sponsored by, endorsed by, or supported by Sonos. This project
contains no Sonos source code. “Sonos” and related product names are trademarks
of their respective owners and are used only to identify compatibility with
Sonos products.
