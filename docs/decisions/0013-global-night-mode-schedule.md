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

## Editing and persistence

The dedicated Night schedule sidebar page contains the horizontal week grid.
The tray toggles scheduling above Night sound; speaker controls remain on Speaker. Click and keyboard activation
toggle one cell; drag painting uses the initial cell's opposite state throughout
the gesture. Save commits the grid and explicitly applies its current on/off state, even when
recurring scheduling is disabled or the grid has not changed; Cancel restores saved values. Refreshes preserve
drafts, scroll position, and cell focus. Enable and notification preferences save
immediately. Generic settings writes preserve independently managed schedule fields.
Old schema-version-one configurations default to an empty disabled schedule and
notifications off. Invalid grid dimensions fail validation before persistence.

## Notifications

A separate global On start/On end/On start and end/Never preference filters native entry/exit notifications, including Save tests. Legacy false/true values migrate to Never/On start and end; the existing field and schema version are retained. A notification
requires a confirmed speaker state and an ordinary boundary transition; already
correct speaker state still qualifies. Startup, recovery, selection changes, manual changes, and enforcement corrections
are silent. Explicit Save is the exception: each confirmed save can notify, allowing
repeatable testing without editing the grid or enabling recurring scheduling. The controller consumes
notification intent once and clears superseded or recovery intent.

The shell uses Tauri desktop notifications on Windows/Linux. Because Tauri's desktop
permission helpers unconditionally report granted, macOS uses UserNotifications
for real authorization, delivery, and foreground presentation; Windows checks the
native toast setting. OS permission is requested only from the user's opt-in action.
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

Scheduled start/end notifications and explicit Save confirmations share one body
formatter. Speaker names use the same display-name normalization as device
selection, removing Sonos renderer/model/identifier suffixes while retaining the
human room name, including hyphens within that name. Stable device identity is
unchanged.
