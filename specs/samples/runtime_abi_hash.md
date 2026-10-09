# Runtime ABI — Canonical Hash (Torii)

Endpoint
- `GET /v1/runtime/abi/hash`

Response (first release; single policy V1)
```json
{
  "policy": "V1",
  "abi_hash_hex": "32a1fc6e3ca277e857d4aed4c12c8fd47f76ee9fb4c3f4bbdca13e702d0a32bd"
}
```

Notes
- The hash binds the complete V1 ABI surface, including syscall signatures, pointer types, program metadata and canonical value layouts.
- Contracts may embed this value in manifests (abi_hash) to bind to the node's ABI.
