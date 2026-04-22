---
afad: "3.5"
version: "2.0.1"
domain: CONTRIBUTING
updated: "2026-04-22"
route:
  keywords: [contributing, workflow, docs sync, check.sh, fuzz seeds, release hygiene]
  questions: ["how should I contribute to ffhn?", "which docs do I update when ffhn behavior changes?", "what should I run before asking for review?"]
---

# Contributing

FFHN treats `ffhn-core` as the product and `ffhn-cli` as a renderer. Contributions should preserve that split.

## Normal Workflow

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
3. runtime, persistence, or report semantics: [docs/core.md](docs/core.md), [docs/reports.md](docs/reports.md), and [docs/contracts.md](docs/contracts.md)
4. maintainer workflow, release logic, or QA tooling: [docs/developer-setup.md](docs/developer-setup.md), [docs/quality-gates.md](docs/quality-gates.md), [docs/operations.md](docs/operations.md), [docs/platform-support.md](docs/platform-support.md), [docs/release-protocol.md](docs/release-protocol.md), and [docs/versioning-policy.md](docs/versioning-policy.md)

If you change production behavior, add a public-facing note under `## [Unreleased]` in `changelog.md`.

AFAD-managed Markdown frontmatter versions are validated against the canonical protocol metadata in [.codex/PROTOCOL_AFAD.md](.codex/PROTOCOL_AFAD.md). The checked-in target examples, the generated CLI catalog sections in [README.md](README.md) and [docs/cli.md](docs/cli.md), and the internal agent-parity entrypoints in [.claude/CLAUDE.md](.claude/CLAUDE.md) and [.gemini/GEMINI.md](.gemini/GEMINI.md) are also validated by the automated test suite. Treat those files as maintained contract surfaces, not prose-only references.

## Fixture And Seed Expectations

Update checked-in fixtures and examples when the public contract changes.

That includes:

1. target examples when `ffhn.target` rules or defaults change
2. persisted sample data when report or state semantics change
3. fuzz seeds when target documents, state documents, or report documents change shape

Keep fuzz seeds intentionally small and representative. The automatic gate only compile-smokes the fuzz package; live seed-smoke commands are documented in [fuzz/README.md](fuzz/README.md).

`watchlist/demo` is maintained as starter target configuration, not as a checked-in runtime snapshot. Generated files such as `state.json`, `last_run.json`, `lock/`, and `snapshots/` are local runtime artifacts.

## Manual Fuzzing

Manual sanitizer-backed fuzz runs are optional but useful when you change:

1. `ffhn.target` validation
2. `ffhn.state`, `ffhn.run_report`, `ffhn.batch_run_report`, or `ffhn.status_report`
3. file-target or dry-run extraction behavior

Those manual runs require `cargo-fuzz` and nightly. They are not part of `./check.sh`.

## Release Hygiene

Before cutting or repairing a release:

1. land the production, test, docs, and changelog changes first
2. run `./check.sh`
3. refresh the semver baseline only when the released public `ffhn-core` surface should become the new comparison floor
4. follow [docs/release-protocol.md](docs/release-protocol.md) and keep repository settings aligned with it
5. rely on the checked-in scripts and GitHub workflows instead of assembling release artifacts by hand
