#!/usr/bin/env bash
set -euo pipefail

profile_path='src-tauri/macos-app-store.provisionprofile'
entitlements_path='src-tauri/Entitlements.appstore.plist'
package_path="${APP_STORE_PACKAGE_PATH:-target/release/bundle/macos/sonos-volume-bridge-macos-app-store.pkg}"
unsigned_package_path="${package_path}.unsigned"

required_variables=(
  KEYCHAIN_PASSWORD
  APPLE_APP_STORE_CERTIFICATE
  APPLE_APP_STORE_CERTIFICATE_PASSWORD
  APPLE_MAC_INSTALLER_CERTIFICATE
  APPLE_MAC_INSTALLER_CERTIFICATE_PASSWORD
  APPLE_APP_STORE_PROVISIONING_PROFILE
  APPLE_APP_STORE_SIGNING_IDENTITY
  APPLE_MAC_INSTALLER_IDENTITY
)

for variable_name in "${required_variables[@]}"; do
  if [ -z "${!variable_name:-}" ]; then
    echo "Missing required environment variable: $variable_name" >&2
    exit 64
  fi
done

for generated_path in "$profile_path" "$entitlements_path" "$package_path" "$unsigned_package_path"; do
  if [ -e "$generated_path" ]; then
    echo "Refusing to overwrite existing path: $generated_path" >&2
    exit 64
  fi
done

temp_dir="$(mktemp -d "${TMPDIR:-/tmp}/sonos-app-store-signing.XXXXXX")"
keychain_path="$temp_dir/apple-app-store.keychain-db"
app_certificate_path="$temp_dir/app-store.p12"
installer_certificate_path="$temp_dir/mac-installer.p12"
wwdr_certificate_path="$temp_dir/AppleWWDRCAG3.cer"
original_keychains_path="$temp_dir/original-keychains"
original_default_keychain="$(security default-keychain -d user | sed -E 's/^[[:space:]]*"?([^"[:space:]]+)"?[[:space:]]*$/\1/')"
original_keychains=()
created_profile=0
created_entitlements=0

security list-keychains -d user >"$original_keychains_path"
while IFS= read -r keychain_line; do
  keychain_line="${keychain_line#"${keychain_line%%[![:space:]]*}"}"
  keychain_line="${keychain_line#\"}"
  keychain_line="${keychain_line%\"}"
  [ -n "$keychain_line" ] && original_keychains+=("$keychain_line")
done <"$original_keychains_path"

cleanup() {
  status=$?
  trap - EXIT

  if [ -n "$original_default_keychain" ]; then
    security default-keychain -d user -s "$original_default_keychain" >/dev/null 2>&1 || true
  fi
  if [ "${#original_keychains[@]}" -gt 0 ]; then
    security list-keychains -d user -s "${original_keychains[@]}" >/dev/null 2>&1 || true
  fi
  security delete-keychain "$keychain_path" >/dev/null 2>&1 || true
  [ "$created_profile" -eq 0 ] || rm -f "$profile_path"
  [ "$created_entitlements" -eq 0 ] || rm -f "$entitlements_path"
  rm -f "$unsigned_package_path"
  rm -rf "$temp_dir"

  exit "$status"
}
trap cleanup EXIT

pnpm --dir ui install --frozen-lockfile

if ! cargo tauri --version >/dev/null 2>&1; then
  cargo install tauri-cli --version '^2' --locked
fi

security create-keychain -p "$KEYCHAIN_PASSWORD" "$keychain_path"
security set-keychain-settings -lut 21600 "$keychain_path"
security unlock-keychain -p "$KEYCHAIN_PASSWORD" "$keychain_path"

curl --fail --silent --show-error --location \
  https://www.apple.com/certificateauthority/AppleWWDRCAG3.cer \
  --output "$wwdr_certificate_path"
security import "$wwdr_certificate_path" -k "$keychain_path" >/dev/null
printf '%s' "$APPLE_APP_STORE_CERTIFICATE" | base64 -D >"$app_certificate_path"
printf '%s' "$APPLE_MAC_INSTALLER_CERTIFICATE" | base64 -D >"$installer_certificate_path"
security import "$app_certificate_path" -k "$keychain_path" -P "$APPLE_APP_STORE_CERTIFICATE_PASSWORD" -T /usr/bin/codesign -T /usr/bin/productsign >/dev/null
security import "$installer_certificate_path" -k "$keychain_path" -P "$APPLE_MAC_INSTALLER_CERTIFICATE_PASSWORD" -T /usr/bin/codesign -T /usr/bin/productsign >/dev/null
security set-key-partition-list \
  -S apple-tool:,apple:,codesign: \
  -s \
  -k "$KEYCHAIN_PASSWORD" \
  "$keychain_path" >/dev/null

# Match GitHub Actions: expose only the temporary keychain while signing.
security list-keychains -d user -s "$keychain_path"
security default-keychain -d user -s "$keychain_path"

find_identity_line() {
  identities="$1"
  identity_hint="$2"
  fallback_pattern="$3"
  identity_line="$(printf '%s\n' "$identities" | awk -v identity_hint="$identity_hint" 'index(tolower($0), tolower(identity_hint)) > 0 { print; exit }')"
  if [ -z "$identity_line" ]; then
    identity_line="$(printf '%s\n' "$identities" | awk -v pattern="$fallback_pattern" 'tolower($0) ~ pattern { print; exit }')"
  fi
  printf '%s' "$identity_line"
}

codesigning_identities="$(security find-identity -v -p codesigning "$keychain_path")"
installer_identities="$(security find-identity -v -p basic "$keychain_path")"
app_identity_line="$(find_identity_line "$codesigning_identities" "$(printf '%b' "$APPLE_APP_STORE_SIGNING_IDENTITY")" '3rd party mac developer application|apple distribution')"
installer_identity_line="$(find_identity_line "$installer_identities" "$(printf '%b' "$APPLE_MAC_INSTALLER_IDENTITY")" '3rd party mac developer installer|apple developer installer|mac installer distribution')"

if [ -z "$app_identity_line" ] || [ -z "$installer_identity_line" ]; then
  echo 'Failed to resolve both App Store signing identities.' >&2
  exit 1
fi

export APPLE_SIGNING_IDENTITY="$(printf '%s' "$app_identity_line" | awk '{print $2}')"
export APPLE_MAC_INSTALLER_IDENTITY="$(printf '%s' "$installer_identity_line" | awk '{print $2}')"

if [ "$APPLE_SIGNING_IDENTITY" = "$APPLE_MAC_INSTALLER_IDENTITY" ]; then
  echo 'The App Store application and installer identities resolved to the same certificate.' >&2
  exit 1
fi

printf '%s' "$APPLE_APP_STORE_PROVISIONING_PROFILE" | base64 -D >"$profile_path"
created_profile=1
  chmod 644 "$profile_path"
security cms -D -i "$profile_path" >/dev/null
./scripts/prepare-profile-entitlements.sh "$profile_path" "$entitlements_path"
created_entitlements=1

# Developer ID certificate variables are valid for direct distribution but
# must not influence this App Store bundle.
unset APPLE_CERTIFICATE APPLE_CERTIFICATE_PASSWORD
unset APPLE_API_KEY APPLE_API_ISSUER APPLE_API_PRIVATE_KEY APPLE_API_KEY_PATH

cargo tauri build --bundles app --config src-tauri/tauri.appstore.conf.json --no-sign
app_path='target/release/bundle/macos/Sonos Volume Bridge.app'
app_executable="$app_path/Contents/MacOS/sonos-volume-bridge"
xattr -cr "$app_path"
codesign --force --sign "$APPLE_SIGNING_IDENTITY" --keychain "$keychain_path" --options runtime --entitlements "$entitlements_path" "$app_executable"
codesign --force --sign "$APPLE_SIGNING_IDENTITY" --keychain "$keychain_path" --options runtime --entitlements "$entitlements_path" "$app_path"
./scripts/verify-macos-artifact.sh app-store "$app_path"
xcrun productbuild --component "$app_path" /Applications "$unsigned_package_path"
xcrun productsign --sign "$APPLE_MAC_INSTALLER_IDENTITY" --keychain "$keychain_path" "$unsigned_package_path" "$package_path"
pkgutil --check-signature "$package_path" 2>&1 | grep -F '3rd Party Mac Developer Installer:' >/dev/null

printf '%s\n' "Signed App Store package created: $package_path"
