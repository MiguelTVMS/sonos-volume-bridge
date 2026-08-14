# Documentation index

This folder describes how SonosVolumeBridge works from protocol, runtime, and
operational perspectives.

- [`architecture.md`](architecture.md): architecture boundaries, components, and
  runtime data flow with Mermaid diagrams.
- [`how-it-works.md`](how-it-works.md): user-visible behavior and startup-to-shutdown
  sequence.
- [`sonos-local-protocol.md`](sonos-local-protocol.md): local Sonos UPnP protocol
  notes and safety constraints.
- [`state-machine.md`](state-machine.md): synchronization states and transitions.
- [`development.md`](development.md): local development and verification steps.
- [`verification-matrix.md`](verification-matrix.md): release validation checks.
- [`release.md`](release.md): release process and packaging constraints.
- [`decisions`](decisions): architecture decision records.

When changing behavior or public options, update the docs listed above before
closing the change.
