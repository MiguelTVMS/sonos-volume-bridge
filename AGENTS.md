# SonosVolumeBridge contribution guide

Keep the domain crate free of Tauri, operating-system APIs and networking. The
synchronization crate depends only on domain abstractions; adapters belong in
future crates. Add tests with every behavior change and run `cargo fmt --check`,
`cargo clippy --workspace --all-targets -- -D warnings`, and `cargo test --workspace`.

