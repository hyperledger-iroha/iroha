---
lang: ur
direction: rtl
source: docs/source/samples/runtime_abi_hash.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 21554c026c5670cd349543406117a4e6b6081e82e25611ca6ba6f8fed686af33
source_last_modified: "2026-01-03T18:07:58.958473+00:00"
translation_last_reviewed: 2026-01-30
---

# Runtime ABI — Canonical Hash (Torii)

Endpoint
- `GET /v2/runtime/abi/hash`

Response (first release; single policy V1)
```json
{
  "policy": "V1",
  "abi_hash_hex": "49f99db16b395798f47daa6c844af7fd230e5f249a4b34b970dfaca5cb3ece91"
}
```

Notes
- The hash is the canonical digest of the allowed syscall surface for the policy.
- Contracts may embed this value in manifests (abi_hash) to bind to the node's ABI.
