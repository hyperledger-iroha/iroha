# Torii Contract Lifecycle App API (TORII-APP-4)

Status: Completed 2026-04-04 · refreshed 2026-04-04
Owners: Torii Platform, Smart Contract WG  
Roadmap reference: TORII-APP-4 — Contract lifecycle app endpoints

This note captures the current public contract lifecycle surfaces exposed by
Torii when the `app_api` feature is enabled.

## Overview

- Handlers live in `crates/iroha_torii/src/routing.rs` and are registered
  through `Torii::add_contracts_and_vk_routes`.
- Requests are decoded with `NoritoJson<T>`, so callers may use either
  `application/json` or `application/x-norito`. Responses follow the negotiated
  `Accept` format.
- Public deploys are alias-first. `POST /v1/contracts/deploy` requires
  `contract_alias`, derives the dataspace from that alias, and returns the same
  canonical bundle receipt shape used by `POST /v1/contracts/deploy-bundle`
  with exactly one `contracts[]` entry.
- Runtime calls no longer resend full bytecode or manifests. Torii now builds
  `Executable::ContractCall(ContractInvocation)` and only keeps fee/gas fields
  in transaction metadata.
- Contract-call and contract-view target selectors require exactly one of
  `contract_address` or `contract_alias`.
- `POST /v1/contracts/call` supports three submission modes:
  - provide `private_key` and Torii signs/submits immediately;
  - provide `public_key_hex` + `signature_b64` for detached-submit flows; or
  - provide neither and Torii returns a scaffold plus `signing_message_b64`.
- Multisig contract-call propose/approve endpoints are detached-or-scaffold
  only. Supplying `private_key` fails closed because server-side signing is
  disabled on those routes.
- Historical `/v1/contracts/instance*` server-side-signing routes are no
  longer part of the public lifecycle surface.

## `POST /v1/contracts/deploy`

Uploads compiled `.to` bytecode, verifies the embedded `CNTR` interface,
derives the canonical manifest, stores manifest + bytecode on-chain, activates
the fresh address-backed instance, binds the stable alias, and advances the
authority's deploy nonce in one transaction.

### Request (`DeployContractDto`)

| Field | Type | Notes |
|-------|------|-------|
| `authority` | `AccountId` | Canonical I105 account id. |
| `private_key` | `ExposedPrivateKey` | Signing key used to submit the deploy transaction. |
| `code_b64` | `String` | Base64-encoded compiled IVM artifact (`.to`). |
| `contract_alias` | `ContractAlias` | Stable public alias (`name::dataspace` or `name::domain.dataspace`). |
| `lease_expiry_ms` | `Option<u64>` | Optional unix-ms lease expiry for the alias binding. |

Validation and execution rules:

- `code_b64` must decode successfully.
- The artifact must verify as a self-describing IVM contract artifact with a
  valid `CNTR` section.
- Torii derives the manifest from the verified artifact; callers do not supply
  a manifest override on this route.
- The dataspace is derived from `contract_alias`.
- `contract_address` is derived from `(chain_discriminant, authority,
  deploy_nonce, dataspace_id)`.
- Reusing an existing `contract_alias` is the public upgrade path: Torii
  clears the prior alias binding, deactivates the retired address, binds the
  alias to the new address, and reports `previous_contract_address`.

### Response (`DeployContractBundleReceiptDto`)

| Field | Type | Notes |
|-------|------|-------|
| `ok` | `bool` | `true` when the deploy transaction was queued. |
| `bundle_name` | `String` | Fixed to `single-contract-deploy` on this shortcut. |
| `bundle_digest` | `String` | Canonical digest derived from the receipt payload. |
| `chain_fingerprint` | `String` | Receipt storage scope (`<chain_id>@<block-1-hash>`). |
| `dry_run` | `bool` | Always `false` on this route. |
| `completed_stages` | `Vec<String>` | Completed pipeline stages; single deploys normally return `["plan", "deploy"]`. |
| `failure_point` | `Option<String>` | Populated only when a persisted receipt records a failure. |
| `contracts` | `Vec<DeployContractBundleContractReceiptDto>` | Always contains exactly one contract receipt for this route. |
| `init_calls` | `Vec<DeployContractBundleCallReceiptDto>` | Empty on this shortcut. |
| `assertions` | `Vec<DeployContractBundleAssertionReceiptDto>` | Empty on this shortcut. |

For `POST /v1/contracts/deploy`, the sole `contracts[0]` entry carries the
fresh immutable `contract_address`, the stable `contract_alias`, any
`previous_contract_address` retired by an upgrade, the resolved `dataspace`,
the consumed `deploy_nonce`, `tx_hash_hex`, `code_hash_hex`, `abi_hash_hex`,
and the current receipt `status`. Single deploy receipts normally return
`status = "submitted"` because the route returns after queue admission; bundle
flows that continue into init/assertion stages may advance that status to
`"deployed"` before returning.

## `POST /v1/contracts/call`

Prepares or submits a public contract call against an active deployed contract.

### Request (`ContractCallDto`)

| Field | Type | Notes |
|-------|------|-------|
| `authority` | `AccountId` | Transaction authority. |
| `private_key` | `Option<ExposedPrivateKey>` | Server-side signing path. |
| `public_key_hex` | `Option<String>` | Detached Ed25519 submit path. |
| `signature_b64` | `Option<String>` | Detached Ed25519 signature over `signing_message_b64`. |
| `contract_address` | `Option<ContractAddress>` | Canonical target address. |
| `contract_alias` | `Option<ContractAlias>` | Stable alias target. |
| `entrypoint` | `Option<String>` | Defaults to `main`. Must resolve to a public entrypoint. |
| `payload` | `Option<IrohaJson>` | Optional Norito JSON payload normalized against the manifest schema. |
| `creation_time_ms` | `Option<u64>` | Optional fixed timestamp for deterministic detached flows. |
| `gas_asset_id` | `Option<String>` | Optional metadata override. |
| `fee_sponsor` | `Option<AccountId>` | Optional fee sponsor metadata. |
| `gas_limit` | `u64` | Must be positive. |

Response (`ContractCallResponseDto`) always includes `ok`, `submitted`,
`dataspace`, `contract_address`, `code_hash_hex`, `abi_hash_hex`,
`creation_time_ms`, and `entrypoint`.

Submission-mode fields:

- Immediate submit (`private_key` or detached signature): `submitted = true`
  and `tx_hash_hex` is populated.
- Scaffold mode (no signature material): `submitted = false` and Torii returns
  `transaction_scaffold_b64`, `signed_transaction_b64`, and
  `signing_message_b64`.

## `POST /v1/contracts/call/simulate`

Executes a public contract entrypoint locally without queueing a transaction.

- Request type: `ContractCallSimulateDto`.
- Uses the same address-or-alias selector, entrypoint validation, payload
  normalization, and positive `gas_limit` requirement as `POST /v1/contracts/call`.
- Success response (`ContractCallSimulateResponseDto`) includes:
  `ok = true`, `dataspace`, `contract_address`, `code_hash_hex`,
  `abi_hash_hex`, `entrypoint`, `normalized_payload`, `gas_limit`, `gas_used`,
  `queued_instructions`, and optional decoded `result`.
- Failure response uses the same DTO shape with `ok = false`, plus `error` and
  optional `vm_diagnostic`.

## `POST /v1/contracts/view`

Executes a read-only view entrypoint locally.

- Request type: `ContractViewDto`.
- The selector rules are the same as call/simulate: exactly one of
  `contract_address` or `contract_alias`.
- `entrypoint` defaults to `main` but must resolve to a manifest entrypoint of
  kind `View`.
- `gas_limit` must be positive.
- Success returns `ContractViewResponseDto` with `ok`, `dataspace`,
  `contract_address`, `code_hash_hex`, `abi_hash_hex`, `entrypoint`, and
  decoded `result`.
- VM/view failures return HTTP `422 Unprocessable Entity` with
  `ContractViewErrorResponseDto`, including the same target metadata plus
  `error` and optional `vm_diagnostic`.

## `POST /v1/contracts/view/batch`

Executes multiple read-only view entrypoints in one HTTP round-trip.

- Request type: `ContractViewBatchDto`.
- The top-level `authority` supplies the read authority and host context for
  every item in the batch.
- The top-level `gas_limit` is optional; when present it becomes the default
  item gas limit and must still be positive.
- Each `ContractViewBatchItemDto` follows the same selector rules as
  `ContractViewDto`: exactly one of `contract_address` or `contract_alias`,
  `entrypoint` defaults to `main`, and the selected manifest entrypoint must be
  of kind `View`.
- The response always returns an `items` array with one normalized result per
  request item. Individual failures are reported inline with `ok = false`,
  `error`, and optional `vm_diagnostic`.

## Rollup Endpoints

### `GET /v1/contracts/rollups/swaps/fills`

- Query type: `ContractRollupSwapsFillsParams`.
- Required query: `authority`.
- Optional queries: `limit`, `offset`, `contract_address`, `contract_alias`,
  and `scan_limit`.
- The route walks the router mirror history, stitches it to indexed swap
  events, and returns trader-facing fill cards plus pagination metadata.

### `GET /v1/contracts/rollups/swaps/candles`

- Query type: `ContractRollupSwapsCandlesParams`.
- Required query: `authority`.
- Optional queries: `limit`, `offset`, `contract_address`, `contract_alias`,
  `scan_limit`, and `bucket_ms`.
- The route reuses the fills rollup and buckets the stitched fills into OHLC
  candle windows.

### `GET /v1/contracts/rollups/trader/activity`

- Query type: `ContractEventGetParams`.
- Optional queries: `limit`, `offset`, `authority`, `contract_address`,
  `contract_alias`, `module`, `event_kind`, `participant`, `asset_id`,
  `provenance`, `since_timestamp_ms`, `until_timestamp_ms`, and `result_ok`.
- The route filters the indexed contract-event stream down to the supported
  trader modules and returns a trader-facing activity feed.

### `GET /v1/contracts/rollups/trader/account`

- Query type: `TraderRollupAccountParams`.
- Required query: `authority`.
- Optional query: `scan_limit`.
- The route combines stitched swap fills, derived swap analytics, and supported
  trader-module activity cards into one account summary payload.

## Multisig Contract Calls

### `POST /v1/contracts/call/multisig/propose`

- Request type: `MultisigContractCallProposeDto`.
- The multisig authority is selected by exactly one of
  `multisig_account_id` or `multisig_account_alias`.
- The contract target is selected by exactly one of `contract_address` or
  `contract_alias`.
- `gas_limit` defaults to `5000` when omitted and must be positive when
  supplied.
- The route validates the signer against the live multisig spec, normalizes the
  contract payload, wraps the call in `MultisigPropose`, and returns
  `MultisigContractCallResponseDto` with `proposal_id`, `instructions_hash`,
  `resolved_multisig_account_id`, and either `tx_hash_hex` or
  `signing_message_b64`.

### `POST /v1/contracts/call/multisig/approve`

- Request type: `MultisigContractCallApproveDto`.
- Requires exactly one of `proposal_id` or `instructions_hash`.
- The multisig selector rules match the propose route.
- Returns `MultisigContractCallResponseDto`, including
  `executed_tx_hash_hex` when the approval reached quorum and executed the
  proposal immediately.

## Historical Note

The older public `/v1/contracts/instance` and
`/v1/contracts/instance/activate` shortcuts are no longer part of the current
contract lifecycle. Public callers should use the alias-first deploy route plus
the by-reference call/view routes described above.
