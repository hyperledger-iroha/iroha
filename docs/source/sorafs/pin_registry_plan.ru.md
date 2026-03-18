---
lang: ru
direction: ltr
source: docs/source/sorafs/pin_registry_plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 09ccb3070b3667174455fdbc620d1acaf9dcd361a1cb2a47cfc5da38f07a1e8e
source_last_modified: "2026-01-22T15:38:30.692973+00:00"
translation_last_reviewed: 2026-01-30
---

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
| `PinRecordV1` | Canonical manifest entry. | `manifest_cid`, `chunk_plan_digest`, `por_root`, `profile_handle`, `approved_at`, `retention_epoch`, `pin_policy`, `successor_of`, `governance_envelope_hash`. |
| `AliasBindingV1` | Maps alias -> manifest CID. | `alias`, `manifest_cid`, `bound_at`, `expiry_epoch`. |
| `ReplicationOrderV1` | Instruction for providers to pin manifest. | `order_id`, `manifest_cid`, `providers`, `redundancy`, `deadline`, `policy_hash`. |
| `ReplicationReceiptV1` | Provider acknowledgement. | `order_id`, `provider_id`, `status`, `timestamp`, `por_sample_digest`. |
| `ManifestPolicyV1` | Governance policy snapshot. | `min_replicas`, `max_retention_epochs`, `allowed_profiles`, `pin_fee_basis_points`. |

Implementation reference: see `crates/sorafs_manifest/src/pin_registry.rs` for the
Rust Norito schemas and validation helpers backing these records. Validation
mirrors the manifest tooling (chunker registry lookup, pin policy gating) so the
contract, Torii facades, and CLI share identical invariants.

Tasks:
- Finalise Norito schemas in `crates/sorafs_manifest/src/pin_registry.rs`.
- Generate code (Rust + other SDKs) using Norito macros.
- Update docs (`sorafs_architecture_rfc.md`) once schemas land.

## Contract Implementation

| Task | Owner(s) | Notes |
|------|----------|-------|
| Implement registry storage (sled/sqlite/off-chain) or smart contract module. | Core Infra / Smart Contract Team | Provide deterministic hashing, avoid floating point. |
| Entry points: `submit_manifest`, `approve_manifest`, `bind_alias`, `issue_replication_order`, `complete_replication`, `evict_manifest`. | Core Infra | Leverage `ManifestValidator` from validation plan. Alias binding now flows through `RegisterPinManifest` (Torii DTO surfacing) while dedicated `bind_alias` remains planned for successive updates. |
| State transitions: enforce succession (manifest A -> B), retention epochs, alias uniqueness. | Governance Council / Core Infra | Alias uniqueness, retention limits, and predecessor approval/retirement checks now live in `crates/iroha_core/src/smartcontracts/isi/sorafs.rs`; multi-hop succession detection and replication bookkeeping remain open. |
| Governed parameters: load `ManifestPolicyV1` from config/governance state; allow updates via governance events. | Governance Council | Provide CLI for policy updates. |
| Event emission: emit Norito events for telemetry (`ManifestApproved`, `ReplicationOrderIssued`, `AliasBound`). | Observability | Define event schema + logging. |

Testing:
- Unit tests for each entry point (positive + rejection).
- Property tests for succession chain (no cycles, monotonic epochs).
- Fuzz validation by generating random manifests (bounded).

## Service Facade (Torii/SDK Integration)

| Component | Task | Owner(s) |
|-----------|------|----------|
| Torii Service | Expose `/v1/sorafs/pin` (submit), `/v1/sorafs/pin/{cid}` (lookup), `/v1/sorafs/aliases` (list/bind), `/v1/sorafs/replication` (orders/receipts). Provide pagination + filtering. | Networking TL / Core Infra |
| Attestation | Include registry height/hash in responses; add Norito attestation struct consumed by SDKs. | Core Infra |
| CLI | Extend `sorafs_manifest_stub` or new `sorafs_pin` CLI with `pin submit`, `alias bind`, `order issue`, `registry export`. | Tooling WG |
| SDK | Generate client bindings (Rust/Go/TS) from Norito schema; add integration tests. | SDK Teams |

Operations:
- Add caching layer/ETag for GET endpoints.
- Provide rate limiting / auth consistent with Torii policies.

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
The REST façade now ships with attested listing endpoints:

- `GET /v1/sorafs/pin` and `GET /v1/sorafs/pin/{digest}` return manifests with
  alias bindings, replication orders, and an attestation object derived from the
  latest block hash.
- `GET /v1/sorafs/aliases` and `GET /v1/sorafs/replication` expose the active
  alias catalogue and replication order backlog with consistent pagination and
  status filters.

The CLI wraps these calls (`iroha app sorafs pin list`, `pin show`, `alias list`,
`replication list`) so operators can script registry audits without touching
lower-level APIs.
