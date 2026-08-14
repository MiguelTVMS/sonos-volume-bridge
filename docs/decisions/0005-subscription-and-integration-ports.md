# ADR 0005: Isolate GENA transport from synchronization orchestration

**Status:** Accepted (2026-08-05)

## Decision

GENA transport remains in the `sonos` crate. It performs SUBSCRIBE, renewal,
and UNSUBSCRIBE through local HTTP and exposes a callback listener bound to an
explicit local socket address. The listener uses an unguessable path, accepts
NOTIFY only from the selected Sonos IP, and validates both callback path and
subscription ID before parsing bounded event XML.

The `integration` crate defines async Sonos and local-audio ports. It drives the
existing pure synchronizer, applies only confirmed Sonos state locally, and
offers bounded coalescing of volume events. Callers mark mute changes so they
bypass debounce. Subscription health determines the polling interval.

## Consequences

No UI, native audio, or HTTP implementation leaks into the state machine.
The callback listener must be supplied an interface-address reachable by Sonos;
interface selection after network changes is an application composition concern.
