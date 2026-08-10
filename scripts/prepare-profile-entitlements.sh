#!/usr/bin/env bash
set -euo pipefail

if [ "$#" -ne 2 ]; then
  echo "Usage: $0 <provisioning-profile> <entitlements-output>" >&2
  exit 64
fi

profile="$1"
output="$2"
profile_plist="$(mktemp)"
trap 'rm -f "$profile_plist"' EXIT

security cms -D -i "$profile" >"$profile_plist"
application_identifier="$(/usr/libexec/PlistBuddy -c 'Print :Entitlements:com.apple.application-identifier' "$profile_plist")"
team_identifier="$(/usr/libexec/PlistBuddy -c 'Print :Entitlements:com.apple.developer.team-identifier' "$profile_plist")"

python3 - "$application_identifier" "$team_identifier" "$output" <<'PY'
import plistlib
import sys

application_identifier, team_identifier, output = sys.argv[1:]
entitlements = {
    "com.apple.security.app-sandbox": True,
    "com.apple.security.network.client": True,
    "com.apple.security.network.server": True,
    "com.apple.application-identifier": application_identifier,
    "com.apple.developer.team-identifier": team_identifier,
}
with open(output, "wb") as file:
    plistlib.dump(entitlements, file, sort_keys=False)
PY
