# ADR 0006: Keep Tauri as a thin composition and settings shell

**Status:** Accepted (2026-08-05)

## Decision

Tauri 2 owns application lifecycle, a hidden-by-default settings window, tray
menu, autostart integration, and narrowly scoped settings commands. Domain,
protocol, platform-audio, and integration crates are not called from command
handlers; the Tauri state only owns validated configuration and presentational
status.

Configuration is versioned JSON stored in the per-user application directory.
Writes use a sibling temporary file followed by rename. Invalid files are moved
to a `.corrupt` backup and replaced with safe defaults. The frontend cannot use
arbitrary filesystem, shell, or network APIs; it invokes only explicit Rust
commands.

## Consequences

The settings UI can evolve without changing synchronization policy. The runtime
composition remains an application service started from the Tauri setup path,
not a frontend command. Diagnostic export is intentionally sanitized and is
limited to the explicit diagnostics command.
