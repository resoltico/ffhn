---
afad: "4.0"
domain: MAINTAINER
updated: "2026-07-19"
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

- `ffhn.target` schema version `12`
- `ffhn.state` schema version `17`
- `ffhn.run_report` schema version `17`
- `ffhn.batch_run_report` schema version `17`
- `ffhn.status_report` schema version `13`
- `ffhn.reset_report` schema version `7`
- `ffhn.process_stdin` schema version `4`

The typed parser identity is also contractual: `parser_id = "ffhn.typed-value"` with `parser_grammar_version = 1`.

FFHN evolves these product-owned contracts decisively. A contract change updates the Rust model and validation, relevant schema versions, docs and examples, fuzz harnesses, tests, and the `CHANGELOG.md` Unreleased section in one slice. FFHN does not retain aliases, compatibility parsers, translation layers, or migrations for retired schemas.

## State Boundary

V2 durable state exists only at `<watch-root>/<target>/.ffhn/state.json`. Normal v2 commands never parse v1 artifacts. `ffhn reset --target <ID>` is the explicit clean-break operation: under the target lock it blindly deletes only that target's `.ffhn` storage root, without decoding or inspecting its contents. The next accepted run initializes a new v2 state document.

Normal reads first decode only the state schema envelope. An envelope other than
`schema_name = "ffhn.state"` and `schema_version = 17` is refused with reset-required guidance
before FFHN attempts to decode any state facts. FFHN does not migrate or partially interpret a
retired state; malformed JSON remains unreadable state and also requires explicit reset.

Normal state reads and writes accept only an actual `.ffhn` directory and a regular `state.json`
file. A symlink, directory, or other non-regular state node is invalid state and is refused without
following it; reset remains the only blind recovery operation and removes only the `.ffhn` root
node.

The state contract binds its temporal facts to a source-contract digest. That digest includes the
complete named-condition policy canonicalized by condition identifier, `escalate_after`, and FFHN's policy-evaluation semantics
version; HTML measurement contracts additionally bind HTMLCut's extraction-semantics version,
while JSON measurements do not. A policy-semantics version changes whenever the same accepted
observations could yield different condition decisions. A run whose target definition or semantics
yields a different digest refuses before acquisition or mutation; reset is required before that
changed measurement contract can establish state. Target declaration order is deliberately excluded:
it controls only the operational admission priority of new bounded-outbox candidates.

## Typed Value Policy

Raw selected JSON scalar tokens are retained byte-for-byte as evidence, including string quoting and
escapes. `html_text` retains HTMLCut's plain DOM descendant text; `html_rendered_text` retains its
semantic rendered text; and `html_attribute` retains the selected attribute. Each carries plan and
diagnostic evidence. Comparison uses the declared-type canonical
value. The
current declared types are `text`, `integer`, `decimal`, `money`, `semver`, and `datetime`.
Text retains exact Unicode scalar-sequence identity: JSON accepts only strings and decodes their
escape spelling, while HTML uses its configured comparison projection; no trimming, case folding,
locale rule, or Unicode normalization applies. Decimal and money use `rust_decimal`, semantic
versions use `semver`, and date-times either carry an offset or declare `assumed_offset`;
machine-local time is never a fallback.

## Semver Baseline Policy

The checked-in `semver-baseline/ffhn-core` directory represents the last published `ffhn-core` API, not the current worktree.

1. Refresh it only after the corresponding release has been published.
2. Until a matching local tag exists, treat the current workspace version as an unreleased major line.
3. Refresh from an explicit published reference: `cargo xtask refresh-semver-baseline --git-ref vX.Y.Z`.
4. Never regenerate it from unreleased worktree state.

The baseline protects the published Rust API. It does not authorize retaining obsolete serialized contracts or legacy runtime behavior.
