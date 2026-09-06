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

## Contract address identity

Every V1 contract address uses the single lowercase Bech32m HRP `irohac`.
The HRP is presentation only: parsers reject any other HRP, and network
identity is committed inside the digest rather than inferred from a display
prefix. The 29-byte address payload is
`version_u8 || dataspace_id_u64_be || digest[0..20]`, where `version_u8` is
`1` and `digest` is BLAKE3 over this exact preimage:

```text
"iroha:contract-address:v1"
|| network_id_raw_32
|| dataspace_id_u64_be || deploy_nonce_u64_be
|| deployer_canonical_bytes_len_u32_be || deployer_canonical_bytes
```

`network_id_raw_32` is the exact genesis consensus-header hash, without text or
length framing. `CommitContractDeployment` always supplies the authenticated
`NetworkId` from consensus state and recomputes the address before writing any
binding. Client-side derivation APIs therefore require an explicit canonical
`NetworkId`; a human-readable `ChainId` is not accepted as a security-domain
input. The account-address I105 discriminant is used only to decode or render
the deployer literal and is never a contract-identity input. The CLI spelling
is `iroha contract derive-address --network-id <NETWORK_ID> ...`.

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

Transactions and trigger actions may use `Executable::Batch` to interleave
native ISIs with by-reference `ContractCall` items. The batch is flat and
non-empty. Items execute sequentially against one live transaction view, so a
contract call sees earlier native changes and later native ISIs see the call's
effects. Admission schedules the whole batch as one conservative global
live-state barrier; a failed item, authorization check, or gas check rolls back
the entire batch. Transaction batches containing calls bind one gas limit in
`fee_payment`; all explicit ISI gas and contract-call gas consume that shared
limit, and fees settle once for the transaction. A trigger invocation likewise
executes its complete batch atomically and consumes one shared deterministic
trigger gas budget rather than resetting the cap for each contract-call item.

## Torii endpoints (feature `app_api`)

- Torii does not expose server-side deployment or deployment-receipt routes.
  Clients verify artifacts, sign manifests, and submit native deployment
  instructions locally through the standard transaction pipeline.
- The local submission result uses `DeployContractBundleReceiptDto`, with one
  exact per-contract result in `contracts[]`; it is not a flattened
  single-contract server response.
- `CommitContractDeployment` atomically validates the expected authority nonce,
  derived address, registered artifact, and previous alias target before
  activation and alias rotation. The reserved nonce cannot be written through
  generic account metadata instructions.
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

The artifact-read endpoints are content-addressed reads. Deployment admission,
fees, permissions, routing, and governance are enforced on the locally signed
native transactions rather than by a separate Torii deployment limiter.

## Governance integration & protected namespaces

- Set the custom parameter `gov_protected_namespaces` (JSON array of namespace
  strings) to enable admission gating. Torii exposes helpers under
  `/v1/gov/protected-namespaces` and the CLI mirrors them via
  `iroha_cli app gov protected set` / `iroha_cli app gov protected get`.
- `CanRegisterSmartContractCode` authorizes artifact upload, manifest
  registration, and unreferenced bytecode removal. It does not authorize an
  address lifecycle takeover. `ActivateContractInstance` and
  `DeactivateContractInstance` require the current account owner and the exact
  lifecycle `expected_revision`; raw activation cannot create an address.
- Direct `CommitContractDeployment` creates a revisioned lifecycle owned by its
  submitting account. It rejects every protected namespace, even when the
  submitter holds governance permissions. Protected addresses are created only
  by the certified Parliament deployment effect, which records the proposer as
  provenance and assigns Parliament ownership.
- Account owners may use revision-guarded `SetContractParliamentDelegation`,
  `OfferContractOwnership`, `AcceptContractOwnership`, and
  `CancelContractOwnershipOffer`. Acceptance is a separate transaction, clears
  Parliament delegation, and advances the revision. Certified delegated
  Parliament may activate or deactivate but cannot transfer ownership or alter
  delegation.
- Proposals created with `ProposeDeployContract` (or the Torii
  `/v1/gov/proposals/deploy-contract` endpoint) capture
  `(contract_address, code_hash, abi_hash, abi_version, manifest_provenance)` as
  one typed immutable proposal fingerprint.
- Parliament constructs the certificate automatically after the final required
  body result and Core executes the bound deployment effect at the exact due
  height. Only an `Enacted` exact-match proposal admits the protected deployment;
  clients cannot submit a finalization or enactment instruction.
- Transactions must include `gov_contract_address=<contract-address>`. CLI
  helpers populate the governance metadata automatically when you pass
  `--contract-address` or `--contract-alias`.
- If the lane manifest sets a validator quorum above one, include
  `gov_manifest_approvers` (JSON array of validator account IDs) so the queue can count
  the additional approvals alongside the transaction authority. Lanes also reject
  metadata that references namespaces not present in the manifest's
  `protected_namespaces` set.

## CLI helpers

- `ivm_contract_deploy` uses the same native plan in blocking and emit modes.
  Transactions 1 through N-1 each carry one chunk; the final registration
  transaction carries chunk N plus finalization. Manifest registration and
  the atomic deployment commit remain separate transactions. Emit mode names
  files
  `register-bytes-chunk-NNNN-of-NNNN`, `register-bytes-finalize`,
  `register-manifest`, and `commit-deployment` in submission order. Its JSON
  reports
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
- Governance helpers (`iroha_cli app gov deploy propose`,
  `iroha_cli app gov deploy audit`, and `iroha_cli app gov protected set/get`)
  orchestrate the protected-namespace workflow and expose JSON artefacts for
  auditing. Core owns certificate execution; there is no CLI enactment helper.

## Testing & coverage

- Unit tests under `crates/iroha_core/tests/contract_code_bytes.rs` cover shape,
  quota, authorization, out-of-order and duplicate handling, missing/corrupt
  chunks, hash/artifact failure retention, direct-registration races,
  cancellation, event emission, cleanup, and cap enforcement. State tests
  cover snapshot and tiered restoration of partial uploads.
- Focused Parliament due-certificate tests validate automatic manifest insertion,
  and `crates/iroha_core/tests/gov_protected_gate.rs` exercises protected-namespace
  admission end-to-end.
- Kura has a two-lane interrupted-`FilesApplied` restart regression for exact
  path rollback and chain preservation. CLI tests cover multi-MiB bounded
  transactions, one-chunk and skip behavior, stable metadata, and ordered emit
  files. Torii tests cover bounded plans, bootstrap, committed progress,
  rejection, timeout retention, registered-code skips, and required receipt
  fields.

Refer to `specs/governance_api.md` for detailed referendum payloads and
ballot workflows.
