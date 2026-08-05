# Development

Install Rust 1.85.0 (including `clippy` and `rustfmt`), then run:

```sh
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

Windows Core Audio, macOS Core Audio, Sonos networking, Tauri, and the mock
Sonos server are intentionally not part of Phase 1.

