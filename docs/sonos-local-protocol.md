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
events. The application runtime starts a callback listener on the local network
interface selected to reach the speaker, uses a random callback path, and
forwards only notifications with the active subscription ID from the selected
Sonos peer.

## GENA lifecycle

The client sends `SUBSCRIBE` to RenderingControl's event endpoint with a
callback URL and requested timeout. It records the returned SID and renews with
the SID before expiry; `UNSUBSCRIBE` ends the lifecycle. If events are missing
or renewal fails, the integration runtime switches to conservative polling and
returns to event-driven operation after delivery recovers.

Sonos grouping discovery is represented by the selected player's own stable UDN.
The client targets the selected player rather than automatically choosing a
group coordinator; grouped and home-theater behavior should be verified for the
specific local Sonos setup before relying on it.
