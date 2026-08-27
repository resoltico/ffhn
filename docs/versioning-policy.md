---
afad: "4.0"
domain: MAINTAINER
updated: "2026-08-25"
route:
  keywords: [versioning policy, observation graph, schema naming, semver baseline, workspace version]
  questions: ["how does FFHN version its contracts?", "when should the FFHN semver baseline be refreshed?", "what changes require a schema bump?"]
---

# Versioning Policy

**Purpose**: Define how FFHN versions its release line, serialized observation-graph contracts, semantic counters, and checked-in Rust API baseline.
**Prerequisites**: [contracts.md](contracts.md), [reports.md](reports.md), [targets.md](targets.md), and [release-protocol.md](release-protocol.md).

## Release version

`Cargo.toml` `[workspace.package] version` is FFHN’s sole release-version source. It supplies both workspace crates, `ffhn --version`, tags of the form `vX.Y.Z`, and release asset names. Do not create a second version source in code, documentation, scripts, or workflows.

## Serialized contracts

The current document families are `ffhn.agent`, `ffhn.graph_identity`, `ffhn.source`, `ffhn.source_identity`, `ffhn.measurement`, `ffhn.source_state`, `ffhn.measurement_state`, `ffhn.commit_manifest`, `ffhn.lineage_manifest`, `ffhn.delivery_record`, `ffhn.dead_letter`, `ffhn.event_envelope`, and the operation reports listed in [contracts.md](contracts.md). Every current family starts at schema version 1.

A serialized-contract change updates its Rust model and validation, schema version, docs, fuzz harnesses, tests, and `CHANGELOG.md` Unreleased section together. A new major may replace the whole contract family. FFHN does not retain aliases, compatibility parsers, translation layers, or migrations for retired schemas.

## State and lineage boundary

Durable state lives only under `sources/<source-id>/.ffhn/`; `.ffhn-identity.json` is the sole lineage authority outside that swap scope. Normal readers require the exact current schema and exact source/measurement lineage stamp before interpreting a dependent document. An unknown schema, unreadable artifact, missing authoritative state, or foreign stamp is refused at its owned source or measurement scope.

Reset is the clean-break operation. It mints fresh UUIDv4 lineage and replaces only the selected fixed storage scope through the lineage-manifest protocol. It never migrates, partially interprets, or derives a successor identity from replaced artifacts.

The Source Representation Digest binds representation-affecting acquisition configuration. The Measurement Value Digest binds the SRD, projection, type contract, parser grammar, acquisition semantics, and HTMLCut extraction semantics where applicable. A condition definition digest separately binds normalized policy and policy-evaluation semantics, allowing policy rebasing without changing observation lineage.

Increment `acquisition_semantics_version` whenever the same source configuration and origin state could yield different accepted bytes. Increment `measurement_value_semantics_version`, parser grammar, or HTMLCut’s pinned extraction counter whenever the same projection evidence could yield a different typed value. Increment `policy_evaluation_semantics_version` whenever the same typed observations and condition definition could yield a different decision.

## Typed value policy

Raw JSON scalar tokens and validated HTMLCut evidence are retained as observation evidence; comparison uses the declared-type canonical value. Supported types are `text`, `integer`, `decimal`, `money`, `semver`, and `datetime`. Text preserves exact Unicode scalar-sequence identity without normalization. Decimal and money policy arithmetic is exact. Date-time parsing never falls back to machine-local time.

## Semver baseline policy

The checked-in `semver-baseline/ffhn-core` directory represents the last published `ffhn-core` Rust API, not the current worktree.

1. Refresh it only after the corresponding release has been published.
2. Until a matching local tag exists, treat the workspace version as an unreleased major line.
3. Refresh from an explicit published reference: `cargo xtask refresh-semver-baseline --git-ref vX.Y.Z`.
4. Never regenerate it from unreleased worktree state.

The baseline protects the published Rust API. It never authorizes retaining superseded serialized contracts or runtime behavior.
