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
