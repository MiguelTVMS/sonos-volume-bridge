# Synchronization state machine

`Connecting` waits for a Sonos read. Its first confirmed state is applied to
the local system. `Synchronized` has a current Sonos-confirmed state.
`WaitingForSonosConfirmation` retains only the newest local desired volume.
`SubscriptionDegraded` and `PollingFallback` represent unavailable or stale
event delivery. `SonosUnavailable`, `LocalAudioUnavailable`, and
`UnsupportedLocalDevice` are recoverable runtime states. A restored connection
returns to `Connecting` and reconciles from Sonos.

An application-originated local callback is suppressed. Windows identifies it
through a stable Core Audio event-context GUID; macOS compares it with a
short-lived expected write using an adapter-configured tolerance. A Sonos
confirmation always clears pending intents and wins over any requested value.

Volume changes are debounced and coalesced; mute changes bypass the debounce.
The coordinator does not resend a volume or mute value until it differs from
the last sent value. When GENA delivery is healthy it polls slowly for health;
on subscription loss it polls once per second until event delivery recovers.
