# Node Capabilities — Runtime ABI (Torii)

Endpoint
- `GET /v1/node/capabilities`

Response (first release; single ABI policy V1)
```json
{
  "abi_version": 1,
  "data_model_version": 1,
  "crypto": {
    "sm": {
      "enabled": false,
      "default_hash": "sha2_256",
      "allowed_signing": ["ed25519"],
      "sm2_distid_default": "",
      "openssl_preview": false,
      "acceleration": {
        "scalar": true,
        "neon_sm3": false,
        "neon_sm4": false,
        "policy": "scalar-only"
      }
    },
    "curves": {
      "registry_version": 1,
      "allowed_curve_ids": [1]
    }
  }
}
```

Notes
- `abi_version` is the single ABI version accepted by the node at admission and used by Kotodama compilers.
- `data_model_version` is the data model compatibility version; SDKs should reject submissions when it differs from their built-in value.
- `crypto.curves.allowed_curve_ids` enumerates the [`address_curve_registry`](../references/address_curve_registry.md) identifiers configured in `iroha_config.crypto.curves.allowed_curve_ids`. Use this advert to decide whether ML‑DSA/GOST/SM controllers are usable on the target cluster.

See also
- `GET /v1/runtime/metrics` for a compact JSON summary of runtime metrics (ABI version and upgrade lifecycle counters).
