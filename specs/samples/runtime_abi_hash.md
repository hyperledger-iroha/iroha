# Runtime ABI — Canonical Hash (Torii)

Endpoint
- `GET /v1/runtime/abi/hash`

Response (first release; single policy V1)
```json
{
  "policy": "V1",
  "abi_hash_hex": "bdd5188fba28f664a4c672746934450ef1aa4a2e53fe3f3a5433ea0ff73dd849"
}
```

Notes
- The hash binds the complete V1 ABI surface, including syscall signatures, pointer types, program metadata and canonical value layouts.
- Contracts may embed this value in manifests (abi_hash) to bind to the node's ABI.
