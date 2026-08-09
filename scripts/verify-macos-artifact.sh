#!/usr/bin/env bash
set -euo pipefail

if [ "$#" -ne 2 ]; then
  echo "Usage: $0 <direct|app-store> <path-to-app>" >&2
  exit 64
fi

channel="$1"
app_path="$2"
entitlements="$(mktemp)"
profile_plist="$(mktemp)"
trap 'rm -f "$entitlements" "$profile_plist"' EXIT

test -d "$app_path"
codesign --verify --deep --strict --verbose=4 "$app_path"
codesign --display --entitlements :- "$app_path" >"$entitlements"
test "$(/usr/libexec/PlistBuddy -c 'Print :com.apple.security.app-sandbox' "$entitlements")" = 'true'
test "$(/usr/libexec/PlistBuddy -c 'Print :com.apple.security.network.client' "$entitlements")" = 'true'
test "$(/usr/libexec/PlistBuddy -c 'Print :com.apple.security.network.server' "$entitlements")" = 'true'

case "$channel" in
  direct)
    codesign --display --verbose=4 "$app_path" 2>&1 | grep -F 'Developer ID Application'
    codesign --display --verbose=4 "$app_path" 2>&1 | grep -F 'runtime'
    profile="$app_path/Contents/embedded.provisionprofile"
    if [ -f "$profile" ]; then
      security cms -D -i "$profile" >"$profile_plist"
      bundle_identifier="$(/usr/libexec/PlistBuddy -c 'Print :CFBundleIdentifier' "$app_path/Contents/Info.plist")"
      profile_application_identifier="$(/usr/libexec/PlistBuddy -c 'Print :Entitlements:application-identifier' "$profile_plist")"
      test "${profile_application_identifier#*.}" = "$bundle_identifier"
      test "$(/usr/libexec/PlistBuddy -c 'Print :com.apple.application-identifier' "$entitlements")" = "$profile_application_identifier"
      test "$(/usr/libexec/PlistBuddy -c 'Print :com.apple.developer.team-identifier' "$entitlements")" = "$(/usr/libexec/PlistBuddy -c 'Print :Entitlements:com.apple.developer.team-identifier' "$profile_plist")"
    fi
    ;;
  app-store)
    codesign --display --verbose=4 "$app_path" 2>&1 | grep -F 'Apple Distribution'
    profile="$app_path/Contents/embedded.provisionprofile"
    test -f "$profile"
    security cms -D -i "$profile" >"$profile_plist"
    bundle_identifier="$(/usr/libexec/PlistBuddy -c 'Print :CFBundleIdentifier' "$app_path/Contents/Info.plist")"
    profile_application_identifier="$(/usr/libexec/PlistBuddy -c 'Print :Entitlements:application-identifier' "$profile_plist")"
    test "${profile_application_identifier#*.}" = "$bundle_identifier"
    test "$(/usr/libexec/PlistBuddy -c 'Print :Entitlements:com.apple.security.app-sandbox' "$profile_plist")" = 'true'
    test "$(/usr/libexec/PlistBuddy -c 'Print :com.apple.application-identifier' "$entitlements")" = "$profile_application_identifier"
    test "$(/usr/libexec/PlistBuddy -c 'Print :com.apple.developer.team-identifier' "$entitlements")" = "$(/usr/libexec/PlistBuddy -c 'Print :Entitlements:com.apple.developer.team-identifier' "$profile_plist")"
    ;;
  *)
    echo "Unsupported distribution channel: $channel" >&2
    exit 64
    ;;
esac
