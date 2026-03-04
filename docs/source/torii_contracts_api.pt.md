---
lang: pt
direction: ltr
source: docs/source/torii_contracts_api.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: c23da2a36d12529d92f51f46a82d9cc14c8c453401ac3e7bb55ad1a051adb8e5
source_last_modified: "2026-01-22T15:38:30.729723+00:00"
translation_last_reviewed: 2026-01-30
---

# Torii Contracts API (Manifests & Deploy)

This document describes the app-facing HTTP endpoints for publishing and fetching smart contract manifests. These endpoints are thin wrappers around on-chain transactions and read-only queries; consensus semantics remain on-chain.

## Endpoints

- POST `/v1/contracts/code`
  - Wraps `RegisterSmartContractCode` into a signed transaction and submits it.
  - Request body: `RegisterContractCodeDto` (JSON or Norito JSON) — see schema below.
  - Response: HTTP 202 Accepted on successful enqueue; standard transaction admission errors otherwise.
  - Authorization: The `authority` must hold the `CanRegisterSmartContractCode` permission. Granting this permission is restricted by governance (default executor allows granting only in genesis).
  - Telemetry: increments `torii_contract_errors_total{endpoint="code"}` on handler errors and `torii_contract_throttled_total{endpoint="code"}` on limiter hits.
  - Failure responses:
    | Scenario | HTTP status | Body | Notes |
    | --- | --- | --- | --- |
    | Successful enqueue | `202 Accepted` | _empty body_ | Transaction hash is available from the signed payload; poll pipeline status for the final outcome. |
    | Authority lacks `CanRegisterSmartContractCode` | `202 Accepted` (initial response) | _see example below_ | The request is enqueued, but the pipeline later emits a rejection with `ValidationFail::NotPermitted`. |
    | `manifest.code_hash` or `manifest.abi_hash` is not 32‑byte lowercase hex | `400 Bad Request` | `invalid JSON body: invalid hex string ... manifest.code_hash` | Parsing happens in the Norito JSON extractor before the transaction is built. |
| Transaction queue is full | `429 Too Many Requests` | Queue rejection JSON (`code`, `message`, `queue`, optional `retry_after_seconds`) | Clients receive `Retry-After` along with queue depth/capacity headers; defer submission until load subsides or increase queue capacity. |

    Example rejection emitted by `/v1/pipeline/transactions/status` when the authority lacks the required permission (pretty-printed for clarity):

    ```json
    {
      "kind": "Transaction",
      "content": {
        "hash": "…",
        "status": {
          "kind": "Rejected",
          "content": "<base64 TransactionRejectionReason>"
        }
      }
    }
    ```

    The base64 `content` decodes to `TransactionRejectionReason::Validation(ValidationFail::NotPermitted("not permitted: CanRegisterSmartContractCode"))`.

    Example queue rejection body:

    ```json
    {
      "code": "queue_full",
      "message": "transaction queue is at capacity",
      "queue": {
        "state": "saturated",
        "queued": 65536,
        "capacity": 65536,
        "saturated": true
      },
      "retry_after_seconds": 1
    }
    ```

- GET `/v1/contracts/code/{code_hash}`
  - Fetches the on-chain `ContractManifest` by its content-addressed `code_hash`.
  - Path param: `code_hash` — 32‑byte hex string.
  - Response body: `ContractCodeRecordDto` (JSON) with `manifest` populated.

- POST `/v1/contracts/deploy`
  - Accepts base64 `.to` bytecode with authority and private key; computes `code_hash` (program body) and `abi_hash` (from header `abi_version`).
  - Request body: `DeployContractDto`; response body: `DeployContractResponseDto`.
  - Submits two ISIs in a single transaction: `RegisterSmartContractCode` (manifest) and `RegisterSmartContractBytes` (code storage).
  - Body size is limited by the `max_contract_code_bytes` custom parameter (default 16 MiB); raise the cap before uploading larger programs.
  - Telemetry: increments `torii_contract_errors_total{endpoint="deploy"}` on handler errors and `torii_contract_throttled_total{endpoint="deploy"}` when the limiter fires.
- POST `/v1/contracts/instance`
  - Accepts base64 `.to` bytecode and a target `(namespace, contract_id)` pair.
  - Wraps `RegisterSmartContractCode`, `RegisterSmartContractBytes`, and `ActivateContractInstance` into a single transaction so deployment and activation happen atomically.
  - Response body: `{ ok, namespace, contract_id, code_hash_hex, abi_hash_hex }`.
  - Telemetry: increments `torii_contract_errors_total{endpoint="instance"}` on handler errors and `torii_contract_throttled_total{endpoint="instance"}` when throttled.

- GET `/v1/contracts/code-bytes/{code_hash}`
  - Fetches stored code bytes for a given `code_hash`.
  - Response body: `{ code_b64 }`.

- POST `/v1/contracts/instance/activate`
  - Submits `ActivateContractInstance` binding `(namespace, contract_id) -> code_hash`.
  - Request body: `ActivateInstanceDto`; response body: `ActivateInstanceResponseDto`.
  - Telemetry: increments `torii_contract_errors_total{endpoint="activate"}` on handler errors and `torii_contract_throttled_total{endpoint="activate"}` on throttle events.

- GET `/v1/contracts/instances/{ns}`
  - Lists active contract instances for `ns`. Mirrors the governance listing endpoint.
  - Query params: `contains`, `hash_prefix`, `offset`, `limit`, `order` (same semantics as governance endpoint).
  - Response: `{ namespace, instances: [{ contract_id, code_hash_hex }], total, offset, limit }`.

## Schemas

### RegisterContractCodeDto

Represents a request to register a contract manifest by wrapping the instruction into a signed transaction.

```jsonc
{
  "authority": "ih58...",           // AccountId (string form)
  "private_key": "ed25519:...",              // ExposedPrivateKey (bare or prefixed multihash hex)
  "manifest": {
    "code_hash": "0123…cdef",                // 32-byte hex (optional in model, recommended here)
    "abi_hash":  "89ab…7654",                // 32-byte hex (optional)
    "compiler_fingerprint": "rustc-1.79 llvm-16", // optional
    "features_bitmap": 0                       // optional u64
  }
}
```

Notes:
- If `manifest.code_hash` is provided, the node stores the manifest keyed by `code_hash`.
- If present, `manifest.abi_hash` is validated against the node’s ABI policy.

### DeployContractDto

Upload compiled bytecode and let Torii derive the manifest and hashes.

```jsonc
{
  "authority":   "ih58...", // AccountId (string form)
  "private_key": "ed25519:0123…",    // ExposedPrivateKey (bare or prefixed multihash hex)
  "code_b64":    "Base64Payload=="
}
```

Notes:
- `code_b64` must decode to a valid IVM program header with `abi_version == 1`.
- The handler recomputes the manifest internally; callers do not provide one on this shortcut.
- The decoded bytecode length must not exceed `max_contract_code_bytes`; exceeding the limit triggers an `InvariantViolation` (`code bytes exceed cap`) during transaction admission.

### DeployContractResponseDto

```jsonc
{
  "ok": true,
  "code_hash_hex": "0123…cdef",
  "abi_hash_hex":  "89ab…7654"
}
```

### Type encodings (JSON)

- `Hash` values (e.g., `code_hash`, `abi_hash`) are encoded as 64‑char lowercase hex strings (32 bytes).
- `AccountId` strings use canonical IH58 literals (no `@domain` suffix).
  Optional `@<domain>` suffixes are accepted as routing hints, and aliases
  (`<label>@<domain>`) / `<public_key>@<domain>` are accepted via the resolver
  (`/v1/accounts/resolve`).
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

### DeployAndActivateInstanceDto

Represents a request to deploy bytecode and immediately bind `(namespace, contract_id)` to the resulting code hash.

```jsonc
{
  "authority":   "ih58...",
  "private_key": "ed25519:…",
  "namespace":   "apps",
  "contract_id": "calc.v1",
  "code_b64":    "…",
  "manifest": {                      // optional override; code/ABI hashes are validated
    "compiler_fingerprint": "rustc-1.79 llvm-16",
    "features_bitmap": 0,
    "access_set_hints": {
      "read_keys": ["account:ih58..."],
      "write_keys": ["asset:usd#wonderland"]
    }
  }
}
```

Notes:
- The node computes the canonical `code_hash` and ensures any provided `manifest.code_hash` matches it.
- The node recomputes the ABI hash from the IVM header (currently ABI v1). If `manifest.abi_hash` is supplied it must equal the computed hash; otherwise the computed value is inserted.
- Additional manifest metadata (compiler fingerprint, features bitmap, access hints) is preserved verbatim.

### Response: DeployAndActivateInstanceResponseDto

```jsonc
{
  "ok": true,
  "namespace": "apps",
  "contract_id": "calc.v1",
  "code_hash_hex": "0123…cdef",
  "abi_hash_hex": "89ab…7654"
}
```

### ActivateInstanceDto

Bind an existing manifest/code hash to a namespace contract identifier.

```jsonc
{
  "authority":   "ih58...",
  "private_key": "ed25519:0123…",
  "namespace":   "apps",
  "contract_id": "calc.v1",
  "code_hash":   "89ab…7654"
}
```

Notes:
- `code_hash` must be exactly 32 bytes of lowercase hex. A leading `0x` is tolerated and stripped.
- Manifests and code bytes for `code_hash` must already exist; otherwise the transaction will fail downstream.
- Protected namespaces still enforce `gov_namespace`/`gov_contract_id` metadata checks; populate them via CLI helpers or custom transactions.

### ActivateInstanceResponseDto

```jsonc
{
  "ok": true
}
```

### Norito payloads

All DTOs derive both `JsonSerialize` and `NoritoSerialize`. Clients may submit either plain JSON or Norito-backed JSON. When emitting Norito via Kotodama tests or automation, use `norito::json::json!` with the same field names and encodings shown above so the `NoritoJson<T>` extractor can decode the payload deterministically.

### Rate limiting & telemetry

- `torii.deploy_rate_per_origin_per_sec` and `torii.deploy_burst_per_origin` configure the token bucket shared by `/v1/contracts/{code,deploy,instance,instance/activate}`. Defaults: 4 req/s with a burst of 8 per origin token (`X-API-Token`, remote IP, endpoint tuple).
- Requests rejected by the limiter increment `torii_contract_throttled_total{endpoint}` where `endpoint` is `code`, `deploy`, `instance`, or `activate`.
- Any handler error (invalid body, permission missing, queue failure) increments `torii_contract_errors_total{endpoint}`. Track alongside queue metrics for alerting.

## Examples

Register a manifest (wrap into signed tx):

```bash
curl -s -X POST \
  -H 'Content-Type: application/json' \
  -d '{
        "authority": "ih58...",
        "private_key": "ed25519:…",  
        "manifest": { "code_hash": "<32-byte-hex>", "abi_hash": null }
      }' \
  http://127.0.0.1:8080/v1/contracts/code
```

Fetch a manifest by hash:

```bash
curl -s http://127.0.0.1:8080/v1/contracts/code/<32-byte-hex> | jq .

Deploy code and then fetch code bytes:

```bash
curl -s -X POST \
  -H 'Content-Type: application/json' \
  -d '{
        "authority": "ih58...",
        "private_key": "ed25519:…",
        "code_b64": "…"
      }' \
  http://127.0.0.1:8080/v1/contracts/deploy | jq .

curl -s http://127.0.0.1:8080/v1/contracts/code-bytes/<32-byte-hex> | jq .
```

Deploy and activate an instance atomically:

```bash
curl -s -X POST \
  -H 'Content-Type: application/json' \
  -d '{
        "authority": "ih58...",
        "private_key": "ed25519:…",
        "namespace": "apps",
        "contract_id": "calc.v1",
        "code_b64": "…"
      }' \
  http://127.0.0.1:8080/v1/contracts/instance | jq .
```

Activate an existing instance with previously uploaded artifacts:

```bash
curl -s -X POST \
  -H 'Content-Type: application/json' \
  -d '{
        "authority": "ih58...",
        "private_key": "ed25519:…",
        "namespace": "apps",
        "contract_id": "calc.v1",
        "code_hash": "<32-byte-hex>"
      }' \
  http://127.0.0.1:8080/v1/contracts/instance/activate | jq .
```

### Computing `abi_hash` for manifests

Manifests may include an `abi_hash` that binds the program to the node’s IVM ABI policy. You can compute this hash locally using the CLI:

```bash
# ABI v1
iroha tools ivm abi-hash --policy v1 --uppercase

The command prints a 32‑byte hex digest. Embed this value in `manifest.abi_hash`. Nodes verify that `abi_hash` equals their runtime policy hash and reject mismatches at admission.

## Security and governance

- Only accounts with `CanRegisterSmartContractCode` may submit manifests; by default, granting this permission is restricted to genesis. Networks may customize governance to expand who can grant it.
- GET is read-only and content‑addressed by `code_hash`. Nodes may still apply access controls consistent with their governance policies.
