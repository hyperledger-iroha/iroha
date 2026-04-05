---
lang: ka
direction: ltr
source: docs/source/torii_contracts_api.md
status: needs-update
generator: scripts/sync_docs_i18n.py
source_hash: d87270b8bd4f18c4d06ad0adc36f3db1f5279762cac2bf1e11354ead6d3b538e
source_last_modified: "2026-04-04T12:14:28.164058+00:00"
translation_last_reviewed: 2026-04-05
---

> Translation sync note (2026-04-05): this locale temporarily mirrors the updated English canonical text so the self-describing contract artifact and deploy API docs stay accurate while a refreshed translation is pending.

# Torii Contracts API (Bytecode Deploy & Fetch)

This document describes the app-facing HTTP endpoints for deploying self-describing contract bytecode and fetching the stored manifest/bytecode. These endpoints are thin wrappers around on-chain transactions and read-only queries; consensus semantics remain on-chain.

## Endpoints

- GET `/v1/contracts/code/{code_hash}`
  - Fetches the on-chain `ContractManifest` by its content-addressed `code_hash`.
  - Path param: `code_hash` — 32‑byte hex string.
  - Response body: `ContractCodeRecordDto` (JSON) with `manifest` populated.

- POST `/v1/contracts/deploy`
  - Accepts base64 `.to` bytecode with authority, private key, and a stable `contract_alias`; verifies the embedded `CNTR` contract interface, computes `code_hash` from the full artifact body after the fixed IVM header, computes `abi_hash` from the enforced ABI policy declared by the verified header, derives a fresh immutable `contract_address`, and binds the alias in the requested dataspace (`universal` by default).
  - Request body: `DeployContractDto`; response body: `DeployContractResponseDto`.
  - Submits a single transaction that registers the manifest, stores the bytecode, activates the fresh address-backed instance, and binds the alias.
  - Reusing an existing `contract_alias` performs an in-place upgrade: the response reports the new `contract_address`, the previous address, and `upgraded = true`.
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

### DeployContractResponseDto

```jsonc
{
  "ok": true,
  "contract_alias": "router::universal",
  "contract_address": "tairac1…",
  "previous_contract_address": null,
  "upgraded": false,
  "dataspace": "universal",
  "deploy_nonce": 0,
  "tx_hash_hex": "0123…cdef",
  "code_hash_hex": "0123…cdef",
  "abi_hash_hex":  "89ab…7654"
}
```

### Type encodings (JSON)

- `Hash` values (e.g., `code_hash`, `abi_hash`) are encoded as 64‑char lowercase hex strings (32 bytes).
- `AccountId` strings use canonical I105 literals (domainless encoded literal).
  Strict parser paths accept only canonical I105 literals (no `@<domain>` suffix).
- `ExposedPrivateKey` accepts either a bare multihash hex string or its algorithm-prefixed variant (e.g., `ed25519:…`). Responses normalise to bare multihash hex. Multihash hex is canonical: varint bytes are lowercase, payload bytes are uppercase, and `0x` prefixes are rejected.

### GET response: ContractCodeRecordDto

```jsonc
{
  "manifest": {
    "code_hash": "0123…cdef",
    "abi_hash":  "89ab…7654",
    "compiler_fingerprint": "rustc-1.79 llvm-16",
    "features_bitmap": 0
  }
}
```

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

The command prints a 32‑byte hex digest. Embed this value in `manifest.abi_hash`. Nodes verify that `abi_hash` equals their runtime policy hash and reject mismatches at admission.

## Security and governance

- Manifest and bytecode registration are public. Torii prepends a domainless
  self-registration for contract lifecycle writes so a signer can materialize
  its authority account in the same transaction when the network allows it.
- Alias-backed public deployment is the only supported app-facing activation
  flow. `ContractAddress` remains immutable per deployment, while
  `ContractAlias` is the stable public handle. Governance-controlled namespace
  binding remains an internal/governance concern, not a public contracts API.
- GET is read-only and content‑addressed by `code_hash`. Nodes may still apply access controls consistent with their governance policies.
