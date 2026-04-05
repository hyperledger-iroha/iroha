---
lang: es
direction: ltr
source: docs/source/contract_deployment.md
status: needs-update
generator: scripts/sync_docs_i18n.py
source_hash: d3aefb7ebdeb3b9b2d41a3a04ca6c7007e2eb6ac9580a85b7fbdeb9392375660
source_last_modified: "2026-04-04T12:14:12.575161+00:00"
translation_last_reviewed: 2026-04-05
---

> Translation sync note (2026-04-05): this locale temporarily mirrors the updated English canonical text so the self-describing contract artifact and deploy API docs stay accurate while a refreshed translation is pending.

Status: implemented and exercised by Torii, CLI, and core admission tests (Nov 2025).

## Overview

- Deploy compiled IVM bytecode (`.to`) by submitting it to Torii or by issuing
  `RegisterSmartContractCode`/`RegisterSmartContractBytes` instructions
  directly.
- Contract `.to` artifacts are self-describing: the required `CNTR` section
  embeds the contract interface ahead of the executable stream, and Torii
  derives the on-chain `ContractManifest` from that section after verification.
- Nodes recompute `code_hash` and the canonical ABI hash locally; mismatches
  reject deterministically.
- Stored artifacts live under the on-chain `contract_manifests` and
  `contract_code` registries. Manifests reference hashes only and remain small;
  code bytes are keyed by `code_hash`.
- Protected namespaces can require an enacted governance proposal before a
  deployment is admitted. The admission path looks up the proposal payload and
  enforces `(contract_address, code_hash, abi_hash)` equality when the
  namespace is protected.

## Stored Artifacts & Retention

- `RegisterSmartContractCode` inserts/overwrites the manifest for a given
  `code_hash`. When the same hash already exists, it is replaced with the new
  manifest.
- `RegisterSmartContractBytes` stores the compiled program under
  `contract_code[code_hash]`. If bytes for a hash already exist they must match
  exactly; differing bytes raise an invariant violation.
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
  the submitting key, and then stored together with the uploaded bytecode.
- Transactions targeting protected namespaces must include metadata key
  `gov_contract_address`. The admission path compares the derived dataspace and
  address against enacted `DeployContract` proposals; if no matching proposal
  exists the transaction is rejected with `NotPermitted`.

## Torii endpoints (feature `app_api`)

- `POST /v1/contracts/deploy`
  - Request body: `DeployContractDto` (see `docs/source/torii_contracts_api.md` for field details).
  - Torii decodes the base64 payload, verifies the embedded `CNTR` interface,
    derives the manifest from the artifact itself, allocates a fresh immutable
    `contract_address`, binds the requested stable `contract_alias` to that
    address, prepends a domainless self-registration for the authority, and
    submits the resulting transaction on behalf of the caller.
  - Redeploying the same `contract_alias` performs an in-place upgrade:
    Torii deploys a new address, rebinds the alias atomically, and deactivates
    the previous address.
  - Response: `{ ok, contract_alias, contract_address, previous_contract_address?, upgraded, dataspace, deploy_nonce, tx_hash_hex, code_hash_hex, abi_hash_hex }`.
  - Errors: invalid base64, invalid contract artifact, size cap exceeded,
    governance gating for protected namespaces, or fee/balance failures.
- `GET /v1/contracts/code/{code_hash}`
  - Returns `{ manifest: { code_hash, abi_hash } }`.
    Additional manifest fields are preserved internally but omitted here for a
    stable API.
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
- Unprotected namespaces are public: any signer may register code bytes,
  register manifests, and deploy alias-backed public contracts there.
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

- `iroha_cli app contracts deploy --authority <id> --private-key <hex> --code-file <path> --contract-alias <name::dataspace>`
  submits the alias-first Torii deploy request (computing hashes on the fly).
- `iroha_cli app contracts manifest build --code-file <path> [--sign-with <hex>]` computes
  `code_hash`/`abi_hash` for compiled `.to`, derives the manifest from the
  embedded `CNTR`, and optionally signs it for inspection, printing JSON or
  writing to `--out`.
- `iroha_cli app contracts simulate --authority <id> --private-key <hex> --code-file <path> --gas-limit <u64>`
  runs an offline VM pass and reports ABI/hash metadata plus the queued ISIs
  (counts and instruction ids) without touching the network.
- `iroha_cli app contracts manifest get --code-hash <hex>` fetches the manifest via Torii
  and optionally writes it to disk.
- `iroha_cli app contracts code get --code-hash <hex> --out <path>` downloads
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
