---
lang: ru
direction: ltr
source: docs/source/contract_deployment.md
status: needs-translation
source_hash: ac63aad38fa823c9aa84b7f4a9832e216aad0cc1bb204fac38d08397294428d9
source_last_modified: "2026-07-11T04:33:26.599199+00:00"
translation_last_reviewed: null
---

> Translation status note (2026-07-12): the canonical English source has changed. This stale English-language mirror is retained only as localization input and must not be treated as synchronized.

Status: implemented and exercised by Torii, CLI, and core admission tests (May 2026).

## Overview

- Deploy compiled IVM bytecode (`.to`) by submitting it to Torii or by issuing
  `RegisterSmartContractCode`/`RegisterSmartContractBytes` instructions
  directly.
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

## Torii endpoints (feature `app_api`)

- Torii does not expose server-side deployment or deployment-receipt routes.
  Clients verify artifacts, sign manifests, and submit native deployment
  instructions locally through the standard transaction pipeline.
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
- **Canonical V1 security note:** Every raw bytecode registration, manifest
  registration, activation, deactivation, and bytecode removal requires
  `CanRegisterSmartContractCode`. Protected namespaces additionally require
  `CanEnactGovernance`; an empty protection list is never permissionless.
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

- Unit tests under `crates/iroha_core/tests/contract_code_bytes.rs` cover code
  storage, idempotency, and the size cap.
- `crates/iroha_core/tests/gov_enact_deploy.rs` validates manifest insertion via
  enactment, and `crates/iroha_core/tests/gov_protected_gate.rs` exercises
  protected-namespace admission end-to-end.
- Torii routes include request/response unit tests, and the CLI commands have
  integration tests ensuring JSON round-trips remain stable.

Refer to `docs/source/governance_api.md` for detailed referendum payloads and
ballot workflows.
