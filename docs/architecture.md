# Architecture

Phase 1 establishes the policy core. The `domain` crate owns values, mappings,
confirmed state, pending intent, and expected-write suppression. The
`synchronization` crate accepts normalized events and emits side effects.

```text
Native audio adapter ──events──> Synchronizer ──effects──> Sonos adapter
       ^                                |                         |
       └──────── apply confirmed state ─┴──── Sonos confirmed ────┘
```

Future adapters will serialize side effects, coalesce volume commands, and use
bounded channels. The policy core never connects peer computers: every instance
only observes and commands the same selected Sonos speaker.

