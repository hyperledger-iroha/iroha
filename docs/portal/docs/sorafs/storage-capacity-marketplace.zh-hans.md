---
lang: zh-hans
direction: ltr
source: docs/portal/docs/sorafs/storage-capacity-marketplace.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 868edd6aa7401c64b8757db188edb13aa8e6ca8959966b6fea02e44bc298c6b7
source_last_modified: "2026-01-05T09:28:11.910794+00:00"
translation_last_reviewed: 2026-02-07
id: storage-capacity-marketplace
title: SoraFS Storage Capacity Marketplace
sidebar_label: Capacity Marketplace
description: SF-2c plan for the capacity marketplace, replication orders, telemetry, and governance hooks.
---

:::note Canonical Source
:::

# SoraFS Storage Capacity Marketplace (SF-2c Draft)

The SF-2c roadmap item introduces a governed marketplace where storage
providers declare committed capacity, receive replication orders, and earn fees
proportional to delivered availability. This document scopes the deliverables
required for the first release and breaks them into actionable tracks.

## Objectives

- Express provider capacity commitments (total bytes, per-lane limits, expiry)
  in a verifiable form consumable by governance, SoraNet transport, and Torii.
- Allocate pins across providers according to declared capacity, stake, and
  policy constraints while maintaining deterministic behaviour.
- Meter storage delivery (replication success, uptime, integrity proofs) and
  export telemetry for fee distribution.
- Provide revocation and dispute processes so dishonest providers can be
  penalised or removed.

## Domain Concepts

| Concept | Description | Initial Deliverable |
|---------|-------------|---------------------|
| `CapacityDeclarationV1` | Norito payload describing provider ID, chunker profile support, committed GiB, lane-specific limits, pricing hints, staking commitment, and expiry. | Schema + validator in `sorafs_manifest::capacity`. |
| `ReplicationOrder` | Governance-issued instruction assigning a manifest CID to one or more providers, including redundancy level and SLA metrics. | Norito schema shared with Torii + smart contract API. |
| `CapacityLedger` | On-chain/off-chain registry tracking active capacity declarations, replication orders, performance metrics, and fee accrual. | Smart contract module or off-chain service stub with deterministic snapshot. |
| `MarketplacePolicy` | Governance policy defining minimum stake, audit requirements, and penalty curves. | Config struct in `sorafs_manifest` + governance document. |

### Implemented Schemas (Status)

## Work Breakdown

### 1. Schema & Registry Layer

| Task | Owner(s) | Notes |
|------|----------|-------|
| Define `CapacityDeclarationV1`, `ReplicationOrderV1`, `CapacityTelemetryV1`. | Storage Team / Governance | Use Norito; include semantic versioning and capability references. |
| Implement parser + validator modules in `sorafs_manifest`. | Storage Team | Enforce monotonic IDs, capacity bounds, stake requirements. |
| Extend chunker registry metadata with `min_capacity_gib` per profile. | Tooling WG | Helps clients enforce per-profile minimum hardware requirements. |
| Draft `MarketplacePolicy` document capturing admission guardrails and penalty schedule. | Governance Council | Publish in docs alongside policy defaults. |

#### Schema Definitions (Implemented)

- `CapacityDeclarationV1` captures signed capacity commitments per provider, including canonical chunker handles, capability references, optional lane caps, pricing hints, validity windows, and metadata. Validation ensures non-zero stake, canonical handles, deduplicated aliases, per-lane caps within the declared total, and monotonic GiB accounting.【crates/sorafs_manifest/src/capacity.rs:28】
- `ReplicationOrderV1` binds manifests to governance-issued assignments with redundancy targets, SLA thresholds, and per-assignment guarantees; validators enforce canonical chunker handles, unique providers, and deadline constraints before Torii or the registry ingest the order.【crates/sorafs_manifest/src/capacity.rs:301】
- `CapacityTelemetryV1` expresses epoch snapshots (declared vs utilised GiB, replication counters, uptime/PoR percentages) that feed fee distribution. Bounds checks keep utilisation within declarations and percentages within 0 – 100 %.【crates/sorafs_manifest/src/capacity.rs:476】
- Shared helpers (`CapacityMetadataEntry`, `PricingScheduleV1`, lane/assignment/SLA validators) provide deterministic key validation and error reporting that CI and downstream tooling can reuse.【crates/sorafs_manifest/src/capacity.rs:230】
- `PinProviderRegistry` now surfaces the on-chain snapshot via `/v1/sorafs/capacity/state`, combining provider declarations and fee ledger entries behind deterministic Norito JSON.【crates/iroha_torii/src/sorafs/registry.rs:17】【crates/iroha_torii/src/sorafs/api.rs:64】
- Validation coverage exercises canonical handle enforcement, duplicate detection, per-lane bounds, replication assignment guards, and telemetry range checks so regressions surface immediately in CI.【crates/sorafs_manifest/src/capacity.rs:792】
- Operator tooling: `sorafs_manifest_stub capacity {declaration, telemetry, replication-order}` converts human-readable specs into canonical Norito payloads, base64 blobs, and JSON summaries so operators can stage `/v1/sorafs/capacity/declare`, `/v1/sorafs/capacity/telemetry`, and replication order fixtures with local validation.【crates/sorafs_car/src/bin/sorafs_manifest_stub/capacity.rs:1】 Reference fixtures live in `fixtures/sorafs_manifest/replication_order/` (`order_v1.json`, `order_v1.to`) and are generated via `cargo run -p sorafs_car --bin sorafs_manifest_stub -- capacity replication-order`.

### 2. Control Plane Integration

| Task | Owner(s) | Notes |
|------|----------|-------|
| Add `/v1/sorafs/capacity/declare`, `/v1/sorafs/capacity/telemetry`, `/v1/sorafs/capacity/orders` Torii handlers with Norito JSON payloads. | Torii Team | Mirror validator logic; reuse Norito JSON helpers. |
| Propagate `CapacityDeclarationV1` snapshots into orchestrator scoreboard metadata and gateway fetch plans. | Tooling WG / Orchestrator team | Extend `provider_metadata` with capacity references so multi-source scoring respects lane limits. |
| Feed replication orders into orchestrator/gateway clients to drive assignments and failover hints. | Networking TL / Gateway team | Scoreboard builder consumes governance-signed replication orders. |
| CLI tooling: extend `sorafs_cli` with `capacity declare`, `capacity telemetry`, `capacity orders import`. | Tooling WG | Provide deterministic JSON + scoreboard outputs. |

### 3. Marketplace Policy & Governance

| Task | Owner(s) | Notes |
|------|----------|-------|
| Ratify `MarketplacePolicy` (minimum stake, penalty multipliers, audit cadence). | Governance Council | Publish in docs, capture revision history. |
| Add governance hooks so Parliament can approve, renew, and revoke declarations. | Governance Council / Smart Contract team | Use Norito events + manifest ingestion. |
| Implement penalty schedule (fee reduction, bond slashing) tied to telemetered SLA violations. | Governance Council / Treasury | Align with `DealEngine` settlement outputs. |
| Document dispute process and escalation matrix. | Docs / Governance | Link to dispute runbook + CLI helpers. |

### 4. Metering & Fee Distribution

| Task | Owner(s) | Notes |
|------|----------|-------|
| Expand Torii metering ingest to accept `CapacityTelemetryV1`. | Torii Team | Validate GiB-hours, PoR success, uptime. |
| Update `sorafs_node` metering pipeline to report per-order utilisation + SLA stats. | Storage Team | Align with replication orders and chunker handles. |
| Settlement pipeline: convert telemetry + replication data into XOR-denominated payouts, produce governance-ready summaries, and record ledger state. | Treasury / Storage Team | Wire into Deal Engine / Treasury exports. |
| Export dashboards/alerts for metering health (ingestion backlog, stale telemetry). | Observability | Extend Grafana pack referenced by SF-6/SF-7. |

- Torii now exposes `/v1/sorafs/capacity/telemetry` and `/v1/sorafs/capacity/state` (JSON + Norito) so operators can submit epoch telemetry snapshots and inspectors can retrieve the canonical ledger for auditing or evidence packaging.【crates/iroha_torii/src/sorafs/api.rs:268】【crates/iroha_torii/src/sorafs/api.rs:816】
- `PinProviderRegistry` integration ensures replication orders are accessible through the same endpoint; CLI helpers (`sorafs_cli capacity telemetry --from-file telemetry.json`) now validate/publish telemetry from automation runs with deterministic hashing and alias resolution.
- Metering snapshots produce `CapacityTelemetrySnapshot` entries pinned to the `metering` snapshot, and Prometheus exports feed the ready-to-import Grafana board at `docs/source/grafana_sorafs_metering.json` so billing teams can monitor GiB·hour accrual, projected nano-SORA fees, and SLA compliance in real time.【crates/iroha_torii/src/routing.rs:5143】【docs/source/grafana_sorafs_metering.json:1】
- When metering smoothing is enabled, the snapshot includes `smoothed_gib_hours` and `smoothed_por_success_bps` so operators can compare EMA-trended values against the raw counters that governance uses for payouts.【crates/sorafs_node/src/metering.rs:401】

### 5. Dispute & Revocation Handling

| Task | Owner(s) | Notes |
|------|----------|-------|
| Define `CapacityDisputeV1` payload (complainant, evidence, target provider). | Governance Council | Norito schema + validator. |
| CLI support to file disputes and respond (with evidence attachments). | Tooling WG | Ensure deterministic hashing of evidence bundle. |
| Add automated checks for repeated SLA breaches (auto-escalate to dispute). | Observability | Alert thresholds and governance hooks. |
| Document revocation playbook (grace period, evacuation of pinned data). | Docs / Storage Team | Link to policy doc and operator runbook. |

## Testing & CI Requirements

- Unit tests for all new schema validators (`sorafs_manifest`).
- Integration tests that simulate: declaration → replication order → metering → payout.
- CI workflow to regenerate sample capacity declarations/telemetry and ensure signatures remain in sync (extend `ci/check_sorafs_fixtures.sh`).
- Load tests for the registry API (simulate 10k providers, 100k orders).

## Telemetry & Dashboards

- Dashboard panels:
  - Capacity declared vs utilised per provider.
  - Replication order backlog and average assignment delay.
  - SLA compliance (uptime %, PoR success rate).
  - Fee accrual and penalties per epoch.
- Alerts:
  - Provider below minimum committed capacity.
  - Replication order stuck > SLA.
  - Metering pipeline failures.

## Documentation Deliverables

- Operator guide for declaring capacity, renewing commitments, and monitoring utilisation.
- Governance guide for approving declarations, issuing orders, handling disputes.
- API reference for the capacity endpoints and replication order format.
- Marketplace FAQ for developers.

## GA Readiness Checklist

Roadmap item **SF-2c** gates production rollout on concrete evidence across accounting,
dispute handling, and onboarding. Use the artefacts below to keep the acceptance criteria
in sync with the implementation.

### Nightly accounting & XOR reconciliation
- Export the capacity state snapshot and the XOR ledger export for the same window, then run:
  ```bash
  python3 scripts/telemetry/capacity_reconcile.py \
    --snapshot artifacts/sorafs/capacity/state_$(date +%F).json \
    --ledger artifacts/sorafs/capacity/ledger_$(date +%F).ndjson \
    --label nightly-capacity \
    --json-out artifacts/sorafs/capacity/reconcile_$(date +%F).json \
    --prom-out "${SORAFS_CAPACITY_RECONCILE_TEXTFILE:-artifacts/sorafs/capacity/reconcile.prom}"
  ```
  The helper exits non-zero on missing/overpaid settlements or penalties and emits a Prometheus
  textfile summary.
- Alert `SoraFSCapacityReconciliationMismatch` (in `dashboards/alerts/sorafs_capacity_rules.yml`)
  fires whenever reconciliation metrics report gaps; dashboards live under
  `dashboards/grafana/sorafs_capacity_penalties.json`.
- Archive the JSON summary and hashes under `docs/examples/sorafs_capacity_marketplace_validation/`
  alongside governance packets.

### Dispute & slashing evidence
- File disputes through `sorafs_manifest_stub capacity dispute` (tests:
  `cargo test -p sorafs_car --test capacity_cli`) so payloads stay canonical.
- Run `cargo test -p iroha_core -- capacity_dispute_replay_is_deterministic` and the penalty
  suites (`record_capacity_telemetry_penalises_persistent_under_delivery`) to prove disputes and
  slashes replay deterministically.
- Follow `docs/source/sorafs/dispute_revocation_runbook.md` for evidence capture and escalation;
  link strike approvals back into the validation report.

### Provider onboarding & exit smoke tests
- Regenerate declaration/telemetry artefacts with `sorafs_manifest_stub capacity ...` and replay
  the CLI tests before submission (`cargo test -p sorafs_car --test capacity_cli -- capacity_declaration`).
- Submit via Torii (`/v1/sorafs/capacity/declare`) then capture `/v1/sorafs/capacity/state` plus
  Grafana screenshots. Follow the exit flow in `docs/source/sorafs/capacity_onboarding_runbook.md`.
- Archive signed artefacts and reconciliation outputs inside
  `docs/examples/sorafs_capacity_marketplace_validation/`.

## Dependencies & Sequencing

1. Finish SF-2b (admission policy) — marketplace relies on vetted providers.
2. Implement schema + registry layer (this doc) before Torii integration.
3. Complete metering pipeline before enabling payouts.
4. Final step: enable governance-controlled fee distribution once metering data is verified in staging.

Progress should be tracked in the roadmap with references to this document. Update the roadmap once each major section (schema, control plane, integration, metering, dispute handling) reaches feature complete status.
