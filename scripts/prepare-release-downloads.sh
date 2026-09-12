#!/usr/bin/env bash
# Add stable download names while preserving versioned release assets.
set -euo pipefail
assets_dir="${1:?Usage: prepare-release-downloads.sh ASSETS_DIR RELEASE_TAG}"
release_tag="${2:?Release tag is required}"
suffixes=(macos.zip windows-unsigned.exe linux-amd64.deb)

# Validate every input before creating aliases.
for suffix in "${suffixes[@]}"; do
  source_path="$assets_dir/sonos-volume-bridge-$release_tag-$suffix"
  if [[ ! -s "$source_path" ]]; then
    echo "Missing or empty release installer: $source_path" >&2
    exit 1
  fi
done
for suffix in "${suffixes[@]}"; do
  cp "$assets_dir/sonos-volume-bridge-$release_tag-$suffix" \
    "$assets_dir/sonos-volume-bridge-$suffix"
done
