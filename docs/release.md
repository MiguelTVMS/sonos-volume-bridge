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

Leave **Build Mac App Store package** unchecked (the default) while the app is
under Apple evaluation. Check it only when a new signed App Store package is
needed. This option controls package creation; the workflow does not submit the
package to Apple automatically. Other release packages and GitHub publication
continue when the App Store job is skipped. When selected, its signing job must
succeed before GitHub publication, main promotion, or Microsoft Store submission. The standalone App Store signing
verification workflow remains available for manual checks.

Leave **Submit release to Microsoft Store (GA only)** unchecked (the default)
to skip Store submission. Check it for a GA release when you want to upload the
MSIX after GitHub publication succeeds. Alpha and Beta never submit, even when
checked. Windows installers and MSIX packages are still built normally.

The workflow validates `develop`, commits the version bump to `develop`, verifies that the prepared commit belongs to trusted `develop` history, then
detaches every subsequent build, packaging, signing, and publication checkout at
that exact commit before executing release code, creates and pushes its annotated
`vX.Y.Z` tag with the GitHub Actions bot identity, then creates or updates the
GitHub Release with downloadable assets, a channel-aware installation and signing summary, and GitHub-generated change notes. Pull requests with `feature`, `enhancement`, `bug`, `fix`, `maintenance`, `refactor`, or `documentation` labels are grouped in those notes. It never merges branches. After a GA release is published, a separate job opens
an approval-required PR from a release branch pinned to the validated release
commit into `main`. Publication does not wait for this PR. Alpha and beta
releases do not open promotion PRs. Merge the PR using a merge commit to retain
the tagged commit in main's history; do not squash or rebase it.

The repository must allow GitHub Actions to create pull requests. PR validation workflows exclude `main`, so release-promotion PRs do not rerun
application CI. Normal development PRs still run CI. Only promote validated
release commits into main; route fixes through develop first. Required-check
rules targeting main must not require these excluded PR workflows.
Repository-managed security scanning, including default CodeQL, is separate
from these workflow files and may still run. The promotion unit tests run in CI
for changes to the helper, tests, or release workflows.
No branch protection bypass or automatic approval is used. If PR creation fails,
the release remains published; rerun the failed job after resolving the cause.
Repeated runs reuse the release branch and open PR. Divergence caused by previous promotion merges is allowed only when a trial
merge produces exactly the validated release tree. Conflicts, extra content, a conflicting
release branch, or a previously closed PR require manual review.

See the repository Releases page for the current published version.

The release workflow compiles one macOS ARM64 executable, Ubuntu AMD64 and
ARM64 Debian packages, and Windows x64 and ARM64 executables per version. The protected macOS direct-download job downloads the exact
executable produced by the unprivileged build job, imports the Developer ID
identity into an ephemeral keychain, bundles a sandboxed application, signs it
with Hardened Runtime, submits it to Apple for notarization, staples the ticket,
and verifies the result before the installers are published. The protected Mac App
Store job runs only when **Build Mac App Store package** is checked. It
independently imports its Apple Distribution and Mac Installer
Distribution identities, embeds the Mac App Store provisioning profile, verifies
the sandbox entitlements and profile, then produces a signed upload `.pkg`.
Each Windows architecture uploads its compiled output for two independent
packaging jobs. One produces the clean Microsoft Store MSIX payload while the
other applies Tauri's NSIS-specific metadata to produce the direct-download
installer. A packaging failure can therefore be isolated to its installer type
without compiling the application again.
The Windows installer remains unsigned, so SmartScreen may still warn before
installation.

## Ubuntu packaging

The release workflow builds native AMD64 and ARM64 Debian packages in parallel
on `ubuntu-latest` and `ubuntu-24.04-arm`, respectively, and uploads both to the
GitHub Release. The matrix uses separate architecture-specific caches and
artifacts. `scripts/collect-linux-installer.sh` verifies Debian architecture
metadata before assigning each release filename. Publication waits for both
architectures to succeed. It targets Ubuntu systems using PulseAudio or PipeWire's
PulseAudio compatibility service; users need `pulseaudio-utils` for `pactl`.

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
distribution uses x64 and ARM64 MSIX packages with the reserved Partner Center identity.
Run the `Microsoft Store Package` workflow on `develop`, download the
`microsoft-store-upload` artifact, and upload its combined `.msixupload` file to
the draft Store submission. Both architectures are verified inside one bundle. The Store signs accepted packages and delivers their updates. The
MSIX declares `en-US`, so its upload enables the English Store listing.

After the first Store submission is certified and live, the `Release` workflow
builds and validates the MSIX from the same versioned commit as the other
platform packages. For a GA release with **Submit release to Microsoft Store (GA only)** checked,
it publishes the GitHub Release first and then submits the MSIX to Store product `9N7JKGXCMST0`. Alpha and Beta releases
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
direct-download app as a drag-to-Applications DMG and creates a signed
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
  `APPLE_MAC_INSTALLER_IDENTITY` variables; Apple Distribution certificate,
  Mac Installer Distribution certificate, their passwords, keychain password,
  and the base64 Mac App Store provisioning profile as secrets.

For App Store packaging, the installer `.p12` file must contain the dedicated
`3rd Party Mac Developer Installer` identity. Reusing the app `.p12` here causes
`xcrun productbuild` and the updated signing workflow to fail with a strict
identity mismatch message.

To confirm both `.p12` bundles before updating GitHub secrets:

- Extract identities and subject lines:
  - `openssl pkcs12 -legacy -in /path/to/Certificates.p12 -passin pass:'$PASS' -clcerts -nokeys | openssl x509 -noout -subject -issuer`
  - `openssl pkcs12 -legacy -in /path/to/Installer.p12 -passin pass:'$PASS' -clcerts -nokeys | openssl x509 -noout -subject -issuer`
- Expected result:
  - `Certificates.p12` shows `3rd Party Mac Developer Application`.
  - Installer `.p12` shows `3rd Party Mac Developer Installer`.

Never commit certificates, private keys, or provisioning profiles. The App Store
profile must match the bundle identifier in `src-tauri/tauri.conf.json`.

The direct-download and Mac App Store editions are separate installations.
Users must uninstall the direct-download edition before installing the App Store
edition. The sandbox uses a different settings container, so existing settings
are not migrated and must be configured again.

## Release checklist

- Rust, frontend, audit, Ubuntu, Windows, and macOS CI checks pass.
- The Sonos and Ubuntu/Windows/macOS manual probes pass.
- Diagnostic export contains no serial numbers, host paths, full XML, or secrets.
- Default safety cap and mapping have been reviewed.
- README, architecture, protocol, development, and release notes are current.
- Run the manual [hardware verification matrix](verification-matrix.md) and
  attach the redacted results to the release issue.
- Approve the protected `apple-signing` environment when the signing job is
  ready to start.
- Approve the protected `apple-app-store` environment when the Mac App Store
  signing job is ready to start, then upload its signed `.pkg` after validation.
- Run the `Release` workflow from `develop`, then merge the pinned release PR into `main`
  after its GitHub Release and downloads have been verified. Do not add Apple
  credentials to this repository.

### Release download filenames

The GitHub Release publishing job runs `scripts/prepare-release-downloads.sh`
to rename build outputs to five permanent filenames, with one file per package.
The release tag identifies the version; duplicate versioned files are not uploaded:

- `sonos-volume-bridge-macos.dmg` (website download)
- `sonos-volume-bridge-windows-x64-unsigned.exe`
- `sonos-volume-bridge-windows-arm64-unsigned.exe`
- `sonos-volume-bridge-linux-x64.deb`
- `sonos-volume-bridge-linux-arm64.deb`

The website uses `releases/latest/download/<filename>` so stable downloads follow
the latest non-prerelease without a website deployment. Missing or empty source
installers fail preparation before any inputs are renamed. Preparation can be
repeated safely. Store packages remain separate workflow artifacts. Existing
published releases are not rewritten by this change.

The macOS DMG contains the notarized app and an Applications shortcut. The release
job also signs the disk image, requires an Accepted notarization response, staples
and validates its ticket, and verifies integrity and Gatekeeper assessment before
upload. Publish the first GA release containing the DMG before deploying the new
website link; earlier releases do not provide the stable DMG asset.

## Windows native architecture builds

Windows builds run on `windows-latest` for x64 (AMD64) and `windows-11-arm`
for ARM64. Each executable, NSIS installer and Store MSIX has a distinct
architecture-qualified artifact name. Executable PE headers are checked before
publishing build artifacts and before MSIX packaging; a mismatched requested
architecture fails before packaging. MSIX staging, manifest and filenames use
the executable architecture. Store submission combines both packages into one
bundle/upload so one architecture does not replace the other in a later submission.
Windows and Linux direct-download filenames consistently use x64 and arm64.
Debian package metadata and native build validation retain the required amd64 name.
Future releases omit the old architecture-free Windows filename and macOS ZIP;
links to those filenames on a specific older release remain valid.

Windows PR validation runs the native tests and builds an NSIS installer on both
architectures. This exercises the custom installer with the actual Tauri bundler,
including its Restart Manager includes. After upgrading Tauri, verify this build
before releasing: the template originally omitted `Win/RestartManager.nsh`,
which caused NSIS compilation to fail before producing an installer.

## Release build gate

The `all-platform-builds` job requires successful macOS executable, Ubuntu
AMD64/ARM64 DEB, Windows x64/ARM64 executable, NSIS and MSIX jobs, plus the
combined Store upload. Bundle creation and validation run without Store credentials
for every release channel, before signing and GitHub publication. Submission
downloads the exact verified upload rather than rebuilding it. Default GitHub
success semantics block the gate on any failed, cancelled or skipped required
build. macOS signing/notarization and optional Mac App Store packaging depend
on this gate; Microsoft Store submission also waits for GitHub publication.
Mac App Store upload remains manual. Release graph regression tests check all
required dependencies and reject cycles. Optional Apple packaging is deliberately
outside the unsigned build gate to avoid a dependency cycle. The publication
gate accepts its skipped result only when the option is unchecked; a selected
Apple package must succeed. Cancellation and failure in any required dependency
block publication. Tests cover the success, failure, skipped and cancelled cases.

Windows PR CI builds native MSIX packages on both architectures and runs the
production combined-upload script. It unbundles the result and verifies both
original packages are preserved, then checks rejection of a missing architecture,
a misleading architecture filename, and mismatched package versions.

## Retry or debug an existing Microsoft Store release

Use **Actions → Microsoft Store Package → Run workflow** with the desired
`release_tag` and `mode: validate` first. This finds the retained combined upload
from the original release run, verifies the selected version and both Windows
architectures, and parses the production publishing command with the pinned
Store CLI. The CLI requires local configuration even for help; validation temporarily uses
non-authenticating placeholders and restores the original configuration afterward.
Validation does not authenticate, upload, or modify Store state.
It does not rebuild packages, create a release, or bump the version.

For an actual retry, run from `develop` with the same tag and `mode: submit`.
Only published GA releases are accepted. The existing protected Microsoft Store
environment still applies. The retry reuses the exact retained artifact even
when the original run failed at Store submission. Expired artifacts and runs
without successful package verification and GitHub publication are rejected;
there is no automatic rebuild fallback. Manual and normal submissions share a
concurrency group to prevent overlapping Store changes.

Leave `release_tag` empty in validation mode to retain the original standalone
package-build workflow. Run it from the desired source ref when building fresh
packages is explicitly intended.

The CLI is pinned to `v0.4.3`, whose publish option is `--inputDirectory`.
The shared publishing script requires a directory containing exactly one
`.msixupload`. Native Windows CI invokes that same script with `--help`, using
the actual pinned CLI, so unsupported publishing arguments fail before release.
Successful submission is separate from Store certification and availability.
