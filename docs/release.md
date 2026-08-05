# Release

## Versioning

Use semantic versioning. A release is created only after `develop` passes the
full CI matrix and the supported Windows and macOS manual audio probes have been
tested against a real output device and selected Sonos speaker.

## Windows packaging

Tauri produces a per-user installer suitable for signing. Configure the signing
certificate only through CI secrets; never commit certificates, private keys, or
passwords. Verify install, autostart, tray behavior, upgrade, and uninstall in a
non-administrator Windows account.

## macOS packaging

Build on Apple Silicon at minimum. Use a Developer ID certificate and notarize
the application through CI secrets. Verify menu-bar behavior, no Dock icon in
normal background operation, autostart, wake/reconnect, and uninstall.

## Release checklist

- Rust, frontend, audit, Windows, and macOS CI checks pass.
- The Sonos and Windows/macOS manual probes pass.
- Diagnostic export contains no serial numbers, host paths, full XML, or secrets.
- Default safety cap and mapping have been reviewed.
- README, architecture, protocol, development, and release notes are current.
- Run the manual [hardware verification matrix](verification-matrix.md) and
  attach the redacted results to the release issue.
- Run the `Release candidate` workflow and retain its Windows and macOS bundles
  for the manual verification period. Signing and notarization are intentionally
  separate protected CI steps; do not add credentials to this repository.
