# Development

Install Rust 1.98.1 (including `clippy` and `rustfmt`), Node.js 26, and pnpm 12. Run the complete local verification suite with:

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

Run the settings tests with `pnpm --dir ui test`. For rendered UI coverage, run
`pnpm --dir ui exec playwright install chromium` once, then
`pnpm --dir ui run test:ui`. The browser suite starts a separate preview server
and exercises the production settings renderer with mocked device commands.
Both suites run in the UI tests pull-request workflow, followed by a frontend build.

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

Local development uses the Rust version in `rust-toolchain.toml`. CI installs the
latest stable Rust through `dtolnay/rust-toolchain@v1` and sets
`RUSTUP_TOOLCHAIN=stable` so the local pin does not override that selection.
Lockfile and workflow changes also trigger Rust checks.

CI actions use their latest stable major tags. Node.js and pnpm are selected by
major version (26 and 12), allowing stable minor and patch updates. The browser
test job has a ten-minute timeout to bound failures during setup or teardown.
Playwright launches Vite directly through Node. A nested `pnpm run dev` leaves
Vite in a separate process group with pnpm 11.27.1 and 12.6.0 on Linux, causing
shutdown to hang after every browser test passes. The browser suite must both
pass its assertions and exit successfully to validate server cleanup.

The frontend uses TypeScript 7 for builds. Its `typescript` dependency aliases
`@typescript/typescript6` to supply the compiler API required by typescript-eslint;
`@typescript/native` aliases the stable TypeScript 7 package and provides `tsc`.
See [Microsoft's compatibility guidance](https://devblogs.microsoft.com/typescript/announcing-typescript-7-0/#running-side-by-side-with-typescript-6.0).

Windows bindings use the compatible 0.62.2 baseline defined in the workspace.
The audio adapter and application shell inherit these versions. Upgrade `windows`
and `windows-core` together and validate on Windows before changing this baseline.
`windows-core` 0.100 is incompatible with the published `windows` 0.62 bindings.
The vendored Linux GLib patch remains tied to Tauri's dependency graph.

## Dependency refresh (2026-09-25)

Direct Rust and frontend dependencies were checked against stable registry releases,
and the lockfiles were refreshed within upstream compatibility constraints. The
updated direct crates are Tauri 2.11.6, the single-instance plugin 2.4.5, rand
0.10.3, and thiserror 2.0.21. Frontend updates include ESLint 10.11.0, Prettier
3.9.9, typescript-eslint 8.70.1, and Vite 8.3.1. The TypeScript compiler/API aliases
remain necessary for lint-tool compatibility.

GitHub Actions use the latest stable release tags verified for this refresh,
including configure-pages 6.0.0, upload-pages-artifact 5.0.0, and deploy-pages
5.0.1. The Pages actions run on GitHub-hosted Ubuntu runners. PR checks do not
perform a production Pages deployment; verify the Website workflow when these
changes reach the release branch used for publishing.

The following upstream constraints remain:

- `windows` 0.62.2 is the latest published stable bindings crate. Keep
  `windows-core` at 0.62.2: the COM implementation macros require a direct matching
  core dependency, and upgrading only core to 0.100.0 breaks Windows compilation.
- The GTK/GLib dependency chain pins `toml_datetime` 0.6.3 and `toml_edit` 0.20.2
  through `proc-macro-crate` 2.0.2, which also constrains the older `toml` branch.
- Tauri's code-generation dependency chain uses `crypto-common` 0.1.7, which pins
  `generic-array` exactly to 0.14.7.

Local verification used Homebrew Rust 1.98.1, the existing Node.js 24 installation,
and pnpm 12. The frontend lockfile also accepts frozen installation with CI's
pnpm 11. Homebrew Rust updates use `brew update` followed by `brew upgrade rust`.

## Settings presentation preview

Run `pnpm --dir ui dev` and open `/preview.html` on the local development server.
Use `?platform=macos`, `?platform=windows`, or `?platform=linux` to review each
presentation. Add `&appearance=dark` or `&appearance=light` to force a color scheme.
The preview uses sample data and mocks all Tauri commands; it does
not control speakers or write app configuration. It is not included in the
production frontend build. Test at a 740-pixel width on macOS, 960 on Windows
(also its 760-pixel minimum), and 600 on Linux, and both default
and minimum window heights, including dark mode, keyboard focus, and increased contrast.

## Night schedule branch testing

For the pre-PR Windows/Linux trial, check out `feat/night-mode-schedule` and follow
[the branch trial instructions](verification-matrix.md#windows-and-linux-branch-trial).
The Branch desktop check workflow runs native checks and publishes debug binaries
without opening a PR. Production builds always follow the machine clock; there is
no forced AM/PM layout preview. Schedule tests run with ordinary frontend and Rust
suites. Physical speaker, sleep/wake, and packaged notification checks remain in
the verification matrix.
