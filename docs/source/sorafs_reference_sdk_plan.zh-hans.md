---
lang: zh-hans
direction: ltr
source: docs/source/sorafs_reference_sdk_plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 7dd10204c16e6c1da45d66d6223fcdf7db71ea2ee8835511827ca2230809c28a
source_last_modified: "2026-06-25T17:49:59+00:00"
translation_last_reviewed: 2026-06-25
title: SoraFS Reference SDK & Validator
summary: SF-11 implementation status for reference validators, CLI and FFI surfaces, release packaging, and remaining live-release evidence.
---

# SoraFS Reference SDK & Validator

## Goals & Scope
- Ship a Rust reference crate and CLI that sign, verify, and enforce SoraFS governance policies for adverts, replication orders, PoR/PDP/PoTR artefacts, repairs, and governance envelopes.
- Provide deterministic machine-readable outcomes so operators, auditors, and CI systems can gate deployments without bespoke tooling.
- Expose bindings and artefacts other language teams can reuse (via FFI or generated code) while keeping Norito as the only canonical wire format.

## Status
The local SF-11 reference validator and SDK foundations are implemented for
provider adverts, admission envelopes, admission renewals/revocations,
replication orders, signed replication orders, orderbook payloads, PoR
challenge/proof pairs, PDP commitments/challenges/proofs, PoTR receipts, repair payloads, fixture bundles,
governance log nodes, governance DAG blocks and signed-head chains, runtime
signing helpers, C FFI validation, cookbook fixtures, and manifest/CAR replay.
Remaining SF-11 work is release evidence and SDK distribution: per-target
published archives, signed release manifests, published downstream binding
packages, and live operator smoke records.
`scripts/check_sorafs_reference_sdk_release_evidence.py` now provides the
fail-closed SF-11 release evidence gate for those artifacts, including
cross-artifact `release_manifest_digest_hex` binding from release archives,
downstream packages, cookbook smoke, FFI/header contract, and governance
approval evidence back to a valid signed manifest in the same bundle, and
`scripts/run_sorafs_reference_sdk_release_evidence.py` provides the reviewed
collection planner/runner. The JavaScript SDK already exposes the Rust-backed
orderbook and PDP reference validators from both the package root and
`@iroha/iroha-js/sorafs`; the Python SDK exposes the same orderbook and PDP
outcome contract from `iroha_python.sorafs` and the package root.
Kotlin/JVM, Java Android, and Swift now expose matching source-level wrappers
through the shared `connect_norito_bridge` native facade. JavaScript, Python,
Kotlin/JVM, Java Android, and Swift also expose the encoded orderbook
order/cancel/settlement-receipt signing helper for callers that already have
Norito payload bytes. Rust, JavaScript, Python, Kotlin/JVM, Java Android, and
Swift now also expose field-level orderbook order/cancel/settlement-receipt
builders that construct, sign, validate, and encode canonical Norito payload
bytes from SDK field values.

The existing `sorafs_manifest` crate exposes `ValidationOutcomeV1`,
`validate_provider_advert_bytes`, `validate_provider_admission_envelope_bytes`,
`validate_replication_order_bytes`,
`validate_signed_replication_order_bytes`, `validate_orderbook_payload_bytes`,
`validate_por_challenge_proof_bytes`,
`validate_pdp_commitment_challenge_proof_bytes`, `validate_potr_receipt_bytes`,
`validate_repair_payload_bytes`, and the `sorafs_car` crate exposes
`validate_manifest_car_replay` and
`validate_manifest_car_replay_bytes`. The `sorafs-validate advert` /
`sorafs-validate admission` / `sorafs-validate order` /
`sorafs-validate orderbook` / `sorafs-validate por` /
`sorafs-validate pdp` / `sorafs-validate potr` /
`sorafs-validate repair` / `sorafs-validate bundle` /
`sorafs-validate governance` CLI commands cover Norito `ProviderAdvertV1`,
`ProviderAdmissionEnvelopeV1`, `ReplicationOrderV1`, `PorChallengeV1`,
`PorProofV1`, `PdpCommitmentV1`, `PdpChallengeV1`, `PdpProofV1`,
`PotrReceiptV1`, orderbook/settlement payloads, and repair payloads,
fixture-directory bundle cross-link checks, governance log node
validation, governance DAG block validation, signed governance DAG head-chain
validation, provider advert signing, signed replication-order envelopes, and
governance log node Ed25519 signing.
`soranet_trustless_verifier --validation-outcome` emits the same
`ValidationOutcomeV1` contract for manifest/CAR replay.

## Architecture Overview
| Component | Purpose | Notes |
|-----------|---------|-------|
| `crates/sorafs_manifest::reference` | Core library module providing validation/signing helpers, policy enforcement, and error outcomes. | Reuses canonical `sorafs_manifest` payload modules and `sorafs_car` replay helpers; no duplicate codecs. |
| `sorafs-validate` (binary) | CLI wrapping the reference validators with task-focused subcommands and consistent output. | The current `sorafs_manifest` slice is dependency-free and uses `norito::json`; the future full wrapper may use `clap` without adding direct `serde_json`. |
| `reference_ffi` helpers | C ABI surface for SDKs (Go/Swift/Node) built on top of the Rust validators. | Implemented: returns `ValidationOutcomeV1` Norito JSON buffers plus explicit free function; `crates/sorafs_manifest/include/sorafs_reference.h` is the checked C header for downstream bindings. `connect_norito_bridge` also exposes mobile SDK orderbook signing and field-level builder entry points. |
| `docs/examples/sorafs_reference_sdk/` | Runnable cookbook with ready-to-run CLI and SDK smoke scenarios plus committed sample payloads. | Mirrors committed fixtures and exercises validator, signing, bundle, and manifest/CAR replay paths. |

Internal modules (library):
- `validator::advert`, `validator::admission`, `validator::order`, `validator::por`, `validator::potr`, `validator::repair`, `validator::governance`.
- `policy` module encapsulates default thresholds (TTLs, SLAs, retry budgets).
- `outcome` module implements `ValidationOutcomeV1` (see below).
- Signing helpers expose implemented builders for adverts, signed
  replication-order envelopes, SFM-2 orderbook order/cancel/receipt payloads,
  and governance log nodes; signed replication orders use the
  `sorafs.replication_order.signature.v1` domain string and canonical Norito
  order-envelope bytes.
- The orderbook encoded-payload signing helper is available as
  `sorafs_manifest::sign_orderbook_payload_bytes_ed25519_v1(...)` and through
  JavaScript `signOrderbookPayload(...)`, Python
  `sign_orderbook_payload(...)`, Kotlin/JVM
  `SorafsReferenceValidators.signOrderbookPayload(...)`, Java Android
  `SorafsReferenceValidators.signOrderbookPayload(...)`, and Swift
  `SorafsReferenceValidators.signOrderbookPayload(...)` for callers that
  already have Norito `OrderRequestV1`, `OrderCancelV1`, or
  `SettlementReceiptV1` bytes.
- Field-level orderbook builders are available in Rust as
  `build_signed_orderbook_order_request_bytes_ed25519_v1(...)`,
  `build_signed_orderbook_order_cancel_bytes_ed25519_v1(...)`, and
  `build_signed_orderbook_settlement_receipt_bytes_ed25519_v1(...)`, in
  JavaScript as `buildSignedOrderbookOrderRequest(...)`,
  `buildSignedOrderbookOrderCancel(...)`, and
  `buildSignedOrderbookSettlementReceipt(...)`, and in Python as
  `build_signed_orderbook_order_request(...)`,
  `build_signed_orderbook_order_cancel(...)`, and
  `build_signed_orderbook_settlement_receipt(...)`. Kotlin/JVM, Java Android,
  and Swift expose the same field-level builders through the shared
  `connect_norito_bridge` native facade.

## CLI Surface
All commands accept `--format {table,json,yaml}` (default `table`) and `--telemetry-out <path>` to write the raw `ValidationOutcomeV1`.

Current implementation slice: `cargo run -p sorafs_manifest --bin sorafs-validate -- advert --input <advert.to> --format json` validates Norito `ProviderAdvertV1` payloads, including canonical body validation and Ed25519 signature verification. `cargo run -p sorafs_manifest --bin sorafs-validate -- admission --input <envelope.to> --format json` validates Norito `ProviderAdmissionEnvelopeV1` payloads, including structural policy, digest binding, and council signature verification; `cargo run -p sorafs_manifest --bin sorafs-validate -- admission --envelope <envelope.to> --renewal <renewal.to> --format json` validates governed renewal payloads against the previous envelope, and `--revocation <revocation.to>` validates revocation digests and council signatures against the governed envelope. `cargo run -p sorafs_manifest --bin sorafs-validate -- order --order <order.to> --format json` validates Norito `ReplicationOrderV1` payloads against the canonical order schema, chunker registry handle rules, provider assignments, and SLA/deadline policy; `cargo run -p sorafs_manifest --bin sorafs-validate -- order --signed-order <signed-order.to> --format json` validates `SignedReplicationOrderV1` envelopes and verifies Ed25519 signatures over the domain-separated canonical order signing bytes. `cargo run -p sorafs_manifest --bin sorafs-validate -- orderbook --kind settlement-receipt --input <receipt.to> --format json` validates Norito orderbook and streaming-settlement payloads, including structural constraints, fee/timestamp policy, signature material, byte ranges, and settlement accounting. `cargo run -p sorafs_manifest --bin sorafs-validate -- por --challenge <challenge.to> --proof <proof.to> --format json` validates Norito `PorChallengeV1` and `PorProofV1` payloads, pair binding, deadline policy, and sample coverage. `cargo run -p sorafs_manifest --bin sorafs-validate -- pdp --commitment <commitment.to> --challenge <challenge.to> --proof <proof.to> --format json` validates Norito `PdpCommitmentV1`, `PdpChallengeV1`, and `PdpProofV1` payloads, commitment/challenge/proof binding, sample windows, coverage, tree roots, and signature material. `cargo run -p sorafs_manifest --bin sorafs-validate -- potr --receipt <receipt.to> --profile hot --format json` validates Norito `PotrReceiptV1` payloads, latency/deadline consistency, optional tier profile, range bounds, timestamps, and detached signatures. `cargo run -p sorafs_manifest --bin sorafs-validate -- repair --task <repair-task.to> --format json` validates Norito repair task records, evidence, reports, slash proposals, escalation policy/approval payloads, task events, worker action payloads, audit events, and signed auditor request signatures. `cargo run -p sorafs_manifest --bin sorafs-validate -- bundle --bundle fixtures/sorafs_manifest --now 120 --format json` validates known fixture-directory artifacts, validates discovered orderbook and PDP payload fixtures, checks PoR challenge/proof binding, checks PDP commitment/challenge/proof binding, enforces shared manifest digests for order/proof/receipt/repair artifacts, verifies provider-admission provider consistency, and checks manifest-bearing providers against replication-order assignments. `cargo run -p sorafs_manifest --bin sorafs-validate -- governance --node fixtures/sorafs_manifest/governance/node_v1.to --cid bafygovernancelognode --format json` validates `GovernanceLogNodeV1` payload shape, embedded payload policy, publisher metadata, signature material, Ed25519 and Dilithium3/ML-DSA publisher signature verification, and optional node-CID binding. `cargo run -p sorafs_manifest --bin sorafs-validate -- governance --block <block.to> --cid hex:<block-cid-hex> --format json` validates a Norito `GovernanceDagBlockV1`, recomputes its canonical block CID, checks embedded node policy and signature material, and verifies the block publisher signature. `cargo run -p sorafs_manifest --bin sorafs-validate -- governance --head <head.to> --block <block-0.to> --block <block-1.to> --format json` validates a signed `GovernanceDagHeadV1` against a parent-linked block chain, including head signature, expected head CID, chain topology, and block-count binding. `cargo run -p sorafs_manifest --bin sorafs-validate -- sign --kind advert --input <advert.to> --out <signed-advert.to> --key <runtime-key-file> --now 120 --format json` signs the canonical advert body with an Ed25519 seed supplied at runtime, `cargo run -p sorafs_manifest --bin sorafs-validate -- sign --kind order --input <order.to> --out <signed-order.to> --key <runtime-key-file> --format json` signs the domain-separated canonical order envelope, `cargo run -p sorafs_manifest --bin sorafs-validate -- sign --kind orderbook --payload-kind order-request --input <orderbook-order.to> --out <signed-orderbook-order.to> --key <runtime-key-file> --format json` signs SFM-2 orderbook order, cancel, or settlement-receipt payloads, and `cargo run -p sorafs_manifest --bin sorafs-validate -- sign --kind governance --input <node.to> --out <signed-node.to> --key <runtime-key-file> --format json` signs the canonical governance node payload. `cargo run -p sorafs_car --features cli --bin soranet_trustless_verifier -- --manifest <manifest.to> --car <payload.car> --validation-outcome --generated-at <unix-seconds>` replays `ManifestV1` policy, CARv2 roots, CAR digest/size, content length, chunk plan, payload digest, and PoR root into `ValidationOutcomeV1`. These sign and validation commands write or print output only after validation succeeds where applicable and return code `0` for success or `2` for validation/policy/signature/Norito payload errors.

| Command | Description | Key flags | Input |
|---------|-------------|-----------|-------|
| `sorafs-validate advert` | Validate `ProviderAdvertV1` payloads (signature, TTL, capability set). | Implemented: `--input <file>`, `--format table\|json\|yaml`, `--telemetry-out <path>`, `--now <unix-seconds>`. Governed policy overrides belong to a signed release-wrapper policy document; the current local validator uses deterministic defaults. | Norito bytes. |
| `sorafs-validate admission` | Verify `ProviderAdmissionEnvelopeV1` onboarding, renewal, and revocation payloads (schema, digest bindings, retention epoch, council signatures). | Implemented: `--input <file>` (or `--envelope <file>` alias), optional `--renewal <file>` or `--revocation <file>`, `--format table\|json\|yaml`, `--telemetry-out <path>`. External governance keyset selection remains a signed deployment-policy concern; the local validator verifies the key material encoded in the governed envelope. | Norito bytes. |
| `sorafs-validate order` | Check `ReplicationOrderV1` payloads (schema, manifest digest presence, chunk profile, provider assignments, SLA/deadline policy). | Implemented: `--order <file>` (or `--input <file>` alias) for bare orders, `--signed-order <file>` for `SignedReplicationOrderV1` envelopes, `--format table\|json\|yaml`, `--telemetry-out <path>`. Manifest/CAR replay is handled by `soranet_trustless_verifier --validation-outcome` to keep CAR parsing in `sorafs_car`. | Norito bytes. |
| `sorafs-validate orderbook` | Validate orderbook and streaming-settlement payloads (`OrderRequestV1`, `OrderCancelV1`, `TradeEventV1`, `SettlementChannelV1`, `SettlementReceiptV1`). | Implemented: `--kind <payload-kind> --input <file>` or aliases `--order <file>`, `--cancel <file>`, `--trade <file>`, `--channel <file>`, `--receipt <file>`, `--format table\|json\|yaml`, `--telemetry-out <path>`. Pure Rust helpers now cover pair matching, deterministic full-book snapshot matching, fee/escrow calculation, settlement-channel opening, and receipt application; runtime matcher service wiring, durable escrow mutation, and signature authorization remain SFM-2 rollout work. | Norito bytes. |
| `sorafs-validate por` | Validate `PorChallengeV1` and `PorProofV1` pairs (typed payloads, challenge/manifest/provider binding, deadline, sample coverage). | Implemented: `--challenge <file>`, `--proof <file>`, `--format table\|json\|yaml`, `--telemetry-out <path>`. Manifest/CAR replay is implemented by `soranet_trustless_verifier --validation-outcome`; governed epoch overrides belong to signed release-wrapper policy. | Norito bytes. |
| `sorafs-validate pdp` | Validate `PdpCommitmentV1`, `PdpChallengeV1`, and `PdpProofV1` payloads plus commitment/challenge/proof binding. | Implemented: `--commitment <file>`, `--challenge <file>`, `--proof <file>`, with pair or single-payload validation accepted where useful, `--format table\|json\|yaml`, `--telemetry-out <path>`. Bundle validation discovers committed PDP fixtures and enforces manifest/provider/sample binding. | Norito bytes. |
| `sorafs-validate potr` | Validate `PotrReceiptV1` receipts (deadline/latency consistency, tier profile, range bounds, timestamps, signatures). | Implemented: `--receipt <file>`, optional `--profile hot\|warm\|archive\|cold`, `--format table\|json\|yaml`, `--telemetry-out <path>`. Committed PoTR receipt fixtures are covered by bundle validation; live probe-bundle and orchestrator metric cross-checks are rollout evidence. | Norito bytes. |
| `sorafs-validate repair` | Validate repair payloads (`RepairEvidenceV1`, `RepairReportV1`, `RepairTaskRecordV1`, slash proposals, escalation policy/approval, signed auditor requests, worker payloads, task/audit events). | Implemented: `--kind <payload-kind> --input <file>` or aliases such as `--task <file>`, `--evidence <file>`, `--report <file>`, `--signed-auditor-request <file>`, `--worker-signature <file>`, `--event <file>`, `--audit-event <file>`, `--format table\|json\|yaml`, `--telemetry-out <path>`. | Norito bytes. |
| `sorafs-validate governance` | Validate `GovernanceLogNodeV1` payloads, `GovernanceDagBlockV1` blocks, and signed `GovernanceDagHeadV1` chains. | Implemented: `--node <file>` (or `--input <file>` alias) with optional `--cid <node-cid>`; `--block <file>` with optional `--cid <block-cid\|hex:HEX>`; or `--head <file> --block <file> [--block <file>...]`, plus `--format table\|json\|yaml` and `--telemetry-out <path>`. Node validation covers embedded payload policy, publisher metadata, Ed25519 and Dilithium3/ML-DSA publisher signatures, and optional node-CID binding. Block/head validation covers canonical block-CID derivation, embedded node policy, block signatures, parent linkage, signed head binding, and block-count binding. | Norito bytes. |
| `sorafs-validate bundle` | Run a composite check on a fixture bundle (admission artifacts plus order/proofs/receipts/repair payloads and orderbook fixtures). | Implemented: `--bundle <dir>`, `--format table\|json\|yaml`, `--telemetry-out <path>`, `--now <unix-seconds>`. Manifest/CAR policy replay is implemented by `soranet_trustless_verifier --validation-outcome`. | Directory matching fixture layout. |
| `soranet_trustless_verifier --validation-outcome` | Replay `ManifestV1` policy and a full CARv2 stream into the reference outcome contract. | Implemented: `--manifest <manifest.to>`, `--car <payload.car>`, optional `--config <toml>`, `--json-out <path>`, `--quiet`, `--generated-at <unix-seconds>`. | Manifest Norito or JSON plus CAR bytes. |
| `sorafs-validate sign` | Produce signed reference payloads using operator or governance keys. | Implemented: `--kind advert --input <advert.to> --out <signed-advert.to> (--key-hex <hex> \| --key <path>)`, `--kind order --input <order.to> --out <signed-order.to> (--key-hex <hex> \| --key <path>)`, `--kind orderbook --payload-kind order-request\|order-cancel\|settlement-receipt --input <payload.to> --out <signed-payload.to> (--key-hex <hex> \| --key <path>)`, `--kind governance --input <node.to> --out <signed-node.to> (--key-hex <hex> \| --key <path>)`, `--format table\|json\|yaml`, `--telemetry-out <path>`, `--now <unix-seconds>` for adverts. | Norito bytes -> Norito bytes. |

Exit codes: `0` success, `2` validation/policy/signature errors, `3` I/O errors, `4` configuration errors, `10` internal faults.

## FFI Surface
`crates/sorafs_manifest::reference_ffi` exposes a C ABI for SDK bindings that
need the Rust reference validators without linking Rust-native APIs. The
`connect_norito_bridge` facade re-exports the orderbook and PDP validator
surface for Kotlin/JVM, Java Android, and Swift consumers that already load the
shared mobile bridge. Each
validator returns a `SorafsReferenceFfiBuffer` containing `ValidationOutcomeV1`
rendered with Norito JSON; callers must release it with
`sorafs_reference_free_buffer`. The public C binding contract lives at
`crates/sorafs_manifest/include/sorafs_reference.h`; the
`ci/check_sorafs_reference_ffi_header.sh` guard compares that header with
`reference_ffi.rs`, verifies selector values, and syntax-checks the header as C
and C++ when local compilers are available.

Implemented functions cover provider adverts, admission envelopes, admission
renewals, admission revocations, replication orders, signed replication-order
envelopes, orderbook payloads, PoR challenge/proof pairs, PDP
commitment/challenge/proof payloads, PoTR receipts, repair payloads,
governance log nodes, and fixture bundle payload arrays:

- `sorafs_reference_validate_provider_advert_json`
- `sorafs_reference_validate_provider_admission_json`
- `sorafs_reference_validate_provider_admission_renewal_json`
- `sorafs_reference_validate_provider_admission_revocation_json`
- `sorafs_reference_validate_replication_order_json`
- `sorafs_reference_validate_signed_replication_order_json`
- `sorafs_reference_validate_orderbook_json`
- `sorafs_reference_validate_por_json`
- `sorafs_reference_validate_pdp_commitment_json`
- `sorafs_reference_validate_pdp_challenge_json`
- `sorafs_reference_validate_pdp_proof_json`
- `sorafs_reference_validate_pdp_commitment_challenge_json`
- `sorafs_reference_validate_pdp_challenge_proof_json`
- `sorafs_reference_validate_pdp_json`
- `sorafs_reference_validate_potr_json`
- `sorafs_reference_validate_repair_json`
- `sorafs_reference_validate_governance_json`
- `sorafs_reference_validate_bundle_json`

FFI selectors are exported as constants for repair payload kinds, bundle payload
kinds including orderbook and PDP bundle members, orderbook payload kinds, and
PoTR profiles. Invalid selectors or null pointers paired with non-zero lengths
return `SFS-FFI-001` instead of unwinding across the ABI. JavaScript package
wrappers for orderbook and PDP validation are exposed through the native
`iroha_js_host` binding; Python wrappers use `iroha_python_rs`; Kotlin/JVM,
Java Android, and Swift wrappers use `connect_norito_bridge`. Remaining
downstream work is signed release packaging, publication, and live SDK smoke
evidence for those bindings.

## Validation Policies
Default policy constants used by the implemented validators:
- Advert TTL must be non-zero and ≤ 24h; `provider_id` must be non-zero and capability metadata must pass the canonical advert schema.
- Admission envelopes must reference approved senior council keyset (multi-signature) and admission window ≤ 7 days.
- Replication orders validate:
  - Manifest digest presence and order/assignment binding.
  - Order version supported by local `sorafs_manifest`.
  - Pricing schedule conforms to `sorafs_pricing.md` tiers.
- Orderbook payloads validate:
  - Order/cancel/trade/channel/receipt schema versions and non-zero identifiers.
  - Positive price, quantity, escrow, byte-count, debit, timestamp, and nonce
    fields where required.
  - Ed25519 key/signature lengths and settlement accounting where provider
    credit plus fee must equal buyer debit.
- Manifest/CAR replay validates manifest policy, declared CAR digest and size,
  content length, root CID, chunk profile, chunk digests, payload digest, chunk
  plan, and PoR root through `sorafs_car::validate_manifest_car_replay*` and
  `soranet_trustless_verifier --validation-outcome`.
- PoR:
  - Challenge/proof share identical seed, epoch, and manifest digest (via `derive_challenge_seed`).
  - Response deadline must be after issue time, and proof responses must arrive before the encoded deadline; governed deadline changes are release-wrapper policy, not ad hoc CLI flags.
  - Sample coverage meets tier thresholds from SF-9.
- PDP:
  - Commitments, challenges, and proofs share manifest digest, provider ID, epoch, sample window, coverage, and deterministic sample labels.
  - Hot/segment roots, tree heights, proof leaves, and signature material must pass structural checks before pair binding is accepted.
  - PDP bundle validation is fixture-backed and fail-closed until the SF-13 runtime admission path is enabled.
- PoTR:
  - Probe durations respect hot/warm/cold SLAs (90s / 5m / 30m).
  - Receipt signature matches provider key; aggregated proofs cross-check with orchestrator metrics if provided.
- Repair tasks:
  - Evidence matches PoR/PoTR failure digests.
  - Escalation reason required for `escalated` status; `repair_task_id` must exist in history.
- Governance nodes:
  - Node CID can be compared against caller-supplied input.
  - Payload kind resolved; unsupported types flagged.
  - DAG blocks recompute canonical block CID bytes and verify embedded node and
    block publisher signatures.
  - Signed DAG heads verify parent-linked chains, expected head CID, and
    advertised block count before acceptance.

Governed policy override support is intentionally outside the local deterministic validator. Release wrappers that need overrides should load a Norito policy document with an allowlist of fields such as `advert_ttl_min` and `por_deadline_max`, and production pipelines must require a governed signature before applying overrides.

## Error & Outcome Contract
`ValidationOutcomeV1` (Norito + JSON) includes:
```norito
struct ValidationOutcomeV1 {
    status: ValidationStatus, // Ok | Error
    code: String,             // SFS-VAL-001 etc.
    category: ValidationCategory, // validation | policy | signature | io | norito | internal
    message: String,
    action: Option<String>,
    docs_url: Option<String>,
    telemetry_tags: Vec<String>,
    context: ValidationContextV1,
    inputs: Vec<ValidationInputV1>, // file paths / CIDs
    version: u8,                    // outcome schema version
    generated_at: Timestamp,
}
```
`ValidationContextV1` is emitted as structured key/value fields (manifest CID, provider ID, etc.). The outcome is printed by CLI (`table` format compresses key fields).

Error catalogue:
- `SFS-OK-000` validation succeeded.
- `SFS-VAL-001` required digest or identifier invalid or mismatch.
- `SFS-VAL-002` unsupported schema version.
- `SFS-VAL-003` chunk profile incompatibility.
- `SFS-VAL-004` provider advert structure invalid.
- `SFS-VAL-005` replication order structure invalid.
- `SFS-VAL-006` provider admission envelope structure invalid.
- `SFS-VAL-007` provider admission digest binding mismatch.
- `SFS-VAL-008` PoR challenge structure invalid.
- `SFS-VAL-009` PoR proof structure invalid.
- `SFS-VAL-010` PoTR receipt structure invalid.
- `SFS-SIG-001` invalid signature.
- `SFS-SIG-002` provider admission council signature invalid.
- `SFS-SIG-003` PoTR receipt signature invalid.
- `SFS-SIG-004` signed auditor request signature invalid.
- `SFS-SIG-005` governance log signature material invalid or publisher verification failed.
- `SFS-SIG-006` signed replication order or governance DAG block signature invalid.
- `SFS-SIG-007` orderbook or governance DAG head signature material invalid.
- `SFS-SIG-008` PDP proof signature material missing or malformed.
- `SFS-POL-001` advert TTL violation.
- `SFS-POL-002` PoR deadline exceeded.
- `SFS-POL-003` replication order deadline or SLA policy violation.
- `SFS-POL-004` provider admission retention policy violation.
- `SFS-POL-005` repair timestamp or SLA deadline policy violation.
- `SFS-POL-006` manifest pin policy failed during manifest/CAR replay.
- `SFS-POL-007` orderbook timestamp, nonce, or fee policy violation.
- `SFS-CAR-001` CAR stream replay failed against manifest commitments.
- `SFS-CAR-002` trustless manifest/CAR replay metadata derivation failed.
- `SFS-POR-001` missing sample coverage.
- `SFS-POR-002` proof sample digest mismatch.
- `SFS-POR-003` PoR challenge/proof binding mismatch.
- `SFS-PDP-001` PDP sample window or coverage invalid.
- `SFS-PDP-002` PDP commitment, challenge, or proof structure invalid.
- `SFS-PDP-003` PDP commitment/challenge/proof binding mismatch.
- `SFS-POTR-001` deadline proof late.
- `SFS-POTR-002` PoTR receipt tier mismatch.
- `SFS-OBK-001` orderbook payload structure invalid.
- `SFS-OBK-002` orderbook settlement accounting invalid.
- `SFS-REP-001` repair evidence is incomplete or internally invalid.
- `SFS-REP-002` repair task, report, event, or worker payload structure invalid.
- `SFS-BND-001` fixture bundle payload validation failed or too few linkable artifacts were present.
- `SFS-BND-002` fixture bundle artifacts name different manifest digests.
- `SFS-BND-003` fixture bundle provider links disagree with each other or with replication-order assignments.
- `SFS-GOV-001` governance log node structure or embedded payload validation failed.
- `SFS-GOV-002` repair escalation or slash governance payload invalid.
- `SFS-GOV-003` governance log node CID does not match the expected CID.
- `SFS-GOV-004` governance DAG block CID does not match the expected CID or canonical payload.
- `SFS-GOV-005` governance DAG block structure or embedded node validation failed.
- `SFS-GOV-006` governance DAG chain topology, parent linkage, or expected head validation failed.
- `SFS-GOV-007` governance DAG head manifest structure failed validation.
- `SFS-GOV-008` governance DAG head block-count binding failed validation.
- `SFS-FFI-001` FFI caller supplied an invalid ABI argument.
- `SFS-FFI-002` FFI panicked or failed to render outcome JSON.
- `SFS-IO-001` input read failure.
- `SFS-NORITO-001` decode error.
- `SFS-INT-001` internal panic/unexpected.

CLI prints `docs_url` pointing to `docs/portal/docs/sorafs/reference-sdk/errors.md`.

## Library API Highlights
- `sorafs_manifest::reference::validate_provider_advert_bytes(...)`.
- `sorafs_manifest::reference::validate_provider_admission_envelope_bytes(...)`.
- `sorafs_manifest::reference::validate_provider_admission_renewal_bytes(...)`.
- `sorafs_manifest::reference::validate_provider_admission_revocation_bytes(...)`.
- `sorafs_manifest::reference::validate_replication_order_bytes(...)`.
- `sorafs_manifest::reference::validate_signed_replication_order_bytes(...)`.
- `sorafs_manifest::reference::validate_orderbook_payload_bytes(...)`.
- `sorafs_manifest::reference::validate_por_challenge_proof_bytes(...)`.
- `sorafs_manifest::reference::validate_pdp_commitment_bytes(...)`.
- `sorafs_manifest::reference::validate_pdp_challenge_bytes(...)`.
- `sorafs_manifest::reference::validate_pdp_proof_bytes(...)`.
- `sorafs_manifest::reference::validate_pdp_commitment_challenge_bytes(...)`.
- `sorafs_manifest::reference::validate_pdp_challenge_proof_bytes(...)`.
- `sorafs_manifest::reference::validate_pdp_commitment_challenge_proof_bytes(...)`.
- `sorafs_manifest::reference::validate_potr_receipt_bytes(...)`.
- `sorafs_manifest::reference::validate_repair_payload_bytes(...)`.
- `sorafs_manifest::reference::validate_fixture_bundle_payloads(...)`.
- `sorafs_manifest::reference::validate_governance_log_node_bytes(...)`.
- `sorafs_manifest::reference::validate_governance_dag_block_bytes(...)`.
- `sorafs_manifest::reference::validate_governance_dag_head_chain_bytes(...)`.
- `sorafs_car::validate_manifest_car_replay_bytes(...)` and
  `sorafs_car::validate_manifest_car_replay(...)`.
- `reference_ffi` functions return `SorafsReferenceFfiBuffer` values
  containing `ValidationOutcomeV1` Norito JSON for the same validator families.

Public APIs avoid direct filesystem dependencies; CLI binaries own file I/O and
convert decoded or raw Norito payloads into the shared validation functions.

## Integration & Automation
- **Cookbook smoke:** `docs/examples/sorafs_reference_sdk/run_reference_sdk_cookbook.sh`
  runs the committed validator, signing, bundle, and manifest/CAR replay
  scenarios.
- **Release packaging:** `scripts/package_sorafs_validate_release.sh` builds or
  packages `sorafs-validate`, stages `include/sorafs_reference.h`, runs fixture
  smoke checks, records per-file, binary, FFI-header, archive, and manifest
  digests under an untracked output directory, and can emit a detached manifest
  signature when supplied a release signing key.
- **CI guard:** PR checks can run `sorafs-validate bundle` and the cookbook
  script against committed fixtures; `ci/check_sorafs_reference_ffi_header.sh`
  fails if Rust FFI exports, selector constants, or C signatures drift from the
  checked header. No dedicated payload-validation workflow file exists in this
  tree.
- **Telemetry:** CLI `--telemetry-out` writes the raw `ValidationOutcomeV1`
  contract so operators can scrape `telemetry_tags` and error codes.
- **Torii integration:** production upload validation remains a rollout item; no
  dedicated upload-validation route is currently shipped.

## Testing & Fixtures
- Unit tests cover policy edge cases, error codes, FFI error handling, and
  positive validation paths in `crates/sorafs_manifest/src/reference.rs` and
  `reference_ffi.rs`.
- CLI tests cover argument parsing, output formats, signing paths, and failure
  codes for `sorafs-validate`, including committed orderbook and PDP fixture
  discovery from `sorafs-validate bundle --bundle fixtures/sorafs_manifest`.
- The cookbook replays committed fixtures for adverts, admission, orders,
  orderbook settlement receipts, PoR, PDP, PoTR, repair, governance nodes,
  bundle cross-links, and manifest/CAR replay; focused Rust and CLI tests cover
  governance DAG block and signed-head validation until committed DAG fixtures
  are added.
- Release smoke checks run through `scripts/package_sorafs_validate_release.sh`.
- Cross-target release evidence is still a production gate; archive published
  checksums and smoke outputs for each supported release target and require the
  SF-11 release evidence gate to pass before declaring those artifacts
  production-ready.

## Documentation Requirements
- Implemented: `docs/examples/sorafs_reference_sdk/` sample scripts, README, and
  smoke runner that emits `ValidationOutcomeV1` JSON for committed fixtures.
- Implemented: portal reference SDK error catalogue at
  `docs/portal/docs/sorafs/reference-sdk/errors.md`.
- Remaining: publish operator, metrics, and binding-generation guides once the
  release artifacts and downstream packages are cut.

## Packaging & Release
- Rust APIs currently ship through `sorafs_manifest::reference` and
  `sorafs_car`; a standalone Rust reference SDK package is not present in this
  workspace.
- `scripts/package_sorafs_validate_release.sh` builds or packages `sorafs-validate` for the selected target, stages the checked `include/sorafs_reference.h` C header, runs committed-fixture smoke checks, writes binary, archive, and manifest SHA256 files, records staged-file, FFI-header, and smoke-output hashes in the manifest, normalizes tar/gzip metadata for reproducible archive hashes, and can sign/verify that manifest with `--manifest-signing-key` plus optional `--manifest-public-key`.
- Provide per-target archives for x86_64/aarch64 macOS and Linux by running the helper once per target triple.
- Sign published archives or binaries in the release pipeline after the helper records deterministic digests; when the release signer is available, sign the helper-generated manifest in the same run and archive the `.manifest.json.sig` file beside the hashes. Keep generated `dist/*` artifacts untracked and commit only `dist/.gitkeep`.
- Release notes template references new/changed validations and error codes.

## Release Evidence Gate

Operators should keep SF-11 release promotion fail-closed until payload-free
release evidence passes the checked-in gate:

```bash
python3 scripts/check_sorafs_reference_sdk_release_evidence.py \
  @scripts/examples/sorafs_reference_sdk_release_evidence.args.example
```

For reviewed collection planning, use the runner in dry-run mode before
executing it against captured evidence paths:

```bash
python3 scripts/run_sorafs_reference_sdk_release_evidence.py \
  @scripts/examples/sorafs_reference_sdk_release_collection.args.example \
  --dry-run
```

The checker recognizes `sorafs.reference_sdk.*` SF-11 release schemas for
release archives, signed manifests, downstream bindings, cookbook smoke,
FFI/header contract, and governance approval. It fails closed on stale evidence,
raw archive, binary, manifest, package, smoke-output, transaction, token,
secret, or private-key material, missing x86_64/aarch64 macOS and Linux release
targets, missing binary/archive checksums, missing deterministic-archive proof,
tracked generated `dist/*` artifacts beyond `dist/.gitkeep`, unsigned or
unverified release manifests, missing governed release-key fingerprints, missing
JavaScript/Python/Kotlin/JVM/Java Android/Swift package publication evidence,
SDK export or `ValidationOutcomeV1` drift, missing native bridge/header binding,
failed published-archive cookbook smoke, missing fixture bundle or manifest/CAR
replay, smoke duration above threshold, FFI header drift, and governance packets
not bound to the governed release key roster, targets, downstream packages,
smoke evidence, and a `release_manifest_digest_hex` matching a valid signed
manifest artifact in the same bundle. Release-manifest binding failures are
recorded on the offending artifact before required-kind validity is computed,
so the JSON summary matches the fail-closed release decision.

The release evidence scripts have focused Python coverage in:

- `scripts/tests/check_sorafs_reference_sdk_release_evidence_test.py`
- `scripts/tests/run_sorafs_reference_sdk_release_evidence_test.py`

## Rollout Status
Implemented locally:
- Reference validation APIs for adverts, admission envelopes, admission
  renewals/revocations, orders, signed orders, orderbook payloads, PoR, PDP,
  PoTR, repair, governance nodes, governance DAG blocks and signed-head chains,
  bundles, and manifest/CAR replay.
- `sorafs-validate` commands for validation and runtime signing, plus the
  `soranet_trustless_verifier --validation-outcome` manifest/CAR replay path.
- `reference_ffi` C ABI functions returning `ValidationOutcomeV1` Norito JSON,
  including orderbook payload selectors and bundle selectors for orderbook and
  PDP fixture members, with `crates/sorafs_manifest/include/sorafs_reference.h`
  and `ci/check_sorafs_reference_ffi_header.sh` providing the local binding
  contract guard.
- Cookbook fixtures and smoke scripts under `docs/examples/sorafs_reference_sdk/`.
- Release-packaging helper that stages binary/archive/manifest digests and
  optional detached manifest signatures under untracked
  `dist/sorafs-validate-release/`.
- Fail-closed SF-11 release evidence gate, collection planner, operator argfile
  templates, and focused tests for release archives, signed manifests,
  downstream bindings, cookbook smoke, FFI/header contract, and governance
  approval, including cross-artifact signed-manifest digest binding.

Remaining production gates:
- Run the packaging helper for the supported release targets and publish signed
  release manifests outside the repository using governed release keys, then
  require those artifacts to pass the SF-11 release evidence gate.
- Ship/publish downstream SDK binding packages and release artifacts for the
  local JavaScript, Python, Kotlin/JVM, Java Android, and Swift wrappers and
  attach their digests to the SF-11 downstream-bindings evidence packet.
- Archive live operator smoke evidence for the published `sorafs-validate`
  archives and cookbook replay before declaring SF-11 fully released, and
  require that evidence to pass the SF-11 gate.
