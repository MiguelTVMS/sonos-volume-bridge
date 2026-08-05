# SonosVolumeBridge contribution guide

Keep the domain crate free of Tauri, operating-system APIs and networking. The
synchronization crate depends only on domain abstractions; adapters belong in
future crates. Add tests with every behavior change and run `cargo fmt --check`,
`cargo clippy --workspace --all-targets -- -D warnings`, and `cargo test --workspace`.

## Phase tracking workflow

Every implementation phase must be tracked by a GitHub issue in this repository.

1. Create the phase issue before making implementation changes.
2. Keep that issue updated with concise progress, decisions, validation results,
   and blockers while the phase is in progress.
3. Keep all issue text, code, documentation, tests, and commit messages in English.
4. At phase completion, commit and push `develop`, then close the current issue
   with a completion summary.
5. Create the issue for the next phase before stopping.
6. Stop at that phase boundary and ask the user for explicit approval before
   beginning implementation of the next phase.

Use `gh` for GitHub issue operations when it is available. Preserve historical
phases as closed issues if issue tracking starts after implementation has begun.
