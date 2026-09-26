# Synchronization state machine

`Connecting` waits for a Sonos read. With two-way synchronization enabled,
its first confirmed state is applied to the local system. With two-way
synchronization disabled, Sonos observations update connection state without
changing local audio, and the initial local state is sent to Sonos.
`Synchronized` has a current Sonos-confirmed state.
`WaitingForSonosConfirmation` retains only the newest local desired volume.
`SubscriptionDegraded` and `PollingFallback` represent unavailable or stale
event delivery. `SonosUnavailable`, `LocalAudioUnavailable`, and
`UnsupportedLocalDevice` are recoverable runtime states. A restored connection
returns to `Connecting` and reconciles from Sonos.

An application-originated local callback is suppressed. Windows identifies it
through a stable Core Audio event-context GUID; macOS compares it with a
short-lived expected write using an adapter-configured tolerance; Ubuntu uses
the PulseAudio-compatible `pactl` event stream with short-lived expected-write
tracking. A Sonos
confirmation always clears pending intents. It wins over any requested value
only when two-way synchronization is enabled.

Volume changes are debounced and coalesced; mute changes bypass the debounce.
The coordinator does not resend a volume or mute value until it differs from
the last sent value. When GENA delivery is healthy it polls slowly for health;
on subscription loss it polls once per second until event delivery recovers.

Night Mode scheduling is an independent optional controller. Disabled or unselected
means no recurring effects. Explicit Save remains a one-shot apply operation even
while recurring scheduling is disabled. Unsupported/unavailable means paused with configuration retained.
Reconciliation establishes the expected current value without a notification.
Scheduled entry enforces on; scheduled exit applies off once and permits subsequent
manual changes. Transient failures retry with bounded backoff; confirmed normal
boundaries produce notification intent. Explicit Save separately requests a
notification after confirmation, including idempotent saves. On start/On end/On start and end/Never filters that intent without changing controller state. Selection or schedule changes discard old
pending intent. Notification preferences never reset this controller. See ADR 0013.

Opt-in UI demo builds run this state machine and the normal Night Mode scheduler
against a loopback simulated Sonos speaker. Local audio remains native. See ADR 0014.
