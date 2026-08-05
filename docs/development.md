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
