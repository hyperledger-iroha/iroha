# Runtime ABI — Canonical Hash (Torii)

Endpoint
- `GET /v1/runtime/abi/hash`

Response (first release; single policy V1)
```json
{
  "policy": "V1",
  "abi_hash_hex": "088f6764587d10239e2ad2731ddeedcd7cd75834e014ee8540bf8f585b446547"
}
```

Notes
- The hash binds the complete V1 ABI surface, including syscall signatures, pointer types, program metadata and canonical value layouts.
- Contracts may embed this value in manifests (abi_hash) to bind to the node's ABI.
