#!/usr/bin/env bash
# Publish one permanent filename per package; the release tag supplies the version.
set -euo pipefail
assets_dir="${1:?Usage: prepare-release-downloads.sh ASSETS_DIR RELEASE_TAG}"
release_tag="${2:?Release tag is required}"
suffixes=(macos.zip macos.dmg windows-x64-unsigned.exe windows-arm64-unsigned.exe linux-amd64.deb linux-arm64.deb)
sources=()
destinations=()

# Validate the complete set before moving anything. Accept normalized files on retry.
for suffix in "${suffixes[@]}"; do
  source_path="$assets_dir/sonos-volume-bridge-$release_tag-$suffix"
  destination_suffix="$suffix"
  if [[ "$suffix" == windows-x64-unsigned.exe ]]; then
    # Retain the established x64 URL without publishing a second copy.
    destination_suffix=windows-unsigned.exe
  fi
  destination_path="$assets_dir/sonos-volume-bridge-$destination_suffix"
  if [[ ! -e "$source_path" ]]; then
    source_path="$destination_path"
  fi
  if [[ ! -s "$source_path" ]]; then
    echo "Missing or empty release installer: $source_path" >&2
    exit 1
  fi
  sources+=("$source_path")
  destinations+=("$destination_path")
done
for index in "${!sources[@]}"; do
  if [[ "${sources[$index]}" != "${destinations[$index]}" ]]; then
    mv "${sources[$index]}" "${destinations[$index]}"
  fi
done
