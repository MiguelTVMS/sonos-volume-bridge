# ADR 0012: Platform-specific settings presentation

**Status:** Accepted (2026-09-25)

## Decision

Keep one accessible settings form and command contract, with separate CSS
presentations selected from the desktop WebView's host operating-system user
agent. macOS uses grouped inset rows, colored section icons, subtle sidebar
selection, and compact controls inspired by System Settings. Windows uses Segoe,
outlined cards, and a selection indicator. Linux uses the system font and an
Ubuntu accent. Unknown hosts retain the base presentation.

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

Apple references: [Pop-up buttons](https://developer.apple.com/design/human-interface-guidelines/pop-up-buttons),
[Toggles](https://developer.apple.com/design/human-interface-guidelines/toggles), and
[Sliders](https://developer.apple.com/design/human-interface-guidelines/sliders).

macOS pop-up controls size to the selected label rather than the longest option
or the trailing column. A hidden, accessibility-excluded label provides intrinsic
width and updates on selection changes; long values remain capped to the row.
