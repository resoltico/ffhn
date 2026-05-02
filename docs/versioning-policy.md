---
afad: "4.0"
version: "5.0.0"
domain: MAINTAINER
updated: "2026-05-03"
route:
  keywords: [versioning policy, schema naming, htmlcut interop profile, semver baseline, workspace version]
  questions: ["how does ffhn version its contracts?", "when should the ffhn semver baseline be refreshed?", "what is frozen versus generic in ffhn versioning?"]
---

# Versioning Policy

**Purpose**: Define how FFHN versions release tags, public contracts, the frozen HTMLCut interop profile it consumes, and the checked-in semver baseline.
**Prerequisites**: [contracts.md](contracts.md), [reports.md](reports.md), [targets.md](targets.md), and [release-protocol.md](release-protocol.md).

## 1. Version Sources

FFHN keeps one release-version source of truth:

- `Cargo.toml` `[workspace.package] version`

That version feeds:

- both published crates
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
- `ffhn.notification_payload`
- `ffhn.batch_run_report`
- `ffhn.status_report`
- the core-owned CLI command and document contract
- stable embedded field vocabularies and named subobjects inside those documents, such as `reason_code`, notification-event values, and the shared structured process-error detail
- the stable embeddable `ffhn-core` API outside frozen upstream interop profiles

These surfaces may change aggressively when architecture quality requires it. FFHN does not carry backwards-compatibility shims, aliases, or migration layers for generic surfaces.

When a generic public contract changes:

- update the Rust types and validators
- update any stable schema version where the serialized document changed
- update docs, examples, and contract lint in the same change
- update tests and fuzz seeds so they assert the new contract explicitly
- document the released effect in `changelog.md`

### 2.2 Frozen HTMLCut interop profile

FFHN also consumes one frozen upstream interop profile:

- module: `htmlcut_core::interop::v1`
- profile string: `htmlcut-v1`
- documents surfaced through FFHN artifacts: `htmlcut.plan`, `htmlcut.result`, and `htmlcut.error`

Once FFHN ships a release that depends on a frozen HTMLCut profile, do not mutate FFHN’s expectations for that profile casually or hide a drift behind prose-only updates.

If FFHN needs a different upstream frozen profile:

- adopt the new HTMLCut interop module explicitly
- update FFHN validators, persisted artifacts, and docs in one coherent change
- update checked-in examples, tests, and fuzzing inputs that assert the interop identity

Do not silently blur multiple interop profiles under one retained FFHN contract.

## 3. Schema Naming Rules

FFHN schema families use stable document names plus explicit integer schema versions.

Examples:

- `ffhn.target`
- `ffhn.extraction_record`
- `ffhn.state`
- `ffhn.run_report`
- `ffhn.notification_payload`
- `ffhn.batch_run_report`
- `ffhn.status_report`

Rules:

- keep schema names product-owned and generic
- keep `schema_name` and `schema_version` on every maintained public document
- use the Rust validators and tests as the canonical contract enforcement surface, not prose examples

## 4. HTMLCut `interop_profile` Routing

`interop_profile` is part of the frozen FFHN-visible contract because it is persisted into FFHN artifacts and reports.

Maintainer expectations:

- every retained interop artifact must carry the expected `htmlcut-v1` identity
- validators must reject mismatched profile values
- docs, examples, tests, and fuzz seeds must stay aligned with the shipped interop profile

## 5. Release-Time Expectations

Release preparation is expected to converge the whole shipped contract, not just bump a version.

Before a release is tagged:

- the workspace version is correct
- changelog, README, and maintained docs describe the same shipped surface
- checked-in examples still validate
- the maintainer gate passes

FFHN optimizes for a coherent released system, not for preserving obsolete shapes.

## 6. Semver Baseline Policy

The checked-in semver baseline represents the last published `ffhn-core` API, not the current worktree.

Rules:

- refresh it only after the corresponding release is actually published
- treat the current workspace version as an unreleased major line until a matching Git tag `vX.Y.Z` exists locally
- refresh it from an explicit published Git ref with `cargo xtask refresh-semver-baseline --git-ref vX.Y.Z`
- never regenerate it from unreleased local worktree state
- treat it as the comparison target for future semver checks, not as a staging area during feature work
