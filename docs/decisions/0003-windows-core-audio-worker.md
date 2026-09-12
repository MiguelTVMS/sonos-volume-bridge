# ADR 0003: Confine Windows Core Audio to a worker thread

**Status:** Accepted (2026-08-05)

## Decision

The Windows adapter uses the maintained `windows` and `windows-core` crates.
Core Audio COM interfaces are created, registered, used, and released on one
dedicated MTA worker thread. The public controller is a thread-safe command
handle; it contains no COM interface pointers.

The adapter attaches a stable application event-context GUID to `SetMute` and
`SetMasterVolumeLevelScalar`. `IAudioEndpointVolumeCallback` compares incoming
notification contexts with that GUID and labels bridge writes as application
originated. It sends events to a bounded Tokio broadcast channel without
waiting. `IMMNotificationClient` forwards default multimedia render changes
through a bounded standard-library command queue; the worker then unregisters
the old endpoint callback and attaches the new endpoint.

## Consequences

This prevents COM apartment violations and avoids blocking Core Audio callback
threads. Fixed output-device IDs are resolved through `IMMDeviceEnumerator`;
follow-default mode reattaches only on `eRender`/`eMultimedia` changes.

The adapter has no Tauri, Sonos, or synchronization dependency. Hardware and
driver behavior still needs validation on real Windows endpoints.
