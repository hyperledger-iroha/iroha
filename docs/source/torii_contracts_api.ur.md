---
lang: ur
direction: rtl
source: docs/source/torii_contracts_api.md
status: complete
source_hash: 457eadb1a14964e738f069e958282138e4c5a8af7dee56b2f9c2c0d2ab75a7fd
source_last_modified: "2026-07-11T04:43:34.096342+00:00"
translation_last_reviewed: 2026-07-11
---

> Translation sync note (2026-07-11): this locale is an English-language mirror synchronized with the current canonical API and V1 security contract; a refreshed localized translation remains pending.

# Torii Contracts API (Bytecode Deploy & Fetch)

This document describes the app-facing HTTP endpoints for deploying self-describing contract bytecode and fetching the stored manifest/bytecode. These endpoints are thin wrappers around on-chain transactions and read-only queries; consensus semantics remain on-chain.

## Endpoints

- GET `/v1/contracts/code/{code_hash}`
  - Fetches the on-chain `ContractManifest` by its content-addressed `code_hash`.
  - Path param: `code_hash` — 32‑byte hex string.
  - Response body: `ContractCodeRecordDto` (JSON) with `manifest` populated.

- POST `/v1/contracts/deploy`
  - Accepts base64 `.to` bytecode with authority, private key, and a stable `contract_alias`; verifies the embedded `CNTR` contract interface, computes the domain-separated `code_hash` over the complete artifact including the fixed IVM execution header, computes `abi_hash` from the enforced ABI policy declared by the verified header, derives a fresh immutable `contract_address`, and binds the alias in the requested dataspace (`universal` by default).
  - Request body: `DeployContractDto`; response body: `DeployContractBundleReceiptDto` with exactly one `contracts[]` receipt entry.
  - Submits a single transaction that registers the manifest, stores the bytecode, activates the fresh address-backed instance, and binds the alias.
  - Reusing an existing `contract_alias` performs an in-place `kaizen`/`改善`: `contracts[0]` reports the new `contract_address`, the previous address, and `kaizen = true`.
  - Body size is limited by the `max_contract_code_bytes` custom parameter (default 16 MiB); raise the cap before uploading larger programs.
  - Telemetry: increments `torii_contract_errors_total{endpoint="deploy"}` on handler errors and `torii_contract_throttled_total{endpoint="deploy"}` when the limiter fires.

- GET `/v1/contracts/code-bytes/{code_hash}`
  - Fetches stored code bytes for a given `code_hash`.
  - Response body: `{ code_b64 }`.

## Schemas

### DeployContractDto

Upload compiled bytecode and let Torii derive the manifest and hashes.

```jsonc
{
  "authority":   "<i105-account-id>", // AccountId (string form)
  "private_key": "ed25519:0123…",    // ExposedPrivateKey (bare or prefixed multihash hex)
  "code_b64":    "Base64Payload==",
  "contract_alias": "router::universal",
  "lease_expiry_ms": null
}
```

Notes:
- `code_b64` must decode to a valid self-describing IVM `1.1` contract artifact with `abi_version == 1` and an embedded `CNTR` section.
- `contract_alias` is required and is the stable public lifecycle handle. The dataspace is derived from its alias suffix.
- `lease_expiry_ms` is optional. When omitted or `null`, the alias binding is permanent.
- The handler recomputes the manifest internally; callers do not provide one on this shortcut.
- The decoded bytecode length must not exceed `max_contract_code_bytes`; exceeding the limit triggers an `InvariantViolation` (`code bytes exceed cap`) during transaction admission.

### DeployContractBundleReceiptDto

```jsonc
{
  "ok": true,
  "bundle_name": "single-contract-deploy",
  "bundle_digest": "0123…cdef",
  "chain_fingerprint": "chain@0123…cdef",
  "dry_run": false,
  "completed_stages": ["plan", "deploy"],
  "failure_point": null,
  "contracts": [
    {
      "name": "router::universal",
      "contract_alias": "router::universal",
      "contract_address": "tairac1…",
      "previous_contract_address": null,
      "kaizen": false,
      "dataspace": "universal",
      "deploy_nonce": 0,
      "tx_hash_hex": "0123…cdef",
      "code_hash_hex": "0123…cdef",
      "abi_hash_hex": "89ab…7654",
      "status": "submitted"
    }
  ],
  "hajimari_calls": [],
  "assertions": []
}
```

### Type encodings (JSON)

- `Hash` values inside a canonical manifest (for example `code_hash` and
  `abi_hash`) use the checksummed Norito JSON literal
  `hash:<64 uppercase hex>#<4 uppercase checksum hex>`. SDK convenience APIs
  may validate that literal and expose the underlying 64-character lowercase
  hex, but must not accept malformed checksums or non-canonical spellings.
- Receipt fields whose names end in `_hex` remain raw 64-character lowercase
  hex by definition; they are not `Hash` JSON values.
- `AccountId` strings use canonical I105 literals (domainless encoded literal).
  Strict parser paths accept only canonical I105 literals (no `@<domain>` suffix).
- `ExposedPrivateKey` accepts either a bare multihash hex string or its algorithm-prefixed variant (e.g., `ed25519:…`). Responses normalise to bare multihash hex. Multihash hex is canonical: varint bytes are lowercase, payload bytes are uppercase, and `0x` prefixes are rejected.

### GET response: ContractCodeRecordDto

```jsonc
{
  "code_hash": "0123…cdef",
  "abi_hash": "89ab…7654",
  "manifest": {
    "seiyaku_name": "Treasury",
    "code_hash": "hash:0123…CDEF#ABCD",
    "abi_hash":  "hash:89AB…7654#1234",
    "compiler_fingerprint": "kotodama_lang/…",
    "features_bitmap": 0,
    "access_set_hints": { "read_keys": [], "write_keys": [], "dynamic_reads": [], "dynamic_writes": [] },
    "entrypoints": [],
    "states": [],
    "error_codes": null,
    "kotoba": null,
    "provenance": null
  }
}
```

The `manifest` value is the complete canonical Norito JSON representation of
`ContractManifest`; Torii does not truncate or rename its fields. Entrypoint
descriptors therefore retain exact argument/return schemas, access metadata,
and trigger declarations. The top-level `code_hash` and `abi_hash` convenience
fields are raw lowercase hex derived from the exact same manifest values; the
optional `code_bytes` field is omitted by this endpoint.

### Norito payloads

All DTOs derive both `JsonSerialize` and `NoritoSerialize`. Clients may submit either plain JSON or Norito-backed JSON. When emitting Norito via Kotodama tests or automation, use `norito::json::json!` with the same field names and encodings shown above so the `NoritoJson<T>` extractor can decode the payload deterministically.

### Rate limiting & telemetry

- `torii.deploy_rate_per_origin_per_sec` and `torii.deploy_burst_per_origin` configure the token bucket shared by `/v1/contracts/deploy`. Defaults: 4 req/s with a burst of 8 per origin token (`X-API-Token`, remote IP, endpoint tuple).
- Requests rejected by the limiter increment `torii_contract_throttled_total{endpoint}` where `endpoint` is `deploy`.
- Any handler error (invalid body, permission missing, queue failure) increments `torii_contract_errors_total{endpoint}`. Track alongside queue metrics for alerting.

## Examples

Fetch a manifest by hash:

```bash
curl -s http://127.0.0.1:8080/v1/contracts/code/<32-byte-hex> | jq .
```

Deploy code and then fetch code bytes:

```bash
curl -s -X POST \
  -H 'Content-Type: application/json' \
  -d '{
        "authority": "<i105-account-id>",
        "private_key": "ed25519:…",
        "code_b64": "…",
        "contract_alias": "router::universal"
      }' \
  http://127.0.0.1:8080/v1/contracts/deploy | jq .

curl -s http://127.0.0.1:8080/v1/contracts/code-bytes/<32-byte-hex> | jq .
```

Redeploy the same alias and get the fresh immutable address:

```bash
curl -s -X POST \
  -H 'Content-Type: application/json' \
  -d '{
        "authority": "<i105-account-id>",
        "private_key": "ed25519:…",
        "code_b64": "…",
        "contract_alias": "router::universal"
      }' \
  http://127.0.0.1:8080/v1/contracts/deploy | jq .
```

### Computing `abi_hash` for manifests

Manifests may include an `abi_hash` that binds the program to the node’s IVM ABI policy. You can compute this hash locally using the CLI:

```bash
# ABI v1
iroha tools ivm abi-hash --policy v1 --uppercase
```

The command prints a 32‑byte hex digest. Embed this value in `manifest.abi_hash`. Nodes verify that `abi_hash` equals their runtime policy hash and reject mismatches at admission.

## Security and governance

- **Canonical V1 security note:** Manifest and bytecode registration, activation,
  deactivation, and bytecode removal require the signing authority to hold
  `CanRegisterSmartContractCode`. Torii account self-registration does not grant
  this lifecycle authority.
- Alias-backed public deployment is the only supported app-facing activation
  flow. `ContractAddress` remains immutable per deployment, while
  `ContractAlias` is the stable public handle. Governance-controlled namespace
  binding remains an internal/governance concern, not a public contracts API.
- GET is read-only and content‑addressed by `code_hash`. Nodes may still apply access controls consistent with their governance policies.
