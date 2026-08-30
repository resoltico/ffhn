# Contributing

FFHN treats `ffhn-core` as the product and `ffhn-cli` as a renderer. Contributions should preserve that split.

## Normal Workflow

Preferred contributor path: open the repository through the committed devcontainer documented in
[docs/developer-devcontainer.md](docs/developer-devcontainer.md), then run repo commands from that
container shell.

1. make the production change in `ffhn-core` or the thin CLI/rendering change in `ffhn-cli`
2. run targeted tests while you work
3. dry-run affected measurements with `cargo run -p ffhn-cli -- measure --graph-root <root> --source <id> --dry-run`
4. if the change affects live persistence, run a live source and inspect `cargo run -p ffhn-cli -- status --graph-root <root> --source <id>`
5. run `./check.sh` before asking for review

## Documentation Sync Rules

FFHN documentation is expected to move with the code.

Naming rule for reader-facing material:

1. use `FFHN` for the product in prose, headings, changelog entries, and explanatory documentation
2. use `ffhn` only for literal identifiers such as the command name, crate names, schema ids, file names, release asset names, and URLs
3. when you expand the acronym, use `Focused Fragment History Notifier`

Update these pages when behavior changes:

1. CLI behavior, exit codes, or graph discovery: [docs/cli.md](docs/cli.md) and [README.md](README.md)
2. source/measurement schemas, projections, typed-value parameters, or routes: [docs/targets.md](docs/targets.md), [docs/getting-started.md](docs/getting-started.md), and [docs/contracts.md](docs/contracts.md)
3. runtime, persistence, event, delivery, or report semantics: [docs/core.md](docs/core.md), [docs/reports.md](docs/reports.md), [docs/architecture.md](docs/architecture.md), and [docs/contracts.md](docs/contracts.md)
4. maintainer workflow, release logic, or QA tooling: [docs/developer-setup.md](docs/developer-setup.md), [docs/quality-gates.md](docs/quality-gates.md), [docs/operations.md](docs/operations.md), [docs/platform-support.md](docs/platform-support.md), [docs/release-protocol.md](docs/release-protocol.md), and [docs/versioning-policy.md](docs/versioning-policy.md)
5. contributor-container workflow or Docker-backed maintainer environment: [docs/developer-devcontainer.md](docs/developer-devcontainer.md), [docs/developer-setup.md](docs/developer-setup.md), and [docs/quality-gates.md](docs/quality-gates.md)

If you change production behavior, add a public-facing note under `## [Unreleased]` in `CHANGELOG.md`.

Whenever you touch a runnable Markdown snippet or example README, execute that documented flow in a disposable temp directory instead of only editing the prose.

Whenever you touch `.devcontainer/`, [scripts/devcontainer-cli-helper.Dockerfile](scripts/devcontainer-cli-helper.Dockerfile), [scripts/devcontainer-prepare-user-home.sh](scripts/devcontainer-prepare-user-home.sh), [scripts/validate-devcontainer.sh](scripts/validate-devcontainer.sh), [scripts/run-devcontainer-check.sh](scripts/run-devcontainer-check.sh), or the contributor-container docs, run `./scripts/validate-devcontainer.sh` in the same change. If the change affects the usable headless container workflow, also run `./scripts/run-devcontainer-check.sh`.

AFAD-managed docs under [docs/](docs) and [fuzz/](fuzz) carry AFAD frontmatter, and those frontmatter versions are validated against the canonical protocol metadata in [.codex/PROTOCOL_AFAD.md](.codex/PROTOCOL_AFAD.md). The root [README.md](README.md), [CONTRIBUTING.md](CONTRIBUTING.md), and [changelog.md](changelog.md) stay human-first special docs instead of carrying enforced AFAD metadata. Public Markdown local links and maintained repo-file path mentions, public command and document identifiers, source/measurement TOML snippets, and the single repository-root [AGENTS.md](AGENTS.md) entrypoint are also validated by the automated test suite. FFHN does not maintain shadow agent-entrypoint files under `.codex/`.

## Fixture And Seed Expectations

Update checked-in fixtures and examples when the public contract changes.

That includes:

1. source and measurement examples when configuration rules or defaults change
2. persisted fixtures when identity, state, manifest, event, delivery, dead-letter, or report semantics change
3. fuzz seeds and harnesses when configuration or durable documents change shape

Keep fuzz seeds intentionally small and representative. The automatic gate only compile-smokes the fuzz package; live seed-smoke commands are documented in [fuzz/README.md](fuzz/README.md).

## Manual Fuzzing

Manual sanitizer-backed fuzz runs are optional but useful when you change:

1. `ffhn.source` or `ffhn.measurement` validation
2. identity, state, manifest, event, delivery-record, or dead-letter validation
3. file-source, HTTP, JSON Pointer, or HTMLCut acquisition behavior

Those manual runs require `cargo-fuzz` and nightly. They are not part of `./check.sh`.

## Mutation Testing

Install the optional pinned mutation tool, then run both complete first-party scopes from a copied workspace:

```bash
./scripts/bootstrap-rust-tools.sh install-mutation-tool
cargo xtask mutants
```

Use `--scope runtime` for `ffhn-core` plus `ffhn-cli`, or `--scope tooling` for `xtask`. Each parallel cargo-mutants scratch checkout builds into its own checkout-local Cargo artifact roots, so no worker can reuse or overwrite another mutant's evidence. Mutation testing is deliberately separate from `./check.sh`; pull requests receive diff-scoped checks, while scheduled and manually dispatched campaigns run every maintained shard and retain their complete evidence.

For a local test-writing loop, retain the selected scope's caught and unviable evidence with:

```bash
cargo xtask mutants --scope runtime --iterate
```

`--iterate` cannot combine with `--shard` or `--in-diff`, and it never replaces a clean complete campaign.

## Release Hygiene

Before cutting or repairing a release:

1. land the production, test, docs, and changelog changes first
2. run `./check.sh`
3. refresh the semver baseline only when the released public `ffhn-core` surface should become the new comparison floor
4. follow [docs/release-protocol.md](docs/release-protocol.md) and keep repository settings aligned with it
5. rely on the checked-in scripts and GitHub workflows instead of assembling release artifacts by hand
