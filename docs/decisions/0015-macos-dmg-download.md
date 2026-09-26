# ADR 0015: macOS DMG downloads

**Status:** Accepted

## Decision

Publish a drag-to-Applications DMG as the primary macOS download, containing the
existing signed and notarized app. Keep the ZIP asset for compatibility. Both
formats reuse the release executable; no separate app compilation is required.

Sign and notarize the DMG itself, require an Accepted response, staple and validate
its ticket, and check image integrity and Gatekeeper before uploading it. Release
alias preparation requires both formats before creating any permanent filenames.
The website links to the stable DMG alias. Deploy that link only after the first
GA release providing the alias is published.

## Verification

Release-download tests exercise alias generation, byte preservation, rejection of
missing/empty installers, and the website link. Local unsigned DMG verification
covers the app and Applications shortcut. Apple signing, notarization and
Gatekeeper verification require the protected release workflow; local unsigned
builds do not establish those results.
