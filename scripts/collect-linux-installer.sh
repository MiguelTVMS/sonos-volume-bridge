#!/usr/bin/env bash
# Validate the native package architecture before assigning a release filename.
set -euo pipefail
bundle_dir="${1:?Bundle directory is required}"
release_tag="${2:?Release tag is required}"
release_arch="${3:?Release architecture is required}"
case "$release_arch" in
  amd64|arm64) ;;
  *) echo 'Unsupported Linux release architecture' >&2; exit 1 ;;
esac
shopt -s nullglob
installers=("$bundle_dir"/*.deb)
if [[ ${#installers[@]} -ne 1 || ! -s "${installers[0]}" ]]; then
  echo 'Expected exactly one nonempty Debian installer' >&2
  exit 1
fi
installer="${installers[0]}"
if [[ "$(dpkg-deb -f "$installer" Architecture)" != "$release_arch" ]]; then
  echo 'Debian installer architecture does not match release architecture' >&2
  exit 1
fi
cp "$installer" "sonos-volume-bridge-${release_tag}-linux-${release_arch}.deb"
