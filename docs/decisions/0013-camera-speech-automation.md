# ADR 0013: Passive camera-triggered Speech Enhancement

Status: Implemented behind default-off platform features; hardware validation pending.

## Decision

Camera automation is optional and targets only the currently selected compatible
speaker. The first implementation supplies macOS and Windows candidate adapters;
Linux implementation and investigation are deferred to issue #125.

The domain owns timing and ownership policy. Integration owns normalized camera
and speech ports and the coordinator. `platform-camera` contains native observation.
The shell owns a single application-lifetime coordinator, separate from cancellable
volume generations. All Settings, tray, and automated Speech Enhancement writes
share its serialization boundary and continue through the Sonos adapter.

macOS observes CoreMediaIO device-list and system-wide running properties. It
registers listeners before the final initial snapshot, reads again after device
changes, and rescans every five seconds. No capture session is created.
Windows enumerates video-source activation metadata without activating it and uses
Media Foundation sensor activity reports. Only enumerated camera keys participate;
partial reports update existing entries, and missing initial reports remain unknown.
A five-second inventory reconciliation removes disconnected devices. A worker
heartbeat protects against a stalled worker; it cannot prove a silent native event
source has delivered every transition. Real hardware validation is mandatory.

`camera-automation-macos` and `camera-automation-windows` are default-off Cargo
features. Existing release jobs do not enable either. CI explicitly compiles/tests
native candidates. Distribution-specific validation is required before opting a
release build in; artifacts must be built separately if distribution gates differ.
No additional permissions are requested in v1. A distribution that cannot observe
reliably under existing permissions remains unavailable.

## State and lifecycle

The persisted opt-in defaults to false. Camera observation starts only after opt-in;
capability presentation may read speaker support beforehand. Aggregate activity
must remain active for one second before enabling and inactive for three seconds
before restoring. Multiple cameras share one activity period. Unknown is never
interpreted as inactive.

Activation reads authoritative Speech Enhancement. An already-on value is not owned.
An off-to-on change becomes owned only after verified readback. Restoration reads
again and only writes off while ownership remains valid and the value is still on.
Manual actions revoke ownership before writing. Observed external off values pause
automation for the remainder of the activity period. Duplicate events do not write.
An ambiguous enable result is not blindly retried during the same active period.

Unrelated configuration saves preserve ownership. Speaker selection, disabling,
resetting, and orderly exit attempt old-speaker cleanup with a five-second deadline.
Cleanup failure leaves a visible warning and abandons stale restoration intent.
New manual/configuration requests invalidate pending reads before they can start a
write. Already-started operations complete under the same lock so cleanup can act
on the correct old speaker. Speaker operations have bounded timeouts.

Read/detection failure, resume, and observed execution gaps revoke ownership.
Restart reconciles without a persisted restoration journal. Consequently, crashes,
forced termination, or unobservable gaps may leave Speech Enhancement on. This is
intentional: overwriting an unseen manual choice would be worse.

## Limits and privacy

Camera activity is a video-call proxy, including recording/scanning and other uses;
audio-only meetings are not detected. No process identities are queried, no frames
are requested, no cameras are opened, and no activity history is persisted or logged.
Transient device keys exist only within the adapter. Camera status is excluded from
exported diagnostics. See [camera automation privacy](../camera-automation-privacy.md).

Sonos events do not identify the controller, and read/write is not atomic. A
same-value external write or a change racing between read and write cannot be
reliably distinguished. Unchanged readback is not claimed as proof of authorship.

## Verification

Shared tests exercise the production coordinator and shell lifecycle with fake
speaker ports. Native inventory tests cover partial reports, overlapping cameras,
hot-plug, removal, and uncertainty. The verification matrix records the remaining
native callback, packaging, permission, suspend, virtual-camera, and physical Sonos
checks. No distribution is declared validated by this change.

References: Apple's CoreMediaIO `kCMIODevicePropertyDeviceIsRunningSomewhere`
documentation and SDK headers; Microsoft's `IMFSensorActivityMonitor`,
`IMFSensorActivitiesReport`, `IMFSensorProcessActivity` documentation and
Windows-Camera SensorActivityMonitorConsoleApp sample. The sample's process/name
logging is deliberately not used.
