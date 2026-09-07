# Runtime ABI — Canonical Hash (Torii)

Endpoint
- `GET /v1/runtime/abi/hash`

Response (first release; single policy V1)
```json
{
  "policy": "V1",
  "abi_hash_hex": "eafe4ea7c8abe41b61757c506f607b733b4e848d0e348d6e495cd07ff8a98941"
}
```

Notes
- The hash is the canonical digest of the allowed syscall surface for the policy.
- Contracts may embed this value in manifests (abi_hash) to bind to the node's ABI.
