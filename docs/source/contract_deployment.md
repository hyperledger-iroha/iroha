---
title: Contract Deployment (.to) — API & Workflow
---

Status: implemented and exercised by Torii, CLI, and core admission tests (July 2026).

## Overview

- Deploy compiled IVM bytecode (`.to`) through the bounded native upload
  protocol used by Torii and the CLI. `RegisterSmartContractBytes` remains a
  low-level atomic instruction for callers that already operate within a
  bounded transaction envelope.
- Contract `.to` artifacts are self-describing: the required `CNTR` section
  embeds the contract interface ahead of the executable stream, and Torii
  derives the on-chain `ContractManifest` from that section after verification.
- Nodes recompute `code_hash` over the complete deployable `.to` bytes,
  including the execution header, `CNTR`, literals, and code, and recompute the
  canonical ABI hash locally; mismatches reject deterministically.
- Stored artifacts live under the on-chain `contract_manifests` and
  `contract_code` registries. Manifests reference hashes only and remain small;
  code bytes are keyed by `code_hash`.
- Protected namespaces can require an enacted governance proposal before a
  deployment is admitted. The admission path looks up the proposal payload and
  enforces `(contract_address, code_hash, abi_hash)` equality when the
  namespace is protected.

## Stored Artifacts & Retention

- `RegisterSmartContractBytes` stores the compiled program under
  `contract_code[code_hash]` after verifying the self-describing `CNTR`
  artifact and recomputing its canonical hash. If bytes for a hash already
  exist they must match exactly; differing bytes raise an invariant violation.
- `RegisterSmartContractCode` inserts/overwrites the manifest for a given
  `code_hash` only after the matching bytecode is already stored. The stored
  bytes must verify as a `CNTR` artifact whose embedded manifest payload
  matches the submitted manifest payload.
- Code size is capped by the custom parameter `max_contract_code_bytes`
  (default 16 MiB). Override it with a `SetParameter(Custom)` transaction before
  registering larger artifacts.
- Retention is unbounded: manifests and code remain available until explicitly
  removed in a future governance workflow. There is no TTL or automatic GC.

## Native chunk upload lifecycle

User-facing deployment splits every artifact into fixed 65,536-byte chunks.
The consensus API consists of:

- `UploadSmartContractCodeChunk { code_hash, total_size, chunk_index,
  chunk_count, chunk }`;
- `FinalizeSmartContractCodeUpload { code_hash, total_size, chunk_count }`; and
- `CancelSmartContractCodeUpload { code_hash }`.

Pending uploads are owned by `(authority, code_hash)` and survive ordinary
state snapshots and tiered-state restoration. The descriptor must use the
exact ceiling chunk count for `total_size`; every non-final chunk is exactly
65,536 bytes and the final chunk has the exact remaining length. Checked
integer conversions reject unrepresentable sizes or indices. An authority may
have at most four pending uploads, and the sum of their declared sizes may not
exceed that authority's current `max_contract_code_bytes` cap. A parameter
update that would lower the cap below any authority's pending declared total is
rejected before changing configuration. The configured cap and each declared
artifact are also bounded at 2,147,483,647 bytes so descriptor acceptance is
identical on supported 32-bit and 64-bit peers.

Chunks may arrive out of order. Replaying the same bytes at the same index is
idempotent, while changing a descriptor or replaying an index with different
bytes is rejected. Finalization requires every index, rebuilds bytes in index
order, and rechecks the exact size, domain-separated code hash, IVM/CNTR
artifact, cycle ceiling, and current code-size cap. It then registers through
the same atomic helper as `RegisterSmartContractBytes` and removes the pending
upload. A failed finalization keeps all staged chunks so the caller can retry
after correcting the missing prerequisite. `CancelSmartContractCodeUpload` is
owner-scoped, idempotent cleanup.

Safe retry behavior follows directly from those rules: resend any uncertain
chunk unchanged, inspect or wait for committed progress, then retry
finalization. Cancel only when the artifact should be abandoned; another
authority cannot cancel or overwrite the owner's upload. This is a
first-release state format, so nodes do not migrate snapshots containing the
retired IVM state-staging scheme.

## Admission pipeline

- Contract deployment parses the artifact, requires IVM `1.1`, requires the
  embedded `CNTR` section, and verifies the embedded interface against the
  decoded executable stream before any manifest is stored.
- Verification fails closed on malformed sections, duplicate/invalid
  entrypoints, invalid `entry_pc` targets, invalid trigger callbacks, feature
  / ABI mismatches, or unsupported metadata.
- The canonical manifest is built from the verified `CNTR` payload, signed by
  the submitting key, and then stored after the uploaded bytecode has been
  verified and written under the same `code_hash`.
- Transactions targeting protected namespaces must include metadata key
  `gov_contract_address`. The admission path compares the derived dataspace and
  address against enacted `DeployContract` proposals; if no matching proposal
  exists the transaction is rejected with `NotPermitted`.

## Runtime invocation authorization

Every invocation selects one declared entrypoint and requires that
entrypoint's exact typed permission before argument decoding, VM execution, or
proof verification. The authorization context binds the authority, permission,
contract address, code hash, and complete alias record. This rule is identical
for direct `ContractCall`, raw-IVM and `ContractCall` trigger callbacks, nested
contract calls, transaction overlays, and proved-overlay replay.

Authorization at dispatch is not a durable-write lease. Before each queued
instruction or state write is applied, the overlay revalidates the permission
and live address/code/alias binding. A revoked permission, deactivated
contract, or changed binding therefore rejects the overlay without applying
the affected effect or write.

## Torii endpoints (feature `app_api`)

- `POST /v1/contracts/deploy`
  - Request body: `DeployContractDto` (see `docs/source/torii_contracts_api.md` for field details).
  - Torii decodes the base64 payload, verifies the embedded `CNTR` interface,
    derives the manifest from the artifact itself, allocates a fresh immutable
    `contract_address`, and executes the native chunk plan on behalf of the
    caller. For multi-chunk artifacts it commits the first upload together with
    any required domainless authority bootstrap, submits the remaining
    pre-stage chunks in order, and waits until their committed progress is
    visible before admitting the final transaction. That final transaction
    uploads the last chunk, finalizes code registration, registers the
    manifest, activates the instance, and binds the requested alias.
  - When the signing authority does not exist at transaction start, its first
    deployment transaction begins with the exact ordered prefix
    `Register<Account>(self)`, `Grant<CanRegisterSmartContractCode>(self)`,
    then either `UploadSmartContractCodeChunk` or, for matching code already
    stored on-chain, `RegisterSmartContractCode`. The transaction execution
    policy, mirrored by the built-in and default runtime executors, permits this
    one atomic deployment bootstrap only for an absent transaction authority.
    It does not permit an existing account to self-grant, a grant to another
    account, a differently encoded permission, or a reordered prefix.
    For a one-chunk artifact this prefix, chunk upload, finalization, manifest
    registration, and activation all occur in the final transaction.
  - Matching code already present under `code_hash` skips the upload stages.
    When its manifest is absent, the final transaction registers it and
    activates/binds the new instance; a missing authority can use that manifest
    registration as the third bootstrap instruction. When both matching code
    and manifest already exist, an existing authorized account skips both
    registrations, while a missing authority is rejected because activation
    alone cannot qualify for the narrow self-grant exception. A rejected stage
    fails immediately. A progress timeout reports expected and observed
    committed chunks while retaining resumable pending state.
  - Redeploying the same `contract_alias` performs an in-place `kaizen`/`改善`:
    Torii deploys a new address, rebinds the alias atomically, and deactivates
    the previous address.
  - Response: `DeployContractBundleReceiptDto` with bundle metadata plus one
    entry in `contracts[]` for this single-contract shortcut. Every contract
    receipt contains required `upload_stage_tx_hashes`; `tx_hash_hex` is the
    final deployment transaction hash.
  - Errors: invalid base64, invalid contract artifact, size cap exceeded,
    governance gating for protected namespaces, or fee/balance failures.
- `GET /v1/contracts/code/{code_hash}`
  - Returns `{ code_hash, abi_hash, manifest: <ContractManifest> }`. The two
    top-level convenience values are raw lowercase hex; `manifest` uses the
    complete canonical Norito JSON representation, including `seiyaku_name`,
    both checksummed `Hash` literals, exact entrypoint argument/return schemas,
    state and error declarations, access metadata, trigger descriptors,
    localization data, and signed provenance when present. Fields are never
    silently truncated. V1 aggregate schemas are one flat preorder tape. A
    `List` node contains only `capacity`; its exact element subtree immediately
    follows it. Missing or trailing nodes and the retired nested `element`
    representation are rejected.
- `GET /v1/contracts/code-bytes/{code_hash}`
  - Returns `{ code_b64 }` with the stored `.to` image encoded as base64.

All contract lifecycle endpoints share a dedicated deploy limiter configured via
`torii.deploy_rate_per_origin_per_sec` (tokens per second) and
`torii.deploy_burst_per_origin` (burst tokens). Defaults are 4 req/s with a burst of
8 for each token/key derived from `X-API-Token`, the remote IP, or the endpoint hint.
Set either field to `null` to disable the limiter for trusted operators. When the
limiter fires, Torii increments the
`torii_contract_throttled_total{endpoint="deploy"}` telemetry counter and
returns HTTP 429; any handler error increments
`torii_contract_errors_total{endpoint=…}` for alerting.

## Governance integration & protected namespaces

- Set the custom parameter `gov_protected_namespaces` (JSON array of namespace
  strings) to enable admission gating. Torii exposes helpers under
  `/v1/gov/protected-namespaces` and the CLI mirrors them via
  `iroha_cli app gov protected set` / `iroha_cli app gov protected get`.
- Every raw bytecode registration, manifest registration, activation,
  deactivation, or bytecode removal requires `CanRegisterSmartContractCode`.
  Protected namespaces additionally require `CanEnactGovernance`; an empty
  `gov_protected_namespaces` list never makes lifecycle mutation permissionless.
- Proposals created with `ProposeDeployContract` (or the Torii
  `/v1/gov/proposals/deploy-contract` endpoint) capture
  `(contract_address, code_hash, abi_hash, abi_version)`.
- Once the referendum passes, `EnactReferendum` marks the proposal Enacted and
  admission will accept deployments that carry matching metadata and code.
- Transactions must include `gov_contract_address=<contract-address>`. CLI
  helpers populate the governance metadata automatically when you pass
  `--contract-address` or `--contract-alias`.
- If the lane manifest sets a validator quorum above one, include
  `gov_manifest_approvers` (JSON array of validator account IDs) so the queue can count
  the additional approvals alongside the transaction authority. Lanes also reject
  metadata that references namespaces not present in the manifest's
  `protected_namespaces` set.

## CLI helpers

- `iroha contract deploy --authority <id> --private-key <hex> --code-file <path> --contract-alias <name::dataspace>`
  submits the alias-first Torii deploy request (computing hashes on the fly).
- `ivm_contract_deploy` uses the same native plan in blocking and emit modes.
  Transactions 1 through N-1 each carry one chunk; the final registration
  transaction carries chunk N plus finalization. Manifest registration and
  activation remain separate transactions. Emit mode names files
  `register-bytes-chunk-NNNN-of-NNNN`, `register-bytes-finalize`,
  `register-manifest`, and `activate` in submission order. Its JSON reports
  `register_bytes_tx_strategy = "native_chunks"`, chunk size/count,
  `register_bytes_stage_tx_hashes`, and the finalization hash in
  `register_bytes_tx_hash`. `--skip-register-bytes` omits the complete
  upload/finalize sequence.
- `iroha contract manifest build --code-file <path> [--sign-with <hex>]` computes
  `code_hash`/`abi_hash` for compiled `.to`, derives the manifest from the
  embedded `CNTR`, and optionally signs it for inspection, printing JSON or
  writing to `--out`.
- `iroha contract simulate --authority <id> --private-key <hex> --code-file <path> --gas-limit <u64>`
  runs an offline VM pass and reports ABI/hash metadata plus the queued ISIs
  (counts and instruction ids) without touching the network.
- `iroha contract manifest get --code-hash <hex>` fetches the manifest via Torii
  and optionally writes it to disk.
- `iroha contract code get --code-hash <hex> --out <path>` downloads
  the stored `.to` image.
- Governance helpers (`iroha_cli app gov deploy propose`, `iroha_cli app gov enact`,
  `iroha_cli app gov protected set/get`) orchestrate the protected-namespace workflow and
  expose JSON artefacts for auditing.

## Testing & coverage

- Unit tests under `crates/iroha_core/tests/contract_code_bytes.rs` cover shape,
  quota, authorization, out-of-order and duplicate handling, missing/corrupt
  chunks, hash/artifact failure retention, direct-registration races,
  cancellation, event emission, cleanup, and cap enforcement. State tests
  cover snapshot and tiered restoration of partial uploads.
- `crates/iroha_core/tests/gov_enact_deploy.rs` validates manifest insertion via
  enactment, and `crates/iroha_core/tests/gov_protected_gate.rs` exercises
  protected-namespace admission end-to-end.
- Kura has a two-lane interrupted-`FilesApplied` restart regression for exact
  path rollback and chain preservation. CLI tests cover multi-MiB bounded
  transactions, one-chunk and skip behavior, stable metadata, and ordered emit
  files. Torii tests cover bounded plans, bootstrap, committed progress,
  rejection, timeout retention, registered-code skips, and required receipt
  fields.

Refer to `docs/source/governance_api.md` for detailed referendum payloads and
ballot workflows.
