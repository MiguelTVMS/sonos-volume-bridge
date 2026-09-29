# 0018: Rename the product while preserving installed identity

## Decision

The product, Cargo packages, executable and Debian package become Speaker Volume Bridge. Store records, bundle/package identities, serialized configuration, settings directories, notification identity and startup registration keys retain their existing values. The GitHub repository is renamed in place; its previous name must remain unused so redirects continue working.

Process inspection lives in the desktop shell (`legacy_app`), using the public sysinfo adapter. Domain and synchronization crates receive no platform dependencies. The monitor runs before initial synchronization, every five seconds, when Settings gains focus and on a manual recheck. It compares current-user executable paths, resolves compatibility symlinks and excludes the current process and demo paths. Inspection without sufficient information is unknown, never clear.

The shell's write barrier drains active writes before entering a conflict. It rejects speaker writes, audio synchronization and scheduled Night Mode writes while paused. Runtime cancellation interrupts initialization as well as active sessions, dropping queued session work. Recovery creates a new runtime from the latest persisted configuration and requests normal schedule reconciliation. Explicitly stopped synchronization stays stopped. Unknown inspection after a confirmed conflict retains the pause.

The Settings alert is authoritative for confirmed conflicts only. Initial unknown inspection is a neutral Diagnostics status, without a yellow banner or notification. Unknown inspection after a confirmed conflict retains the warning and pause, with wording that explicitly describes the previous detection. A conflict episode produces one local native notification if permission is already available; the monitor never prompts for permission. Notification activation opens Settings. The upgrade action opens a fixed project-guide address in the default browser through the desktop shell. No server, telemetry, automatic process termination or automatic uninstall is introduced.

The single-instance plugin is patched to use a new process namespace while retaining the bundle ID. Linux uses the plugin's explicit D-Bus namespace option. Demo builds have their own namespace and bypass inspection. This permits the renamed Settings window to open alongside the old app while preventing two renamed instances.

## Upgrade and validation boundaries

Windows keeps the existing installer registry keys and MSIX identities and migrates executable targets. The NSIS installer moves the former default installation folder to the new product name before copying files, while retaining custom paths and settings. It refuses a pre-existing destination and aborts if the directory cannot be moved. Startup, owned shortcuts and existing toast activation registration follow the new path. Debian metadata replaces/conflicts with the old package and installs an old-command compatibility symlink. macOS retains bundle identity but changes the bundle filename; users must quit and replace the direct-download bundle without deleting application data.

The Mac App Store sandbox can restrict process information. No private API or broad entitlement is added. Actual signed sandbox behavior is a release gate: local unit tests cannot establish sandbox visibility or native notification delivery. See [the rebrand checklist](../rebrand-upgrade.md). If sandbox enumeration cannot reliably establish absence, ship the unknown/manual guidance state rather than claiming a successful check.
