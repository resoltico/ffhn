---
afad: "3.5"
version: "2.0.1"
domain: DOCS
updated: "2026-04-22"
route:
  keywords: [docs index, architecture, cli contract, target schema, reports, operations, platform support, quality gates, release protocol, versioning policy]
  questions: ["where is the ffhn documentation index?", "which ffhn doc explains the CLI contract?", "where are the target and report contracts documented?", "where is the ffhn release process documented?"]
---

# FFHN Docs

Use this page as the routing index for the maintained FFHN documentation set.

- [architecture.md](architecture.md): repo boundaries, crate responsibilities, runtime ownership, and how FFHN relates to HTMLCut
- [cli.md](cli.md): `ffhn run` and `ffhn status`, stdout/stderr behavior, `--all` discovery rules, and exit codes
- [targets.md](targets.md): `ffhn.target` shape, validation rules, defaults, target-id rules, storage, and notifications
- [core.md](core.md): `ffhn-core` runtime flow, live versus dry-run behavior, locking, and batch execution semantics
- [reports.md](reports.md): `ffhn.state`, `ffhn.run_report`, `ffhn.batch_run_report`, `ffhn.status_report`, and reason-code semantics
- [contracts.md](contracts.md): frozen schema inventory, durable filesystem layout, and the FFHN versus HTMLCut boundary
- [developer-setup.md](developer-setup.md): fresh-machine bootstrap, required tools, optional `cargo-fuzz`, and disk-usage guidance
- [quality-gates.md](quality-gates.md): what `./check.sh` and `cargo xtask` actually enforce
- [operations.md](operations.md): local maintainer commands, CI workflows, release targets, and publication scripts
- [platform-support.md](platform-support.md): maintained standalone release target matrix, package contents, and public asset naming
- [release-protocol.md](release-protocol.md): GitHub CLI driven public release choreography for FFHN
- [versioning-policy.md](versioning-policy.md): version-source, contract, frozen-interop, and semver-baseline policy
- [../CONTRIBUTING.md](../CONTRIBUTING.md): contributor workflow and documentation/test expectations
- [../fuzz/README.md](../fuzz/README.md): manual fuzz inventory and the maintained seed-smoke commands

The changelog stays intentionally separate from this index. Use [../changelog.md](../changelog.md) for release history, not for current-state reference behavior.
