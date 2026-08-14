# ADR 0002: Bounded local Sonos UPnP client

**Status:** Accepted (2026-08-05)

## Decision

The `sonos` crate uses Tokio UDP sockets for SSDP, Reqwest with Rustls and no
proxy configuration for local HTTP/SOAP, and quick-xml's streaming reader for
device descriptions and event payloads. It exposes typed errors and accepts
only HTTP URLs with private, loopback, or link-local literal IP hosts.

The UDN is the selected device's stable identity. Its IP-address `LOCATION` is
only a discovery/reconnection input. RenderingControl commands target the
selected player in this phase; group coordinator selection remains deliberately
unimplemented until its behavior is defined and testable.

Response reading enforces a fixed byte cap even when a peer omits or lies about
`Content-Length`. No callback listener is opened in Phase 2.

## Consequences

This eliminates proxy and public-URL paths from normal control traffic and
keeps parsing bounded. It intentionally rejects hostname-based locations,
including `.local`, rather than performing an unsafe DNS resolution path. A
future discovery implementation may safely resolve such names only after
checking every resolved address.
