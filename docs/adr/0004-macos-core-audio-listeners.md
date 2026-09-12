# ADR 0004: Use focused Core Audio FFI with expected-write suppression

**Status:** Accepted (2026-08-05)

## Decision

The macOS adapter uses a minimal, documented FFI surface for the Core Audio
`AudioObject*` APIs and links only the system CoreAudio framework. It listens to
the default output device, output volume, and mute properties using
`AudioObjectAddPropertyListener`.

Before a bridge-originated write, the worker records an `ExpectedLocalWrite`
with a monotonic generation, short expiry, and caller-supplied volume tolerance.
The callback matches and consumes only the expected state; all other callbacks
are reported as user-originated. This avoids a global boolean or timing-only
suppression scheme.

When a mutable master volume is unavailable, the adapter discovers output
buffers and applies volume consistently to writable channel elements. An output
device with neither a writable master nor writable output channels returns
`UnsupportedDevice`.

## Consequences

Listeners are detached before replacement and while shutting down. Callback
functions only send bounded events or reattach commands. Real-device testing is
required because Core Audio device properties vary across drivers and virtual
audio devices.
