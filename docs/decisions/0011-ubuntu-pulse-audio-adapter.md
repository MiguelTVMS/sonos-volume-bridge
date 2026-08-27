# ADR 0011: Use PulseAudio-compatible control on Ubuntu

## Decision

The Linux platform-audio adapter uses the `pactl` command interface for sink
enumeration, volume and mute control, and change notifications. This supports
both PulseAudio and the PipeWire PulseAudio compatibility service used by
current Ubuntu releases.

## Consequences

Ubuntu packages must provide `pactl` (the `pulseaudio-utils` package). The
application reports the local audio service as unavailable when that command or
the user audio server is unavailable. Fixed output selections use PulseAudio
sink names, while follow-default mode follows the current default sink.
