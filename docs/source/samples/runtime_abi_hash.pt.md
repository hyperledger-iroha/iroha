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
  "abi_hash_hex": "17c61cb3a6ee164213afe410169161def1d7025b84f0b9e385a93619a862513b"
}
```

Notes
- The hash is the canonical digest of the allowed syscall surface for the policy.
- Contracts may embed this value in manifests (abi_hash) to bind to the node's ABI.
