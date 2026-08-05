# Synchronization state machine

`Connecting` waits for a Sonos read. Its first confirmed state is applied to
the local system. `Synchronized` has a current Sonos-confirmed state.
`WaitingForSonosConfirmation` retains only the newest local desired volume.
`Degraded` is entered on connection loss; a restored connection returns to
`Connecting` and must reconcile from Sonos.

An application-originated local callback is suppressed. Future Windows code
will identify it through Core Audio event context; macOS will compare it with a
short-lived expected write with an adapter-configured tolerance. A confirmation
always clears pending intents and wins over any requested value.

