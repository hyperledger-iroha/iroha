# Torii Contract Lifecycle App API (TORII-APP-4)

Status: Completed 2026-03-24 · refreshed 2025-11-06  
Owners: Torii Platform, Smart Contract WG  
Roadmap reference: TORII-APP-4 — Contract lifecycle app endpoints

This note captures the request/response contracts, validation rules, and
telemetry surfaced by the Torii contract lifecycle endpoints so SDKs and
tooling can depend on stable Norito DTOs.

## Overview

- Handlers live in `crates/iroha_torii/src/routing.rs` and are wired when Torii
  is built with the `app_api` feature through
  `Torii::add_contracts_and_vk_routes`.【crates/iroha_torii/src/routing.rs:3631】【crates/iroha_torii/src/routing.rs:5892】【crates/iroha_torii/src/routing.rs:3809】【crates/iroha_torii/src/lib.rs:6551】
- Requests are decoded via `NoritoJson<T>`, so callers may send either
  `Content-Type: application/json` (Norito-backed JSON) or
  `application/x-norito`. Responses honour the `Accept` header the same way.
- Each endpoint constructs a signed transaction with the supplied authority and
  private key, then queues it through `handle_transaction_with_metrics`, which
  records `torii_lane_admission_latency_seconds{lane_id,endpoint}` when
  telemetry is enabled.【crates/iroha_torii/src/routing.rs:3293】
- DTOs embed full Norito types: `AccountId`, `ExposedPrivateKey`, `ContractManifest`,
  and plain strings for namespace/contract identifiers. The manifest schema is
  defined in `iroha_data_model::smart_contract::manifest::ContractManifest` and
  includes optional compiler metadata and access-set hints.【crates/iroha_data_model/src/smart_contract.rs:87】
- Router-level integration tests cover the standalone deploy path, direct
  activation, and the combined deploy+activate workflow, keeping these schemas
  regression-tested.【crates/iroha_torii/tests/contracts_deploy_integration.rs:1】【crates/iroha_torii/tests/contracts_instance_activate_integration.rs:1】【crates/iroha_torii/tests/contracts_activate_integration.rs:1】

## `POST /v2/contracts/code`

Submits a pre-built `RegisterSmartContractCode` transaction when clients already
possess the manifest (including hashes) and only need Torii to queue it.【crates/iroha_torii/src/routing.rs:3631】

### Request (`RegisterContractCodeDto`)

| Field | Type | Notes |
|-------|------|-------|
| `authority` | `AccountId` | Canonical I105 account id (domainless encoded literal). Torii strict parser paths accept only canonical I105 and reject non-I105 literals and any `@<domain>` suffix. |
| `private_key` | `ExposedPrivateKey` | Bare multihash hex as emitted by `ExposedPrivateKey::to_string()`; no `ed25519:` prefix is included.【crates/iroha_crypto/src/lib.rs:1994】 |
| `manifest` | `ContractManifest` | Optional fields; if `code_hash`/`abi_hash` are present they must match node-side validation.【crates/iroha_data_model/src/smart_contract.rs:87】 |

Sample JSON request:

```json
{
  "authority": "3xsmkps1KPBn9dtpE5qHRhHEZCpiAe8d9j6H9A42TV6kc1TpaqdwnSksKgQrsSEHznqvWKBMc1os69BELzkLjsR7EV2gjV14d9JMzo97KEmYoKtxCrFeKFAcy7ffQdboV1uRt",
  "private_key": "ED010820F1D2C3B4A596877899AABBCCDDEEFF00112233445566778899AABBCC",
  "manifest": {
    "code_hash": "f4d0bc7a2fa8c98bf5f5d6a638f3b939e1436a8a567164d72d41308c0ea2db9f",
    "abi_hash": "59bf03d5f0795884183abdb0297c7c9f6cfdcccd21d8a11a3ccf71027284e9a1",
    "compiler_fingerprint": "kotodama-0.8.0 (rustc 1.80)",
    "features_bitmap": null,
    "access_set_hints": {
      "read_keys": ["account:alice#wonderland"],
      "write_keys": ["account:alice#wonderland.detail:vm_state"]
    }
  }
}
```

### Response

- Success: `202 Accepted` with an empty body (`()`); the handler does not emit a
  JSON payload, and execution results surface later via pipeline status APIs.
- Failure: manifest mismatches map to `ValidationFail::Conversion` (HTTP 400);
  queue admission errors bubble up as `PushIntoQueue` failures with the
  endpoint label `/v2/contracts/code`.

## `POST /v2/contracts/deploy`

Accepts compiled `.to` bytecode, derives the manifest and hashes, and queues a
transaction containing `RegisterSmartContractCode` + `RegisterSmartContractBytes`
instructions so the bytecode is stored on-chain.【crates/iroha_torii/src/routing.rs:5892】

### Request (`DeployContractDto`)

| Field | Type | Notes |
|-------|------|-------|
| `authority` | `AccountId` | Same canonical form as above. |
| `private_key` | `ExposedPrivateKey` | Bare multihash hex string.【crates/iroha_crypto/src/lib.rs:1994】 |
| `code_b64` | `String` | Base64 representation of the compiled IVM program (`.to`). |

`prepare_contract_deployment` enforces:

- Base64 decoding must succeed (`ValidationFail::Conversion` on failure).【crates/iroha_torii/src/routing.rs:4898】
- The program metadata must decode with `abi_version == 1`; other versions are rejected (`"unsupported abi_version …"`).【crates/iroha_torii/src/routing.rs:4920】
- Any manifest override must match the computed `code_hash`/`abi_hash`; otherwise a conversion error is returned.【crates/iroha_torii/src/routing.rs:4942】
- The handler always persists the canonical manifest with both hashes populated.

Sample request and response:

```json
{
  "authority": "0x020001200000000000000000000000000000000000000000000000000000000000000000",
  "private_key": "ED010820F1D2C3B4A596877899AABBCCDDEEFF00112233445566778899AABBCC",
  "code_b64": "AAECAwQFBgcICQoLDA0ODw=="
}
```

```json
{
  "ok": true,
  "code_hash_hex": "f4d0bc7a2fa8c98bf5f5d6a638f3b939e1436a8a567164d72d41308c0ea2db9f",
  "abi_hash_hex": "59bf03d5f0795884183abdb0297c7c9f6cfdcccd21d8a11a3ccf71027284e9a1"
}
```

Applying the queued block persists both the manifest and bytecode in `World`,
after which `/v2/contracts/code-bytes/{hash}` can retrieve the uploaded program.
The integration test `contracts_deploy_and_fetch_code_bytes` exercises this
round-trip and asserts the stored base64 matches the uploaded bytes.【crates/iroha_torii/tests/contracts_deploy_integration.rs:45】

## `POST /v2/contracts/instance/activate`

Registers a logical contract instance within a namespace, binding it to a
previously deployed code hash via `ActivateContractInstance`. The route expects
the bytecode to be present on-chain (e.g., via the deploy endpoint above).【crates/iroha_torii/src/routing.rs:3806】

### Request (`ActivateInstanceDto`)

| Field | Type | Notes |
|-------|------|-------|
| `authority` | `AccountId` | Canonical I105 account id (domainless encoded literal). Torii strict parser paths accept only canonical I105 and reject non-I105 literals and any `@<domain>` suffix. |
| `private_key` | `ExposedPrivateKey` | Bare multihash hex string.【crates/iroha_crypto/src/lib.rs:1994】 |
| `namespace` | `String` | Governance namespace hosting the instance (e.g., `apps.market`). |
| `contract_id` | `String` | Logical identifier under the namespace (e.g., `calc.v1`). |
| `code_hash` | `String` | 32-byte hex digest; optional `0x` prefix is stripped before validation.【crates/iroha_torii/src/routing.rs:3818】 |

The handler rejects any value that does not decode to exactly 32 bytes and then
queues `ActivateContractInstance` with the provided identifiers.【crates/iroha_torii/src/routing.rs:3825】

Sample interaction:

```json
{
  "authority": "3xsmkps1KPBn9dtpE5qHRhHEZCpiAe8d9j6H9A42TV6kc1TpaqdwnSksKgQrsSEHznqvWKBMc1os69BELzkLjsR7EV2gjV14d9JMzo97KEmYoKtxCrFeKFAcy7ffQdboV1uRt",
  "private_key": "ED010820F1D2C3B4A596877899AABBCCDDEEFF00112233445566778899AABBCC",
  "namespace": "apps.market",
  "contract_id": "calc.v1",
  "code_hash": "0xf4d0bc7a2fa8c98bf5f5d6a638f3b939e1436a8a567164d72d41308c0ea2db9f"
}
```

```json
{
  "ok": true
}
```

Invalid hex or mismatched byte lengths surface as `ValidationFail::Conversion`
errors (HTTP 400). The standalone activation suite and the deploy+activate flow
verify the Norito shapes and resulting registry entries.【crates/iroha_torii/tests/contracts_instance_activate_integration.rs:1】【crates/iroha_torii/tests/contracts_activate_integration.rs:1】

---

For scenarios where deployment and activation must occur in a single request,
see `POST /v2/contracts/instance` and the shared DTOs
(`DeployAndActivateInstanceDto`) documented inline with the handler for future
expansion.【crates/iroha_torii/src/routing.rs:3862】【crates/iroha_torii/src/routing.rs:5085】
