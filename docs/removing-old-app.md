# Fully removing Sonos Volume Bridge

Speaker Volume Bridge replaces the app previously named Sonos Volume Bridge.
Quit and remove the old app before using the renamed app. Running both can send
conflicting volume or Night Mode commands to the same speaker. Closing Settings
does not quit either app; use **Quit** in its menu bar or system tray menu.

These steps remove the app and its automatic startup entry. The optional clean
reset also removes saved preferences. It does not reset or uninstall your Sonos
speaker or the official Sonos controller app.

## macOS

1. If the old app still opens, turn off **Start at login**. If that fails, continue
   with the System Settings steps below.
2. Quit **Sonos Volume Bridge** and **Speaker Volume Bridge** from their menu bar
   menus. In Activity Monitor, search for either name or `sonos-volume-bridge`
   and `speaker-volume-bridge`. Quit any remaining bridge process.
3. Open **System Settings > General > Login Items & Extensions**. Under **Open at
   Login**, select any **Sonos Volume Bridge** or **Speaker Volume Bridge** entry
   and click the minus button. If an entry for either app remains under background
   activity, turn it off. Older macOS versions call this page **Login Items**.
4. In Finder's **Applications**, move **Sonos Volume Bridge.app** to Trash. Check
   your home folder's **Applications** and **Downloads** for another copy. For a
   completely fresh installation, remove **Speaker Volume Bridge.app** as well.
   Eject any mounted bridge disk image. Delete only the bridge apps from Trash
   when you are ready; other trashed files can stay there.
5. If you previously created your own LaunchAgent, open your home **Library**
   folder using Finder's **Go** menu while holding Option, then open
   **LaunchAgents**. Remove only a bridge-specific entry whose contents point to
   the removed app. Do not remove unrelated agents. The current app uses macOS
   login items and does not require a LaunchAgents file.
6. Restart the Mac if an old process or startup entry remains. macOS may retain
   a disabled background-item name for a while after removal.

### Optional: erase saved settings for a clean installation

**This also resets the renamed app's preferences.** The old and new names share
the same settings identity within a distribution. Keep these files if you want
to preserve your speaker choice, volume preferences and Night schedule. For a
clean reset, back them up first and keep both apps closed during removal.

In your home **Library** folder, remove the app's folders when present:

| Location inside Library | App-specific folder or file |
| --- | --- |
| Application Support | `ms.miguel.sonosvolumebridge.desktop` (contains `config.json`) |
| Logs | `ms.miguel.sonosvolumebridge.desktop` |
| Caches | `ms.miguel.sonosvolumebridge.desktop` |
| Preferences | `ms.miguel.sonosvolumebridge.desktop.plist` |
| WebKit | `ms.miguel.sonosvolumebridge.desktop` |
| Saved Application State | `ms.miguel.sonosvolumebridge.desktop.savedState` |

For a Mac App Store installation, the sandbox keeps app data in **Library >
Containers**. Remove only the container identified as
`ms.miguel.sonosvolumebridge.desktop`; Finder may display the app name. This also
removes its enclosed settings, caches and logs. If macOS refuses removal, leave
that folder in place rather than changing permissions on the Library folder.
Check both the direct-download data locations and the container if you previously
used both editions. Missing folders are normal.

### Reinstall and check

Download the current DMG, drag **Speaker Volume Bridge.app** into Applications,
eject the DMG, and launch the app from Applications. Choose your speaker and audio
output again if you erased settings. Test **Start at login** on and off; leave it
at your preferred setting. If removal still fails, remove the app's **Open at
Login** entry in System Settings and retry. Do not reset all macOS login items to
repair one app.

Apple documents [removing login items](https://support.apple.com/guide/mac-help/mh15189/mac)
and [uninstalling apps](https://support.apple.com/102610).

## Windows

1. Turn off **Start at login** if the old app opens, then quit both bridge apps
   from their system tray menus. Use Task Manager to end a remaining bridge
   process if necessary.
2. In **Settings > Apps > Startup** or Task Manager's **Startup apps**, disable
   any old bridge entry.
3. Open **Settings > Apps > Installed apps** and uninstall the old bridge. The
   entry may already use the new name after an update. For a full clean reinstall,
   uninstall the current bridge as well. Use the existing installer/uninstaller
   instead of deleting a program folder before uninstalling it.
4. For a direct-download installation, select the uninstaller's application-data
   removal option only if you want a clean reset. If leftover data remains, open
   your home **AppData** folder and remove only
   `ms.miguel.sonosvolumebridge.desktop` from **Roaming** and **Local**. These
   folders are shared with the renamed direct-download app; removing them erases
   its preferences too. For a Microsoft Store app, use its **Advanced options >
   Reset** before uninstalling if you want to erase its data.
5. Remove leftover shortcuts pointing to the old executable. Check the Startup
   folder by opening **Run** and entering `shell:startup`; check
   `shell:common startup` too if you manually added an all-user shortcut. Remove
   only shortcuts for the bridge. Restart if Windows reports files still in use.
6. Reinstall one edition, select your speaker/output, and check **Start at login**.

## Ubuntu / Linux

1. Turn off **Start at login**, then quit both bridge apps from their tray menus.
   Use System Monitor to stop a remaining bridge process if necessary.
2. Use the package manager to remove the package that is installed. The former
   package is `sonos-volume-bridge`; the renamed package is
   `speaker-volume-bridge`. For example, remove the old package with
   `sudo apt remove sonos-volume-bridge`. For a full clean reinstall of the
   renamed app, use `sudo apt remove speaker-volume-bridge` instead.
3. Open **Startup Applications** and remove the bridge entry. In your home
   folder, show hidden files and check **.config > autostart** for
   `sonos-volume-bridge.desktop` and any manually created bridge duplicates.
   Remove only the entries for this app. If you created a user service yourself,
   stop and disable that specific service before removing its definition.
4. To erase preferences as well, back up and remove only the
   `ms.miguel.sonosvolumebridge.desktop` folder under your home **.config**,
   **.local > share**, and **.cache**, where present. Custom XDG directory settings
   can move those locations. These data folders are shared with the renamed app.
5. Install the current Debian package and select your speaker/output again.
   Check that only one bridge startup entry is enabled.

The current package deliberately supplies `sonos-volume-bridge` as a command
alias for `speaker-volume-bridge`. That alias is not a second installation; let
the package manager maintain and remove it.

## Final check

- Only **Speaker Volume Bridge** is installed and running.
- The old app does not reopen after signing out and back in.
- **Start at login** matches your preference.
- Speaker selection, audio output, maximum volume and Night schedule are correct.

You do not need to delete shared settings for an ordinary upgrade. Use the clean
reset steps only when you intend to start over.
