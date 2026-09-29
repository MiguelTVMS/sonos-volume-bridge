# Speaker Volume Bridge upgrade

Formerly Sonos Volume Bridge. SVB and svb.miguel.ms stay unchanged. Sonos compatibility remains the only speaker integration in this release.

## User upgrade steps

- Quit Sonos Volume Bridge from its menu bar or system tray before upgrading.
- **macOS direct download:** install Speaker Volume Bridge.app, remove the old Sonos Volume Bridge.app bundle, and keep application settings/data. Check Start at login after replacing the bundle. Do not run the Store and direct-download editions together.
- **Windows:** update the existing installation or the existing Microsoft Store product. The installer migrates the former default installation folder to Speaker Volume Bridge, preserves custom installation paths and settings, and updates shortcuts and startup targets. Do not create a second Store product. Quit the installed app before upgrading. A blocked folder move stops the installer without removing the existing folder; an existing destination is never merged or overwritten.
- **Linux:** install the new speaker-volume-bridge Debian package. It replaces sonos-volume-bridge; the old shell command remains a compatibility symlink. Check your desktop environment's startup list and remove a manually created duplicate startup entry if present.

If the old app runs, Settings displays a persistent warning and synchronization pauses. Quit the old app and select Check again, or wait for the next five-second check. The new app never terminates the old app. If process inspection is unavailable before any conflict was detected, only Diagnostics shows a neutral check-unavailable status; no yellow alert appears. If you know the old app is running, quit it manually. A previously confirmed conflict keeps its warning and pause until absence is confirmed. Native notifications depend on existing OS permission and are sent once per conflict episode.

### macOS Start at login recovery

If disabling Start at login reports “Operation not permitted” after replacing
the old app, open **System Settings > General > Login Items & Extensions**.
Under **Open at Login**, remove any **Sonos Volume Bridge** or **Speaker Volume
Bridge** entry using the minus button. Launch the installed app from Applications
and retry disabling Start at login. Removing a bundle can leave the saved app
preference enabled even when its macOS login registration is already absent.
The login adapter accepts that absent state without attempting removal. Other
removal failures retain the saved preference and show the recovery steps.
Application settings do not need to be deleted for this recovery.

## Engineering and CI responsibilities

CI checks Rust, the frontend, website, workflow contracts and package assembly. CI builds the renamed installers and both new and legacy download filenames. It cannot reserve names, prove a real upgrade, exercise physical Sonos hardware, approve Apple jobs, select an App Store build or confirm Store certification.

Before release, an engineer must verify clean install and upgrade on macOS, Windows x64/ARM64 and Linux AMD64/ARM64. Confirm preserved settings, stopped/running state, shortcuts, startup preferences, notification activation and uninstall. Test old app present at startup, launched later, exit/recheck, repeated episodes, denied notification permission, unknown inspection and compatibility symlinks. Confirm no speaker or Night Mode writes during a conflict and no replay of queued volume changes. Test multiple renamed instances, demo isolation and real Sonos discovery/control.

**Release is blocked until actual sandboxed Mac App Store and packaged Windows builds pass those checks.** Local tests are not evidence of signed/sandboxed behavior. Keep results in private release evidence.

## Publication sequence

1. Save/reserve the new name on the existing Apple and Microsoft records.
2. Complete implementation, automated checks, local UI review and approved PR.
3. Rename the GitHub repository in place when the PR is ready; verify redirects, integrations and the custom domain. Never reuse the old repository name.
4. Merge through the approved PR to develop. Choose the next Minor increment, enable Apple package creation and disable automatic Microsoft submission for this first transition.
5. Verify installers and download aliases, approve promotion to main, deploy the website and publish the reviewed wiki.
6. Complete the existing Microsoft draft without discarding unrelated changes. Upload Apple's package, select the build, update screenshots and submit the branding review response. Complete Apple's displayed trader-status requirement if distributing in the EU.
7. Confirm each Store outcome separately before resuming routine Microsoft CI submissions. Fix forward with a higher version after publication.

Apple name: Speaker Volume Bridge. Subtitle: Sync Mac and speaker volume. Retain accurate Sonos compatibility and independent-development wording.

## Intentional legacy identifiers

[The allowlist](legacy-identifiers.json) records files retaining the former product name or stable identifiers and their reasons. `python3 scripts/check-legacy-identifiers.py` rejects undocumented references, including new files. Protocol-specific Sonos names remain accurate and are outside the rebrand.

Ready-to-review [Store copy and Apple reply](store-rebrand-copy.md) are staged for the verified build.
