# Hardware verification matrix

## Linux ARM64 build

Local validation (2026-09-26): Ubuntu 26.04 ARM64, Rust 1.98.1, Node.js
24.21.0 LTS and pnpm 12.6.0 passed the standalone debug Tauri build, ELF
architecture and linked-library checks, Rust formatting, workspace Clippy and
tests, UI demo Rust tests, and frontend build, tests, lint and formatting.
Graphical and physical-device scenarios below remain manual verification.

Automated coverage: the Branch desktop check workflow builds on native
`ubuntu-24.04-arm`, runs frontend and Rust checks, verifies the executable's ELF
machine is AArch64, and uploads an artifact with the architecture in its name.
PR Rust quality checks run workspace and UI demo tests on the same architecture.
Workflow configuration alone is not evidence of a successful build; confirm the
ARM64 job passes before treating an artifact as verified.

Manual verification on an ARM64 Ubuntu desktop or VM:

1. Follow the native build commands in [development](development.md#linux-arm64).
2. Confirm `file` and `readelf -h` identify the executable as AArch64.
3. Start the normal app in a graphical session. Open Settings from the tray,
   select a local output and Sonos speaker, save, quit, and relaunch.
4. Verify saved selections, volume/mute synchronization, tray controls, and
   Night schedule interactions using the existing scenarios below.
5. Build again with `--debug --features ui-demo`, launch without a physical
   speaker, and repeat the UI demo scenarios below, including saved startup.
6. If testing a Debian package, confirm `dpkg-deb -f <package.deb> Architecture`
   reports `arm64`, install it, and repeat the desktop checks.

Coverage limits: CI compilation and automated tests do not verify a graphical
session, tray integration, audio service access, physical Sonos hardware, or
package installation. Building on Ubuntu 24.04 does not establish compatibility
with older distributions. Windows ARM64 and macOS builds are separate targets.

## Windows 11 settings presentation

- Automated: the normal frontend suite validates platform selection, native
  window identity, resize bounds, tray-first startup, and retained Windows chrome.
- Automated browser tests run the production renderer through the isolated preview:
  all seven pages at default/minimum size in light/dark mode, visible sidebar footer,
  card nesting, keyboard switch/select/slider changes across rerenders, forced colors,
  reduced motion, visible focus, and macOS/Linux presentation isolation.
- Regression mutation verified: removing the compact-height rules makes the
  minimum-size browser test fail because the speaker footer extends below the
  window. Restoring the rules makes the same test pass.
- Visual checks: launch `cargo tauri dev`, open Settings from the tray, and visit
  Devices, Speaker, Volume, General, Diagnostics, and About. Check the 960-by-760
  default and 760-by-460 minimum content sizes. Confirm all sidebar entries and
  the selected speaker remain reachable, cards scroll vertically, and long
  output names and diagnostics values do not create horizontal overflow.
- Repeat with light/dark appearance, increased contrast, reduced motion, and
  keyboard navigation. Switches must toggle with Space; selectors and sliders
  retain keyboard operation and visible focus. Use the isolated preview for
  interaction tests that should not change physical speaker settings.
- Development verification: Windows native light-mode window and isolated browser
  previews are checked locally. CI covers configuration and shared interaction
  logic, not native rendering, system contrast themes, or physical Sonos hardware.

Run this matrix for every release candidate on a private local network with one
supported Sonos speaker and one physical output device per operating system.
Complete this matrix before treating a release as ready for general use. The
macOS direct-download package is Developer ID signed and notarized. The Mac App
Store package is Apple Distribution signed with an embedded provisioning
profile. The Windows installer is currently unsigned.
Record the operating-system version, Sonos firmware version, speaker model, and
result in the release issue. Do not record device serial numbers, LAN addresses,
or diagnostic payloads.

## Shared Sonos checks

| Check | Expected result |
| --- | --- |
| Platform settings presentation | On each OS, check all six settings sections at default and minimum window heights, light/dark appearance, increased contrast, keyboard navigation, disabled controls, and long device names. Confirm the matching platform theme and no clipped controls. |
| Fresh-install settings save | With no saved configuration and Start at login off, choose a speaker and output and change a volume option. Confirm saving and reconnecting work, then relaunch and verify persistence without toggling Start at login. |
| Speaker control freshness | Change Speech Enhancement in another controller, then focus Settings or switch sections and verify the new state. Enable and disable from this app and verify authoritative state after refresh. Open the tray after hovering its icon and check matching controls. |
| Discovery and selection | Discovery lists the selected speaker by friendly name and stable UDN. Saving selection connects without exposing an address to the frontend. |
| Cached-address reconnect | Restart the app with network unchanged. The selected UDN is resolved through its cached local description URL before SSDP is used. |
| Sonos-originated volume change | A physical Sonos volume change updates local output after Sonos confirmation. |
| Local-originated volume change | A local volume change is coalesced, capped, reaches Sonos, and returns to `Synchronized` only after confirmation. |
| Mute synchronization | Mute is sent immediately when enabled; disabling mute synchronization leaves the other endpoint unchanged. |
| GENA lifecycle | Confirm a callback event, renewal before expiry, and status recovery from `Subscription degraded`. |
| Polling fallback | Temporarily block callback delivery. Confirm `Polling fallback` and recovery to event-driven synchronization after delivery resumes. |
| Reconnect and shutdown | Disconnect/reconnect the speaker network, then quit. Confirm reconnect backoff, no stale status update, and best-effort unsubscribe. |

## Windows checks

| Check | Expected result |
| --- | --- |
| Default output replacement | Change the default multimedia render endpoint. The adapter detaches, reattaches, and resumes synchronization. |
| Fixed output selection | Select a fixed endpoint and confirm default-device changes do not move synchronization. |
| Application write suppression | Sonos-confirmed local writes do not trigger a second Sonos command. |
| Device failure | Disconnect or disable the endpoint. The tray reports `Local audio unavailable` and recovers when available. |

## macOS checks

| Check | Expected result |
| --- | --- |
| First left-click after startup | Fully quit and relaunch with a selected speaker. After the initial speaker read completes, left-click the tray before any right-click. All supported Night sound, Loudness, Status light, and Speech enhancement controls appear with correct values. The automated startup test covers the refresh trigger and control mapping; this check covers native menu presentation and real device responses. |
| Bundle signature | `codesign --verify --deep --strict --verbose=4` succeeds for the downloaded app bundle and reports the expected Developer ID identity. |
| Notarization ticket | `xcrun stapler validate` succeeds for the downloaded app bundle. |
| Gatekeeper assessment | `spctl --assess --type execute --verbose=4` accepts the downloaded app bundle. |
| Sandbox entitlements | The signed app reports App Sandbox plus incoming and outgoing network entitlements, with no unrelated sandbox entitlement. |
| App Store profile | The App Store build contains `Contents/embedded.provisionprofile`; its App ID matches the bundle identifier and its entitlements authorize App Sandbox. |
| Clean-account launch | On a clean macOS account, install and launch each distribution separately. Confirm there are no sandbox denials affecting discovery, control, callbacks, reconnection, polling, Core Audio, tray operation, login launch, settings, diagnostics, or uninstall. |
| Default output replacement | Change the default output device and confirm safe listener replacement and recovery. |
| Master/channel volume | Test a device with master volume and, where available, a channel-only device. Both apply the expected local value. |
| Expected-write suppression | Sonos-confirmed local writes within configured tolerance do not produce a second Sonos command. |
| Unsupported device | A device without software volume reports `Unsupported local device` clearly. |

## Release decision

All required rows must pass on Windows and macOS. Document an exception in the
release issue with its device class, user impact, mitigation, and a follow-up
issue before declaring a release candidate ready.

### Speaker capability and push regression checks

- Automated: programmable local HTTP speaker fixtures exercise successful off/zero
  reads, explicit unsupported faults, timeout and malformed-reply recovery,
  independent feature failures, model-specific Speech Enhancement, advertised
  service URLs, and switching between speakers without sharing capabilities.
- Automated: real NOTIFY callbacks followed by SOAP reads confirm an external
  Speech Enhancement change and preserve capabilities omitted from the event.
  Frontend mock views test the same updater used on render and push refresh.
- Regression mutation: treating temporary read failures as unsupported makes the
  recovery test fail; restoring the availability classifier makes it pass.
- Manual (pending): leave Speaker settings open, toggle Speech Enhancement in an
  external controller, and confirm the displayed value updates without navigation;
  repeat off/on and check the tray. Disconnect/reconnect the speaker and verify
  controls recover. Repeat on legacy and Ultra soundbars when available. Mocks do
  not verify real firmware behavior, native menu timing, or network reachability.

### Slider and live telemetry regressions

- Automated shared production gesture binding with mocked input events: pointer
  drag/pause/release, keyboard repeat/release, cancellation, accessibility changes,
  and exactly one final commit. Readback of both echoes and external changes must
  produce zero additional writes. Rapid writes serialize and recover after failure.
- Automated telemetry delivery: a push updates immediately and supersedes an older
  pending snapshot read. Partial volume/mute notifications request fresh reads.
- Mutation checks: committing on change while held, or accepting stale snapshot
  reads, each makes its corresponding regression test fail.
- Manual: drag and hold Bass/Treble/maximum volume through a backup refresh, then
  release; confirm no jump during drag and only the final value applies. Change
  volume externally with Diagnostics open and check prompt updates. Native drag
  timing and physical notification latency are not simulated by the mock suite.

## Global Night Mode schedule

Automated coverage exercises calendar boundaries, DST, week wrapping, read/write
confirmation, external-off restoration, selected-speaker rebinding, disabled/no-target
behavior, manual-off rejection, retries and notification intent. Browser tests use
the production editor through the isolated preview; they do not touch real speakers.

Manual checks for the local candidate (not yet completed on physical hardware):

1. Select a compatible speaker. Open the Night schedule sidebar item. Verify the grid is absent from Speaker. Paint a block containing the current time, save,
   and enable. Confirm Night Mode on and the locked Settings/tray explanation.
   Disable it from another controller; it should restore within the backup-read
   interval plus network latency. Volume synchronization must continue.
2. Disable scheduling: Night Mode stays unchanged and manual off succeeds. Re-enable
   outside a selected block, manually turn on, and refresh Settings: it stays on.
3. Cross a start/end boundary. Confirm one on/off application and, with the matching notification
   mode and OS permission, one native notification after confirmation. Consecutive
   selected cells must not notify. Try with Settings open and closed.
4. Deny notifications, then enable them in system settings. Confirm clear guidance,
   working synchronization, and subsequent delivery without a background permission
   prompt. Check OS Do Not Disturb suppression. Windows requires an installed app;
   development-shell notification branding is not representative.
5. While selected, disconnect/reconnect the speaker and sleep/wake across a boundary.
   Confirm the current expected value is applied without replayed transitions or
   notifications. Repeat with volume fallback polling disabled and audio unavailable.
6. Switch between compatible, unsupported, and unavailable speakers while writes
   are pending. Only the backend's current selection may receive new commands;
   switching sends no cleanup commands to the former speaker. The global grid stays.
7. Verify light/dark presentation and keyboard painting on each desktop OS. Scroll
   the grid while a draft is dirty and allow background refreshes: preserve cells,
   focus, and scroll. Save/Cancel must not alter another speaker's configuration.

Coverage limits: native wake delivery, OS permission prompts/toast presentation,
physical speaker support, and network timing remain manual checks. Linux without
logind uses the clock-discontinuity fallback and cannot guarantee immediate native
wake delivery. Native notification display is controlled by the OS.

8. On Night schedule, confirm there is no repeated selected-speaker label. Leave recurring
   scheduling disabled, select the current block and Save: Night Mode turns on.
   Save unchanged again: no notification and no redundant Sonos write. Clear the current block and Save: Night Mode turns off. Permission denial
   must not prevent application, and failed confirmation must not announce success.

9. Open the native tray menu with a compatible speaker selected. Verify Night schedule is immediately above Night sound and its checkmark reflects whether scheduling is enabled and no editor shortcut remains. Toggle
   it on and verify the saved schedule takes effect, then uncheck Night schedule and verify Night Mode stays unchanged and manual off is available.
   Repeat disabling with the speaker unavailable. Native menu placement and click
   delivery need this local check; CI covers the menu model and shared command policy.

10. On macOS, launch the app, open the tray immediately, hover repeatedly, then
    click the schedule toggle and Night sound while background speaker reads finish.
    Verify the process stays running and the controls update in place. Repeat after
    speaker loss/recovery. CI covers the shared menu-diff sequence; native Cocoa
    action-target lifetime and menu tracking require this packaged-app check.

11. In the schedule form, verify On start/On end/On start and end/Never at both boundaries and
    with Save. Confirm Portuguese locale uses 24-hour labels and US English uses
    AM/PM, including midnight tooltips. Hover a grid cell and verify the tooltip
    appears promptly and disappears on leaving or painting.
    On Windows, repeat in light and dark modes: hover a middle-row cell so the
    tooltip overlaps the grid and verify no cells show through its background.
    CI exercises the production renderer's hover sequence and checks an opaque
    computed background in both modes; native WebView rendering remains manual.

12. With macOS language English (US), region Portugal and a 24-hour clock, verify
    the schedule shows 13:00 rather than 01:00 PM. Change the system clock format,
    refocus Settings, and verify labels/tooltips update without losing grid edits.
    CI covers a US WebView locale with a native 24-hour override at startup.

### Windows and Linux branch trial

Use `feat/night-mode-schedule` before opening a PR. The Branch desktop check
workflow builds and tests on Windows and Linux and uploads an unsigned debug
executable for each platform. Windows requires the WebView2 runtime; Linux needs
GTK3, WebKitGTK 4.1, and an AppIndicator implementation. Native notifications may
require an installed/packaged app identity, so the standalone executable does not
replace packaged-app notification validation.

Run `pnpm --dir ui install --frozen-lockfile` and
`pnpm dlx @tauri-apps/cli@2 dev` from a local checkout for a source trial.
Verify rectangular add/clear gestures, the checked tray schedule control, selected
speaker changes, wake/recovery and all four notification modes. Windows clock
format comes from the user's regional time format. Linux honors GNOME's explicit
clock format when available, otherwise LC_TIME; other desktop-specific overrides
remain dependent on that desktop's locale configuration. No AM/PM preview override
is enabled in branch builds.

### Night schedule notification dropdown sizing

The macOS Settings window opens at 740 by 700 points. Check Night schedule at
that default size for unnecessary vertical scrolling; horizontal resizing must
remain disabled. Browser coverage also checks the page with save feedback shown.

On macOS, open Settings → Night schedule and select On start, then On end.
After each immediate save, verify the complete selected text remains visible
beside the dropdown arrow. Leave and reopen Settings to check the restored value;
repeat at the minimum window width. The browser regression selects all four modes
through the production form, waits for the save rerender, and checks the sizing
label and rendered text width. It fails before the mount-order fix. Browser
coverage does not establish native WebView popup rendering; repeat this check in
the packaged macOS app before release.

## UI demo builds

Autostart isolation regression: the normal Rust suite builds normal and demo
mock applications through the shared startup identity configuration and checks
the package names consumed by the autostart plugin remain distinct. This test
fails with identifier-only isolation and passes with the demo package suffix.
It does not write real OS login entries. On Windows and Linux, enable Start at
login in the normal app, then enable and disable it in the demo. Confirm the
normal registry Run entry or autostart desktop file is unchanged and still
launches the normal executable after signing in again.

The Windows presentation browser test uses native select change events for
portable save/rerender coverage, retaining keyboard checks for switches and
sliders. Native dropdown popup keyboard behavior must be checked on Windows:
focus Follow, choose an output by keyboard, and verify it survives navigation.

Build without flags and confirm there is no demo label. Build with `--debug
--features ui-demo`, start without a speaker, and verify Settings opens with
Living Room (simulated). Check normal tray controls, speaker settings, local
output selection, volume/mute synchronization, diagnostics/export and reset.
Demo saved settings/logs are separate; no real Sonos devices are discovered or
contacted. Local audio and OS services operate normally.

Regression sequence for Windows Night Sound: start a fresh demo, open Night
schedule, select the current day's current half-hour cell and Save schedule.
With recurrence disabled, turn Night sound off on Speaker. Enable schedule and
return to Speaker: Night sound becomes on, its switch is disabled and the message
explains that scheduling must be disabled first. Try the tray Night sound action
as well. Disable the schedule: Night sound stays on but manual off works. Save
an empty schedule and enable: manual on/off is available outside active periods.
Repeat after restarting with an enabled active schedule, including opening Speaker
immediately before the first scheduler tick. Cross a half-hour start/end boundary
and verify on enforcement, one-time off at exit, and the selected notification mode.

Restart: demo app settings persist and the real runtime reconciles fresh simulated
speaker state. Repeat with `ui-windows`, `ui-macos`, and `ui-ubuntu`; native window
constraints and chrome remain host-specific. Check keyboard navigation and resizing
at default/minimum sizes and across display scaling settings.

Automated routing tests cover the delayed startup handshake and native commands
in both normal and demo builds (the old desktop mock routing fails these tests).
Native tests use actual SOAP/GENA clients and production scheduler/command entry
points to cover enabling, startup timing, enforcement, manual-off rejection,
disabling and outside-period control. They run in the ordinary CI test suite and
feature-enabled checks. Browser tests cover presentation only. Native WebView,
tray interaction, notification delivery, wake/time changes and physical hardware
still require target-OS checks; simulated firmware cannot establish hardware behavior.

Windows native tests require the Common Controls v6 manifest. The shell build
script supplies it to library test executables; application binaries retain the
full Tauri manifest. Without this, the test process fails before running tests.
Local Windows verification of the revised native demo: startup showed Connected
with the simulated speaker. Saving the current Saturday half-hour and enabling
scheduling made Night sound checked and disabled, with the schedule-lock message
and next transition visible. Left the active demo period enabled for UI testing.
Rust workspace/demo suites, formatting and Clippy passed; frontend unit tests,
lint/format/build and all 21 browser tests passed. Native notification delivery,
clock boundaries and restart persistence were not manually exercised in this pass.

## Schedule notification speaker names

With a speaker whose discovery name contains a Sonos Media Renderer suffix, enable
start/end notifications and save a schedule once inside and once outside its
current block. Confirm each notification uses only the human speaker name. Verify
scheduled start and end notifications likewise. Names with ordinary hyphens must
remain intact. Automated tests cover the shared production notification formatter
for all four paths and fail when raw discovery names are used. Native delivery
and banner layout still require a real notification-capable host.


## Ubuntu settings presentation

- Automated browser tests cover every page at 1080 by 800 and 1080 by 460 in
  light and dark modes, keyboard switch and slider changes, and persisted schedule
  selection. The rounded-cell regression was reproduced before the CSS fix.
- On native Ubuntu, start `ui-demo` from a closed app and compare with GNOME
  Settings: wide window, neutral sidebar, white rounded groups, row separators,
  trailing dropdowns, orange switches and white slider thumbs. Check text renders
  in Ubuntu/system sans-serif throughout, with no unexpected serif paragraphs.
- Confirm horizontal resizing is blocked. At the default height, open Night
  schedule and Save: the grid, actions and save notice must fit without scrolling.
  Resize vertically to the minimum; scroll to every setting and action. Switch pages, change a dropdown, drag a slider, and save/reopen a schedule.
  Schedule cells must stay rectangular before and after updates.
- Repeat in dark mode, with increased text scaling and keyboard-only input.
  Confirm select popups, native title bar and tray behavior in WebKitGTK.
- Browser coverage cannot establish native title-bar appearance or desktop theme
  integration. HTML controls follow Yaru styling but are not native GTK widgets.

Ubuntu window follow-up: verify width stays at 1080 while height can change.
At the 800-pixel default height, Night schedule must fit after Save without
scrolling. On Volume, the Test speaker volume button must be inset from the card
and usable with Enter. Automated browser regressions cover these cases.

## Ubuntu top-bar icon contrast

Start with Ubuntu in light application mode and the app closed. Launch the UI
demo: the top-bar glyph must be white immediately, including before connection
completes. Switch the application theme dark then light; the glyph must remain
white. In a normal build, disconnect and reconnect the selected speaker and check
the red disconnected badge appears and clears without changing glyph contrast.
The CI regression exercises the production image-selection path in this sequence
and failed before the fix. It cannot verify native AppIndicator rendering or
custom desktop panels; manually check against Ubuntu's default dark top bar.

## Linux release architectures

Release CI builds native AMD64 and ARM64 Debian packages and checks their
architecture metadata before upload. Automated release-script tests use real
Debian package fixtures and reject mismatched, missing, empty or ambiguous
installers. Alias tests require both Linux packages and check the website URLs.
After the first release, download each architecture from its website button,
check `dpkg-deb -f <package.deb> Architecture`, and install on the matching Ubuntu
machine. The ARM64 stable link is unavailable until that asset reaches a GA release.

## macOS release DMG

After a GA release, download the DMG from the website. Verify its stapled ticket
and Gatekeeper assessment, open it, drag the app to Applications, eject the image,
and launch the installed app. Confirm Settings, tray and saved configuration work.
Repeat on a clean supported Mac to check quarantine handling. Release CI verifies
signing, notarization acceptance, stapling and image integrity; alias tests cannot
establish native installation or Apple notarization behavior.

## Reset during autosave

Change a General setting and immediately open Diagnostics and reset. Wait past
the autosave debounce, return to General, and confirm defaults remain restored
and no success notice appears. Repeat with a slow
configuration write. Browser regressions freeze the debounce clock or hold IPC
completion to exercise both sequences deterministically through the real UI.

## Linux Night schedule status consolidation

- Automated: the Linux browser suite opens Night schedule from startup, checks
  the third row below notifications, enables scheduling, saves, navigates away
  and returns. Status and error feedback must stay in that row, with no duplicate
  status below the card or confirmation below the editor. Successful saves remain
  silent; a failed save shows an error, cleared on a successful retry. The regression fails
  before the fix and passes after it; default-window fit is also checked.
- Manual: launch the normal Linux app, open Settings > Night schedule, enable
  scheduling, and save a block. Confirm the status updates in the third white row
  below notifications. Navigate away and back, then test an active period and
  an unavailable speaker. Confirm current status and errors remain in the row.
  Browser mocks cover layout and frontend orchestration, not native event delivery
  or real speaker transitions. Native checks remain to be performed.

## Wayland close button after first show

1. In an Ubuntu Wayland session, launch the normal app with Settings initially hidden.
2. Open Settings from the tray and click the native close button once, without
   resizing, maximizing, or double-clicking the title bar first.
3. Confirm Settings hides and synchronization and the tray continue running.
4. Reopen from the tray and repeat the immediate close at least three times.
5. Confirm title-bar dragging, minimize, maximize, and vertical resizing still work.

Before the dependency repair, close could ignore clicks until a resize; repeated
clicks could maximize instead. The lockfile regression fails on that dependency
and passes with the upgrade. The normal Rust suite exercises the shared close
handler through first show and reopen without resize. Native GTK/compositor input
is outside mock coverage; manual validation is pending.

## Linux schedule notification delivery

- Automated: the normal Rust suite starts a private D-Bus notification service
  and a subprocess using the production sender inside Tokio. Applied-on, applied-off,
  scheduled-start, and scheduled-end must all arrive. Before the fix all four were
  missing because the blocking plugin sender panicked inside the async runtime;
  the same regression passes with asynchronous delivery. Linux tests require
  `dbus-daemon`, installed by CI. The test never targets the desktop session bus.
- Manual: in the normal Ubuntu app, select On start and end. Save once with the
  schedule enabled and the current half-hour selected, and again with it cleared. Confirm a native banner
  only when the edit enters or leaves the enabled schedule. Repeat each save and confirm
  there is no additional notification. Then enable the
  schedule and verify an actual start and end boundary. Enabling during a selected
  period also notifies after speaker confirmation; disabling does not notify. Confirm Never suppresses both directions, On start only shows
  on notifications, and On end only shows off notifications.
- Coverage limit: the service test verifies delivery and message content, not
  GNOME banner rendering or desktop suppression. Native edit-triggered banner
  confirmation is recorded below.

The save-transition regression additionally exercises the shared application and
notification orchestration with all four preferences, recurring scheduling on
and off, and the sequence outside → enter → unchanged save → leave → unchanged
save. Only permitted entry/exit transitions with recurrence enabled reach the private
service after speaker confirmation. Disabled recurrence stays silent.

Also turn Night Mode on manually while outside the schedule, then save an edit
that includes the current half-hour. With On start selected, the entry notification
should appear even though the speaker was already on. Editing another day while
remaining inside the current period should not notify. Save with recurrence off
and confirm no notification, even if the one-shot apply changes Night Mode.

### GNOME notification source lifetime

The private-service regression also checks that repeated notifications use the
same sender and that it still owns its bus name after the production sender
returns. The previous per-send connection was closed immediately after Ubuntu
accepted delivery, causing GNOME to remove the app notification source. This
regression fails before persistent-connection delivery and passes afterward.

Manual reproduction: keep Settings open, enable the schedule and On start and
end notifications. Clear the current half-hour block and save, then select it
and save. Confirm both banners appear and remain available in Ubuntu's
notification list after the send completes. Repeat without closing Settings.
The mock service checks connection lifetime; actual GNOME presentation still
requires this native verification.

Native verification (2026-09-27): the user confirmed that both Ubuntu banners
appeared after clearing the current half-hour and saving, then selecting it and
saving again with Settings open. This verifies edit-triggered exit and entry
presentation; timed boundaries and other desktop sessions remain separate checks.

### Enabling during an active period

Automated Linux regression calls the production enable command before the worker's
first tick with a simulated speaker and private notification service. It covers
all four notification preferences, selected and unselected current-time periods,
repeated enable, and disable. It verifies speaker confirmation and exactly two
start notifications (Start and Both). The regression receives zero notifications
before the fix and passes afterward. Existing worker tests cover reconciliation.

Native check: with scheduling disabled, select/save the current half-hour, then
manually turn Night Mode off. Choose On start or On start and end and enable the
schedule from Settings; confirm Night Mode turns on and one Ubuntu banner appears.
Repeat using the tray. Repeat outside selected periods and with On end/Never;
expect no banner. Disable scheduling and confirm Night Mode stays unchanged with
no end banner. The private service cannot prove GNOME banner presentation or
desktop suppression settings.

Native verification (2026-09-27): after the enable-notification fix, the user
reported that everything was working. This confirms the reported active-period
enable notification in the normal Ubuntu app. The report does not separately
verify every preference, tray interaction, timed boundary, or first-open close
sequence listed above.

## Windows Night schedule status and notifications

Open the normal Windows app, navigate to Night schedule, enable scheduling, edit
blocks and save. The third card must contain current state, next change when
available, notification guidance and error feedback. Successful saves must show no
confirmation. The Status label must be on the left with wrapping text on the right
in the 960-by-820 default Windows window. There must be no second
status line beneath the card or notice beneath Save. Disable and re-enable the
schedule, navigate to General, save a setting and return; errors must stay visible
in the appropriate page while successful actions stay silent. Check light/dark and
minimum/default window sizes.

On a fresh Windows user profile, launch with `cargo tauri dev` without demo features.
Choose a compatible speaker, select On start and end, and save a block covering
the current time. Confirm a Sonos Volume Bridge notification, then save an empty
grid and confirm the off notification. Enable a schedule spanning the next
half-hour boundary and verify a single start/end notification at matching
boundaries. Repeat with On start, On end and Never to check filtering. Repeat
with an installed desktop build and a packaged build. With app notifications
disabled, confirm the status guidance and that scheduling still works; restore
notifications and confirm recovery. Do not disturb may send notifications directly
to Notification Center instead of showing a banner.

CI browser coverage uses the production form and mocked IPC to check initial
state, toggling, grid editing, saving, next transition, notification guidance,
failure feedback and navigation. The layout regression fails before consolidation.
Error-only notice regressions on all three platforms fail with the old save
confirmation. They wait for the production form to finish saving, then exercise
a failed save and successful retry, preserving visible errors and clearing them
after recovery. Settings autosaves are also verified to remain silent.
Rust CI exercises shared Windows notification orchestration from an unregistered
sender through the initial permission check and Save/start/end delivery, and
preserves denied/unavailable outcomes. Removing first-use preregistration fails
that regression. Existing integration tests cover schedule boundary/filter policy.

The opt-in `cargo test -p sonos-volume-bridge native_development_notification_smoke
-- --ignored --nocapture` sends a real Windows test notification and checks native
API success. It passed locally after the fix. Automated success does not prove a
visible banner, native WebView layout, hardware timing or packaged delivery;
complete the manual sequences above before release.
