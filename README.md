# SonosVolumeBridge

SonosVolumeBridge is a lightweight Windows and macOS menu-bar/tray application
that keeps a selected Sonos speaker's volume and, optionally, mute state in
step with one system audio output. Sonos is the confirmed source of truth:
computers never communicate with one another directly.

## Project status

Version 0.1.1 is published with unsigned macOS and Windows packages. The local
Sonos client, Windows and macOS audio adapters, synchronization runtime,
settings shell, CI, and GitHub Release workflow are implemented. Physical
Windows/macOS and Sonos verification remains the release gate; see the
[hardware verification matrix](docs/verification-matrix.md).

## What it does

- Discovers local-network Sonos speakers and stores selection by stable UDN.
- Follows the system default output or a selected fixed output device.
- Applies a configurable volume mapping and maximum Sonos-volume cap.
- Debounces/coalesces local volume changes; mute changes bypass the debounce.
- Receives Sonos GENA events, renews subscriptions, and falls back to
  conservative polling when events are unavailable.
- Uses a single background menu-bar/tray process with a hidden-by-default
  settings window, automatic configuration saving, diagnostics, and optional
  start at login.

The app is not a general Sonos controller and does not stream, capture, proxy,
or alter audio.

## Safety and privacy

The application uses only local-network Sonos APIs for synchronization. It does
not require a cloud service, Home Assistant, or internet access. A configurable
maximum Sonos volume cap is enforced before a local volume change is sent to
Sonos. Device descriptions and callback sources are constrained to the selected
local peer to limit unsafe URL and event input.

## Architecture

The workspace separates pure policy in `crates/domain` from the
adapter-agnostic state machine in `crates/synchronization`. See
[the architecture](docs/architecture.md) and
[the state machine](docs/state-machine.md).

Sonos local interfaces are not a formally supported public control API and may
change with firmware.

## Development

See [development instructions](docs/development.md) and [release
instructions](docs/release.md). The project is licensed under the MIT License.
