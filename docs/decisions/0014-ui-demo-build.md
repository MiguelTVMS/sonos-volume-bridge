# ADR 0014: Native desktop demo with a simulated Sonos speaker

**Status:** Accepted (2026-09-26), revised after native schedule testing.

## Decision

The default-off `ui-demo` feature requires a debug build. It runs the ordinary
application shell, commands, configuration store, tray, synchronization runtime,
Night Mode scheduler, clock/wake adapters and notifications. Only the speaker is
simulated. The frontend keeps native IPC after its demo-mode handshake; it never
loads the browser preview backend in a desktop build.

A loopback-only, stateful Sonos protocol simulator supplies device discovery,
SOAP reads/writes and GENA callbacks. The production Sonos client and normal
application orchestration operate on it. Demo discovery and resolution are
restricted to this speaker, including when saved settings contain another address.
No domain or synchronization logic depends on demo mode.

A separate `.ui-demo` app identity isolates its single-instance handling, saved
configuration and logs. A fresh demo preselects the simulated speaker. Settings
open at startup and show a demo label. Speaker state resets at process restart;
saved app settings persist and the ordinary runtime reconciles them on startup.
Local audio outputs, volume synchronization, notifications, login registration,
diagnostics and exports behave normally. Simulated sound settings affect only the
simulated speaker; the app can change the actual local output volume and mute.

The mutually exclusive `ui-windows`, `ui-macos` and `ui-ubuntu` features imply
`ui-demo` and override presentation. Window chrome and constraints remain native.
The development-only browser preview retains its lightweight in-memory backend
for layout tests; it is not the behavioral desktop demo.

## Coverage and limits

Command-routing regressions cover startup timing and verify that demo commands
reach native IPC. Native regression tests exercise real SOAP/GENA clients, the
production schedule worker, enabling a saved current period, manual-off rejection
before its first tick, enforced on, disabling, and outside-period manual control.
They run in the ordinary Rust suite and in the feature-enabled CI suite.

Native desktop verification covers Settings, tray controls, persisted startup,
local audio, clock boundaries and notifications. The simulator is not physical
speaker firmware and cannot validate acoustic output, network discovery failures,
or hardware-specific protocol quirks. See the verification matrix.
