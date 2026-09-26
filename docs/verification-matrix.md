# Hardware verification matrix

## Windows 11 settings presentation

- Automated: the normal frontend suite validates platform selection, native
  window identity, resize bounds, tray-first startup, and retained Windows chrome.
- Automated browser tests run the production renderer through the isolated preview:
  all six pages at default/minimum size in light/dark mode, visible sidebar footer,
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

## Camera-triggered Speech Enhancement (#89)

**Release status: not hardware validated. Both build features remain off.** Linux is
out of scope and tracked separately in #125. Run the following independently for
macOS direct, macOS App Store, Windows NSIS, and Windows MSIX distributions before
enabling that distribution's corresponding Cargo feature. Never reuse a binary
between distribution gates that differ.

| Scenario | Exact interaction and expected result |
| --- | --- |
| Privacy and disabled default | Start a fresh feature-off build: option is visibly unavailable. In a native candidate build leave opt-in off, then enable it with cameras idle. No camera LED, permission prompt, capture session, or media access should be caused by observation. No identifiers or activity history appear in logs/exports. |
| Camera classes | Repeat with built-in, USB, and exposed virtual cameras. Start capture in a separate app: after one second speech turns on and Settings/tray identify automation. Stop capture: after three seconds speech returns off. Virtual source initialization without consumers must not falsely count as a call. |
| Already enabled | Enable speech manually before starting the camera. Start/stop capture: speech remains on and is not labelled as an automation-owned change. |
| Startup timing | Start capture first, then launch the bridge with opt-in saved. The initial snapshot detects ongoing capture; normal debounce and guarded activation apply without needing a new camera event. |
| Overlap and gaps | Start two cameras, stop one, and confirm speech stays on. Stop the last camera: restoration waits three seconds. Restart capture during that gap: no off/on churn. Capture bursts shorter than one second do not write. |
| Manual override | During owned speech, toggle speech from Settings and separately from tray; repeat using another controller and allow authoritative refresh. The manual value remains, automation pauses until the next complete camera period, and no stale restoration overwrites it. |
| Settings and switching | During owned speech save an unrelated volume setting: ownership persists. Select another compatible speaker: old speaker restores before new activation. Repeat with old speaker unreachable: warning remains visible, and no delayed restoration targets the replacement. |
| Failure and recovery | Interrupt observation, disconnect the speaker, restart camera services, and reconnect devices. Unknown never becomes inactive; ownership is abandoned after a gap and later recovery cannot disable a pre-existing on value. Volume sync remains operational when only camera observation fails. |
| Suspend and hot-plug | Suspend during owned speech; resume with camera stopped and running. Repeat unplug/replug, including replacement during the inactive grace period. No stale ownership restores. Confirm OS callbacks/resume delivery and re-enumeration under each package type. |
| Disable/reset/quit/crash | While owned, disable, reset, or normally quit: guarded cleanup completes or times out within its deadline. Force-terminate instead and restart: no restoration journal exists, so a remaining on value is preserved. |

CI covers shared production orchestration, ownership, debounce, configuration
compatibility, partial inventory aggregation, UI explanations, and native feature
compilation. It does not validate camera-driver completeness, native event-source
silence, packaging permissions, actual capture, physical Sonos firmware, or timing
of OS suspend notifications. Record sanitized pass/fail outcomes privately; do not
attach camera/app identifiers or activity traces to issues or PR descriptions.

The production-coordinator regression for an explicit manual “on” action was
mutation-checked: deliberately retaining automation ownership produced an unwanted
restoration write and failed the test; restoring ownership revocation passed it.
Listener orchestration tests also verify registration precedes the initial read,
a device change during registration yields unknown, failed registration retries,
and shutdown removes listeners. These tests do not substitute for native hardware
and distribution validation.
