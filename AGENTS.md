# SonosVolumeBridge contribution guide

## Project constraints

- Keep the domain crate free of Tauri, OS APIs, and networking details.
- The synchronization crate must depend only on domain abstractions; adapters belong in platform crates.
- For each behavior change, add/adjust tests.
- Before merging, run:
  - `cargo fmt --check`
  - `cargo clippy --workspace --all-targets -- -D warnings`
  - `cargo test --workspace`

## Development from a personal fork

Contributions are expected to be done from a fork and synchronized through pull requests.

1. Keep your fork configured:
   - `git remote -v` should show `origin` (your fork) and `upstream` (the canonical repository).
   - Sync before each new phase: `git fetch upstream`.
2. Branch policy:
   - Create short, descriptive branches from `upstream/develop` using Gitflow names such as `feat/<topic>` or `fix/<topic>`.
3. Before starting work:
   - Rebase onto latest upstream: `git switch develop`, `git reset --hard upstream/develop`, `git checkout <your-branch>` and `git rebase upstream/develop` (or create branch from the freshly updated local `develop`).
4. During development:
   - Keep branch scoped to one phase or one clear behavior change.
   - Make incremental, reviewable commits.
5. Push discipline:
   - Push to your fork with normal pushes by default.
   - Avoid force push after reviewers start; if needed, explain clearly in PR notes.
6. PR readiness:
   - Open PR to the upstream `develop` branch.
   - Explain changes clearly in the PR description: what changed, why it changed, and expected impact.
   - Include concise summary, test commands, and notable tradeoffs.
   - Mention any deferred cleanup explicitly.
   - Any code that will reach `develop` or `main` must be merged through an approved PR.

## Gitflow is the project workflow

- This project uses Gitflow.
- Primary long-lived branches are `develop` and `main`.
- Feature and fix branches use:
  - `feat/<topic>` for new capabilities.
  - `fix/<topic>` for bug fixes.
  - `chore/<topic>` for housekeeping tasks.
- Release branches use `release/<version>` and are merged into `main` when ready.
- Hot fixes use `hotfix/<topic>` and are merged into both `main` and `develop`.
- Use merge flow that keeps `main` stable and `develop` the integration branch for next work.

## Optional GitHub issue path

This project can use issues, but it is optional.

- Keep each issue limited to implementation coordination only.
- If you use an issue, apply the security rules below before creating or updating it.

## Security and data hygiene for external updates

Before creating or updating an issue or PR description, remove any sensitive or identifying data.

- Do **not** include: credentials, keys/tokens/secrets, tenant/client/seller/store info,
  request/session/process IDs, user/admin/approver identities,
  endpoints, hostnames, IP/MAC addresses, personal device names, local file paths,
  commit hashes, workflow run IDs, package hashes, logs, diagnostics, crash dumps,
  manifests, screenshots, or security-control settings.
- Use short, non-sensitive pass/fail summaries and generic placeholders.
- Keep evidence in an approved private location and reference it without copying identifiers.

If prohibited information is found in an existing or draft issue, stop touching that issue,
report only the issue number + risk category in this thread, and sanitize the issue before
continuing.

Use `gh` for issue operations when available.

## Documentation maintenance

- Keep `/docs` and `/docs/decisions` updated for behavior and architecture changes.
- Before making a behavior-related change in code, read:
  - `docs/architecture.md`
  - `docs/how-it-works.md`
  - `docs/state-machine.md`
  - `docs/decisions/README.md`
- After a behavior change, update the relevant decision records and architecture
  references before finalizing the change.
