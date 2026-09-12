# ADR 0008: Make synchronization direction configurable

**Status:** Accepted (2026-08-06)

## Context and decision

Some users want the local computer output to be the source of truth, while others
want Sonos to be the source. A fixed one-way design blocked both workflows.

The configuration now includes `twoWaySynchronization` (default true). In two-way
mode, confirmed Sonos values are applied back to local output. In one-way mode,
Sonos is observed for health and state reads, while local user actions continue to
be sent to Sonos.

## Consequences

- `twoWaySynchronization` influences startup behavior and the confirmation path,
- state machine behavior remains the same and still accepts Sonos confirmations,
- migration of user preference is safe because the field defaults to true and is
  backward compatible with existing configs.
