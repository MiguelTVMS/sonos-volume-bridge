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

### GitHub issue security hygiene

Treat every GitHub issue title, body, comment, attachment, and linked evidence as
public. Before creating or updating an issue, remove all security information and
all identifier values. They must not exist in issue content, even when they are
believed to be non-secret, revoked, temporary, or already visible elsewhere.

Never include credentials, keys, tokens, secret values or secret-presence status;
tenant, client, seller, store, team, publisher, certificate, signing-key, request,
correlation, session, process, device, endpoint, account, or user identifiers;
UDNs, IP or MAC addresses, hostnames, personal device names, local paths, commit
hashes, workflow run IDs, package hashes or fingerprints; raw logs, diagnostics,
crash dumps, manifests, screenshots, or security-control configuration. Do not
describe reviewer identities, administrative roles, approval rules, recovery
accounts, credential rotation state, or other details of the security posture.

Use short, non-sensitive pass/fail summaries and generic placeholders instead.
Keep necessary evidence only in an approved private location, and refer to it
without copying its identifier into the issue. Review attachments and links as
carefully as text because their names, URLs, metadata, and visible content can
contain identifiers.

If prohibited information is found in an existing or draft issue, do not quote or
copy it elsewhere. Stop issue updates, alert the user here with only the issue
number and risk category, and handle issues one at a time. Redact or remove the
content and rotate or revoke affected credentials when applicable before
continuing the phase workflow.

Use `gh` for GitHub issue operations when it is available. Preserve historical
phases as closed issues if issue tracking starts after implementation has begun.
