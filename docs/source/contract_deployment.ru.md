---
lang: ru
direction: ltr
source: docs/source/contract_deployment.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 0f2b1d7d027d715eac5a3ca8be29dea8f0e76013e948947a4de66108ac561f34
source_last_modified: "2026-01-22T15:38:30.659845+00:00"
translation_last_reviewed: 2026-01-30
---

---
title: Contract Deployment (.to) — API & Workflow
---

Status: implemented and exercised by Torii, CLI, and core admission tests (Nov 2025).

## Overview

- Deploy compiled IVM bytecode (`.to`) by submitting it to Torii or by issuing
  `RegisterSmartContractCode`/`RegisterSmartContractBytes` instructions
  directly.
- Nodes recompute `code_hash` and the canonical ABI hash locally; mismatches
  reject deterministically.
- Stored artifacts live under the on-chain `contract_manifests` and
  `contract_code` registries. Manifests reference hashes only and remain small;
  code bytes are keyed by `code_hash`.
- Protected namespaces can require an enacted governance proposal before a
  deployment is admitted. The admission path looks up the proposal payload and
  enforces `(namespace, contract_id, code_hash, abi_hash)` equality when the
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

- The validator parses the IVM header, enforces `version_major == 1`, and checks
  `abi_version == 1`. Unknown versions reject immediately; there is no runtime
  toggle.
- When a manifest is already present for `code_hash`, validation ensures the
  stored `code_hash`/`abi_hash` equal the computed values from the submitted
  program. A mismatch produces `Manifest{Code,Abi}HashMismatch` errors.
- Transactions targeting protected namespaces must include metadata keys
  `gov_namespace` and `gov_contract_id`. The admission path compares them
  against enacted `DeployContract` proposals; if no matching proposal exists the
  transaction is rejected with `NotPermitted`.

## Torii endpoints (feature `app_api`)

- `POST /v2/contracts/deploy`
  - Request body: `DeployContractDto` (see `docs/source/torii_contracts_api.md` for field details).
  - Torii decodes the base64 payload, computes both hashes, builds a manifest,
    and submits `RegisterSmartContractCode` plus
    `RegisterSmartContractBytes` in a signed transaction on behalf of the
    caller.
  - Response: `{ ok, code_hash_hex, abi_hash_hex }`.
  - Errors: invalid base64, unsupported ABI version, missing permission
    (`CanRegisterSmartContractCode`), size cap exceeded, governance gating.
- `POST /v2/contracts/code`
  - Accepts `RegisterContractCodeDto` (authority, private key, manifest) and submits only
    `RegisterSmartContractCode`. Use when manifests are staged separately from
    bytecode.
- `POST /v2/contracts/instance`
  - Accepts `DeployAndActivateInstanceDto` (authority, private key, namespace/contract_id, `code_b64`, optional manifest overrides) and deploys + activates atomically.
- `POST /v2/contracts/instance/activate`
  - Accepts `ActivateInstanceDto` (authority, private key, namespace, contract_id, `code_hash`) and submits only the activation instruction.
- `GET /v2/contracts/code/{code_hash}`
  - Returns `{ manifest: { code_hash, abi_hash } }`.
    Additional manifest fields are preserved internally but omitted here for a
    stable API.
- `GET /v2/contracts/code-bytes/{code_hash}`
  - Returns `{ code_b64 }` with the stored `.to` image encoded as base64.

All contract lifecycle endpoints share a dedicated deploy limiter configured via
`torii.deploy_rate_per_origin_per_sec` (tokens per second) and
`torii.deploy_burst_per_origin` (burst tokens). Defaults are 4 req/s with a burst of
8 for each token/key derived from `X-API-Token`, the remote IP, or the endpoint hint.
Set either field to `null` to disable the limiter for trusted operators. When the
limiter fires, Torii increments the
`torii_contract_throttled_total{endpoint="code|deploy|instance|activate"}` telemetry counter and
returns HTTP 429; any handler error increments
`torii_contract_errors_total{endpoint=…}` for alerting.

## Governance integration & protected namespaces

- Set the custom parameter `gov_protected_namespaces` (JSON array of namespace
  strings) to enable admission gating. Torii exposes helpers under
  `/v2/gov/protected-namespaces` and the CLI mirrors them via
  `iroha_cli app gov protected set` / `iroha_cli app gov protected get`.
- Proposals created with `ProposeDeployContract` (or the Torii
  `/v2/gov/proposals/deploy-contract` endpoint) capture
  `(namespace, contract_id, code_hash, abi_hash, abi_version)`.
- Once the referendum passes, `EnactReferendum` marks the proposal Enacted and
  admission will accept deployments that carry matching metadata and code.
- Transactions must include the metadata pair `gov_namespace=a namespace` and
  `gov_contract_id=an identifier` (and should set `contract_namespace` /
  `contract_id` for call-time binding). CLI helpers populate these
  automatically when you pass `--namespace`/`--contract-id`.
- When protected namespaces are enabled, queue admission rejects attempts to
  rebind an existing `contract_id` to a different namespace; use the enacted
  proposal or retire the previous binding before deploying elsewhere.
- If the lane manifest sets a validator quorum above one, include
  `gov_manifest_approvers` (JSON array of validator account IDs) so the queue can count
  the additional approvals alongside the transaction authority. Lanes also reject
  metadata that references namespaces not present in the manifest's
  `protected_namespaces` set.

## CLI helpers

- `iroha_cli app contracts deploy --authority <id> --private-key <hex> --code-file <path>`
  submits the Torii deploy request (computing hashes on the fly).
- `iroha_cli app contracts deploy-activate --authority <id> --private-key <hex> --namespace <ns> --contract-id <id> --code-file <path>`
  builds the manifest (signed with the supplied key), registers bytes + manifest,
  and activates the `(namespace, contract_id)` binding in one transaction. Use
  `--dry-run` to print the computed hashes and instruction count without
  submitting, and `--manifest-out` to save the signed manifest JSON.
- `iroha_cli app contracts manifest build --code-file <path> [--sign-with <hex>]` computes
  `code_hash`/`abi_hash` for compiled `.to` and optionally signs the manifest,
  printing JSON or writing to `--out`.
- `iroha_cli app contracts simulate --authority <id> --private-key <hex> --code-file <path> --gas-limit <u64>`
  runs an offline VM pass and reports ABI/hash metadata plus the queued ISIs
  (counts and instruction ids) without touching the network. Attach
  `--namespace/--contract-id` to mirror call-time metadata.
- `iroha_cli app contracts manifest get --code-hash <hex>` fetches the manifest via Torii
  and optionally writes it to disk.
- `iroha_cli app contracts code get --code-hash <hex> --out <path>` downloads
  the stored `.to` image.
- `iroha_cli app contracts instances --namespace <ns> [--table]` lists activated
  contract instances (manifest + metadata driven).
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
