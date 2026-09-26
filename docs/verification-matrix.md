# Hardware verification matrix

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
   Save unchanged again: notification repeats if its direction is selected, with no redundant Sonos
   write. Clear the current block and Save: Night Mode turns off. Permission denial
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

11. In the schedule form, verify Never/On Start/On End/Both at both boundaries and
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
