# Development

Install Rust 1.88.0 (including `clippy` and `rustfmt`), Node.js 22, and pnpm
11. Run the complete local verification suite with:

```sh
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
pnpm --dir ui install --frozen-lockfile
pnpm --dir ui run format
pnpm --dir ui run lint
pnpm --dir ui run build
```

Run the Tauri application during development with:

```sh
pnpm --dir ui install
cargo tauri dev
```

## Visual Studio Code on Windows

Open the repository root in VS Code and accept the recommended extensions. The
workspace includes Rust, Tauri, ESLint, Prettier, TOML, and debugger extension
recommendations, plus LF line-ending and format-on-save settings.

1. Run the **UI: install dependencies** task once.
2. Choose **Tauri: Debug desktop app** from Run and Debug and press F5.

The launch configuration starts the Vite development server, builds the Rust
application through the Visual Studio Build Tools environment, and attaches
the Windows debugger. If Visual Studio Build Tools are installed elsewhere,
update `sonosVolumeBridge.vsDevCmd` in `.vscode/settings.json`.

Additional tasks are available for the Rust workspace verification suite, the
UI production build, and the Windows Core Audio probe.

Exercise the native adapters on their respective platforms:

```sh
cargo run -p sonos-volume-bridge-platform-audio --example windows_audio_probe
cargo run -p sonos-volume-bridge-platform-audio --example macos_audio_probe
```

The `test-support` crate provides a local RenderingControl mock server and
recorded XML fixtures for protocol and integration tests. Application logs are
written as daily rolling files in the application log directory. Use `info` by
default; enable `debug` or `trace` only for a short diagnostic session.

## Phase tracking

Each implementation phase has a GitHub issue. Create the issue before changing
code, post concise progress and validation updates while working, then commit
and push `develop`. Close a phase issue only after the work is complete. Create
the next phase's issue before stopping and wait for explicit approval before
implementing it.
