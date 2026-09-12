# Development

Install Rust 1.98.1 (including `clippy` and `rustfmt`), Node.js 22, and pnpm
11. Run the complete local verification suite with:

```sh
pnpm run ci:all
```

Set up local environment variables from a template:

```sh
cp .env.example .env.development
# edit .env.development with local values
```

Local `pnpm`/Rust CI scripts already load `.env.development` first and then keep
any already-exported environment variables from your shell. So `pnpm run ci:all`
works directly.

If you use `npx dotenvx run --`, use that as well, but keep the local scripts in
`scripts/local/with-env.sh` as the default path:

```sh
./scripts/local/resolve-apple-secrets.sh
pnpm run ci:all
```

## Pre-commit validation

Enable local hooks once per clone:

```sh
git config core.hooksPath .githooks
```

The pre-commit hook runs the same baseline checks automatically for staged Rust and UI
changes:

- Rust: `pnpm run ci:rustfmt`, `pnpm run ci:clippy`, `pnpm run ci:test`
- UI: `pnpm run ci:ui-format`, `pnpm run ci:ui-lint`

Run the validation manually at any time:

```sh
pnpm precommit
```

or run all CI-aligned checks:

```sh
pnpm run ci:all
```

Run the Tauri application during development with:

```sh
pnpm --dir ui install
cargo tauri dev
```

## Ubuntu

Ubuntu development and runtime require the desktop packages needed by Tauri,
plus `pulseaudio-utils` for the PulseAudio-compatible `pactl` interface. This
works with either PulseAudio or PipeWire's `pipewire-pulse` service:

```sh
sudo apt install libwebkit2gtk-4.1-dev libgtk-3-dev libayatana-appindicator3-dev pulseaudio-utils
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

## Microsoft Store package

The Store package is an additional Windows artifact; it does not replace the
NSIS installer used for direct downloads. Build and validate it on Windows with
the Windows SDK installed:

```powershell
cargo tauri build --no-bundle
./scripts/build-msix.ps1
```

The unsigned Partner Center package is written to
`target/release/bundle/msix`. Local installation requires a trusted development
signature whose subject exactly matches the Store publisher in the manifest.
Never commit a certificate or private key.

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

## Codex local environments

`.codex/environments/environment.toml` defines macOS, Windows, and Linux
worktree setup and platform-specific Run app, Check all, and Build app actions.
Setup fetches the locked dependencies and builds the frontend. Build app creates
an unbundled native build for the current operating system.

Install the toolchain listed above and Tauri CLI 2 (`cargo install tauri-cli
--version "^2" --locked`) before setup. macOS requires Xcode Command Line Tools.
Windows requires Visual Studio Build Tools with Desktop development with C++,
a Windows SDK, and WebView2 Runtime. Linux requires `pkg-config`, a C/C++ build
toolchain, the desktop development libraries listed in the Ubuntu section, and
`pulseaudio-utils`. Setup reports missing native prerequisites.

Windows actions use PowerShell and check native command exit codes. macOS and
Linux actions use Bash. Check all runs Rust formatting, Clippy, workspace tests,
and frontend formatting, lint, tests, and build without requiring signing secrets.

## Stable dependency baseline

The workspace and CI use Rust 1.98.1. Update `rust-toolchain.toml`, the workspace
`rust-version`, and workflow toolchain pins together, then run all validation.
Lockfile and workflow changes also trigger Rust checks.

The frontend uses TypeScript 7 for builds. Its `typescript` dependency aliases
`@typescript/typescript6` to supply the compiler API required by typescript-eslint;
`@typescript/native` aliases the stable TypeScript 7 package and provides `tsc`.
See [Microsoft's compatibility guidance](https://devblogs.microsoft.com/typescript/announcing-typescript-7-0/#running-side-by-side-with-typescript-6.0).

Windows bindings remain on the compatible 0.62 family. Upgrade `windows` and
`windows-core` together and validate on Windows before changing this baseline.
The vendored Linux GLib patch remains tied to Tauri's dependency graph.
