# ADR 0001: Pure domain core and authoritative Sonos reconciliation

**Status:** Accepted (2026-08-05)

## Context and decision

SonosVolumeBridge is a local-network desktop application whose speaker is the
sole authority. We use a Cargo workspace with a pure `domain` crate and an
adapter-agnostic `synchronization` crate. The domain has no Tauri, OS, network,
or asynchronous-runtime dependency. The synchronizer produces effects for
future adapters to execute; it never performs I/O.

Every confirmed Sonos observation, whether received by event, polling, or an
explicit read, replaces local pending intent and is mapped back to the local
audio device. A local user change creates only a desired Sonos command. Thus a
computer startup or a conflict cannot overwrite a Sonos-confirmed value.

The default mapping is intentionally deferred to configuration (Phase 6); the
domain supports linear, capped linear, and validated monotonic piecewise curves.
The maximum Sonos cap is enforced for every forward mapping.

## Consequences

This makes the critical policy portable and exhaustively testable. Future
platform adapters must label self-originated callbacks; Windows will use its
event-context GUID and macOS will use `ExpectedLocalWrite` tolerance matching.
The Phase 1 state machine represents a coalescing target through one pending
volume intent, while actual debounce/cancellation belongs to the async adapter
in Phase 5.

## Selected dependencies and technical risks

Phase 1 uses `serde` for portable types and `thiserror` for explicit errors.
Planned dependencies are: Tauri 2 (shell), Tokio (async and bounded broadcast),
the `windows` crate (Core Audio), direct Core Audio FFI isolated in the macOS
adapter, `reqwest` (HTTP/SOAP), `quick-xml` (bounded XML parsing), `axum` or
`hyper` (callback listener), `toml`/`serde_json` with atomic file replacement
(configuration), and `tracing`/`tracing-subscriber` (logging). Versions will be
selected in their implementation phases after re-verifying maintained APIs.

Main risks are Sonos GENA callback reachability, group/coordinator behavior,
hardware-specific macOS volume capabilities, and native callback threading.
Each is intentionally deferred until its adapter and integration tests exist.

