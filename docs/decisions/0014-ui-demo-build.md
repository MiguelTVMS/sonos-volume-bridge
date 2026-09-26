# ADR 0014: Opt-in desktop UI demo builds

**Status:** Accepted (2026-09-26)

## Decision

Add the default-off Cargo feature `ui-demo` for hardware-free Settings testing.
It requires a debug build. With no flag, debug and release builds use normal
native services. The composition root selects the shell before initializing any
runtime services. The demo opens Settings immediately and offers Settings/Quit
in the tray. Its native handler exposes only mode and presentation handshakes:
no AppState, configuration store, discovery, audio, login items, scheduler,
notification adapters, or production command handlers are started. A distinct
runtime identifier separates demo single-instance handling from the normal app.

The frontend waits for the compiled-mode handshake before mounting the production
Settings renderer. A positive result loads an in-memory simulator shared with the
browser preview; a negative result retains native commands. A handshake error
shows a startup error and never silently enables simulation. Native window APIs
remain intact. A visible sidebar label identifies simulated devices. Changes
last for the window session, resetting on reload or app restart.

The mutually exclusive features `ui-windows`, `ui-macos`, and `ui-ubuntu` imply
`ui-demo` and force the corresponding frontend presentation on any host. With no
override, presentation follows the host OS. Native chrome and window constraints
remain host-specific, including fixed macOS width and vertical resizing.

## Coverage and limits

Unit tests cover default routing, delayed/failed handshakes, state copies, writes,
reset, and schedule Save. Browser tests exercise production startup and controls
under every forced presentation. Native mock IPC tests use the demo's actual
handler registration and verify that production device commands are rejected.
CI checks ordinary and feature builds.

This is a Settings UI simulator, not an audio/protocol/scheduler implementation.
Recurring boundaries, native notifications, audio playback, login registration,
diagnostic file export, and tray speaker actions require normal native testing.
Export explains that no file was written. Save, toggles, levels, volume tests,
and reset remain usable without hardware. Domain and synchronization crates
are unchanged.
