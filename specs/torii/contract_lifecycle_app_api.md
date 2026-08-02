# Torii Contract Lifecycle App API (TORII-APP-4)

Status: Completed 2026-04-04 · refreshed 2026-07-18
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
- Contract deployment is client-signed. Torii never accepts a deployment
  private key or exposes a deployment-receipt API; clients submit verified
  upload, manifest-registration, and `CommitContractDeployment` instructions
  through the standard transaction pipeline.
- Runtime calls no longer resend full bytecode or manifests. Torii now builds
  `Executable::ContractCall(ContractInvocation)`, converts boundary JSON into
  one bounded, schema-hashed canonical Norito argument record before signing,
  and signs the exact live `expected_code_hash` into the invocation. Validators
  reject the call if governance rebinds the address before execution, so an
  in-flight signature cannot authorize different code. Transaction metadata
  mirrors the canonical `contract_code_hash` for unsigned-payload inspection; the
  invocation field is the consensus authority. Validators never interpret JSON
  as contract argument transport.
- Contract-call and contract-view target selectors require exactly one of
  `contract_address` or `contract_alias`.
- `POST /v1/contracts/call` supports two modes:
  - provide `public_key_hex` + `signature_b64` for detached-submit flows; or
  - provide neither and Torii returns canonical `transaction_payload_b64` plus
    its exact `signing_message_b64` (`HashOf<TransactionPayload>`).
- Multisig contract-call propose/approve endpoints are detached-or-unsigned-draft
  only. Supplying `private_key` fails closed because server-side signing is
  disabled on those routes.
- Historical `/v1/contracts/instance*` server-side-signing routes are no
  longer part of the public lifecycle surface.

## Locally signed deployment

Deployment clients verify the complete self-describing `.to` artifact before
signing anything. They then submit fixed-size upload chunks, finalize the exact
content-addressed artifact, register its locally signed manifest, and finally
submit `CommitContractDeployment`.

The commit instruction carries the expected authority deployment nonce and
expected previous alias target. Consensus checks those values together with the
derived address and registered code before activating the new address and
rotating the alias. The nonce is reserved consensus state and cannot be changed
through generic metadata instructions.

## `POST /v1/contracts/call`

Prepares or submits a `kotoage` call against an active deployed contract.
Before decoding the argument record, the runtime resolves the selected
entrypoint and authorizes its exact permission against an immutable snapshot of
the contract address, code hash, and complete alias binding. Direct execution,
nested calls, raw-IVM and `ContractCall` trigger callbacks, transaction
overlays, and proved overlays use the same pre-decode rule. Overlay application
then revalidates the live permission and binding before every queued effect or
durable-state write, so revocation, deactivation, or rebinding applies no
partial effects.

### Request (`ContractCallDto`)

| Field | Type | Notes |
|-------|------|-------|
| `authority` | `AccountId` | Transaction authority. |
| `public_key_hex` | `Option<String>` | Detached Ed25519 submit path. |
| `signature_b64` | `Option<String>` | Detached Ed25519 signature over `signing_message_b64`. |
| `contract_address` | `Option<ContractAddress>` | Canonical target address. |
| `contract_alias` | `Option<ContractAlias>` | Stable alias target. |
| `entrypoint` | `String` | Required. Must resolve to a `kotoage` declaration. |
| `payload` | `Option<IrohaJson>` | Optional Norito JSON payload normalized against the manifest schema. |
| `creation_time_ms` | `Option<u64>` | Optional fixed timestamp for deterministic detached flows. |
| `fee_payment` | `FeePaymentIntent` | Required typed payer selection, exact sponsor program/revision when sponsored, charge maxima, and positive gas bound. |

The retired `private_key`, `fee_sponsor`, `gas_asset_id`, and standalone
transaction `gas_limit` fields are rejected. Torii runs the same Core fee quote
used by `POST /v1/fees/quote`, retains the requested payer, exact program
revision, and gas bound, and replaces only the charge maxima before returning
the unsigned payload. Detached clients sign that exact quoted payload.

Direct settlement accepts either the transaction authority or one exact
sponsor program. Receipt-lane (`lane_relay_burn`) Nexus settlement is
exact-sponsor-only: authority-paid requests are rejected with
`relay_capacity_unavailable` because an authority balance is not an
authenticated receipt source lock.

Response (`ContractCallResponseDto`) always includes `ok`, `submitted`,
`dataspace`, `contract_address`, `code_hash_hex`, `abi_hash_hex`,
`creation_time_ms`, and `entrypoint`.

Submission-mode fields:

- Detached submit (`public_key_hex` plus `signature_b64`): `submitted = true`
  and `tx_hash_hex` is populated; unsigned-draft fields are absent.
- Unsigned-draft mode (no signature material): `submitted = false`, both
  transaction and entrypoint hashes remain absent, and Torii returns only the
  canonical Norito `TransactionPayload` bytes in `transaction_payload_b64`
  together with the exact `HashOf<TransactionPayload>` bytes in
  `signing_message_b64`. Torii does not fabricate a signed transaction for
  preparation.

## `POST /v1/contracts/call/simulate`

Executes a `kotoage` entrypoint locally without queueing a transaction.

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
- The top-level `gas_limit` is optional and defaults to `1500000`. Every item
  that omits its own `gas_limit` inherits that effective batch default; any
  supplied batch or per-item limit must be positive.
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
- The selector wire shape contains exactly one of `multisig_account_id` or
  `multisig_account_alias`, but this unsigned transaction-draft route accepts
  only the canonical `multisig_account_id`. An alias selector is rejected with
  `403 multisig_alias_signature_required`; body-asserted signer fields do not
  authenticate alias resolution.
- The contract target is selected by exactly one of `contract_address` or
  `contract_alias`.
- `gas_limit` defaults to `1500000` when omitted and must be positive when
  supplied.
- The route validates the signer against the live multisig spec, normalizes the
  contract payload, wraps the call in `MultisigPropose`, and returns
  `MultisigContractCallResponseDto` with `proposal_id`, `instructions_hash`,
  `resolved_multisig_account_id`, and either `tx_hash_hex` or the exact pair
  `transaction_payload_b64` plus `signing_message_b64`. A signed proposal that reaches quorum immediately also
  returns `executed_tx_hash_hex` equal to `tx_hash_hex`, because the proposal,
  approval, and nested call execute atomically in that transaction. It remains
  null when the proposal is only collecting signatures.

### `POST /v1/contracts/call/multisig/approve`

- Request type: `MultisigContractCallApproveDto`.
- Requires exactly one of `proposal_id` or `instructions_hash`.
- The multisig selector rules match the propose route: unsigned draft
  preparation requires the canonical `multisig_account_id`.
- Returns `MultisigContractCallResponseDto`, including
  `executed_tx_hash_hex` when the approval reached quorum and executed the
  proposal immediately.

## Historical Note

The older server-side deployment and activation shortcuts are not part of the
current contract lifecycle. Clients deploy with locally signed native
transactions and use the by-reference call/view routes described above.
