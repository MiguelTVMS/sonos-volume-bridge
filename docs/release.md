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
both platform packages from that exact commit, creates and pushes its annotated
`vX.Y.Z` tag with the GitHub Actions bot identity, then creates or updates the
GitHub Release with downloadable assets, a channel-aware installation and signing summary, and GitHub-generated change notes. Pull requests with `feature`, `enhancement`, `bug`, `fix`, `maintenance`, `refactor`, or `documentation` labels are grouped in those notes. It never merges branches. Merge
`develop` into `main` only after the release workflow succeeds.

The current published release is [v0.1.1](https://github.com/MiguelTVMS/sonos-volume-bridge/releases/tag/v0.1.1).

The protected macOS release job imports the Developer ID identity into an
ephemeral keychain, signs the application with Hardened Runtime, submits it to
Apple for notarization, staples the ticket, and verifies the result before the
archive is published. The Windows installer remains unsigned, so SmartScreen
may still warn before installation.

Run the manual `Verify macOS Signing` workflow from `develop` after rotating an
Apple certificate or notarization key. It exercises the same protected signing,
notarization, stapling, and verification path without changing the version,
creating a tag, or publishing a release.

## Windows packaging

Tauri produces a per-user installer suitable for signing. Configure the signing
certificate only through CI secrets; never commit certificates, private keys, or
passwords. Verify install, autostart, tray behavior, upgrade, and uninstall in a
non-administrator Windows account.

## macOS packaging

Build on Apple Silicon at minimum. The release workflow packages the notarized
app as a ZIP preserving the `.app` bundle. Apple credentials are scoped to the
protected `apple-signing` GitHub environment and are materialized only in the
macOS signing job's temporary files and keychain. Verify menu-bar behavior, no
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
- Approve the protected `apple-signing` environment when the signing job is
  ready to start.
- Run the `Release` workflow from `develop`, then merge `develop` into `main`
  after its GitHub Release and downloads have been verified. Do not add Apple
  credentials to this repository.
