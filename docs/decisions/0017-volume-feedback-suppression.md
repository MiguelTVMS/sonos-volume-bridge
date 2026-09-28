# ADR 0017: Retain value-based audio echo suppression

**Status:** Accepted

## Decision

macOS and Linux retain up to 64 expected local states for 500 ms per write,
measured using a monotonic clock. Matching callbacks do not consume or extend
an expectation. Overlapping volume and mute writes retain their separate states.
Nonmatching callbacks are forwarded immediately. macOS retains its configured
volume tolerance and includes intermediate averages for per-channel writes;
Linux uses exact normalized states. macOS clears expectations on reattachment.
Windows continues to identify every application callback by its event-context GUID.

The shell reads local state before applying a Sonos observation and skips each
unchanged property. The synchronization policy still accepts Sonos confirmations
from events, explicit reads, and polling in two-way mode. One-way mode never
applies them locally. No global listening pause or Sonos origin inference is added.

## Consequences and validation

Duplicate, overlapping, and intermediate callbacks cannot consume the protection
needed by later matching callbacks. Unrelated changes remain responsive. Without
native origin metadata a genuine change matching a recent expected value is
indistinguishable from an echo until expiry; retaining bounded values is preferable
to dropping all changes during a blackout. Very late callbacks and device-specific
quantization beyond tolerance still require native-device investigation.

Regression tests cover duplicate and overlapping volume/mute callbacks, tolerance,
expiry, independent deadlines, bounded history, per-channel averages, Windows
contexts, unchanged local applications, and all mapping/direction/mute policies.
Native Windows and Linux tests run on both supported architectures; macOS tests
run locally and in CI. Hardware validation remains separate from simulated tests.
