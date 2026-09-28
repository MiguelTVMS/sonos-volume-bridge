# ADR 0016: Microsoft Store release recovery

**Status:** Accepted

## Decision

Retry Store submission independently of versioning and release builds. Resolve a
published tag to a retained combined upload from the release workflow on develop;
require successful package verification and GitHub publication even if submission
failed. Verify the bundle and embedded package version, identity and architecture
before using it. Reject expired or incomplete artifacts rather than rebuilding.

Use one reusable Microsoft Store Publish workflow for release calls and manual
dispatch. Both accept a required published GA tag and a dry-run flag that defaults
to true. The release caller explicitly disables dry run when publication is selected.
Validate the tag text input because GitHub has no dynamic tag picker. Require execution
from develop. Dry runs skip authentication and protected submission entirely.
Run credential-free preflight before the protected submission job, and
serialize manual and normal submissions. Keep standalone packaging separate.

Pin the Store CLI version and share the production command between both paths.
Pass the upload file directly to select the MSIX publisher, bypassing project
detection. After commit, require Store metadata to confirm uploaded packages
for the selected version and both architectures. Poll reads only; never resubmit
automatically when confirmation is delayed or mismatched.
Exercise its argument parsing against the actual CLI in native CI using --help.
This prevents a successful packaging test from being mistaken for submission
validation. Certification remains an independent external result.

## Verification

Tests cover failed-submission recovery, rejection of incomplete artifacts,
version and architecture mismatches, and publication restrictions. Windows CI
validates the production arguments against the pinned CLI without uploading.

Store submission metadata may represent the combined upload as one Neutral
package. Accept that representation only when its uploaded filename and version
match the retained artifact and local verification confirms both embedded x64
and ARM64 packages. Neutral alone does not prove architecture coverage. This
submission check does not establish certification or public availability.
