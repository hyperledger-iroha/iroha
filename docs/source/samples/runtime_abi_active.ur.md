---
lang: ur
direction: rtl
source: docs/source/samples/runtime_abi_active.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 1633a884189f5ce62f293c75655141609a0d37d360e11457763076753b707a6b
source_last_modified: "2026-01-03T18:07:58.960510+00:00"
translation_last_reviewed: 2026-01-30
---

# Runtime ABI — Active Versions (Torii)

Endpoint
- `GET /v2/runtime/abi/active`

Response (first release; single ABI)
```json
{
  "active_versions": [1],
  "default_compile_target": 1
}
```

Notes
- The list is sorted ascending. The default compile target is the highest active version (1 in the first release).

