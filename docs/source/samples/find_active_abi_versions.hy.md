---
lang: hy
direction: ltr
source: docs/source/samples/find_active_abi_versions.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: cb329a2fa67deb1c61a91a63c892ec97a2cf9283b32f6c0bb7c519c5fe6856ab
source_last_modified: "2025-12-29T18:16:36.031451+00:00"
translation_last_reviewed: 2026-02-07
---

# FindAbiVersion — Norito Query Schema (Sample)

Type name: `iroha_data_model::query::runtime::AbiVersion`

Fields
- `abi_version: u16` — the fixed ABI version currently accepted on this node.

Example Norito JSON response (first release; single ABI)
```json
{
  "abi_version": 1
}
```

Query (signed) — `FindAbiVersion`
- Route: `/query` (Norito-encoded `SignedQuery`)
- Singular query box variant: `FindAbiVersion`
- Output variant: `AbiVersion`

Notes
- ABI version 1 is always active in the first release; governance‑activated versions extend the set permanently. The example above reflects the single‑ABI state.
