# Architecture

The workspace keeps policy, protocol, platform, integration, and application
concerns separate.

```text
Native audio adapter ──events──> Synchronizer ──effects──> Sonos adapter
       ^                                |                         |
       └──────── apply confirmed state ─┴──── Sonos confirmed ────┘
```

The `domain` crate owns values, mappings, confirmed state, pending intent, and
expected-write suppression. The `synchronization` crate accepts normalized
events and emits side effects. The `integration` coordinator serializes those
effects, coalesces volume commands through a bounded channel, and never lets
computers communicate with one another directly.

## Adapters

The `sonos` crate owns local SSDP discovery, private-address validation,
device-description parsing, RenderingControl SOAP requests, and GENA
subscription/callback support. It has no Tauri or native-audio dependency.

The `platform-audio` crate defines the shared controller interface. Its Windows
adapter keeps Core Audio COM objects on a dedicated worker thread, forwards
callbacks through a bounded broadcast channel, and follows default multimedia
render endpoint replacement. Its macOS adapter observes default-output, mute,
and volume properties, suppresses expected local writes deterministically, and
uses output-channel controls when master volume is unavailable.

## Runtime and application shell

`src-tauri` is a thin composition root. It owns the tray, hidden settings
window, versioned per-user configuration, autostart, and restricted commands.
The TypeScript frontend has no direct filesystem or network permission; it
receives snapshots and sends validated settings through backend commands.

Each configuration change cancels the previous runtime generation before
starting another. The runtime resolves the selected UDN through its cached
description URL or bounded SSDP discovery, creates the selected local adapter,
and binds a random-path GENA callback listener to the local interface used to
reach the speaker. It renews subscriptions at 80 percent of their lifetime,
uses bounded reconnect backoff, and switches to polling only while event
delivery is unavailable or stale. Runtime failures become redacted status and
log records instead of exposing protocol payloads to the frontend.
