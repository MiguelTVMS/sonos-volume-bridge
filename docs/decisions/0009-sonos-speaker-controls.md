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
