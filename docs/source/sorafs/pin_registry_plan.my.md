---
lang: my
direction: ltr
source: docs/source/sorafs/pin_registry_plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 09ccb3070b3667174455fdbc620d1acaf9dcd361a1cb2a47cfc5da38f07a1e8e
source_last_modified: "2026-01-22T14:35:37.717243+00:00"
translation_last_reviewed: 2026-02-07
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

Status:
- Norito schemas in `crates/sorafs_manifest/src/pin_registry.rs` are the
  first-release schema surface used by core, Torii, fixtures, and reference
  validators.
- Rust code generation is handled through Norito derives; SDK parity now follows
  the normal SDK guard lanes whenever the schema changes.
- Architecture, migration, manifest-pipeline, CLI, OpenAPI, status, and roadmap
  docs already describe the shared validation path and endpoint behavior.

## Contract Implementation

| Task | Owner(s) | Notes |
|------|----------|-------|
| Registry storage and smart-contract state. | Core Infra / Smart Contract Team | Implemented in Iroha world state (`pin_manifests`, `manifest_aliases`, `replication_orders`) with deterministic Norito payload hashing and integer-only policy arithmetic. |
| Entry points: `RegisterPinManifest`, `ApprovePinManifest`, `RetirePinManifest`, `BindManifestAlias`, `IssueReplicationOrder`, `CompleteReplicationOrder`. | Core Infra | Core execution validates aliases, council envelopes, pin policy, replication ownership, and order completion; there is no separate local `bind_alias` backlog. |
| State transitions: enforce succession (manifest A -> B), retention epochs, alias uniqueness, and replication status changes. | Governance Council / Core Infra | `ensure_successor_chain` enforces approved, non-retired, acyclic multi-hop lineage; alias uniqueness/retention and replication issue/complete bookkeeping are covered by unit tests. |
| Governed parameters: load `ManifestPolicyV1` from config/governance state. | Governance Council | Runtime config maps pin-policy constraints into the shared validator. Live policy-change ceremonies are rollout governance evidence, not missing local contract code. |
| Registry telemetry and audit surface. | Observability | Torii exports registry metrics and attested REST snapshots. Additional signed event archives can be layered over those snapshots if governance requires them. |

Coverage:
- Unit tests cover registration, approval, retirement, alias binding, replication
  order issue/complete, permissions, duplicate rejection, and side-effect-free
  failure paths.
- Successor tests cover self references, unknown/pending/retired predecessors,
  cycle closure, and malformed existing predecessor cycles.
- `ci/check_sorafs_fixtures.sh` regenerates chunker, provider-admission, and pin
  registry fixtures and then runs the parity checks that keep the canonical
  schema surface stable.

## Service Facade (Torii/SDK Integration)

| Component | Task | Owner(s) |
|-----------|------|----------|
| Torii Service | Ships `/v1/sorafs/pin`, `/v1/sorafs/pin/{digest}`, `/v1/sorafs/aliases`, and `/v1/sorafs/replication` listing/lookup endpoints with deterministic pagination and filters. | Networking TL / Core Infra |
| Attestation | Listing and detail responses include the attestation object derived from the latest block hash. | Core Infra |
| CLI | `iroha app sorafs pin register`, `pin list`, `pin show`, `alias list`, and `replication list` wrap the REST and ISI surfaces for operator audits. | Tooling WG |
| SDK | Rust request builders and the JavaScript, Python, Swift, and C# guard lanes mirror the manifest payload and pin-register validation surface. | SDK Teams |

Operations:
- GET endpoints use attested snapshots, deterministic pagination, and the cache
  behavior documented in the alias policy where alias proofs are involved.
- Mutating operations go through ISI/governance permissions; REST handling keeps
  the same Torii auth and resource-guard model as the surrounding SoraFS APIs.

## Fixtures & CI

- Fixtures directory: `crates/iroha_core/tests/fixtures/sorafs_pin_registry/` stores signed manifest/alias/order snapshots regenerated by `cargo run -p iroha_core --example gen_pin_snapshot`.
- CI step: `ci/check_sorafs_fixtures.sh` regenerates the snapshot and fails if diffs appear, keeping CI fixtures aligned.
- Integration tests (`crates/iroha_core/tests/pin_registry.rs`) exercise the happy path plus duplicate-alias rejection, alias approval/retention guards, mismatched chunker handles, replica-count validation, governance-policy ceilings/retention/storage allowlists, and successor-guard failures (unknown/pre-approved/retired/self pointers); see `register_manifest_rejects_*` cases for coverage details.
- Unit tests cover alias validation, retention guards, replication order issue/complete, and multi-hop successor-chain cycle rejection in `crates/iroha_core/src/smartcontracts/isi/sorafs.rs`.
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
- Attested REST snapshots and registry metrics are the local audit surface; signed
  governance archives can consume those snapshots through the governance DAG when
  an operator rollout requires durable external evidence.

Alerts:
- Pending replication orders exceeding SLA.
- Alias expiry < threshold.
- Retention violations (manifest not renewed before expiry).

Dashboards:
- Grafana JSON `docs/source/grafana_sorafs_pin_registry.json` tracks manifest lifecycle totals, alias coverage, backlog saturation, SLA ratio, latency vs slack overlays, and missed-order rates for on-call review.

## Runbooks & Documentation

- `docs/source/sorafs/migration_ledger.md`, `docs/source/sorafs/migration_roadmap.md`, and `roadmap.md` carry registry status updates.
- Operator guide: `docs/source/sorafs/runbooks/pin_registry_ops.md` covers metrics, alerting, deployment, backup, and recovery flows.
- Governance and dispute flows are documented through the admission policy, alias policy, capacity marketplace, and dispute/revocation runbooks.
- Endpoint behavior is covered by the SoraFS CLI, node-client protocol, and OpenAPI surfaces.

## Dependencies & Sequencing

1. Endpoint/client submission polish, shared validation, governance config mapping,
   Torii `manifest_b64` validation, and registry DTO policy checks are wired.
2. Norito schema, policy defaults, contract state, service facade, telemetry,
   fixtures, and local integration coverage are implemented.
3. Ongoing SF-4 work is rollout evidence: live registry audits, governance archive
   handoff, and operator-specific policy-change transcripts.

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
