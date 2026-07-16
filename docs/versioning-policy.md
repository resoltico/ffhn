---
afad: "4.0"
domain: MAINTAINER
updated: "2026-07-15"
route:
  keywords: [versioning policy, typed observation, schema naming, semver baseline, workspace version]
  questions: ["how does ffhn version its contracts?", "when should the ffhn semver baseline be refreshed?", "what is frozen versus generic in ffhn versioning?"]
---

# Versioning Policy

**Purpose**: Define how FFHN versions its release line, its v2 typed-observation contracts, and the checked-in semver baseline.
**Prerequisites**: [contracts.md](contracts.md), [reports.md](reports.md), [targets.md](targets.md), and [release-protocol.md](release-protocol.md).

## Release Version

`Cargo.toml` `[workspace.package] version` is FFHN's sole release-version source. It supplies both workspace crates, `ffhn --version`, release tags of the form `vX.Y.Z`, and the release asset names. Do not create a second version source in code, documentation, scripts, or workflows.

## FFHN-Owned Contracts

The maintained v2 document families are:

- `ffhn.target` schema version `9`
- `ffhn.state` schema version `9`
- `ffhn.run_report` schema version `8`
- `ffhn.batch_run_report` schema version `8`
- `ffhn.status_report` schema version `7`
- `ffhn.reset_report` schema version `3`

The typed parser identity is also contractual: `parser_id = "ffhn.typed-value"` with `parser_grammar_version = 1`.

FFHN evolves these product-owned contracts decisively. A contract change updates the Rust model and validation, relevant schema versions, docs and examples, fuzz harnesses, tests, and the `CHANGELOG.md` Unreleased section in one slice. FFHN does not retain aliases, compatibility parsers, translation layers, or migrations for retired schemas.

## State Boundary

V2 durable state exists only at `<watch-root>/<target>/.ffhn/state.json`. Normal v2 commands never parse v1 artifacts. `ffhn reset --target <ID>` is the explicit clean-break operation: under the target lock it blindly deletes only that target's `.ffhn` storage root, without decoding or inspecting its contents. The next accepted run initializes a new v2 state document.

Normal state reads and writes accept only an actual `.ffhn` directory and a regular `state.json`
file. A symlink, directory, or other non-regular state node is invalid state and is refused without
following it; reset remains the only blind recovery operation and removes only the `.ffhn` root
node.

The state contract binds its temporal facts to a source-contract digest. That digest includes the
complete ordered named-condition policy and `escalate_after`; HTML measurement contracts also bind
HTMLCut's extraction-semantics version. A run whose target definition yields a different digest
refuses before acquisition or mutation; reset is required before that changed measurement contract
can establish state.

## Typed Value Policy

Raw selected JSON scalar tokens are retained byte-for-byte as evidence, including string quoting and
escapes. HTML text and attribute projections retain their corresponding public HTMLCut output or
match attribute, plus plan and diagnostic evidence. Comparison uses the normalized typed value. The
current declared types are `integer`, `decimal`, `money`, `semver`, and `datetime`. Parsing is
deterministic: decimal and money use `rust_decimal`, semantic versions use `semver`, and date-times
either carry an offset or declare `assumed_offset`; machine-local time is never a fallback.

## Semver Baseline Policy

The checked-in `semver-baseline/ffhn-core` directory represents the last published `ffhn-core` API, not the current worktree.

1. Refresh it only after the corresponding release has been published.
2. Until a matching local tag exists, treat the current workspace version as an unreleased major line.
3. Refresh from an explicit published reference: `cargo xtask refresh-semver-baseline --git-ref vX.Y.Z`.
4. Never regenerate it from unreleased worktree state.

The baseline protects the published Rust API. It does not authorize retaining obsolete serialized contracts or legacy runtime behavior.
