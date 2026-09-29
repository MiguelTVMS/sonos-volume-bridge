# ADR 0013: Global Night Mode schedule for the selected speaker

**Status:** Accepted (2026-09-26)

## Decision

Persist one global weekly schedule, not a map of speaker schedules. Its seven
Monday-first days each contain 48 half-hour booleans.
The editor uses full-width horizontal day rows of compact half-hour cells with axis labels at 0, 3, 6, 9, 12, 15, 18, and 21 (the repeated midnight endpoint is omitted);
hover tooltips and accessible labels provide the exact interval. Only the currently selected
speaker is controlled, after a supported NightMode read. Selecting another speaker
reconciles that speaker; it sends no cleanup command to the previous speaker.
Unsupported or unavailable speakers pause scheduling without losing configuration.

The domain calculates intervals from an explicitly supplied timestamp and time
zone. Adjacent cells merge, including at midnight and the week boundary. Intervals
include their start and exclude their end. DST gaps resolve to the first valid
instant; folds use the first occurrence. Coincident boundaries collapse in civil
order, avoiding a replay or a zero-duration on/off sequence. Empty and fully selected
weeks are valid. No network, native clock lookup, or Tauri API enters domain policy.

The integration controller handles reads, confirmation, enforcement, and boundary
notification intent through an injected NightModePort. The shell owns its independent
worker, time-zone lookup, native wake events, and bounded recovery. It is independent
of audio availability, the volume state machine, and the volume fallback-polling
setting. Configuration changes and Night Mode writes share a gate. A configuration
cannot change under an in-flight write; queued manual commands reject a changed
selection. The controller rebinds only when selection or the schedule changes.

Inside selected blocks, Night Mode must remain on. Settings and tray explain that
the schedule must be disabled first; shared backend orchestration rejects manual
off. External off observations are restored, backed by five-second reads. Outside
selected blocks, manual changes remain effective. A scheduled end switches off once,
not continuously. Startup, wake, reconnection, schedule edits/enabling, and clock or
time-zone changes reconcile the expected current state without replaying missed
transitions. Disabling scheduling leaves the current speaker value untouched.

An independent, default-off **Disable loudness during night schedule** preference
temporarily enforces Loudness off inside enabled schedule blocks. The integration
layer expresses read, write, confirmation, and restoration policy through an
abstract Loudness port; the shell implements it with the Sonos adapter. Before
changing Loudness from on to off, the shell persists restoration ownership for the
selected speaker. It restores only bridge-owned changes on exit, schedule disable,
or preference disable, then clears that marker. Already-off Loudness is never
restored to on. The marker survives restart and cannot be applied to another
speaker. Loudness failures remain visible and retryable but do not pause Night Mode
or its notifications. Manual Settings and tray attempts to enable Loudness are
locked while the policy is active.

## Editing and persistence

The dedicated Night schedule sidebar page contains the horizontal week grid.
The tray toggles scheduling above Night sound; speaker controls remain on Speaker. Click and keyboard activation
toggle one cell; drag painting uses the initial cell's opposite state throughout
the gesture. Save commits the grid and explicitly applies its current on/off state, even when
recurring scheduling is disabled or the grid has not changed; Cancel restores saved values. Refreshes preserve
drafts, scroll position, and cell focus. Enable, night schedule Loudness, and
notification preferences save immediately. Generic settings writes preserve all
independently managed schedule fields. Old schema-version-one configurations default
to an empty disabled schedule, notifications off, and night schedule Loudness control
off. Invalid grid dimensions fail validation before persistence.

## Notifications

A separate global On start/On end/On start and end/Never preference filters native entry/exit notifications, including entry/exit caused by schedule edits. Legacy false/true values migrate to Never/On start and end; the existing field and schema version are retained. A notification
requires a confirmed speaker state and an ordinary boundary transition; already
correct speaker state still qualifies. Startup, recovery, selection changes, manual changes, and enforcement corrections
are silent. Applying a saved schedule compares the previous and new enabled schedules at one
timestamp. If the edit makes the current time enter or leave the schedule, notify
after speaker confirmation using the matching direction. This also applies when
the speaker already has the correct Night Mode value. Remaining inside/outside,
repeated saves, and saves with recurrence disabled are silent; there is no separate
save-confirmation notification. The controller consumes
notification intent once and clears superseded or recovery intent.

Enabling a previously disabled schedule during a selected period applies Night Mode
immediately and sends a start notification after speaker confirmation when On start or
On start and end is selected. Enabling outside selected periods, enabling an already
enabled schedule, and disabling scheduling do not notify. Disabling leaves the speaker
state unchanged. The worker subsequently reconciles silently to avoid duplicate
notifications.

The shell uses Tauri desktop notifications on Windows and a persistent async
D-Bus connection on Linux. The plugin's Linux path calls a blocking sender inside a
Tokio task, which panics when zbus also uses Tokio. Linux delivery therefore awaits
the native async API, bounded to two seconds, and retains the same sender connection
for the process lifetime. GNOME watches app sender names and removes a source when
its sender disconnects; acknowledging delivery alone does not guarantee a visible
banner. Failed or timed-out delivery clears the connection for a later reconnect
and records a generic delivery failure or timeout without interrupting
synchronization. A private D-Bus regression service
verifies applied-on/off and boundary-on/off delivery through the production sender
inside the async runtime and verifies that the sender remains connected after
multiple sends. It never posts test notifications to the user's desktop.
Because Tauri's desktop permission helpers unconditionally report granted, macOS uses UserNotifications
for real authorization, delivery, and foreground presentation; Windows checks the
native toast setting. Unpackaged Windows runs register both presentation metadata
and a COM activator before first use; display-name-only registration is incomplete.
The activator has a quoted executable launch command and opens Settings, ignoring
notification arguments. A dedicated MTA retains the COM factory for process lifetime.
Startup repairs the executable path after a rebuild or installation change.
Normal and demo activators are distinct. Native macOS and Linux delivery is unchanged.
Unpackaged Windows startup also creates or repairs the Start menu shortcut's sender
ID and toast activator CLSID. Existing shortcut targets are preserved; demo uses a
separate shortcut. Native failure events are observed while retaining at most eight
recent toast objects. This diagnostic retention does not change notification policy.
OS permission is requested only from the user's opt-in action.
Linux notification services do not expose a portable permission prompt/status;
delivery follows desktop settings. OS suppression and notification delivery failures
never affect volume synchronization. Native permission waits run outside the speaker
write gate, and selection/preferences are checked again before delivery.

Native wake adapters use NSWorkspace on macOS, power resume callbacks on Windows,
and logind PrepareForSleep(false) on Linux. A wall-clock discontinuity check backs up
wake delivery. Native presentation and actual sleep/hardware behavior require the
manual verification matrix; mock tests cannot prove OS notification delivery.

The tray offers a checked Night schedule item directly above Night sound, using
the same saved-schedule command as Settings. An enabled schedule can still be
disabled when the selected speaker is unavailable. The editor is in the
Night schedule sidebar page; the tray has no separate editor shortcut.

Dragging the schedule previews a rectangle between the first and current cells.
The first cell decides whether to select or clear the rectangle. Moving back
shrinks it and restores cells outside it to their pre-gesture state.

Clock presentation uses native macOS and Windows preferences and GNOME/LC_TIME
on Linux. The grid itself is shared across platforms. No forced preview format is
shipped. Native menu controls are retained across refreshes to keep action targets
alive during OS menu tracking.

Windows interval tooltips use the opaque platform surface color in both light
and dark modes so underlying grid cells cannot show through the time label.

Linux groups schedule status in a dedicated boxed settings row, directly below
notification preferences. Current status and the next transition appear once;
notification permission guidance and error feedback share that row. Successful
saves show no confirmation, and a successful retry clears the prior error. Disabled
and outside-period states remain visible there. The shared initial status read
also runs in browser previews so startup and subsequent actions use the same UI
orchestration. Other platforms retain their existing status presentation.

Scheduled start/end notifications and entry/exit caused by schedule edits share one body
formatter. Speaker names use the same display-name normalization as device
selection, removing Sonos renderer/model/identifier suffixes while retaining the
human room name, including hyphens within that name. Stable device identity is
unchanged.

Windows notifications use a shell-owned native adapter for permission and delivery.
Unpackaged runs register their application identity and display name in the current
user's AppUserModelId metadata, including the bundled PNG icon in a persistent
local data location. Register these assets before creating the notifier so Windows
does not show a generic sender icon. Packaged runs use the package's notifier. This
removes the mismatch between checking the installed identity and sending under
Tauri's development PowerShell fallback. No notification preference is rewritten.

Before a sender's first notification, Windows can return Element not found for
the permission query. Follow the [Windows Community Toolkit preregistration
sequence](https://github.com/CommunityToolkit/WindowsCommunityToolkit/blob/main/Microsoft.Toolkit.Uwp.Notifications/Toasts/Compat/ToastNotificationManagerCompat.cs):
submit a suppressed, silent toast with a short expiry, wait briefly for sender
registration, remove only that tagged toast, and recheck permission. Actual denied
settings and unrelated errors remain denied. Delivery uses native text nodes and
reports synchronous and asynchronous failures to the application log. A registered
COM activator opens Settings when the user clicks a notification. The Start menu
shortcut carries both the sender identity and activator identity.
Resolve its target from the Windows uninstall registration when a valid installed
executable exists, otherwise use the current standalone executable. Do not retain
an arbitrary previous shortcut target: it may point to an obsolete development
build. Full NSIS uninstall removes both per-user registration keys, while updates
preserve them. These registrations belong to the shell adapter, not the domain.
