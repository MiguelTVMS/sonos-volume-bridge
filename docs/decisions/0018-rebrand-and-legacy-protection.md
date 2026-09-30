# 0018: Rename the product while preserving installed identity

## Decision

The product, Cargo packages, executable and Debian package become Speaker Volume Bridge. Store records, bundle/package identities, serialized configuration, settings directories, notification identity and startup registration keys retain their existing values. The GitHub repository is renamed in place; its previous name must remain unused so redirects continue working.

Upgrade cleanup is a documented user action. The README links prominently to
[the complete removal guide](../removing-old-app.md), which covers quitting the
old app, removing automatic startup entries, uninstalling, and optionally erasing
shared settings before reinstalling.

The original process monitor and automatic conflict pause are retired. Maintaining
process visibility across desktop platforms and the Store sandbox is excessive
for this one-time rename. Remove its worker, global write barrier, conflict epoch,
notifications, commands, UI warning, diagnostics field, and `sysinfo` dependency.
Startup invokes the runtime directly. Normal shutdown/cancellation, speaker
selection serialization and Night Mode enforcement remain in their existing owners.
Users must avoid running the old and renamed apps simultaneously; there is no
automatic conflict detection or pause.

The single-instance plugin is patched to use a new process namespace while retaining the bundle ID. Linux uses the plugin's explicit D-Bus namespace option. Demo builds have their own namespace. Keep this already-shipped namespace so an older renamed build and a newer renamed build still enforce one instance. It no longer supports a process detector.

## Upgrade and validation boundaries

Windows keeps the existing installer registry keys and MSIX identities and migrates executable targets. The NSIS installer moves the former default installation folder to the new product name before copying files, while retaining custom paths and settings. It refuses a pre-existing destination and aborts if the directory cannot be moved. Startup, owned shortcuts and existing toast activation registration follow the new path. Debian metadata replaces/conflicts with the old package and installs an old-command compatibility symlink. macOS retains bundle identity but changes the bundle filename; users must quit and replace the direct-download bundle without deleting application data.

GitHub Release installers use only `speaker-volume-bridge-*` filenames. Legacy
installed identities and the Debian command compatibility symlink do not require
duplicate release assets using the former product name.

Regression coverage exercises manual speaker commands and scheduling without a
process check, and Settings navigation/saves without a legacy service. Native
startup, single-instance behavior and the documented uninstall/reinstall flow
remain manual release checks. Wiki cleanup instructions apply to the current GA
release; removing detection in source is not evidence that it has shipped.
