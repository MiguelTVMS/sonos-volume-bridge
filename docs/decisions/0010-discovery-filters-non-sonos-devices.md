# ADR 0010: Filter discovery to verified Sonos speakers

**Status:** Accepted (2026-08-15)

## Context

Users reported that non-Sonos media devices were appearing in the speaker selection list.
Discovery already required rendering control URLs, but that is not unique to Sonos.

## Decision

For both discovery and runtime device resolution:

- SSDP discovery uses `urn:schemas-upnp-org:device:ZonePlayer:1` as the search target.
- Candidates are accepted only when their unique id starts with `uuid:RINCON_`.
- Non-matching devices are ignored before they reach UI selection.

## Consequences

- `discover_sonos` now returns only Sonos speakers.
- Runtime selection cannot bind to a cached non-Sonos description URL.
- Any non-Sonos renderer that responds with rendering control details remains reachable on the network but is now excluded from speaker selection.
