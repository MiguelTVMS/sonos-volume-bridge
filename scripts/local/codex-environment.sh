#!/usr/bin/env bash
# Entry point for Codex local environments on macOS and Linux.
set -euo pipefail
cd "$(dirname "$0")/../.."

case "${1:-}" in
  setup)
    command -v rustup >/dev/null
    command -v pnpm >/dev/null
    case "$(uname -s)" in
      Darwin) xcode-select -p >/dev/null ;;
      Linux)
        if ! command -v pkg-config >/dev/null || ! pkg-config --exists gtk+-3.0 webkit2gtk-4.1 ayatana-appindicator3-0.1; then
          echo 'Install Linux prerequisites first. On Ubuntu/Debian:' >&2
          echo 'sudo apt-get install build-essential pkg-config libgtk-3-dev libwebkit2gtk-4.1-dev libayatana-appindicator3-dev pulseaudio-utils' >&2
          exit 1
        fi
        if ! command -v pactl >/dev/null; then
          echo 'Install pulseaudio-utils for the Linux audio adapter.' >&2
          exit 1
        fi
        ;;
    esac
    rustup show active-toolchain
    cargo tauri --version
    cargo fetch --locked
    pnpm --dir ui install --frozen-lockfile
    pnpm --dir ui run build
    ;;
  run) exec cargo tauri dev ;;
  check)
    cargo fmt --check
    cargo clippy --workspace --all-targets -- -D warnings
    cargo test --workspace
    pnpm --dir ui run format
    pnpm --dir ui run lint
    pnpm --dir ui test
    pnpm --dir ui run build
    ;;
  build) exec cargo tauri build --no-bundle ;;
  *) echo "Usage: $0 {setup|run|check|build}" >&2; exit 2 ;;
esac
