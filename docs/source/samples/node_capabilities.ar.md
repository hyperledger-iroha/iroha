---
lang: ar
direction: rtl
source: docs/source/samples/node_capabilities.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 34f6eeb1281ddca54adc947e76d5d7068c16927753be3cdfc10c47b3e97d4f14
source_last_modified: "2026-01-23T18:48:24.313698+00:00"
translation_last_reviewed: 2026-01-30
---

# Node Capabilities — ABI Support (Torii)

Endpoint
- `GET /v2/node/capabilities`

Response (first release; single ABI policy V1)
```json
{
  "supported_abi_versions": [1],
  "default_compile_target": 1,
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
- `supported_abi_versions` lists ABI versions currently accepted by the node at admission.
- `default_compile_target` is the highest active ABI version and should be used by Kotodama compilers by default.
- `data_model_version` is the data model compatibility version; SDKs should reject submissions when it differs from their built-in value.
- `crypto.curves.allowed_curve_ids` enumerates the [`address_curve_registry`](../references/address_curve_registry.md) identifiers configured in `iroha_config.crypto.curves.allowed_curve_ids`. Use this advert to decide whether ML‑DSA/GOST/SM controllers are usable on the target cluster.

See also
- `GET /v2/runtime/metrics` for a compact JSON summary of runtime metrics (ABI count and upgrade lifecycle counters).
