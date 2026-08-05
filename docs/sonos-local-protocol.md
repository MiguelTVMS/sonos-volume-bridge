# Sonos local protocol

The client sends SSDP `M-SEARCH` packets to `239.255.255.250:1900`, accepts
successful responses with a local HTTP `LOCATION`, then reads the device
description. The selected identity is the device UDN, never its IP address.

RenderingControl commands use `InstanceID=0` and `Channel=Master`: `GetVolume`,
`SetVolume`, `GetMute`, and `SetMute`. SOAP requests have a three-second total
deadline, bypass proxy configuration, and accept bounded responses only.

Discovered URLs must use HTTP and a private, loopback, or link-local literal IP
address. Host names and public addresses are rejected to avoid treating an
untrusted device description as an SSRF instruction.

The crate parses GENA `LastChange` property payloads into Master volume and mute
events. It deliberately does not start a callback server or subscribe yet.
Subscription renewal, callback binding, source validation, and polling fallback
belong to Phase 5.

## GENA lifecycle

The Phase 5 client sends `SUBSCRIBE` to RenderingControl's event endpoint with
a callback URL and requested timeout. It records the returned SID and renews
with the SID before expiry; `UNSUBSCRIBE` ends the lifecycle. The callback
listener has a random path, binds only to an address selected by the host, and
accepts notifications only from the selected Sonos IP with the active SID.

Sonos grouping discovery is represented by the selected player's own stable UDN.
The client does not yet choose a group coordinator; Phase 5 must make the target
behavior explicit for home-theater and grouped playback.
