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

Ordinary settings saves do not register or unregister login items. Call the
platform login service only when `startAtLogin` differs from the active
configuration. Validate first, apply an explicitly requested login change, then
persist and apply the configuration. A rejected login change leaves the saved
and active configuration unchanged. A later disk-write failure is still reported;
login-service changes and filesystem writes are not one atomic transaction.

Regression tests cover a missing configuration file with an unavailable login
service, unchanged enabled login settings, rejected explicit changes, and both
successful login transitions.

On macOS, disabling is idempotent for both `NotRegistered` and `NotFound`.
Service Management can report `NotFound` for a fresh or replaced app bundle and
return `SMAppServiceErrorDomain` code 1 if asked to unregister that absent item.
After an unregister failure, reread status to allow concurrent removal by System
Settings or macOS cleanup. Only these absent states permit saving the disabled
preference; registered, approval-required and unknown states retain errors.
Enabling still requires registration unless the service is already enabled.
Removal failures show a readable description and the System Settings recovery
path, including the former app name, instead of an Objective-C debug dump.

Adapter regressions cover absent entries, concurrent removal, denied removal,
revoked approval and enabling. Persistence coverage verifies that a failed
removal keeps the saved preference enabled. Native temporary-bundle checks
reproduced the absent-item error; this does not establish the exact prior state
of an affected installed app.
