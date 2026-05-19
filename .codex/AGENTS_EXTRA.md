# FFHN Directives

## Upstream defect reporting

If you discover a defect in the `htmlcut` dependency, record it in `.codex/htmlcut-defects.txt`.

Each entry in the `.codex/htmlcut-defects.txt` must describe a single open upstream defect with enough detail to reproduce, diagnose, and fix it.

Do not add status fields, `FIXED` markers, verification notes, or historical commentary. When an upstream defect is resolved, remove the corresponding entry. If no open defects remain, delete the file.

## Upstream fixes only

Do not work around `htmlcut` defects in FFHN code. Upstream defects must be fixed in their upstream projects. Local workarounds hide the real problem, blur dependency boundaries, and introduce avoidable technical debt.
