---
lang: az
direction: ltr
source: docs/source/torii/contract_lifecycle_app_api.md
status: needs-update
generator: scripts/sync_docs_i18n.py
source_hash: d712ede36c3431573b639fbfa55064a3e210e7d7618bb5a78d588511a33ae89a
source_last_modified: "2026-03-20T07:39:53+00:00"
translation_last_reviewed: 2026-03-20
---

> Translation sync note (2026-03-20): this locale temporarily mirrors the updated English canonical text so the self-describing contract artifact and deploy API docs stay accurate while a refreshed translation is pending.

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
- DTOs embed full Norito types: `AccountId`, `ExposedPrivateKey`, and plain
  strings for namespace/contract identifiers. The stored manifest schema is
  defined in `iroha_data_model::smart_contract::manifest::ContractManifest` and
  includes compiler metadata, verified entrypoints, and access-set hints
  derived from the uploaded artifact.【crates/iroha_data_model/src/smart_contract.rs:87】
- Router-level integration tests cover the standalone deploy path, direct
  activation, and the combined deploy+activate workflow, keeping these schemas
  regression-tested.【crates/iroha_torii/tests/contracts_deploy_integration.rs:1】【crates/iroha_torii/tests/contracts_instance_activate_integration.rs:1】【crates/iroha_torii/tests/contracts_activate_integration.rs:1】

## `POST /v1/contracts/deploy`

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
- The uploaded artifact must verify as a self-describing contract artifact: IVM `1.1`, required `CNTR`, valid section ordering, valid entrypoints, valid trigger callback targets, and supported ABI/feature metadata. Invalid artifacts fail closed with `ValidationFail::Conversion`.
- Torii derives the canonical manifest from the verified `CNTR` payload and signs that manifest with the submitting key. There is no manifest override input on the deploy path.

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
after which `/v1/contracts/code-bytes/{hash}` can retrieve the uploaded program.
The integration test `contracts_deploy_and_fetch_code_bytes` exercises this
round-trip and asserts the stored base64 matches the uploaded bytes.【crates/iroha_torii/tests/contracts_deploy_integration.rs:45】

## `POST /v1/contracts/instance/activate`

Registers a logical contract instance within a namespace, binding it to a
previously deployed code hash via `ActivateContractInstance`. The route expects
the bytecode to be present on-chain (e.g., via the deploy endpoint above).【crates/iroha_torii/src/routing.rs:3806】

### Request (`ActivateInstanceDto`)

| Field | Type | Notes |
|-------|------|-------|
| `authority` | `AccountId` | Canonical I105 account id (domainless encoded literal). Torii strict parser paths accept only canonical I105 and reject non-i105 literals and any `@<domain>` suffix. |
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
see `POST /v1/contracts/instance` and the shared DTOs
(`DeployAndActivateInstanceDto`) documented inline with the handler for future
expansion.【crates/iroha_torii/src/routing.rs:3862】【crates/iroha_torii/src/routing.rs:5085】
