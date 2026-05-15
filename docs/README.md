---
afad: "4.0"
domain: DOCS
updated: "2026-05-15"
route:
  keywords: [docs index, architecture, cli contract, target schema, reports, process errors, developer setup, devcontainer, artifact hygiene, operations, platform support, quality gates, release protocol, versioning policy, quick start, notifications, release assets]
  questions: ["where is the ffhn documentation index?", "what does ffhn stand for?", "which ffhn doc explains the CLI contract?", "where are the target and report contracts documented?", "where is ffhn's structured process-error detail documented?", "where is the ffhn contributor container documented?", "where is ffhn's artifact hygiene documented?", "where is the ffhn release process documented?", "where should i start if i am new to ffhn?", "which doc explains ffhn reports and persisted files?"]
---

# FFHN Docs

FFHN stands for `Focused Fragment History Notifier`.

Use this index when you want the shortest path to the maintained FFHN document that answers a specific question.

## Start Here

- [getting-started.md](getting-started.md): first install, portable quick start, and the checked-in sample flow
- [cli.md](cli.md): command grammar, output formats, exit codes, and `run` versus `status`
- [targets.md](targets.md): how to write a valid `ffhn.target` file, including selection, compare, and notification sections
- [run-reports.md](run-reports.md): how to read `ffhn.run_report` and `ffhn.batch_run_report`
- [reports.md](reports.md): persisted FFHN files such as `ffhn.state`, `ffhn.last_run_snapshot`, and `ffhn.status_report`

## Understand FFHN

- [architecture.md](architecture.md): repository boundaries, crate responsibilities, runtime ownership, and the FFHN versus HTMLCut split
- [core.md](core.md): execution flow, locking, batch behavior, live versus dry-run semantics, and notification timing
- [contracts.md](contracts.md): the frozen schema inventory, durable filesystem layout, and contract ownership
- [platform-support.md](platform-support.md): maintained release targets, package contents, and public asset naming
- [versioning-policy.md](versioning-policy.md): version ownership, semver rules, and the frozen interop baseline

## Operate And Release

- [operations.md](operations.md): maintainer commands, CI workflows, release-target scripts, and publication paths
- [quality-gates.md](quality-gates.md): what `./check.sh` and `cargo xtask` prove before change acceptance
- [hygiene.md](hygiene.md): managed Cargo artifact roots, cleanup commands, and repository-local storage discipline
- [release-protocol.md](release-protocol.md): the maintained GitHub CLI release choreography

## Contribute

- [developer-devcontainer.md](developer-devcontainer.md): preferred contributor-container workflow, Docker path, and container validation
- [developer-setup.md](developer-setup.md): fresh-machine bootstrap, required tools, optional `cargo-fuzz`, and local disk-usage guidance
- [../CONTRIBUTING.md](../CONTRIBUTING.md): contributor workflow, review expectations, and repository norms

## Adjacent Material

- [../examples/README.md](../examples/README.md): checked-in runnable examples and their intended use
- [../fuzz/README.md](../fuzz/README.md): manual fuzzing inventory and maintained seed-smoke commands
- [../changelog.md](../changelog.md): release history and public change narrative

This index covers current-state reference material. Use the changelog for release history, not for the live contract.
