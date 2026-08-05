# Development

Install Rust 1.85.0 (including `clippy` and `rustfmt`), then run:

```sh
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

Windows Core Audio is present in Phase 3 and must be exercised on a Windows machine:

```sh
cargo run -p sonos-volume-bridge-platform-audio --example windows_audio_probe
```

On macOS, exercise the native listener and output controls with:

```sh
cargo run -p sonos-volume-bridge-platform-audio --example macos_audio_probe
```

macOS Core Audio, Tauri, and the production GENA callback listener remain for
later phases. The `test-support` crate has a small local RenderingControl mock
server for integration tests.

For the settings UI, install Node.js dependencies in `ui`, then run:

```sh
npm --prefix ui run build
npm --prefix ui run lint
npm --prefix ui run format
```

Logs are written as daily rolling files in the application log directory. Use
`info` by default; enable `debug` or `trace` only for a short diagnostic session.

## Phase tracking

Each implementation phase has a GitHub issue. Create the issue before changing
code, post concise progress and validation updates while working, then commit,
push `develop`, and close it when the phase is complete. Create the next phase's
issue before stopping and wait for explicit approval before implementing it.
