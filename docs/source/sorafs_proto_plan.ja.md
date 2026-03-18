---
lang: ja
direction: ltr
source: docs/source/sorafs_proto_plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: e843072093b18bc3fcae6c4a2898cd2b091dd7e9d445986c730899d1b9805421
source_last_modified: "2026-01-04T10:50:53.683454+00:00"
translation_last_reviewed: 2026-01-30
---

---
title: SoraFS Wire Format & Schema Reference
summary: Final specification for SF-10 covering Norito payloads, fixtures, and cross-language ingestion.
---

# SoraFS Wire Format & Schema Reference

## Goals & Scope
- Define the canonical Norito payloads that power adverts, admission, replication orders, PoR/PoTR artefacts, and governance DAG entries in SoraFS.
- Document how the schemas map to the Rust source (`crates/sorafs_manifest`) and how SDKs in other languages ingest the same bytes without bespoke codecs.
- Capture fixture generation, versioning, and release processes so teams can validate changes deterministically.

This document completes **SF-10 — Publish `sora-proto` schemas** and supersedes the earlier draft plan.

## Canonical Modules & Payloads
| Domain | Rust module | Primary structs | Notes |
|--------|-------------|-----------------|-------|
| Provider adverts | `crates/sorafs_manifest/src/provider_advert.rs` | `ProviderAdvertV1`, `ProviderAdvertBodyV1`, `AdvertSignature` | Describes capability TLVs, transport hints, escrow policy, and signature shells. |
| Provider admission | `crates/sorafs_manifest/src/provider_admission.rs` | `ProviderAdmissionProposalV1`, `ProviderAdmissionEnvelopeV1`, `RenewalV1`, `RevocationV1` | Governance proposals and envelopes for onboarding, renewal, and revocation. |
| Replication orders & capacity | `crates/sorafs_manifest/src/capacity.rs` | `ReplicationOrderV1`, `PlacementDirectiveV1`, `CapacityDeclarationV1`, `PricingScheduleV1` | Manifests, chunk assignments, pricing hints, and capacity scheduling. |
| Deal & incentives | `crates/sorafs_manifest/src/deal.rs` | `DealTermsV1`, `DealMicropaymentV1`, `DealSettlementV1` | Wire format for incentive accounting. |
| PoR / Audit | `crates/sorafs_manifest/src/por.rs` | `PorChallengeV1`, `PorProofV1`, `AuditVerdictV1`, helper functions (`derive_challenge_seed`, etc.) | Merkle sampling layout referenced by the PoR coordinator. |
| PoTR | `crates/sorafs_manifest/src/potr.rs` | `PotrProbeV1`, `PotrReceiptV1`, `PotrVerdictV1` | Deadline proof schema for retrieval latency tracking (SF-14). |
| Repair & governance | `crates/sorafs_manifest/src/repair.rs`, `crates/sorafs_manifest/src/governance.rs` | `RepairTaskV1`, `RepairEvidenceV1`, `GovernanceLogNodeV1` | Artefacts produced by repair automation and the governance DAG pipeline. |

Each struct derives `NoritoSerialize`/`NoritoDeserialize` and `JsonSerialize` so the same definitions drive binary and JSON views. Any schema change MUST land here first.

## Versioning Policy
- **Identifiers.** Numeric enums use explicit `#[repr(u8/u16)]` discriminants. Keep them stable across versions; new values append at the end.
- **Defaulting.** Optional fields leverage `#[norito(default)]` so omitted values decode deterministically. Builders in each module provide helper constructors to enforce invariants.
- **Validation.** `validate()` methods exist on major structs (e.g., `ProviderAdvertV1::validate`). CI executes round-trip and validation tests across all fixtures.
- **Release gates.** schema changes require:
  1. Updating the relevant struct(s) and inline docs.
  2. Refreshing fixtures (see below).
  3. Recording the update in `status.md` and referencing golden test updates.
  4. Notifying SDK maintainers via the release checklist.

## Fixtures & Generators
Deterministic fixtures live under `fixtures/sorafs_manifest/`:
- `provider_admission/`, `provider_advert/`
- `replication_order/`
- `por/`, `potr/`
- `governance/`
- `ci_sample/` and `ci_sample_sf2/` bundles that combine manifest, CAR, proof, and audit artefacts.

Generators in `crates/sorafs_manifest/src/bin/` refresh these fixtures:
- `generate_provider_advert_fixture.rs`
- `generate_replication_order_fixture.rs`
- `generate_por_fixtures.rs`
- `generate_governance_fixture.rs`

A typical regeneration flow:
```bash
cargo run -p sorafs_manifest --bin generate_replication_order_fixture \
  -- --manifest fixtures/sorafs_manifest/ci_sample/manifest.json \
  --out fixtures/sorafs_manifest/replication_order/order.json
```
Each generator writes:
1. Norito bytes (`*.to`).
2. Human-readable JSON commentary (`*.json`).
3. Manifest summarising hashes (`manifest.summary.json`).

CI jobs (`.github/workflows/sorafs-cli-fixture.yml`) diff fixture directories to prevent silent drift.

## Cross-Language Ingestion
| Language / Surface | Entry point | Validation strategy |
|--------------------|-------------|---------------------|
| Rust (SDKs, services) | `sorafs_manifest::{...}` + `norito::{to_bytes, from_bytes}` | Unit tests in `crates/sorafs_manifest/tests/*` round-trip every fixture. |
| CLI / pipelines | `sorafs_car`, `sorafs_orchestrator`, `sorafs_cli` | Commands provide `--json-out` / `--por-json-out` to surface Norito commentary while emitting canonical bytes to disk. |
| JavaScript / TypeScript | `iroha_js_host::sorafs` helpers (`decodeReplicationOrder`, `decodeProviderAdvert`) | Validates `Sora-Proof` headers and advert signatures using the shared Norito definitions. |
| Python | `iroha_python.sorafs` module | Dataclasses mirror the Norito structs and rely on the Rust library for decoding to avoid duplicate codecs. |
| Swift / Kotlin | Mobile SDKs consume Norito via FFI glue that forwards to the Rust codec (`IrohaSwift` / `iroha_android`). |

Guidelines:
- Never transcode Norito payloads into alternative formats for transport; store and forward the raw bytes.
- Always surface both the raw Norito blob and the decoded representation in telemetry/logs to aid audits.
- Bindings MUST fail fast when they encounter an unknown version or missing helper.

## Deterministic Signing Domains
- Providers sign adverts using `AdvertSignature` (Ed25519 today). The signing domain string lives in `provider_advert.rs` and must not change without a new schema version.
- Governance signatures across admission and replication orders share the `GovernanceSignatureV1` struct; the domain string is `sorafs:governance:v1`.
- Deal, PoR, and PoTR artefacts embed their own domain constants (see `deal.rs`, `por.rs`, `potr.rs`). When adding algorithms beyond Ed25519, extend the `SignatureAlgorithm` enum while ensuring canonical ordering.

## Governance DAG Alignment
- The governance publishing tool uses `GovernanceLogNodeV1` to wrap Norito payloads stored in the DAG (`governance/sorafs/...`). Nodes include metadata: CID, payload kind, timestamp, and signature set.
- Validators call `governance::decode_log_node(bytes)` before enforcing policy checks. Unknown payload kinds fail closed.
- Weekly PoR and repair reports (see SF-9) append to the DAG using the same node schema; filenames follow `por/report/<ISO-week>.json` conventions.

## Open Work & Maintenance
- **Documentation:** keep module-level docs (`//!`) in sync with this reference. Public APIs must explain versioning guarantees.
- **Golden tests:** ensure each fixture directory has a matching test in `crates/sorafs_manifest/tests/` or `crates/sorafs_car/tests/`.
- **SDK regeneration:** release checklist includes `./scripts/sorafs_regenerate_fixtures.sh` and language binding updates.
- **Telemetry integration:** metrics referencing schema versions (e.g., `sorafs_advert_version_total{version="1"}`) help detect mixed deployments.

With this reference in place, schema authors, SDK teams, and governance tooling share a single authoritative source describing every Norito payload shipped by SoraFS.
