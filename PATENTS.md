# Patent Notes

ffhn's own code is licensed under the MIT License, which does not include
an explicit patent grant or patent retaliation clause.

## Dependency Patent Grants

ffhn allows third-party dependency licenses through `deny.toml`, and those SPDX license families
have different patent postures. The exact dependency inventory for a given release lives in
[NOTICE](NOTICE) and `Cargo.lock`; this note explains the policy implications rather than trying to
freeze a crate-by-crate list in prose.

| License family | Explicit patent grant | Notes |
|:---------------|:----------------------|:------|
| MIT | No explicit grant | ffhn itself is MIT-licensed. |
| Apache-2.0 | Yes | Section 3 grants patent rights from contributors to their contributions. |
| MPL-2.0 | Yes, scoped | Section 2.1 grants patent rights within the scope of the covered files. |
| ISC | No explicit grant | Plain permissive grant, no standalone patent clause. |
| BSD-3-Clause | No explicit grant | Plain permissive grant, no standalone patent clause. |
| 0BSD | No explicit grant | Public-domain-like or permissive terms without a dedicated patent clause. |
| Unlicense | No explicit grant | Public-domain-like or permissive terms without a dedicated patent clause. |
| Unicode-3.0 | No explicit grant | Data-license terms, not a patent grant. |
| CDLA-Permissive-2.0 | No explicit grant | Data/content sharing license, not a patent grant. |

Apache-2.0 includes an explicit patent grant in Section 3 from each contributor
to the covered code. MPL-2.0 includes a patent grant in Section 2.1
scoped to the licensed files.

## Repository-Level Patent Posture

This repository does not publish a separate project-level patent license,
retaliation clause, or patent non-assert covenant beyond:

- ffhn's own MIT license
- whatever patent terms are present in the allowed third-party dependency licenses

If a stronger project-level patent covenant is desired, it must be added
explicitly. It should not be inferred from this note alone.

## Legal Disclaimer

This document is informational only and does not constitute legal advice. For
patent-related concerns, consult qualified legal counsel.
