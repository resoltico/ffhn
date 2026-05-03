---
afad: "4.0"
domain: DOCS
updated: "2026-05-01"
route:
  keywords: [docs index, architecture, cli contract, target schema, reports, process errors, developer setup, devcontainer, operations, platform support, quality gates, release protocol, versioning policy]
  questions: ["where is the ffhn documentation index?", "what does ffhn stand for?", "which ffhn doc explains the CLI contract?", "where are the target and report contracts documented?", "where is ffhn's structured process-error detail documented?", "where is the ffhn contributor container documented?", "where is the ffhn release process documented?"]
---

# FFHN Docs

FFHN stands for `Focused Fragment History Notifier`.

Use this page as the routing index for the maintained FFHN documentation set.

- [getting-started.md](getting-started.md): source build, release-package install, portable quick start, and the checked-in sample flow
- [architecture.md](architecture.md): repo boundaries, crate responsibilities, runtime ownership, and how FFHN relates to HTMLCut
- [cli.md](cli.md): `ffhn run` and `ffhn status`, stdout/stderr behavior, `--all` discovery rules, and exit codes
- [targets.md](targets.md): `ffhn.target` shape, validation rules, defaults, target-id rules, storage, and notifications
- [core.md](core.md): `ffhn-core` runtime flow, live versus dry-run behavior, locking, and batch execution semantics
- [reports.md](reports.md): `ffhn.state`, `ffhn.extraction_record`, `ffhn.notification_payload`, `ffhn.status_report`, and snapshot artifact semantics
- [run-reports.md](run-reports.md): `ffhn.run_report`, `ffhn.batch_run_report`, reason-code semantics, notification-delivery reporting, and the shared structured process-error detail
- [contracts.md](contracts.md): frozen schema inventory, durable filesystem layout, and the FFHN versus HTMLCut boundary
- [developer-devcontainer.md](developer-devcontainer.md): preferred contributor-container workflow, Docker path, and devcontainer validation
- [developer-setup.md](developer-setup.md): fresh-machine bootstrap, required tools, optional `cargo-fuzz`, and disk-usage guidance
- [quality-gates.md](quality-gates.md): what `./check.sh` and `cargo xtask` actually enforce
- [operations.md](operations.md): local maintainer commands, CI workflows, release targets, and publication scripts
- [platform-support.md](platform-support.md): maintained standalone release target matrix, package contents, and public asset naming
- [release-protocol.md](release-protocol.md): GitHub CLI driven public release choreography for FFHN
- [versioning-policy.md](versioning-policy.md): version-source, contract, frozen-interop, and semver-baseline policy
- [../CONTRIBUTING.md](../CONTRIBUTING.md): contributor workflow and documentation/test expectations
- [../examples/README.md](../examples/README.md): index of checked-in runnable examples
- [../fuzz/README.md](../fuzz/README.md): manual fuzz inventory and the maintained seed-smoke commands

The changelog stays intentionally separate from this index. Use [../changelog.md](../changelog.md) for release history, not for current-state reference behavior.
