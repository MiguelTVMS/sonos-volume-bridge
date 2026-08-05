# Release

## Versioning

Use semantic versioning. The `version` in the root `Cargo.toml` workspace table
is the only version source; Tauri reads it through `src-tauri/Cargo.toml`.
Never set a second version in `tauri.conf.json`.

To make a release, open **Actions → Release → Run workflow** on `develop` and
choose one increment:

- `Fix` increases the patch number.
- `Minor` increases the minor number and resets the patch number.
- `Major` increases the major number and resets the minor and patch numbers.

The workflow validates `develop`, commits the version bump to `develop`, builds
both platform packages from that exact commit, then creates and pushes its
`vX.Y.Z` tag before creating or updating the GitHub Release with downloadable
assets. It never merges branches. Merge `develop` into `main` only after the
release workflow succeeds.

The release assets are explicitly unsigned until the protected signing and
notarization step replaces them. Gatekeeper and SmartScreen may warn in the
meantime.

## Windows packaging

Tauri produces a per-user installer suitable for signing. Configure the signing
certificate only through CI secrets; never commit certificates, private keys, or
passwords. Verify install, autostart, tray behavior, upgrade, and uninstall in a
non-administrator Windows account.

## macOS packaging

Build on Apple Silicon at minimum. The release workflow packages the app as a
ZIP preserving the `.app` bundle. Use a Developer ID certificate and notarize
through protected CI secrets in a later workflow. Verify menu-bar behavior, no
Dock icon in normal background operation, autostart, wake/reconnect, and
uninstall.

## Release checklist

- Rust, frontend, audit, Windows, and macOS CI checks pass.
- The Sonos and Windows/macOS manual probes pass.
- Diagnostic export contains no serial numbers, host paths, full XML, or secrets.
- Default safety cap and mapping have been reviewed.
- README, architecture, protocol, development, and release notes are current.
- Run the manual [hardware verification matrix](verification-matrix.md) and
  attach the redacted results to the release issue.
- Run the `Release` workflow from `develop`, then merge `develop` into `main`
  after its GitHub Release and downloads have been verified. Signing and
  notarization are intentionally separate protected CI steps; do not add
  credentials to this repository.
