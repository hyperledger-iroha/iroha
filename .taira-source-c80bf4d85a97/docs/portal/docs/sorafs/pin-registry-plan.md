---
id: pin-registry-plan
title: SoraFS Pin Registry Implementation Plan
sidebar_label: Pin Registry Plan
description: SF-4 implementation plan covering registry state machine, Torii facade, tooling, and observability.
---

:::note Canonical Source
:::

# SoraFS Pin Registry Implementation Plan (SF-4)

SF-4 delivers the Pin Registry contract and supporting services that store
manifest commitments, enforce pin policies, and expose APIs to Torii, gateways,
and orchestrators. This document expands the validation plan with concrete
implementation tasks, covering on-chain logic, host-side services, fixtures,
and operational requirements.

## Scope

1. **Registry state machine**: Norito-defined records for manifests, aliases,
   successor chains, retention epochs, and governance metadata.
2. **Contract implementation**: deterministic CRUD operations for pin lifecycle
   (`ReplicationOrder`, `Precommit`, `Completion`, eviction).
3. **Service facade**: gRPC/REST endpoints backed by the registry that Torii
   and SDKs consume, including pagination and attestation.
4. **Tooling & fixtures**: CLI helpers, test vectors, and documentation to keep
   manifests, aliases, and governance envelopes in sync.
5. **Telemetry & ops**: metrics, alerts, and runbooks for registry health.

## Data Model

### Core Records (Norito)

| Struct | Description | Fields |
|--------|-------------|--------|
| `PinManifestRecord` | Chain-authoritative manifest lifecycle entry. The envelope digest and exact 36-byte CIDv1/dag-cbor/BLAKE3-256 content root are distinct commitments. | `digest`, `root_cid`, `chunker`, `chunk_digest_sha3_256`, `por_root`, `content_length`, `policy`, `submitted_by`, `submitted_epoch`, `alias`, `successor_of`, `metadata`, `status`, `retirement_reason`, `council_envelope_digest`, `pin_fee_payment`. |
| `PinManifestFinalizedRecordV1` | Immutable read result binding one native manifest record to the finalized block used for the query. | `finalized_cursor` (`height`, `block_hash`), `manifest`. |
| `AliasBindingV1` | Maps alias -> manifest CID. | `alias`, `manifest_cid`, `bound_at`, `expiry_epoch`. |
| `ReplicationOrderV1` | Instruction for providers to pin manifest. | `order_id`, `manifest_cid`, `providers`, `redundancy`, `deadline`, `policy_hash`. |
| `ReplicationReceiptV1` | Provider acknowledgement. | `order_id`, `provider_id`, `status`, `timestamp`, `por_sample_digest`. |
| `ManifestPolicyV1` | Governance policy snapshot. | `min_replicas`, `max_retention_epochs`, `allowed_profiles`, `pin_fee_basis_points`. |

Implementation reference: the authoritative manifest lifecycle and finalized
read schemas live in `crates/iroha_data_model/src/sorafs/pin_registry.rs`.
Supporting alias, replication, and policy envelopes live in
`crates/sorafs_manifest/src/pin_registry.rs`. Consensus admission derives and
validates the stored commitments; Torii and operator tooling consume the exact
native finalized record rather than maintaining a second pin-record format.

Status:
- The native `PinManifestRecord` and `PinManifestFinalizedRecordV1` are the V1
  manifest-registry surface used by core, Torii, fixtures, and reference
  validators.
- Rust code generation uses Norito derives; SDK parity follows the normal guard
  lanes whenever the native schema changes.
- Architecture, manifest-pipeline, CLI, OpenAPI, status, and roadmap documents
  describe the shared validation path and endpoint behavior.

## Contract Implementation

| Task | Owner(s) | Notes |
|------|----------|-------|
| Registry storage and smart-contract state. | Core Infra / Smart Contract Team | Implemented in Iroha world state (`pin_manifests`, `manifest_aliases`, `replication_orders`) with deterministic Norito payload hashing and integer-only policy arithmetic. |
| Entry points: `RegisterPinManifest`, `ApprovePinManifest`, `RetirePinManifest`, `BindManifestAlias`, `IssueReplicationOrder`, `CompleteReplicationOrder`, `ExpireReplicationOrder`. | Core Infra | Registration carries the complete canonical manifest, resource-bounds and validates it in consensus, and derives all stored commitments. Core execution also validates aliases, council envelopes, governance permissions, canonical replication payloads, completion, and deadline-bound expiration. |
| State transitions: enforce succession (manifest A -> B), retention epochs, alias uniqueness, and replication status changes. | Governance Council / Core Infra | `ensure_successor_chain` enforces approved, non-retired, acyclic multi-hop lineage; alias uniqueness, retention, and replication issue/complete bookkeeping are covered by unit tests. |
| Governed parameters: load `ManifestPolicyV1` from config/governance state. | Governance Council | Runtime config maps pin-policy constraints into the shared validator. Live policy-change ceremonies are rollout governance evidence, not missing local contract code. |
| Registry telemetry and audit surface. | Observability | Torii exports registry metrics and attested REST snapshots. Additional signed event archives can be layered over those snapshots if governance requires them. |

Coverage:
- Unit tests cover registration, approval, retirement, alias binding, replication
  order issue/complete, permissions, duplicate rejection, and side-effect-free
  failure paths.
- Successor tests cover self references, unknown/pending/retired predecessors,
  cycle closure, and malformed existing predecessor cycles.
- `ci/check_sorafs_fixtures.sh` regenerates chunker, provider-admission, and pin
  registry fixtures and runs the parity checks that keep the canonical schema
  surface stable.

## Service Facade (Torii/SDK Integration)

| Component | Task | Owner(s) |
|-----------|------|----------|
| Torii Service | Ships `/v1/sorafs/pin`, `/v1/sorafs/pin/{digest_hex}`, `/v1/sorafs/aliases`, and `/v1/sorafs/replication`. The manifest-detail route returns exact native `PinManifestFinalizedRecordV1` JSON and accepts only the optional paired expected finalized height/hash precondition; pagination and filters remain on list routes. | Networking TL / Core Infra |
| Finality binding | Listing responses retain their listing attestation. A manifest-detail response carries the native `finalized_cursor` beside the authoritative `PinManifestRecord`; a stale requested cursor fails with HTTP 409. | Core Infra |
| CLI | `iroha app sorafs pin register`, `pin list`, `pin show`, `alias list`, and `replication list` wrap the REST and ISI surfaces for operator audits. | Tooling WG |
| SDK | Rust request builders and the JavaScript, Python, Swift, and C# guard lanes mirror the manifest payload and pin-register validation surface. | SDK Teams |

Operations:
- List endpoints use attested snapshots, deterministic pagination, and the cache
  behavior documented in the alias policy where alias proofs are involved.
- `GET /v1/sorafs/pin/{digest_hex}` returns only `finalized_cursor` and the
  native `manifest`. The retired `limit`, attestation, embedded alias/order
  arrays, counts, and truncation fields are absent; callers use
  `/v1/sorafs/aliases` and `/v1/sorafs/replication` for bounded list queries.
- Mutating operations go through ISI/governance permissions; REST handling keeps
  the same Torii auth and resource-guard model as the surrounding SoraFS APIs.

## Fixtures & CI

- Fixtures directory: `crates/iroha_core/tests/fixtures/sorafs_pin_registry/` stores signed manifest/alias/order snapshots regenerated by `cargo run -p iroha_core --example gen_pin_snapshot`.
- CI step: `ci/check_sorafs_fixtures.sh` regenerates the snapshot and fails if diffs appear, keeping CI fixtures aligned.
- Integration tests (`crates/iroha_core/tests/pin_registry.rs`) exercise the happy path plus duplicate-alias rejection, alias approval/retention guards, mismatched chunker handles, replica-count validation, and successor-guard failures (unknown/pre-approved/retired/self pointers); see `register_manifest_rejects_*` cases for coverage details.
- Unit tests now cover alias validation, retention guards, and successor checks in `crates/iroha_core/src/smartcontracts/isi/sorafs.rs`; multi-hop succession detection once state machine lands.
- Golden JSON for events used by observability pipelines.

## Telemetry & Observability

Metrics (Prometheus):
- `torii_sorafs_registry_manifests_total{status="pending|approved|retired"}`
- `torii_sorafs_registry_aliases_total`
- `torii_sorafs_registry_orders_total{status="pending|completed|expired"}`
- `torii_sorafs_replication_sla_total{outcome="met|missed|pending"}`
- `torii_sorafs_replication_completion_latency_epochs{stat="avg|p95|max|count"}`
- `torii_sorafs_replication_deadline_slack_epochs{stat="avg|p95|max|count"}`
- Existing provider telemetry (`torii_sorafs_capacity_*`, `torii_sorafs_fee_projection_nanos`) remains in scope for end-to-end dashboards.

Logs:
- Structured Norito event stream for governance audits (signed?).

Alerts:
- Pending replication orders exceeding SLA.
- Alias expiry < threshold.
- Retention violations (manifest not renewed before expiry).

Dashboards:
- Grafana JSON `docs/source/grafana_sorafs_pin_registry.json` tracks manifest lifecycle totals, alias coverage, backlog saturation, SLA ratio, latency vs slack overlays, and missed-order rates for on-call review.

## Runbooks & Documentation

- Update `docs/source/sorafs/migration_ledger.md` to include registry status updates.
- Operator guide: `docs/source/sorafs/runbooks/pin_registry_ops.md` (now published) covering metrics, alerting, deployment, backup, and recovery flows.
- Governance guide: describe policy parameters, approval workflow, dispute handling.
- API reference pages for each endpoint (Docusaurus docs).

## Dependencies & Sequencing

1. Complete validation plan tasks (ManifestValidator integration).
2. Finalise Norito schema + policy defaults.
3. Implement contract + service, wire telemetry.
4. Regenerate fixtures, run integration suites.
5. Update docs/runbooks and mark roadmap items complete.

Each roadmap checklist item under SF-4 should reference this plan when progress is made.
The REST façade now ships with attested list endpoints and finalized native
manifest readback:

- `GET /v1/sorafs/pin` returns the attested manifest catalogue.
- `GET /v1/sorafs/pin/{digest_hex}` returns exact
  `PinManifestFinalizedRecordV1` JSON with the finalized cursor and native
  manifest record.
- `GET /v1/sorafs/aliases` and `GET /v1/sorafs/replication` expose the active
  alias catalogue and replication order backlog with consistent pagination and
  status filters.

The CLI wraps these calls (`iroha app sorafs pin list`, `pin show`, `alias list`,
`replication list`) so operators can script registry audits without touching
lower-level APIs.
