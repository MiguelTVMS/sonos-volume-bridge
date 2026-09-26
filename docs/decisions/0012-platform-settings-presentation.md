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


## Ubuntu presentation (2026-09-26)

Linux uses `ui/src/linux.css` on both x86-64 and ARM64. The native window starts
at 1080 by 800 logical pixels. Width is fixed at 1080; vertical resizing remains
available down to 460. The default height fits Night schedule and its save notice
without scrolling.
A 270-pixel sidebar, neutral selection, symbolic icons, centered content, white
rounded groups with separated rows, trailing selectors, 48-by-26 switches and
white slider thumbs follow the Ubuntu Settings reference. At compact widths,
controls move below their labels. Ubuntu Sans and system sans-serif fallbacks
keep text consistent. Dark mode, focus, reduced motion and contrast remain supported.

These are accessible HTML controls in the existing WebView, not GTK4 widgets.
Native window chrome and select popups remain managed by the platform. The
frontend mirrors Yaru/GNOME presentation without replacing the shared command
contract or adopting a second native UI implementation. Slider fill is derived
from the current input value during render, refresh and user input; it does not
write configuration. Schedule cells explicitly retain square corners rather than
inheriting the rounded action-button shape.

References: [Ubuntu Yaru theme guidance](https://github.com/ubuntu/yaru/wiki/%233-Yaru-theme-suite-workflow-and-guidelines),
[GNOME boxed lists](https://developer.gnome.org/hig/patterns/containers/boxed-lists.html),
and [GNOME switches](https://developer.gnome.org/hig/patterns/controls/switches.html).

Browser tests navigate the production form, select and save schedule blocks,
check square cells before and after rerenders, and exercise switches and sliders
at default/minimum heights in both color schemes. The square-cell regression failed
with the previous generic Linux button radius and passes with the scoped styling.
Native Ubuntu WebKit and desktop integration still require the verification matrix.

The Ubuntu Volume test action keeps a 16-pixel inset and normal button sizing
instead of inheriting full settings-row padding. Browser regressions verify the
inset, keyboard activation, and that the default Night schedule view fits after
Save; both layout defects were reproduced with the previous presentation.

Ubuntu tray icons use the white glyph for both application color schemes: the
default Ubuntu top bar stays dark independently of the Settings window theme.
The disconnected red badge remains intact. Registration, theme notifications and
connection refreshes share this image selection. Other platforms retain their
existing theme-based selection. Custom Linux panels with light backgrounds are
not covered by this Ubuntu-specific contrast policy.
