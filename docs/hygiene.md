---
afad: "4.0"
domain: HYGIENE
updated: "2026-05-14"
route:
  keywords: [artifact hygiene, cargo target dir, cargo build dir, disk usage, cleanup, cache policy, hygiene report]
  questions: ["where does ffhn put cargo build output?", "how do I clean ffhn build artifacts?", "how do I inspect ffhn disk usage?", "what does cargo xtask hygiene do?"]
---

# Artifact Hygiene

FFHN treats build artifacts as a maintained subsystem, not as accidental byproducts.

The goals are:

1. keep the repository checkout small and readable
2. keep heavy Cargo output out of fragile bind mounts and release worktrees
3. make every large artifact root reportable, classed, and safely reclaimable

## Canonical Artifact Roots

For normal host-native work, [../.cargo/config.toml](../.cargo/config.toml) points Cargo at one managed sibling artifact store:

1. `../.ffhn-artifacts/target`
2. `../.ffhn-artifacts/build`

`cargo xtask coverage` uses sibling managed roots instead of the normal workspace roots:

1. `../.ffhn-artifacts/coverage-target`
2. `../.ffhn-artifacts/coverage-build`

The maintained semver lane uses isolated scratch directories under those managed roots:

1. `../.ffhn-artifacts/target/semver-checks`
2. `../.ffhn-artifacts/build/semver-checks`

Inside the contributor devcontainer, the same policy applies but the roots live under the mounted user cache volume instead of beside the repository checkout:

1. `/home/vscode/.cache/ffhn-artifacts/target`
2. `/home/vscode/.cache/ffhn-artifacts/build`

Managed roots carry both `CACHEDIR.TAG` and `.ffhn-artifact.toml` marker files so they are easy to identify, classify, and delete safely.

## Maintained Commands

Inspect the current artifact inventory:

```bash
cargo xtask hygiene report
```

Render the same inventory as structured JSON:

```bash
cargo xtask hygiene report --format json
```

Fail if the current checkout violates the maintained hygiene policy:

```bash
cargo xtask hygiene verify
```

Remove disposable scratch, legacy repo-local target trees, and other safe-to-delete clutter:

```bash
cargo xtask hygiene clean --mode safe
```

Remove every rebuildable artifact root, including the managed Cargo caches:

```bash
cargo xtask hygiene clean --mode rebuildable
```

## Policy

FFHN's maintained hygiene policy is:

1. repo-local `target/` is legacy clutter, not the canonical Cargo output root
2. repo-local `fuzz/target/` is legacy clutter, not the canonical fuzz build root
3. Cargo-target-like scratch directories under `tmp/` are disposable investigation state and should not accumulate
4. coverage and semver scratch must run in isolated managed roots, not in the repository tree
5. heavyweight maintained gates must prepare, verify, and clean artifact roots automatically

That policy is enforced by `cargo xtask`:

1. `cargo xtask check` safe-cleans legacy scratch, prepares the managed roots, verifies policy, runs the full gate, then safe-cleans and verifies again
2. `cargo xtask coverage` uses managed coverage roots and leaves the repo-local tree clean
3. `cargo xtask semver-check` prepares isolated semver scratch under the managed roots and deletes it again after the lane finishes

## Manual Cleanup

For ordinary maintenance, prefer the hygiene commands over ad hoc deletion:

```bash
cargo xtask hygiene clean --mode safe
cargo xtask hygiene report
```

If you want to reclaim every rebuildable Cargo artifact too:

```bash
cargo xtask hygiene clean --mode rebuildable
```

Manual fuzzing also writes `fuzz/artifacts/`, which is not managed by Cargo's target-dir layout. Remove that separately when you are done with a campaign:

```bash
rm -rf fuzz/artifacts
```

## Why FFHN Does This

This layout keeps several failure modes visible and contained:

1. a release worktree does not quietly inflate because it inherited a repo-local `target/`
2. Docker bind mounts do not carry huge compiler caches back into the source tree
3. semver and coverage scratch no longer masquerade as durable project state
4. a maintainer can answer "what is taking space?" with one command instead of guesswork
