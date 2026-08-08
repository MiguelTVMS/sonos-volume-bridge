#!/usr/bin/env sh

set -eu

run_step() {
  printf '\n=== %s ===\n' "$1"
  shift
  "$@"
}

ROOT_DIR="$(git rev-parse --show-toplevel)"
cd "$ROOT_DIR"

staged_files="$(git diff --cached --name-only --diff-filter=ACMR || true)"

if [ -z "$staged_files" ]; then
  echo "No staged files found. Skipping pre-commit validation."
  exit 0
fi

run_rust_checks=false
run_ui_checks=false

if printf '%s\n' "$staged_files" | grep -E -q '^(Cargo\.toml|crates/|src-tauri/)'; then
  run_rust_checks=true
fi

if printf '%s\n' "$staged_files" | grep -E -q '^(ui/|ui/pnpm-lock.yaml$)'; then
  run_ui_checks=true
fi

if [ "$run_rust_checks" = true ]; then
  if ! command -v cargo >/dev/null 2>&1; then
    echo "cargo is required for Rust checks, but it was not found in PATH."
    exit 1
  fi
  if ! command -v pnpm >/dev/null 2>&1; then
    echo "pnpm is required for validation aliases, but it was not found in PATH."
    exit 1
  fi

  run_step "Running Rust format check" pnpm run ci:rustfmt
  run_step "Running Rust clippy lint" pnpm run ci:clippy
  run_step "Running Rust tests" pnpm run ci:test
fi

if [ "$run_ui_checks" = true ]; then
  if ! command -v pnpm >/dev/null 2>&1; then
    echo "pnpm is required for UI checks, but it was not found in PATH."
    exit 1
  fi

  if [ ! -d "ui/node_modules" ]; then
    run_step "Installing UI dependencies" pnpm --dir ui install --frozen-lockfile
  fi

  run_step "Running UI format check" pnpm run ci:ui-format
  run_step "Running UI lint" pnpm run ci:ui-lint
fi

if [ "$run_rust_checks" = false ] && [ "$run_ui_checks" = false ]; then
  echo "No Rust or UI files were staged. No validation executed."
fi



