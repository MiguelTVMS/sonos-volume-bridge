# Development

Install Rust 1.85.0 (including `clippy` and `rustfmt`), then run:

```sh
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

Windows Core Audio, macOS Core Audio, Tauri, and the production GENA callback
listener are intentionally not part of Phase 2. The `test-support` crate has a
small local RenderingControl mock server for integration tests.
