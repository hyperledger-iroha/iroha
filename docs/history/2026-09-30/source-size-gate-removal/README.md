# Source line-count gate retirement

The user explicitly removed code line-count policy. The repository-wide
source-file budget, its baseline and tests, and its PR/release invocations are
retired. Source-contract guards retain their semantic checks and accept
whitespace growth. Runtime resource bounds, canonical encoding checks, artifact
authentication and shipping feature/dependency validation remain independent.

Build-efficiency provenance schema 4 authenticates the original Git lineage,
including the retired source-budget records, as history. It no longer reads a
candidate source-budget file or applies current line limits. Exact original
bytes and SHA-256 inventories are retained here as non-executable `.txt` files.

Focused and complete unit validation of the resulting candidate is recorded
separately; this policy change supplies no release or settlement qualification.
