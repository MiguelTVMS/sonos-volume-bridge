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

The current published release is [v0.3.0](https://github.com/MiguelTVMS/sonos-volume-bridge/releases/tag/v0.3.0).

The release workflow compiles one macOS executable and one Windows executable
per version. The protected macOS direct-download job downloads the exact
executable produced by the unprivileged build job, imports the Developer ID
identity into an ephemeral keychain, bundles a sandboxed application, signs it
with Hardened Runtime, submits it to Apple for notarization, staples the ticket,
and verifies the result before the archive is published. The protected Mac App
Store job independently imports its Mac App Distribution and Mac Installer
Distribution identities, embeds the Mac App Store provisioning profile, verifies
the sandbox entitlements and profile, then produces a signed upload `.pkg`.
The Windows build job uploads its one
compiled output for two independent
packaging jobs. One produces the clean Microsoft Store MSIX payload while the
other applies Tauri's NSIS-specific metadata to produce the direct-download
installer. A packaging failure can therefore be isolated to its installer type
without compiling the application again.
The Windows installer remains unsigned, so SmartScreen may still warn before
installation.

Rust caches use one shared logical key across CI and release job names. The
cache action still isolates entries by operating system, Rust toolchain, Cargo
manifests, lockfile, and relevant compiler environment, while allowing jobs
with compatible inputs to reuse downloaded tools and compilation outputs.

Run the manual `Verify macOS Signing` workflow from `develop` after rotating a
Developer ID certificate or notarization key. It exercises the direct-download
signing, notarization, stapling, and verification path without changing the
version, creating a tag, or publishing a release.

## Windows packaging

Tauri produces a per-user installer suitable for signing. Configure the signing
certificate only through CI secrets; never commit certificates, private keys, or
passwords. Verify install, autostart, tray behavior, upgrade, and uninstall in a
non-administrator Windows account.

The NSIS installer remains the direct-download artifact. Microsoft Store
distribution uses a separate x64 MSIX with the reserved Partner Center identity.
Run the `Microsoft Store Package` workflow on `develop`, download the
`microsoft-store-msix` artifact, and upload its `.msix` file to the draft Store
submission. The Store signs accepted packages and delivers their updates. The
MSIX declares `en-US`, so its upload enables the English Store listing.

After the first Store submission is certified and live, the `Release` workflow
builds and validates the MSIX from the same versioned commit as the other
platform packages. For a GA release, it publishes the GitHub Release first and
then submits the MSIX to Store product `9N7JKGXCMST0`. Alpha and Beta releases
still build the MSIX for validation but intentionally skip Store submission.

Create a GitHub environment named `microsoft-store` and configure these secrets
before publishing the next GA release:

- `AZURE_AD_TENANT_ID`
- `AZURE_AD_APPLICATION_CLIENT_ID`
- `AZURE_AD_APPLICATION_SECRET`
- `SELLER_ID`

The Microsoft Entra application must be associated with the Partner Center
account and assigned the Manager role. Never commit these credentials. If an
automatic submission fails, correct the environment configuration and rerun the
failed workflow. The manual `Microsoft Store Package` workflow remains the
recovery path for building an uploadable package without submitting it.

Before submitting, verify the tray, Settings window, local-network discovery,
Windows audio control, single-instance behavior, settings persistence, startup
task, upgrade, and clean uninstall from a development-signed package. The
package version is derived from the workspace semantic version as
`major.minor.patch.0`; every Store update must increase it.

## macOS packaging

Build on Apple Silicon at minimum. macOS 13 or later is required because the
sandboxed app uses Apple's `SMAppService` login-item API instead of a
filesystem LaunchAgent. The release workflow packages the notarized
direct-download app as a ZIP preserving the `.app` bundle and creates a signed
`.pkg` for Mac App Store upload. The sandbox entitlement set grants only App
Sandbox plus incoming and outgoing network access. This is required for Sonos
discovery, control, and event callbacks. Core Audio, local configuration,
diagnostics, logging, menu-bar operation, and launch-at-login must be validated
in the sandboxed app on a clean macOS account.

Apple credentials are scoped to the protected `apple-signing` and
`apple-app-store` GitHub environments and are materialized only in each signing
job's temporary files and keychain. Configure these values before dispatching a
release:

- `apple-signing`: `APPLE_SIGNING_IDENTITY`, `APPLE_TEAM_ID`, `APPLE_API_ISSUER`,
  and `APPLE_API_KEY` variables; Developer ID certificate, certificate password,
  keychain password, notarization private key, and optionally a base64 Developer
  ID provisioning profile as secrets.
- `apple-app-store`: `APPLE_APP_STORE_SIGNING_IDENTITY` and
  `APPLE_MAC_INSTALLER_IDENTITY` variables; Mac App Distribution certificate,
  Mac Installer Distribution certificate, their passwords, keychain password,
  and the base64 Mac App Store provisioning profile as secrets.

Never commit certificates, private keys, or provisioning profiles. The App Store
profile must match the bundle identifier in `src-tauri/tauri.conf.json`.

The direct-download and Mac App Store editions are separate installations.
Users must uninstall the direct-download edition before installing the App Store
edition. The sandbox uses a different settings container, so existing settings
are not migrated and must be configured again.

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
- Approve the protected `apple-app-store` environment when the Mac App Store
  signing job is ready to start, then upload its signed `.pkg` after validation.
- Run the `Release` workflow from `develop`, then merge `develop` into `main`
  after its GitHub Release and downloads have been verified. Do not add Apple
  credentials to this repository.
