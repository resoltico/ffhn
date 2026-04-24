# AGENTS.md — Agent Entry Protocol

This file is the repository entry point for agent work. It defines load order, precedence, repository-wide exceptions, and the universal minimum that applies before any language-specific or documentation-specific rule.

## 1. Required context loading

When opening a repository, load context in this order:

1. Read this file completely.
2. Load `.codex/UNIVERSAL_ENGINEERING_CONTRACT.md`. This is the cross-language engineering contract.
3. Load `.codex/AGENTS_EXTRA.md` if it exists. This contains project-specific instructions.
4. Load the language/runtime protocol for each touched surface:
   - Java 26+ / Gradle: `.codex/AGENTS_JAVA26_GRADLE.md`
   - Kotlin 2.4+ / Gradle: `.codex/AGENTS_KOTLIN24_GRADLE.md`
   - Python 3.13+: `.codex/AGENTS_PYTHON313.md`
   - Rust 1.95+ / Cargo: `.codex/AGENTS_RUST195_CARGO.md`
5. For documentation authoring, documentation refactoring, or code changes that alter documented public contracts, load `.codex/PROTOCOL_AFAD.md` unless the only touched document is the repository root `README.md`.

If a referenced file is absent, continue with the best available context and state the missing file in the work summary when it matters.

## 2. Precedence

Use the most specific applicable instruction, but do not silently relax correctness, security, compatibility, or verification requirements.

Precedence order:

1. Explicit user request for the current task.
2. Project-specific instructions in `.codex/AGENTS_EXTRA.md`.
3. Repository-wide rules in this `AGENTS.md`, including the root `README.md` exception.
4. Applicable language/runtime-specific protocol.
5. Applicable documentation protocol.
6. Universal Engineering Contract.
7. General language, framework, ecosystem, and documentation norms.

When instructions conflict, prefer the stricter or more specific instruction unless it would make the task incorrect. Surface the conflict rather than guessing.

## 3. Universal minimum before changing a system

For every non-trivial change, build the smallest useful system map:

- **Truth:** Where does the relevant state live? What is authoritative? Who can mutate it?
- **Evidence:** What proves the system is working? What would reveal failure?
- **Consequence:** What breaks if the touched component disappears or changes shape?
- **Invariant:** What must remain true after the change?
- **Preservation:** Where should the discovered system theory live after the work?

Use this map to decide what to change, how far to widen the change, what to verify, and what to document.

## 4. Language dispatch

- Java 26+ / Gradle projects use `.codex/AGENTS_JAVA26_GRADLE.md`.
- Kotlin 2.4+ / Gradle projects use `.codex/AGENTS_KOTLIN24_GRADLE.md`.
- Python 3.13+ projects use `.codex/AGENTS_PYTHON313.md`.
- Rust 1.95+ / Cargo projects use `.codex/AGENTS_RUST195_CARGO.md`.
- Other languages use the Universal Engineering Contract plus repository-specific instructions. Do not apply Java-, Kotlin-, Python-, or Rust-specific rules to unrelated systems unless the repository explicitly asks for them.
- If a repository spans multiple languages, use the relevant protocol for each touched surface and the Universal Engineering Contract across all boundaries.

## 5. Documentation dispatch and root README exception

Use `.codex/PROTOCOL_AFAD.md` for agent-maintained documentation that is meant to stay synchronized with code, public APIs, architectural boundaries, operational procedures, or generated/reference material.

The repository root `README.md` is a special case. Treat it as the front window of the store, not as ordinary documentation and not as an AFAD-managed reference file.

Root `README.md` rules:

- Do not add AFAD frontmatter, symbol atoms, exhaustive API signatures, or schema tables to the root `README.md`.
- Optimize for a human first impression: what the project is, why it matters, how to install or run it, the shortest credible example, and where to go next.
- Keep runnable snippets, but prefer brevity over completeness.
- Link to AFAD-managed docs, reference files, guides, changelogs, or runbooks for detail.
- Preserve project-specific brand, tone, and release positioning unless the user asks to change them.

Nested `README.md` files are governed by their actual role. If a nested README is a component guide, package guide, or operational document, use the documentation protocol where it fits. If it is a user-facing landing page for a package, example, or integration, keep it reader-first and do not force reference-atom structure.

`CHANGELOG.md`, `LICENSE`, `NOTICE`, `SECURITY.md`, `CONTRIBUTING.md`, governance files, release notes, and legal/compliance files follow their own conventions unless project-specific instructions opt them into AFAD.

## 6. Work summary requirement

For non-trivial changes, the final work summary must include the verification performed and any important system theory preserved or still missing. Keep the summary proportional to the risk of the change.
