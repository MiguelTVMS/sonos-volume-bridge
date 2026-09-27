# ADR 0016: Microsoft Store release recovery

**Status:** Accepted

## Decision

Retry Store submission independently of versioning and release builds. Resolve a
published tag to a retained combined upload from the release workflow on develop;
require successful package verification and GitHub publication even if submission
failed. Verify the bundle and embedded package version, identity and architecture
before using it. Reject expired or incomplete artifacts rather than rebuilding.

Default to validation without credentials or remote mutations. Actual submission
is an explicit mode, accepts GA releases from develop, and uses the existing
protected Store environment. Serialize manual and normal Store submissions.

Pin the Store CLI version and share the production command between both paths.
Exercise its argument parsing against the actual CLI in native CI using --help.
This prevents a successful packaging test from being mistaken for submission
validation. Certification remains an independent external result.

## Verification

Tests cover failed-submission recovery, rejection of incomplete artifacts,
version and architecture mismatches, and publication restrictions. Windows CI
validates the production arguments against the pinned CLI without uploading.
