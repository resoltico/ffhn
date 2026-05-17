<!--
AFAD:
  afad: "4.0"
  domain: DEPENDENCY
  updated: 2026-05-16
RETRIEVAL_HINTS:
  keywords: [local dependency patch, servo_arc, tendril, miri, ffhn, htmlcut, strict provenance]
  questions: ["why does ffhn vendor selector-stack crates locally?", "how does ffhn verify the local dependency patches?", "when can ffhn remove the local overrides?"]
-->

# Local Dependency Patches

FFHN carries focused dependency patches only when a blocking defect has no
published upstream release that resolves it yet.

## `rust/servo_arc`

- Source: crates.io `servo_arc` `0.4.3`
- Scope: pointer-provenance fixes on the selector stack used by `scraper`,
  `htmlcut-core`, and FFHN's target-validation path
- Reason: the maintained FFHN-to-HTMLCut strict-provenance proof trips a Miri
  provenance failure through `htmlcut-core -> scraper -> selectors -> servo_arc`
- Current state: the local patch preserves tail provenance through
  `HeaderSlice` construction and drop

## `rust/tendril`

- Source: crates.io `tendril` `0.5.0`
- Scope: strict-provenance fixes on the HTML parser stack used by
  `markup5ever`, `html5ever`, `scraper`, `htmlcut-core`, and FFHN
- Reason: once the selector-validation path is repaired, the same maintained
  strict-provenance proof continues into the parser stack and trips a follow-on
  Miri failure through `htmlcut-core -> scraper -> html5ever -> markup5ever ->
  tendril`
- Current state: the local patch preserves heap-header provenance separately
  from the tagged pointer bits, so FFHN's maintained HTMLCut interop proof can
  run under strict provenance

## Verification

- `cargo xtask miri`

## Removal Cue

Remove the local overrides in [Cargo.toml](../Cargo.toml) when FFHN bumps to an
`htmlcut-core` release that already carries these fixes in its published
dependency graph, and confirm that `cargo xtask miri` still passes without the
workspace patches.
