# ADR 0009: Route Sonos speaker feature controls through the same adapter layer

**Status:** Accepted (2026-08-14)

## Context and decision

After core synchronization was stable, users requested direct Sonos speaker settings.
Implementing each feature directly in UI code would duplicate protocol handling and
hardcode setting behavior.

Speaker controls are implemented through the `sonos` adapter and exposed as
runtime operations in `src-tauri` for settings actions. Commands include reading
and writing loudness, status light, night mode, speech enhancement, and tone-like
attributes where supported.

## Consequences

- protocol safety and local URL validation remain in adapter land,
- unsupported settings are reported clearly with generic user-facing errors,
- controls are optional from a synchronization perspective, so no policy coupling
  exists with the main volume state machine.

Tray controls are loaded at startup, before any menu interaction, and refreshed
on connection transitions and tray clicks. Reads run asynchronously so a slow
speaker does not block the UI. The menu shows supported controls after the read
completes; results from superseded requests or a different selected speaker are
discarded before updating the native menu on the main thread.

Startup registration and refresh orchestration is shared with a regression test
that delivers no mouse events and verifies all supported control states. Native
menu presentation remains covered by the clean-start hardware verification row.

Settings refreshes all device data when opened/focused and when navigating to a
section. Refresh requests coalesce; responses started before an edit or while a
write is pending cannot replace the form. Speaker changes trigger an authoritative
read after writing. Tray hover and click events refresh its controls, and tray
writes trigger a read afterward. macOS can consume native left-click events, so
hover prefetch complements the startup refresh; it does not guarantee the first
menu frame contains a network response that has not arrived yet.

Legacy soundbars, including Ray, use DialogLevel for both Speech Enhancement
reads and writes. Ultra soundbars use SpeechEnhanceEnabled, with DialogLevel
reserved for intensity. A successful zero from the wrong EQ type is not a reliable
capability signal. Writes read back the same variant and report failure if the
speaker does not confirm the requested state. Malformed reads remain unavailable.

RenderingControl GENA callbacks also accept settings-only LastChange payloads.
They invalidate speaker-control state independently of the volume/mute
synchronizer. The current runtime generation emits a Tauri event and refreshes
the tray; the frontend coalesces bursts for 150 ms, re-reads authoritative settings,
and patches only speaker controls to preserve form edits. Polling fallback checks
speaker settings every five seconds while subscriptions are degraded. Status
lights and devices that omit these events still refresh on focus/navigation.

## Capability evidence and recovery

Each read returns per-control availability independently of the value: supported,
unsupported, or temporarily unavailable. A valid false/zero is still supported.
The adapter probes only reads, concurrently, for the six exposed sound/light/tone
controls. It uses the advertised DeviceProperties control URL instead of assuming
a path. An absent service or explicit UPnP Invalid Action / Optional Action Not
Implemented fault establishes unsupported; invalid arguments, authorization errors,
timeouts, malformed values and generic failures remain unavailable and retryable.
SOAP fault codes are retained even when sent with HTTP 500.

Model metadata selects the authoritative Speech Enhancement variant. Service
advertisements alone do not imply support for every EQ type. Events invalidate
reads; they do not declare support or mark omitted controls unsupported. All
results belong to the currently resolved speaker, with no persistent negative
capability cache. Focus, navigation and push refreshes can restore controls after
recovery. Settings labels distinguish unsupported from temporarily unavailable.

This is conservative detection for the controls exposed by this app, not an
exhaustive product feature catalog. Firmware may acknowledge an unused EQ type;
known Speech Enhancement variants are handled explicitly, and unfamiliar models
still require physical validation. A read proves a value can be queried, not that
every future write will succeed. Balance and automatic TV-input capability
detection are outside this change.

Programmable HTTP mocks cover service discovery, explicit SOAP faults, false/zero
values, timeouts and recovery, malformed values, independent controls, model
variants, speaker changes, and partial GENA notifications followed by fresh reads.
Frontend tests exercise the shared first-render/push control updater with mock
views, including unavailable-to-supported recovery and preserving an edited tone
control. Native event delivery still requires the manual matrix checks.

## Gesture ownership and event delivery

Separate user commits from device observations. Hold slider drafts through pointer
or keyboard gestures and serialize released speaker writes. Block refresh/render
while editing or writing, and invalidate older reads on each gesture. Programmatic
readback changes properties only: it must never feed the user-write queue. This
prevents loops without guessing which controller produced an anonymous GENA event.
Runtime telemetry is pushed immediately; stale fallback reads cannot overwrite a
newer pushed snapshot. Fixed deadlines provide backup reads even when other events
keep arriving. Optional settings without events remain refreshed periodically.
