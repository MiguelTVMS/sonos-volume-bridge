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
