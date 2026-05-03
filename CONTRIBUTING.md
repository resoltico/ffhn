# Contributing

FFHN treats `ffhn-core` as the product and `ffhn-cli` as a renderer. Contributions should preserve that split.

## Normal Workflow

Preferred contributor path: open the repository through the committed devcontainer documented in
[docs/developer-devcontainer.md](docs/developer-devcontainer.md), then run repo commands from that
container shell.

1. make the production change in `ffhn-core` or the thin CLI/rendering change in `ffhn-cli`
2. run targeted tests while you work
3. dry-run affected targets with `cargo run -p ffhn-cli -- run --target <id> --dry-run`
4. if the change affects live persistence, run a live target and inspect `cargo run -p ffhn-cli -- status --target <id>`
5. run `./check.sh` before asking for review

## Documentation Sync Rules

FFHN documentation is expected to move with the code.

Update these pages when behavior changes:

1. CLI behavior, exit codes, or target discovery: [docs/cli.md](docs/cli.md) and [README.md](README.md)
2. target schema, defaults, notifications, or examples: [docs/targets.md](docs/targets.md), [examples/](examples), and [watchlist/demo/target.toml](watchlist/demo/target.toml)
3. runtime, persistence, or FFHN-owned contract semantics such as `ffhn.extraction_record`, `ffhn.state`, `ffhn.run_report`, `ffhn.notification_payload`, `ffhn.batch_run_report`, or `ffhn.status_report`: [docs/core.md](docs/core.md), [docs/reports.md](docs/reports.md), [docs/run-reports.md](docs/run-reports.md), and [docs/contracts.md](docs/contracts.md)
4. maintainer workflow, release logic, or QA tooling: [docs/developer-setup.md](docs/developer-setup.md), [docs/quality-gates.md](docs/quality-gates.md), [docs/operations.md](docs/operations.md), [docs/platform-support.md](docs/platform-support.md), [docs/release-protocol.md](docs/release-protocol.md), and [docs/versioning-policy.md](docs/versioning-policy.md)
5. contributor-container workflow or Docker-backed maintainer environment: [docs/developer-devcontainer.md](docs/developer-devcontainer.md), [docs/developer-setup.md](docs/developer-setup.md), and [docs/quality-gates.md](docs/quality-gates.md)

If you change production behavior, add a public-facing note under `## [Unreleased]` in `changelog.md`.

Whenever you touch a runnable Markdown snippet or example README, execute that documented flow in a disposable temp directory instead of only editing the prose.

Whenever you touch `.devcontainer/`, [scripts/devcontainer-cli-helper.Dockerfile](scripts/devcontainer-cli-helper.Dockerfile), [scripts/devcontainer-prepare-user-home.sh](scripts/devcontainer-prepare-user-home.sh), [scripts/validate-devcontainer.sh](scripts/validate-devcontainer.sh), [scripts/run-devcontainer-check.sh](scripts/run-devcontainer-check.sh), or the contributor-container docs, run `./scripts/validate-devcontainer.sh` in the same change. If the change affects the usable headless container workflow, also run `./scripts/run-devcontainer-check.sh`.

AFAD-managed docs under [docs/](docs), [examples/](examples), and [fuzz/](fuzz) carry AFAD frontmatter, and those frontmatter versions are validated against the canonical protocol metadata in [.codex/PROTOCOL_AFAD.md](.codex/PROTOCOL_AFAD.md). The root [README.md](README.md), [CONTRIBUTING.md](CONTRIBUTING.md), and [changelog.md](changelog.md) stay human-first special docs instead of carrying enforced AFAD metadata. Public Markdown local links and maintained repo-file path mentions, the checked-in target examples, the generated CLI catalog sections in [README.md](README.md) and [docs/cli.md](docs/cli.md), and the single repository-root [AGENTS.md](AGENTS.md) entrypoint are also validated by the automated test suite. FFHN does not maintain shadow agent-entrypoint files under `.codex/`.

## Fixture And Seed Expectations

Update checked-in fixtures and examples when the public contract changes.

That includes:

1. target examples when `ffhn.target` rules or defaults change
2. persisted sample data when `ffhn.extraction_record`, `ffhn.state`, `ffhn.run_report`, `ffhn.notification_payload`, `ffhn.batch_run_report`, or `ffhn.status_report` semantics change
3. fuzz seeds when target documents, state documents, or report documents change shape

Keep fuzz seeds intentionally small and representative. The automatic gate only compile-smokes the fuzz package; live seed-smoke commands are documented in [fuzz/README.md](fuzz/README.md).

`watchlist/demo` is maintained as starter target configuration, not as a checked-in runtime snapshot. The file-target example under [examples/file-target-with-notifications](examples/file-target-with-notifications) is maintained as materialized example assets because file targets require an absolute `file_path`. Generated files such as `state.json`, `last_run.json`, `lock/`, and `snapshots/` are local runtime artifacts.

## Manual Fuzzing

Manual sanitizer-backed fuzz runs are optional but useful when you change:

1. `ffhn.target` validation
2. `ffhn.extraction_record`, `ffhn.state`, `ffhn.run_report`, `ffhn.notification_payload`, `ffhn.batch_run_report`, or `ffhn.status_report`
3. file-target or dry-run extraction behavior

Those manual runs require `cargo-fuzz` and nightly. They are not part of `./check.sh`.

## Release Hygiene

Before cutting or repairing a release:

1. land the production, test, docs, and changelog changes first
2. run `./check.sh`
3. refresh the semver baseline only when the released public `ffhn-core` surface should become the new comparison floor
4. follow [docs/release-protocol.md](docs/release-protocol.md) and keep repository settings aligned with it
5. rely on the checked-in scripts and GitHub workflows instead of assembling release artifacts by hand
