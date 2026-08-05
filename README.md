# SonosVolumeBridge

SonosVolumeBridge is a lightweight Windows and macOS menu-bar/tray application
that keeps a selected Sonos speaker's volume and mute state synchronized with
the system output volume. Sonos is always the source of truth: computers never
communicate with one another directly.

## Project status

Phase 2 is in progress: the pure Rust domain model, synchronization policy, and
local Sonos discovery/RenderingControl client have unit and integration tests.
Native audio control, Tauri UI, subscription callback listener, packaging, and
release artifacts are deliberately not yet present.

## Safety and privacy

The application will not capture, proxy, process, or redirect audio. It will
use no cloud service and requires neither Home Assistant nor internet access for
synchronization. A configurable maximum Sonos volume cap is enforced before a
local volume change is sent to Sonos.

## Architecture

The workspace separates pure policy in `crates/domain` from the
adapter-agnostic state machine in `crates/synchronization`. See
[the architecture](docs/architecture.md) and
[the state machine](docs/state-machine.md).

Sonos local interfaces are not a formally supported public control API and may
change with firmware.

## Development

See [development instructions](docs/development.md). The project is licensed
under the MIT License.
