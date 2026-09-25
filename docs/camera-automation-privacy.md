# Camera automation privacy

This optional feature observes whether a camera is being used. It does not open a
camera, request video frames, capture images, inspect media, inspect applications,
or detect microphones. It cannot detect audio-only meetings and may activate for
other camera uses such as recording or document scanning.

Only aggregate active/inactive/unknown observations leave the platform adapter.
Native camera references are temporary, remain inside that adapter, and are not
included in logs, exported diagnostics, or configuration. Windows reads streaming
booleans without requesting process identifiers or application names. There is no
camera activity history or persisted restoration journal.

The feature defaults off. Both platform release gates currently default off as
well. Linux is unavailable and deferred. Camera observation stops when the option
is disabled. No extra permission request is implemented: platforms/distributions
that cannot support passive observation remain unavailable.

Speech Enhancement is restored only while the app retains confirmed ownership of
its temporary change. Manual actions and observation failures revoke ownership.
After a crash, forced termination, sleep/resume gap, or uncertain disconnection,
Speech Enhancement may remain enabled. The app does not risk undoing an unseen
manual choice. Sonos cannot distinguish another controller setting an already-on
value or guarantee an atomic read/restore operation.
