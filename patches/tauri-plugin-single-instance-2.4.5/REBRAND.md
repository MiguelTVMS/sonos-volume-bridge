# Local changes

Based on tauri-plugin-single-instance 2.4.5 (original licenses retained).

Windows mutex/window namespace and macOS socket namespace have a Speaker Volume Bridge suffix. Bundle identity is unchanged. This lets the renamed app open Settings when the old app is running, while two renamed instances retain the same single-instance namespace. Demo identifiers remain distinct. Linux uses the public dbus_id builder option in the shell.

When upgrading the plugin, retain this namespace until a tested migration replaces it. Test old/new coexistence and new/new rejection on each native OS before release.
