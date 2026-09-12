#!/usr/bin/env bash
set -euo pipefail

root_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
entitlements="$root_dir/src-tauri/Entitlements.plist"
app_store_entitlements="$root_dir/src-tauri/Entitlements.appstore.plist.template"
tauri_config="$root_dir/src-tauri/tauri.conf.json"
app_store_config="$root_dir/src-tauri/tauri.appstore.conf.json"
direct_config="$root_dir/src-tauri/tauri.direct.conf.json"

test "$(/usr/libexec/PlistBuddy -c 'Print :com.apple.security.app-sandbox' "$entitlements")" = 'true'
test "$(/usr/libexec/PlistBuddy -c 'Print :com.apple.security.network.client' "$entitlements")" = 'true'
test "$(/usr/libexec/PlistBuddy -c 'Print :com.apple.security.network.server' "$entitlements")" = 'true'
test "$(/usr/libexec/PlistBuddy -c 'Print :com.apple.security.app-sandbox' "$app_store_entitlements")" = 'true'
test "$(/usr/libexec/PlistBuddy -c 'Print :com.apple.security.network.client' "$app_store_entitlements")" = 'true'
test "$(/usr/libexec/PlistBuddy -c 'Print :com.apple.security.network.server' "$app_store_entitlements")" = 'true'
test "$(/usr/libexec/PlistBuddy -c 'Print :com.apple.application-identifier' "$app_store_entitlements")" = '__APPLICATION_IDENTIFIER__'
test "$(/usr/libexec/PlistBuddy -c 'Print :com.apple.developer.team-identifier' "$app_store_entitlements")" = '__TEAM_IDENTIFIER__'

python3 - "$tauri_config" "$app_store_config" "$direct_config" <<'PY'
import json
import sys

base = json.load(open(sys.argv[1]))
app_store = json.load(open(sys.argv[2]))
direct = json.load(open(sys.argv[3]))

bundle = base["bundle"]
assert bundle["category"] == "Utility"
assert bundle["macOS"]["entitlements"] == "./Entitlements.plist"
assert bundle["macOS"]["minimumSystemVersion"] == "13.0"
assert app_store["bundle"]["macOS"]["entitlements"] == "./Entitlements.appstore.plist"
assert app_store["bundle"]["macOS"]["files"]["embedded.provisionprofile"] == "macos-app-store.provisionprofile"
assert direct["bundle"]["macOS"]["entitlements"] == "./Entitlements.direct.plist"
assert direct["bundle"]["macOS"]["files"]["embedded.provisionprofile"] == "macos-developer-id.provisionprofile"
PY
