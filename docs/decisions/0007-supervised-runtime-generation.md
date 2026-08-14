# ADR 0007: Supervised runtime generation with cancellable restarts

**Status:** Accepted (2026-08-05)

## Context and decision

Runtime configuration can change while callback tasks, listeners, and timers are
already active. To avoid overlapping sessions, the runtime keeps exactly one active
supervision chain and replaces it on configuration updates.

The runtime now uses a generation token owned by `RuntimeManager` and a cancellation
watch channel. Any restart marks the previous generation as stopped, spawns a new
supervised session, and allows only matching generation snapshots to write UI state.

## Consequences

- stale audio events from older sessions cannot overwrite a newer runtime view,
- reconnects are bounded with exponential delay after session failures,
- connection setup failures cleanly trigger status transitions and retry behavior,
- all runtime setup and teardown paths remain localized to the composition root.
