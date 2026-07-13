---
lang: pt
direction: ltr
source: docs/source/samples/runtime_abi_hash.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 21554c026c5670cd349543406117a4e6b6081e82e25611ca6ba6f8fed686af33
source_last_modified: "2026-01-03T18:07:58.958473+00:00"
translation_last_reviewed: 2026-01-30
---

# Runtime ABI — Canonical Hash (Torii)

Endpoint
- `GET /v1/runtime/abi/hash`

Response (first release; single policy V1)
```json
{
  "policy": "V1",
  "abi_hash_hex": "1daac4a4c98904194fb294638d2d62c9b35c658b31d319f0e92b10d0ce9b7883"
}
```

Notes
- The hash is the canonical digest of the allowed syscall surface for the policy.
- Contracts may embed this value in manifests (abi_hash) to bind to the node's ABI.
