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

## Local Sonos protocol adapter

The Phase 2 `sonos` crate owns SSDP discovery, private-address validation,
device-description parsing, RenderingControl SOAP requests, and GENA
`LastChange` parsing. It remains independent of Tauri and native audio.

## Windows platform adapter

The Phase 3 `platform-audio` crate defines the shared controller interface and
contains a Windows-only Core Audio implementation. It keeps COM objects on a
dedicated worker thread, maps Core Audio callbacks to bounded broadcast events,
and handles default multimedia render endpoint replacement. It is not wired to
the synchronization state machine yet.

## Integration and GENA lifecycle

Phase 5 adds an `integration` crate with explicit asynchronous ports for Sonos
and local audio. It debounces only user-originated volume changes, immediately
passes mute changes, and sends local writes through the Sonos-confirmation rule.
The Sonos crate owns GENA SUBSCRIBE/renew/UNSUBSCRIBE and a local callback
listener. Healthy event delivery uses a slow health poll; subscription failure
switches to one-second polling until events recover.

## Tauri application shell

The Phase 6 `src-tauri` application is a thin composition root. It owns the
tray, hidden settings window, versioned per-user configuration, autostart, and
restricted commands. The vanilla TypeScript UI has no direct filesystem or
network permission; it receives a status snapshot and sends validated settings
through explicit backend commands.

## macOS platform adapter

The Phase 4 macOS implementation in `platform-audio` calls the system CoreAudio
framework through focused FFI. It observes default-output, mute, and volume
properties; uses deterministic expected-write suppression; and falls back from
master volume to output-channel controls when necessary. It remains independent
of Sonos, Tauri, and synchronization wiring.

## Supervised runtime composition

Phase 8 makes the Tauri process the composition root without moving protocol or
platform work into commands. A cancellable runtime generation resolves the
selected UDN through its cached description URL or bounded SSDP discovery,
creates the selected local audio adapter, binds the GENA callback listener to
the local interface selected for the speaker, and owns the coordinator.

Configuration replacement cancels the old generation before starting the new
one. Shutdown unsubscribes when a subscription exists. Subscription renewal is
scheduled at 80 percent of the advertised lifetime; failures use bounded retry
and the existing conservative polling fallback. Runtime errors are reduced to
status categories and structured, redacted logs rather than exposing peer
addresses or protocol payloads to the frontend.
