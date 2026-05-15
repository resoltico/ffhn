---
afad: "4.0"
domain: MAINTAINER
updated: "2026-05-14"
route:
  keywords: [versioning policy, schema naming, htmlcut boundary, semver baseline, workspace version]
  questions: ["how does ffhn version its contracts?", "when should the ffhn semver baseline be refreshed?", "what is frozen versus generic in ffhn versioning?"]
---

# Versioning Policy

**Purpose**: Define how FFHN versions release tags, public contracts, the upstream HTMLCut interop
boundary it consumes, and the checked-in semver baseline.
**Prerequisites**: [contracts.md](contracts.md), [reports.md](reports.md), [targets.md](targets.md),
and [release-protocol.md](release-protocol.md).

## 1. Version Sources

FFHN keeps one release-version source of truth:

- `Cargo.toml` `[workspace.package] version`

That version feeds:

- both workspace crates
- `ffhn --version`
- release tags of the form `vX.Y.Z`
- release asset, package, and checksum-manifest names

Do not create parallel version sources in crate manifests, docs, scripts, or workflows.

## 2. Two Contract Classes

FFHN has two different compatibility models.

### 2.1 Generic FFHN-owned contracts

These are the normal FFHN surfaces:

- `ffhn.target`
- `ffhn.extraction_record`
- `ffhn.state`
- `ffhn.run_report`
- `ffhn.last_run_snapshot`
- `ffhn.notification_payload`
- `ffhn.batch_run_report`
- `ffhn.status_report`
- the core-owned CLI command and document contract
- stable embedded field vocabularies and named subobjects inside those documents, such as `RunResult.kind`, `RunFailureCause`, `StatusSummary.kind`, notification-delivery outcome values, and the shared structured process-error detail
- the stable embeddable `ffhn-core` API outside the upstream HTMLCut adapter boundary

These surfaces may change aggressively when architecture quality requires it. FFHN does not carry
compatibility shims, aliases, or migration layers for generic surfaces.

When a generic public contract changes:

1. update the Rust types and validators
2. update any schema version whose serialized document changed
3. update docs, examples, and contract lint in the same change
4. update tests and fuzz seeds so they assert the new contract explicitly
5. document the released effect in `changelog.md`

### 2.2 Consumed upstream HTMLCut boundary

FFHN also consumes one upstream interop boundary:

- module: `htmlcut_core::interop::v1`
- upstream documents: `htmlcut.plan`, `htmlcut.result`, and `htmlcut.error`

FFHN uses that boundary to ask HTMLCut for one extraction, then translates the answer into
FFHN-owned reports, extraction evidence, and persisted artifacts. FFHN does not persist upstream
interop-profile fields inside FFHN-owned documents.

If FFHN needs a different upstream interop boundary:

1. adopt the new HTMLCut interop module explicitly
2. update FFHN's adapter, validators, and translators in one coherent change
3. update checked-in examples, docs, tests, and fuzz inputs that exercise the seam

Do not silently blur multiple upstream profiles under one retained FFHN contract.

## 3. Schema Naming Rules

FFHN schema families use stable document names plus explicit integer schema versions.

Examples:

- `ffhn.target`
- `ffhn.extraction_record`
- `ffhn.state`
- `ffhn.run_report`
- `ffhn.last_run_snapshot`
- `ffhn.notification_payload`
- `ffhn.batch_run_report`
- `ffhn.status_report`

Rules:

- keep schema names product-owned and generic
- keep `schema_name` and `schema_version` on every maintained public document
- use the Rust validators and tests as the canonical contract enforcement surface, not prose examples

## 4. HTMLCut Boundary Expectations

Maintainer expectations for the HTMLCut seam:

1. FFHN validates any upstream interop object before trusting it
2. FFHN translates upstream extraction output into FFHN-owned evidence and persisted artifacts
3. FFHN rejects upstream features outside FFHN's supported contract instead of leaking them through unchanged
4. docs, examples, tests, and fuzz seeds stay aligned with the shipped adapter behavior

## 5. Release-Time Expectations

Release preparation is expected to converge the whole shipped contract, not just bump a version.

Before a release is tagged:

1. the workspace version is correct
2. changelog, README, and maintained docs describe the same shipped surface
3. checked-in examples validate
4. the maintainer gate passes

FFHN optimizes for a coherent released system, not for preserving obsolete shapes.

## 6. Semver Baseline Policy

The checked-in semver baseline represents the last published `ffhn-core` API, not the current
worktree.

Rules:

1. refresh it only after the corresponding release is actually published
2. treat the current workspace version as an unreleased major line until a matching Git tag `vX.Y.Z` exists locally
3. refresh it from an explicit published Git ref with `cargo xtask refresh-semver-baseline --git-ref vX.Y.Z`
4. never regenerate it from unreleased local worktree state
5. treat it as the comparison target for future semver checks, not as a staging area during feature work
