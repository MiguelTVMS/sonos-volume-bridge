# ADR 0012: Platform-specific settings presentation

**Status:** Accepted (2026-09-25)

## Decision

Keep one accessible settings form and command contract, with separate CSS
presentations selected from the desktop WebView's host operating-system user
agent. macOS uses grouped inset rows, colored section icons, subtle sidebar
selection, and compact controls inspired by System Settings. Windows uses Segoe,
outlined cards, and a selection indicator. Linux uses the system font and an
Ubuntu accent. Unknown hosts retain the base presentation.

The Windows presentation follows Windows 11 Settings: a soft background, colored
sidebar icons, a short blue selection marker, larger page headings, individual
outlined setting cards, trailing controls, and 40-by-20 switches. Windows-only
cards include decorative outline icons and secondary descriptions. Caption text
is escaped, and the shared controls retain their labels and keyboard operation.
Windows-only rules live in `ui/src/windows.css`; the form and command handlers remain shared.
The native Windows window starts at 960 by 760 logical pixels and can resize down
to 760 by 460. Below 800 pixels, selectors and sliders move below their labels.
Native title-bar controls remain in place. Light, dark, increased-contrast,
forced-colors, and reduced-motion preferences are handled in the presentation.
The background approximates the reference material with CSS; it is not native Mica.

Platform styling stays in the frontend; domain and synchronization behavior is
unchanged. Color-scheme, increased-contrast, reduced-motion, and keyboard-focus
preferences apply across themes. Icons are decorative; buttons retain text labels.

## Verification

Platform selection has automated coverage for macOS, Windows, Linux, and unknown
hosts. The development-only preview uses mocked Tauri commands and sample devices
for repeatable visual checks without touching speakers or saved configuration.
It is a separate HTML entry point, excluded from the production build.

Check every section at the default and minimum window heights. Verify native
WebView rendering on each OS before release; browser previews do not establish
native platform compatibility.

The macOS window uses a 740-point width and 234-point sidebar, matching the
System Settings reference. Settings use trailing pop-up selectors and compact
white switch and slider thumbs. HTML controls retain native keyboard semantics;
these are WebView controls, not AppKit or SwiftUI controls, so exact system
materials and animations are not guaranteed.

The default macOS height is 700 points to give Night schedule more vertical room.
Width remains fixed at 740 points; users can still resize vertically down to 460.

Apple references: [Pop-up buttons](https://developer.apple.com/design/human-interface-guidelines/pop-up-buttons),
[Toggles](https://developer.apple.com/design/human-interface-guidelines/toggles), and
[Sliders](https://developer.apple.com/design/human-interface-guidelines/sliders).

macOS pop-up controls size to the selected label rather than the longest option
or the trailing column. A hidden, accessibility-excluded label provides intrinsic
width and updates on selection changes; long values remain capped to the row.
Initialize these sizing labels after mounting controls and restoring saved values,
including Night schedule notifications, so rerenders measure the displayed option.

Native window focus controls the macOS accent appearance: the selected sidebar
row and checked switches use the accent while active and neutral gray while
inactive. Focus changes never modify switch values. Subscribe before reading the
initial focus state and ignore a late snapshot after a newer focus event.

The macOS title bar overlays the frontend, retaining native traffic-light window
controls without a separate title strip. A reserved top drag region and content
insets keep the controls unobstructed. Windows and Linux retain standard chrome.

The macOS toolbar includes previous/next section buttons and the current section
title. Navigation follows sidebar order, does not wrap at the ends, and uses the
same page activation path as the sidebar, including diagnostic refreshes.
